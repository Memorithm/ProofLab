//! Durable content-addressed storage for verified ProofLab artifacts.
//!
//! This crate layers atomic filesystem persistence over `prooflab-store`'s
//! integrity-checked in-memory oracle. It does not create proof artifacts and
//! therefore cannot bypass the configured formal-kernel trust boundary.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use prooflab_core::{ProofArtifact, ProofArtifactId};
use prooflab_store::{MemoryProofStore, ProofStore, StoreError};

/// Failure while opening or mutating the durable store.
#[derive(Debug)]
pub enum FsStoreError {
    Io(std::io::Error),
    Decode(serde_json::Error),
    Store(StoreError),
    PathAddressMismatch {
        expected: ProofArtifactId,
        path: PathBuf,
    },
}

impl fmt::Display for FsStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "proof store I/O failure: {error}"),
            Self::Decode(error) => write!(formatter, "proof store decode failure: {error}"),
            Self::Store(error) => write!(formatter, "proof store integrity failure: {error}"),
            Self::PathAddressMismatch { expected, path } => write!(
                formatter,
                "proof artifact address {:?} does not match path {}",
                expected,
                path.display()
            ),
        }
    }
}

impl std::error::Error for FsStoreError {}

impl From<std::io::Error> for FsStoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for FsStoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Decode(value)
    }
}

impl From<StoreError> for FsStoreError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

/// Filesystem-backed verified proof store.
#[derive(Debug, Clone)]
pub struct FsProofStore {
    root: PathBuf,
    memory: MemoryProofStore,
}

impl FsProofStore {
    /// Open or create a durable store and validate every persisted artifact.
    ///
    /// # Errors
    ///
    /// Fails closed on I/O errors, malformed records, path/address mismatch,
    /// content-integrity failure, collisions, or missing proof dependencies.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, FsStoreError> {
        let root = root.as_ref().to_path_buf();
        let artifacts_dir = root.join("artifacts");
        fs::create_dir_all(&artifacts_dir)?;

