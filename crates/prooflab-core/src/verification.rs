use core::fmt;

use serde::{Deserialize, Serialize};

use crate::canonical::{Canonical, CanonicalEncoder, sha256_bytes, sha256_canonical};
use crate::{FormalBackend, FormalStatement, FormalStatementId, ProofArtifactId, ReproMeta};

const VERIFICATION_JOB_DOMAIN: &[u8] = b"prooflab-verification-job:v2\0";

/// Stable identity of an immutable trusted-kernel verification request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VerificationJobId(pub [u8; 32]);

/// Immutable input contract for invoking the configured formal proof kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationJob {
    pub id: VerificationJobId,
    pub formal_statement_id: FormalStatementId,
    pub backend: FormalBackend,
    pub proof_source_digest: [u8; 32],
    pub dependencies: Vec<ProofArtifactId>,
    pub invocation: String,
    pub repro: ReproMeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerificationJobBody {
    formal_statement_id: FormalStatementId,
    backend: FormalBackend,
    proof_source_digest: [u8; 32],
    dependencies: Vec<ProofArtifactId>,
    invocation: String,
    repro: ReproMeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationJobError {
    FormalStatementIntegrity,
    EmptyInvocation,
    MissingReproField(&'static str),
}

impl fmt::Display for VerificationJobError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormalStatementIntegrity => write!(formatter, "formal statement id mismatch"),
            Self::EmptyInvocation => write!(formatter, "kernel invocation is empty"),
            Self::MissingReproField(field) => {
                write!(
                    formatter,
                    "required reproducibility field is empty: {field}"
                )
            }
        }
    }
}

impl std::error::Error for VerificationJobError {}

impl VerificationJob {
    /// Build a content-addressed kernel verification request with no prior proof dependencies.
    ///
    /// This does not execute Lean and cannot confer `PROVED` status. A later
    /// kernel receipt must still be accepted by [`crate::ProofArtifact::new_verified`].
    ///
    /// # Errors
    ///
    /// Returns [`VerificationJobError::FormalStatementIntegrity`] when the
    /// formal statement identity is inconsistent, [`VerificationJobError::EmptyInvocation`]
    /// for a blank kernel invocation, or [`VerificationJobError::MissingReproField`]
    /// when required reproducibility metadata is absent.
    pub fn new(
        formal: &FormalStatement,
        proof_source: &[u8],
        invocation: impl Into<String>,
        repro: ReproMeta,
    ) -> Result<Self, VerificationJobError> {
        Self::new_with_dependencies(formal, proof_source, Vec::new(), invocation, repro)
    }

    /// Build a content-addressed kernel verification request whose identity also
    /// binds the normalized set of prerequisite proof artifacts.
    ///
    /// Dependencies are sorted and deduplicated before identity calculation so
    /// callers cannot create distinct job identities by reordering equivalent
    /// prerequisite sets.
    ///
    /// # Errors
    ///
    /// The same integrity and reproducibility checks as [`Self::new`] apply.
    pub fn new_with_dependencies(
        formal: &FormalStatement,
        proof_source: &[u8],
        mut dependencies: Vec<ProofArtifactId>,
        invocation: impl Into<String>,
        repro: ReproMeta,
    ) -> Result<Self, VerificationJobError> {
        if !formal.check_id() {
            return Err(VerificationJobError::FormalStatementIntegrity);
        }
        let invocation = invocation.into();
        if invocation.trim().is_empty() {
            return Err(VerificationJobError::EmptyInvocation);
        }
        validate_repro(&repro)?;
        dependencies.sort_unstable();
        dependencies.dedup();
        let body = VerificationJobBody {
            formal_statement_id: formal.id,
            backend: formal.backend,
            proof_source_digest: sha256_bytes(proof_source),
            dependencies,
            invocation,
            repro,
        };
        Ok(Self {
            id: VerificationJobId(sha256_canonical(VERIFICATION_JOB_DOMAIN, &body)),
            formal_statement_id: body.formal_statement_id,
            backend: body.backend,
            proof_source_digest: body.proof_source_digest,
            dependencies: body.dependencies,
            invocation: body.invocation,
            repro: body.repro,
        })
    }

