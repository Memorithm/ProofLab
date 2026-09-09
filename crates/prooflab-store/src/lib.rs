//! Content-addressed proof storage and deterministic provenance queries.
//!
//! The behavioral contracts are selectively adapted from
//! `Memorithm/SciRust@267af914bd4a72af531c3d22afd222dd62bd3a70`,
//! especially `sos/sos-store/src/store.rs`: integrity-checked puts/gets,
//! idempotent first-wins storage and deterministic enumeration. The data model
//! remains `ProofLab`-specific.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use prooflab_core::{ProofArtifact, ProofArtifactId};

/// Storage or provenance-integrity failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    IntegrityMismatch(ProofArtifactId),
    MissingArtifact(ProofArtifactId),
    MissingDependency(ProofArtifactId),
    AddressCollision(ProofArtifactId),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntegrityMismatch(id) => write!(formatter, "proof artifact id mismatch: {id:?}"),
            Self::MissingArtifact(id) => write!(formatter, "proof artifact is missing: {id:?}"),
            Self::MissingDependency(id) => write!(formatter, "proof dependency is missing: {id:?}"),
            Self::AddressCollision(id) => write!(formatter, "different proof content shares address: {id:?}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Integrity-checked storage interface for verified proof artifacts.
pub trait ProofStore {
    /// Store a verified artifact idempotently.
    ///
    /// # Errors
    ///
    /// Fails closed when the artifact id is invalid, a declared dependency is
    /// absent, or existing content under the same address differs.
    fn put(&mut self, artifact: &ProofArtifact) -> Result<ProofArtifactId, StoreError>;

    /// Fetch and re-check an artifact.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::IntegrityMismatch`] if stored content no longer
    /// matches its content address.
    fn get(&self, id: ProofArtifactId) -> Result<Option<ProofArtifact>, StoreError>;

    /// Return whether `id` is present.
    #[must_use]
    fn has(&self, id: ProofArtifactId) -> bool;

    /// Return every stored proof id in deterministic sorted order.
    #[must_use]
    fn proof_ids(&self) -> Vec<ProofArtifactId>;

    /// Return all transitive proof dependencies of `root`, sorted by id.
    ///
    /// # Errors
    ///
    /// Fails when the root or any dependency is missing, or when stored content
    /// fails its integrity check.
    fn ancestors(&self, root: ProofArtifactId) -> Result<Vec<ProofArtifactId>, StoreError>;

    /// Return all stored proofs transitively depending on `root`, sorted by id.
    ///
    /// # Errors
    ///
    /// Fails when the root is missing or stored content fails its integrity check.
    fn descendants(&self, root: ProofArtifactId) -> Result<Vec<ProofArtifactId>, StoreError>;
}

/// In-memory backend used for bootstrap, tests and deterministic orchestration.
#[derive(Debug, Clone, Default)]
pub struct MemoryProofStore {
    proofs: BTreeMap<ProofArtifactId, ProofArtifact>,
}

impl MemoryProofStore {
    /// Number of stored proof artifacts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.proofs.len()
    }

    /// Whether the store contains no proof artifacts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.proofs.is_empty()
    }
}

impl ProofStore for MemoryProofStore {
    fn put(&mut self, artifact: &ProofArtifact) -> Result<ProofArtifactId, StoreError> {
        if !artifact.check_id() {
            return Err(StoreError::IntegrityMismatch(artifact.id));
        }
        for dependency in &artifact.body.dependencies {
            if !self.proofs.contains_key(dependency) {
                return Err(StoreError::MissingDependency(*dependency));
            }
        }
        if let Some(existing) = self.proofs.get(&artifact.id) {
            if existing != artifact {
                return Err(StoreError::AddressCollision(artifact.id));
            }
            return Ok(artifact.id);
        }
        self.proofs.insert(artifact.id, artifact.clone());
        Ok(artifact.id)
    }

    fn get(&self, id: ProofArtifactId) -> Result<Option<ProofArtifact>, StoreError> {
        let Some(artifact) = self.proofs.get(&id) else {
            return Ok(None);
        };
        if !artifact.check_id() {
            return Err(StoreError::IntegrityMismatch(id));
        }
        Ok(Some(artifact.clone()))
    }

    fn has(&self, id: ProofArtifactId) -> bool {
        self.proofs.contains_key(&id)
    }