        let mut pending = BTreeMap::new();
        for entry in fs::read_dir(&artifacts_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path)?;
            let artifact: ProofArtifact = serde_json::from_slice(&bytes)?;
            if !artifact.check_id() {
                return Err(StoreError::IntegrityMismatch(artifact.id).into());
            }
            let expected_name = format!("{}.json", id_hex(artifact.id));
            if path.file_name().and_then(|value| value.to_str()) != Some(expected_name.as_str()) {
                return Err(FsStoreError::PathAddressMismatch {
                    expected: artifact.id,
                    path,
                });
            }
            if pending.insert(artifact.id, artifact).is_some() {
                return Err(StoreError::AddressCollision(artifact.id).into());
            }
        }

        let mut memory = MemoryProofStore::default();
        while !pending.is_empty() {
            let ready: Vec<_> = pending
                .iter()
                .filter(|(_, artifact)| {
                    artifact
                        .body
                        .dependencies
                        .iter()
                        .all(|dependency| memory.has(*dependency))
                })
                .map(|(id, _)| *id)
                .collect();

            if ready.is_empty() {
                let artifact = pending.values().next().expect("pending is non-empty");
                let missing = artifact
                    .body
                    .dependencies
                    .iter()
                    .find(|dependency| !memory.has(**dependency))
                    .copied()
                    .unwrap_or(artifact.id);
                return Err(StoreError::MissingDependency(missing).into());
            }

            for id in ready {
                let artifact = pending.remove(&id).expect("ready id exists");
                memory.put(&artifact)?;
            }
        }

        Ok(Self { root, memory })
    }

    /// Persist one verified artifact atomically and update the in-memory index.
    ///
    /// # Errors
    ///
    /// Fails without changing the live index if validation or persistence fails.
    pub fn put(&mut self, artifact: &ProofArtifact) -> Result<ProofArtifactId, FsStoreError> {
        let mut staged = self.memory.clone();
        let id = staged.put(artifact)?;
        let artifacts_dir = self.root.join("artifacts");
        let final_path = artifacts_dir.join(format!("{}.json", id_hex(id)));

        if final_path.exists() {
            let existing: ProofArtifact = serde_json::from_slice(&fs::read(&final_path)?)?;
            if existing != *artifact {
                return Err(StoreError::AddressCollision(id).into());
            }
            if !existing.check_id() {
                return Err(StoreError::IntegrityMismatch(id).into());
            }
            self.memory = staged;
            return Ok(id);
        }

        let serialized = serde_json::to_vec(artifact)?;
        let temp_path = artifacts_dir.join(format!(".{}.{}.tmp", id_hex(id), std::process::id()));
        let mut temp = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)?;
        let write_result = (|| -> Result<(), std::io::Error> {
            temp.write_all(&serialized)?;
            temp.sync_all()?;
            drop(temp);
            fs::rename(&temp_path, &final_path)?;
            File::open(&artifacts_dir)?.sync_all()?;
            Ok(())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(error.into());
        }

        self.memory = staged;
        Ok(id)
    }

    /// Read a verified artifact from the validated in-memory index.
    pub fn get(&self, id: ProofArtifactId) -> Result<Option<ProofArtifact>, FsStoreError> {
        Ok(self.memory.get(id)?)
    }

    /// Return artifact ids in deterministic order.
    #[must_use]
    pub fn proof_ids(&self) -> Vec<ProofArtifactId> {
        self.memory.proof_ids()
    }

    /// Return transitive proof dependencies in deterministic order.
    pub fn ancestors(&self, root: ProofArtifactId) -> Result<Vec<ProofArtifactId>, FsStoreError> {
        Ok(self.memory.ancestors(root)?)
    }

    /// Return transitive dependants in deterministic order.
    pub fn descendants(&self, root: ProofArtifactId) -> Result<Vec<ProofArtifactId>, FsStoreError> {
        Ok(self.memory.descendants(root)?)
    }

    /// Filesystem root of this store.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn id_hex(id: ProofArtifactId) -> String {
    let mut output = String::with_capacity(64);
    for byte in id.0 {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use prooflab_core::{
        Claim, ClaimBody, FormalBackend, FormalStatement, KernelReceipt, ProofArtifact, ReproMeta,
        sha256_bytes,
    };

    use super::*;

    fn temp_store(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("prooflab-{name}-{}-{nonce}", std::process::id()))
    }

    fn proof(name: &str, dependencies: Vec<ProofArtifactId>) -> ProofArtifact {
        let claim = Claim::new(ClaimBody {
            statement: format!("{name} = {name}"),
            assumptions: vec![],
            parents: vec![],
        });
        let source = format!("theorem {name} : True := by trivial\n");
        let formal = FormalStatement::lean4(claim.id, source.as_bytes(), vec!["Mathlib".into()]);
        let mut repro = ReproMeta::bootstrap("sha256:test-environment");
        repro.prooflab_revision = "fs-store-test".into();
        let receipt = KernelReceipt::new(
            FormalBackend::Lean4,
            "lake env lean",
            true,
            Some(0),
            sha256_bytes(b"stdout"),
            sha256_bytes(b"stderr"),
        );
        ProofArtifact::new_verified(&formal, source.as_bytes(), dependencies, repro, receipt)
            .unwrap()
    }

    #[test]
    fn persists_reopens_and_preserves_provenance_queries() {
        let root = temp_store("reopen");
        let parent = proof("parent", vec![]);
        let child = proof("child", vec![parent.id]);

        {
            let mut store = FsProofStore::open(&root).unwrap();
            store.put(&parent).unwrap();
            store.put(&child).unwrap();
            assert_eq!(store.ancestors(child.id).unwrap(), vec![parent.id]);
        }

        let reopened = FsProofStore::open(&root).unwrap();
        assert_eq!(reopened.get(parent.id).unwrap(), Some(parent.clone()));
        assert_eq!(reopened.get(child.id).unwrap(), Some(child.clone()));
        assert_eq!(reopened.descendants(parent.id).unwrap(), vec![child.id]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tampered_persisted_artifact_fails_closed_on_reopen() {
        let root = temp_store("tamper");
        let artifact = proof("tamper", vec![]);
        let mut store = FsProofStore::open(&root).unwrap();
        store.put(&artifact).unwrap();
        let path = root
            .join("artifacts")
            .join(format!("{}.json", id_hex(artifact.id)));
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["body"]["repro"]["environment_digest"] =
            serde_json::Value::String("modified".into());
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        assert!(matches!(
            FsProofStore::open(&root),
            Err(FsStoreError::Store(StoreError::IntegrityMismatch(id))) if id == artifact.id
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