    #[must_use]
    pub fn check_id(&self) -> bool {
        let body = VerificationJobBody {
            formal_statement_id: self.formal_statement_id,
            backend: self.backend,
            proof_source_digest: self.proof_source_digest,
            dependencies: self.dependencies.clone(),
            invocation: self.invocation.clone(),
            repro: self.repro.clone(),
        };
        self.id == VerificationJobId(sha256_canonical(VERIFICATION_JOB_DOMAIN, &body))
    }

    #[must_use]
    pub fn matches_proof_source(&self, proof_source: &[u8]) -> bool {
        self.proof_source_digest == sha256_bytes(proof_source)
    }
}

fn validate_repro(repro: &ReproMeta) -> Result<(), VerificationJobError> {
    for (name, value) in [
        ("prooflab_revision", repro.prooflab_revision.as_str()),
        ("lean_version", repro.lean_version.as_str()),
        ("mathlib_revision", repro.mathlib_revision.as_str()),
        ("environment_digest", repro.environment_digest.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(VerificationJobError::MissingReproField(name));
        }
    }
    Ok(())
}

impl Canonical for VerificationJobId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for VerificationJobBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.formal_statement_id);
        encoder.value(&self.backend);
        encoder.value(&self.proof_source_digest);
        encoder.value(&self.dependencies);
        encoder.value(&self.invocation);
        encoder.value(&self.repro);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Claim, ClaimBody, FormalStatement};

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
        repro.prooflab_revision = "0123456789abcdef0123456789abcdef01234567".into();
        repro
    }

    #[test]
    fn verification_job_is_deterministic_and_source_bound() {
        let formal = formal();
        let a = VerificationJob::new(&formal, b"by rfl", "lake env lean Main.lean", repro())
            .expect("job");
        let b = VerificationJob::new(&formal, b"by rfl", "lake env lean Main.lean", repro())
            .expect("job");
        assert_eq!(a, b);
        assert!(a.check_id());
        assert!(a.matches_proof_source(b"by rfl"));
        assert!(!a.matches_proof_source(b"by simp"));
        assert!(a.dependencies.is_empty());
    }

    #[test]
    fn proof_source_invocation_or_dependencies_change_job_identity() {
        let formal = formal();
        let a =
            VerificationJob::new(&formal, b"by rfl", "lake env lean A.lean", repro()).expect("job");
        let b = VerificationJob::new(&formal, b"by simp", "lake env lean A.lean", repro())
            .expect("job");
        let c =
            VerificationJob::new(&formal, b"by rfl", "lake env lean B.lean", repro()).expect("job");
        let dependency = ProofArtifactId([7; 32]);
        let d = VerificationJob::new_with_dependencies(
            &formal,
            b"by rfl",
            vec![dependency],
            "lake env lean A.lean",
            repro(),
        )
        .expect("job");
        assert_ne!(a.id, b.id);
        assert_ne!(a.id, c.id);
        assert_ne!(a.id, d.id);
    }

    #[test]
    fn dependencies_are_normalized_before_identity() {
        let formal = formal();
        let a = ProofArtifactId([1; 32]);
        let b = ProofArtifactId([2; 32]);
        let first = VerificationJob::new_with_dependencies(
            &formal,
            b"by rfl",
            vec![b, a, a],
            "lake env lean A.lean",
            repro(),
        )
        .expect("job");
        let second = VerificationJob::new_with_dependencies(
            &formal,
            b"by rfl",
            vec![a, b],
            "lake env lean A.lean",
            repro(),
        )
        .expect("job");
        assert_eq!(first, second);
        assert_eq!(first.dependencies, vec![a, b]);
    }

    #[test]
    fn empty_invocation_and_incomplete_repro_fail_closed() {
        let formal = formal();
        assert_eq!(
            VerificationJob::new(&formal, b"by rfl", " ", repro()),
            Err(VerificationJobError::EmptyInvocation)
        );
        assert_eq!(
            VerificationJob::new(
                &formal,
                b"by rfl",
                "lake env lean A.lean",
                ReproMeta::bootstrap("env"),
            ),
            Err(VerificationJobError::MissingReproField("prooflab_revision"))
        );
    }
}
