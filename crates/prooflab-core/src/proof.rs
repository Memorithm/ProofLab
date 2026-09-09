use core::fmt;

use serde::{Deserialize, Serialize};

use crate::canonical::{Canonical, CanonicalEncoder, sha256_bytes, sha256_canonical};
use crate::{ClaimId, DeterminismLevel, FormalBackend, FormalStatement, FormalStatementId, ReproMeta};

const PROOF_ARTIFACT_DOMAIN: &[u8] = b"prooflab-proof-artifact:v1\0";

/// Stable identifier of an immutable verified proof artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProofArtifactId(pub [u8; 32]);

/// Normalized receipt from the configured formal proof kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelReceipt {
    pub backend: FormalBackend,
    pub invocation: String,
    pub accepted: bool,
    pub exit_code: Option<i32>,
    pub stdout_digest: [u8; 32],
    pub stderr_digest: [u8; 32],
}

impl KernelReceipt {
    /// Create a receipt from exact process-output digests.
    #[must_use]
    pub fn new(
        backend: FormalBackend,
        invocation: impl Into<String>,
        accepted: bool,
        exit_code: Option<i32>,
        stdout_digest: [u8; 32],
        stderr_digest: [u8; 32],
    ) -> Self {
        Self {
            backend,
            invocation: invocation.into(),
            accepted,
            exit_code,
            stdout_digest,
            stderr_digest,
        }
    }
}

/// Immutable content that determines a proof artifact's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofArtifactBody {
    pub claim_id: ClaimId,
    pub formal_statement_id: FormalStatementId,
    pub proof_source_digest: [u8; 32],
    pub dependencies: Vec<ProofArtifactId>,
    pub repro: ReproMeta,
    pub kernel: KernelReceipt,
}

/// Content-addressed evidence that the configured kernel accepted a formal proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofArtifact {
    pub id: ProofArtifactId,
    pub body: ProofArtifactBody,
}

/// Failure to construct a trusted proof artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofArtifactError {
    FormalStatementIntegrity,
    KernelRejected,
    KernelExitNotZero(Option<i32>),
    BackendMismatch,
    MissingReproField(&'static str),
}

impl fmt::Display for ProofArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormalStatementIntegrity => write!(formatter, "formal statement id mismatch"),
            Self::KernelRejected => write!(formatter, "formal kernel rejected the proof"),
            Self::KernelExitNotZero(code) => {
                write!(formatter, "formal kernel did not exit successfully: {code:?}")
            }
            Self::BackendMismatch => write!(formatter, "kernel backend does not match formal statement"),
            Self::MissingReproField(field) => {
                write!(formatter, "required reproducibility field is empty: {field}")
            }
        }
    }
}

impl std::error::Error for ProofArtifactError {}

impl ProofArtifact {
    /// Construct a proof artifact only from a successful, integrity-checked kernel result.
    ///
    /// Dependencies are sorted and deduplicated before identity calculation.
    ///
    /// # Errors
    ///
    /// Fails if the formal statement is internally inconsistent, the kernel did
    /// not accept with exit code zero, the backend differs, or required
    /// reproducibility metadata is missing.
    pub fn new_verified(
        formal: &FormalStatement,
        proof_source: &[u8],
        mut dependencies: Vec<ProofArtifactId>,
        repro: ReproMeta,
        kernel: KernelReceipt,
    ) -> Result<Self, ProofArtifactError> {
        if !formal.check_id() {
            return Err(ProofArtifactError::FormalStatementIntegrity);
        }
        if !kernel.accepted {
            return Err(ProofArtifactError::KernelRejected);
        }
        if kernel.exit_code != Some(0) {
            return Err(ProofArtifactError::KernelExitNotZero(kernel.exit_code));
        }
        if kernel.backend != formal.backend {
            return Err(ProofArtifactError::BackendMismatch);
        }
        validate_repro(&repro)?;
        dependencies.sort_unstable();
        dependencies.dedup();
        let body = ProofArtifactBody {
            claim_id: formal.claim_id,
            formal_statement_id: formal.id,
            proof_source_digest: sha256_bytes(proof_source),
            dependencies,
            repro,
            kernel,
        };
        Ok(Self {
            id: ProofArtifactId(sha256_canonical(PROOF_ARTIFACT_DOMAIN, &body)),
            body,
        })
    }