    fn proof_ids(&self) -> Vec<ProofArtifactId> {
        self.proofs.keys().copied().collect()
    }

    fn ancestors(&self, root: ProofArtifactId) -> Result<Vec<ProofArtifactId>, StoreError> {
        let Some(root_artifact) = self.get(root)? else {
            return Err(StoreError::MissingArtifact(root));
        };
        let mut seen = BTreeSet::new();
        let mut stack = root_artifact.body.dependencies;
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(artifact) = self.get(id)? else {
                return Err(StoreError::MissingDependency(id));
            };
            stack.extend(artifact.body.dependencies);
        }
        Ok(seen.into_iter().collect())
    }

    fn descendants(&self, root: ProofArtifactId) -> Result<Vec<ProofArtifactId>, StoreError> {
        if self.get(root)?.is_none() {
            return Err(StoreError::MissingArtifact(root));
        }
        let mut seen = BTreeSet::new();
        let mut frontier = vec![root];
        while let Some(parent) = frontier.pop() {
            for id in self.proof_ids() {
                if seen.contains(&id) || id == root {
                    continue;
                }
                let Some(artifact) = self.get(id)? else {
                    continue;
                };
                if artifact.body.dependencies.contains(&parent) && seen.insert(id) {
                    frontier.push(id);
                }
            }
        }
        Ok(seen.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use prooflab_core::{
        Claim, ClaimBody, FormalBackend, FormalStatement, KernelReceipt, ProofArtifact,
        ProofArtifactId, ReproMeta, sha256_bytes,
    };

    use super::*;

    fn independent_proof(name: &str) -> ProofArtifact {
        proof(name, vec![])
    }

    fn proof(name: &str, dependencies: Vec<ProofArtifactId>) -> ProofArtifact {
        let claim = Claim::new(ClaimBody {
            statement: format!("{name} = {name}"),
            assumptions: vec![],
            parents: vec![],
        });
        let source = format!("theorem {name} : True := by trivial\n");
        let formal = FormalStatement::lean4(claim.id, source.as_bytes(), vec!["Mathlib".into()]);
        let mut repro = ReproMeta::bootstrap("sha256:environment");
        repro.prooflab_revision = "test-revision".into();
        let receipt = KernelReceipt::new(
            FormalBackend::Lean4,
            "lake env lean",
            true,
            Some(0),
            sha256_bytes(b"stdout"),
            sha256_bytes(b"stderr"),
        );
        ProofArtifact::new_verified(
            &formal,
            source.as_bytes(),
            dependencies,
            repro,
            receipt,
        )
        .unwrap()
    }

    #[test]
    fn put_is_idempotent_and_enumeration_is_sorted() {
        let a = independent_proof("a");
        let b = independent_proof("b");
        let mut store = MemoryProofStore::default();
        store.put(&b).unwrap();
        store.put(&a).unwrap();
        store.put(&a).unwrap();
        let mut expected = vec![a.id, b.id];
        expected.sort_unstable();
        assert_eq!(store.proof_ids(), expected);
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn missing_dependency_fails_closed() {
        let missing = ProofArtifactId([9; 32]);
        let child = proof("child", vec![missing]);
        let mut store = MemoryProofStore::default();
        assert_eq!(store.put(&child), Err(StoreError::MissingDependency(missing)));
    }

    #[test]
    fn ancestry_and_impact_queries_are_transitive() {
        let root = independent_proof("root");
        let middle = proof("middle", vec![root.id]);
        let leaf = proof("leaf", vec![middle.id]);
        let mut store = MemoryProofStore::default();
        store.put(&root).unwrap();
        store.put(&middle).unwrap();
        store.put(&leaf).unwrap();

        let mut ancestors = vec![root.id, middle.id];
        ancestors.sort_unstable();
        assert_eq!(store.ancestors(leaf.id).unwrap(), ancestors);

        let mut descendants = vec![middle.id, leaf.id];
        descendants.sort_unstable();
        assert_eq!(store.descendants(root.id).unwrap(), descendants);
    }

    #[test]
    fn tampered_artifact_is_rejected() {
        let mut artifact = independent_proof("tampered");
        artifact.body.repro.environment_digest = "modified".into();
        let mut store = MemoryProofStore::default();
        assert_eq!(store.put(&artifact), Err(StoreError::IntegrityMismatch(artifact.id)));
    }
}