    /// Return whether the stored identifier still matches the artifact body.
    #[must_use]
    pub fn check_id(&self) -> bool {
        self.id == ProofArtifactId(sha256_canonical(PROOF_ARTIFACT_DOMAIN, &self.body))
    }
}

fn validate_repro(repro: &ReproMeta) -> Result<(), ProofArtifactError> {
    for (name, value) in [
        ("prooflab_revision", repro.prooflab_revision.as_str()),
        ("lean_version", repro.lean_version.as_str()),
        ("mathlib_revision", repro.mathlib_revision.as_str()),
        ("environment_digest", repro.environment_digest.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ProofArtifactError::MissingReproField(name));
        }
    }
    Ok(())
}

impl Canonical for ProofArtifactId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for KernelReceipt {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.backend);
        encoder.value(&self.invocation);
        encoder.value(&self.accepted);
        encoder.value(&self.exit_code);
        encoder.value(&self.stdout_digest);
        encoder.value(&self.stderr_digest);
    }
}

impl Canonical for ReproMeta {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        let level = match self.level {
            DeterminismLevel::L0 => 0u8,
            DeterminismLevel::L1 => 1u8,
            DeterminismLevel::L2 => 2u8,
            DeterminismLevel::L3 => 3u8,
        };
        encoder.value(&level);
        encoder.value(&self.prooflab_revision);
        encoder.value(&self.lean_version);
        encoder.value(&self.mathlib_revision);
        encoder.value(&self.environment_digest);
        encoder.value(&self.seed);
    }
}

impl Canonical for ProofArtifactBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.claim_id);
        encoder.value(&self.formal_statement_id);
        encoder.value(&self.proof_source_digest);
        encoder.value(&self.dependencies);
        encoder.value(&self.repro);
        encoder.value(&self.kernel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Claim, ClaimBody};

    fn formal() -> FormalStatement {
        let claim = Claim::new(ClaimBody {
            statement: "n = n".into(),
            assumptions: vec!["n : Nat".into()],
            parents: vec![],
        });
        FormalStatement::lean4(
            claim.id,
            b"theorem refl (n : Nat) : n = n := rfl\n",
            vec!["Mathlib".into()],
        )
    }

    fn repro() -> ReproMeta {
        let mut repro = ReproMeta::bootstrap("sha256:environment");
        repro.prooflab_revision = "f062f108".into();
        repro
    }

    fn receipt(accepted: bool, exit_code: Option<i32>) -> KernelReceipt {
        KernelReceipt::new(
            FormalBackend::Lean4,
            "lake env lean",
            accepted,
            exit_code,
            sha256_bytes(b"stdout"),
            sha256_bytes(b"stderr"),
        )
    }

    #[test]
    fn verified_artifact_is_deterministic() {
        let formal = formal();
        let a = ProofArtifact::new_verified(&formal, b"proof source", vec![], repro(), receipt(true, Some(0))).unwrap();
        let b = ProofArtifact::new_verified(&formal, b"proof source", vec![], repro(), receipt(true, Some(0))).unwrap();
        assert_eq!(a, b);
        assert!(a.check_id());
    }

    #[test]
    fn rejected_kernel_cannot_create_proof_artifact() {
        let formal = formal();
        let result = ProofArtifact::new_verified(
            &formal,
            b"bad proof",
            vec![],
            repro(),
            receipt(false, Some(1)),
        );
        assert_eq!(result, Err(ProofArtifactError::KernelRejected));
    }

    #[test]
    fn incomplete_reproducibility_metadata_is_rejected() {
        let formal = formal();
        let result = ProofArtifact::new_verified(
            &formal,
            b"proof",
            vec![],
            ReproMeta::bootstrap("env"),
            receipt(true, Some(0)),
        );
        assert_eq!(
            result,
            Err(ProofArtifactError::MissingReproField("prooflab_revision"))
        );
    }
}
