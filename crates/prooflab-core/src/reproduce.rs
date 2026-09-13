//! `prooflab reproduce` library semantics.
//!
//! Reproduction checks integrity and environment binding. It does **not**
//! authorize `PROVED` status. Creating or updating a proof artifact still
//! requires an accepted [`crate::KernelReceipt`] through
//! [`crate::ProofArtifact::new_verified`].

use core::fmt;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::env_lock::{DriftReport, EnvironmentLock, EnvironmentLockError, EnvironmentLockId};
use crate::{ProofArtifact, ProofArtifactId};

/// Successful environment/integrity reproduce check.
///
/// This confirms the stored artifact still checks and the observed environment
/// binds the locked pins. It is not a mathematical status transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproduceOk {
    pub artifact_id: ProofArtifactId,
    pub lock_id: EnvironmentLockId,
}

/// Fail-closed reproduce outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReproduceError {
    ArtifactIntegrity,
    LockIntegrity,
    LockConstruction(EnvironmentLockError),
    Drift(DriftReport),
    MissingDependencies(Vec<ProofArtifactId>),
}

impl fmt::Display for ReproduceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactIntegrity => write!(formatter, "proof artifact id mismatch"),
            Self::LockIntegrity => write!(formatter, "environment lock id mismatch"),
            Self::LockConstruction(error) => write!(formatter, "environment lock error: {error}"),
            Self::Drift(report) => write!(formatter, "{report}"),
            Self::MissingDependencies(ids) => {
                write!(formatter, "missing proof dependencies: {ids:?}")
            }
        }
    }
}

impl std::error::Error for ReproduceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LockConstruction(error) => Some(error),
            Self::ArtifactIntegrity
            | Self::LockIntegrity
            | Self::Drift(_)
            | Self::MissingDependencies(_) => None,
        }
    }
}

impl From<EnvironmentLockError> for ReproduceError {
    fn from(value: EnvironmentLockError) -> Self {
        Self::LockConstruction(value)
    }
}

/// Verify a stored proof artifact against an observed environment lock.
///
/// Steps (fail closed):
/// 1. content-address integrity of the artifact;
/// 2. derive the lock pinned by the artifact's `ReproMeta`;
/// 3. integrity of the observed lock;
/// 4. itemized drift comparison (expected = artifact lock, actual = observed);
/// 5. optionally require every declared dependency id to be present.
///
/// Successful return means the artifact and environment bind for reproduction.
/// It does **not** create, update, or authorize `PROVED` status.
///
/// # Errors
///
/// Returns a structured [`ReproduceError`] on integrity failure, lock
/// construction failure, drift, or missing dependencies.
pub fn reproduce(
    artifact: &ProofArtifact,
    observed: &EnvironmentLock,
    available_dependencies: Option<&BTreeSet<ProofArtifactId>>,
) -> Result<ReproduceOk, ReproduceError> {
    if !artifact.check_id() {
        return Err(ReproduceError::ArtifactIntegrity);
    }
    if !observed.check_id() {
        return Err(ReproduceError::LockIntegrity);
    }

    let expected = EnvironmentLock::from_repro(&artifact.body.repro)?;
    if !expected.check_id() {
        return Err(ReproduceError::LockIntegrity);
    }

    let drift = expected.compare(observed);
    if !drift.binds() {
        return Err(ReproduceError::Drift(drift));
    }

    if let Some(available) = available_dependencies {
        let missing: Vec<_> = artifact
            .body
            .dependencies
            .iter()
            .copied()
            .filter(|dependency| !available.contains(dependency))
            .collect();
        if !missing.is_empty() {
            return Err(ReproduceError::MissingDependencies(missing));
        }
    }

    Ok(ReproduceOk {
        artifact_id: artifact.id,
        lock_id: expected.id,
    })
}

/// Derive the environment lock pinned by a proof artifact.
///
/// # Errors
///
/// Fails when the artifact identity is inconsistent or required repro pins are empty.
pub fn lock_for_artifact(artifact: &ProofArtifact) -> Result<EnvironmentLock, ReproduceError> {
    if !artifact.check_id() {
        return Err(ReproduceError::ArtifactIntegrity);
    }
    Ok(EnvironmentLock::from_repro(&artifact.body.repro)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Claim, ClaimBody, FormalBackend, FormalStatement, KernelReceipt, ProofArtifact, ReproMeta,
        sha256_bytes,
    };

    fn artifact_with(repro: ReproMeta, dependencies: Vec<ProofArtifactId>) -> ProofArtifact {
        let claim = Claim::new(ClaimBody {
            statement: "n = n".into(),
            assumptions: vec![],
            parents: vec![],
        });
        let source = b"theorem refl : True := by trivial\n";
        let formal = FormalStatement::lean4(claim.id, source, vec!["Mathlib".into()]);
        let receipt = KernelReceipt::new(
            FormalBackend::Lean4,
            "lake env lean",
            true,
            Some(0),
            sha256_bytes(b"stdout"),
            sha256_bytes(b"stderr"),
        );
        ProofArtifact::new_verified(&formal, source, dependencies, repro, receipt).unwrap()
    }

    fn repro(digest: &str) -> ReproMeta {
        let mut repro = ReproMeta::bootstrap(digest);
        repro.prooflab_revision = "f062f108".into();
        repro
    }

    #[test]
    fn matching_environment_reproduces() {
        let artifact = artifact_with(repro("sha256:environment"), vec![]);
        let observed = EnvironmentLock::from_repro(&artifact.body.repro).unwrap();
        let ok = reproduce(&artifact, &observed, None).expect("reproduce");
        assert_eq!(ok.artifact_id, artifact.id);
        assert_eq!(ok.lock_id, observed.id);
    }

    #[test]
    fn drift_fails_closed_with_structured_fields() {
        let artifact = artifact_with(repro("sha256:environment"), vec![]);
        let observed = EnvironmentLock::new(
            "v4.33.1",
            "0df444a360eaa60ab8c11dca51a86af692955474",
            "f062f108",
            "sha256:different",
        )
        .unwrap();
        match reproduce(&artifact, &observed, None) {
            Err(ReproduceError::Drift(report)) => {
                assert_eq!(report.fields.len(), 1);
                assert_eq!(report.fields[0].field, "environment_digest");
            }
            other => panic!("expected drift, got {other:?}"),
        }
    }

    #[test]
    fn tampered_artifact_fails_integrity() {
        let mut artifact = artifact_with(repro("sha256:environment"), vec![]);
        artifact.body.repro.environment_digest = "tampered".into();
        let observed = EnvironmentLock::bootstrap("f062f108", "sha256:environment").unwrap();
        assert_eq!(
            reproduce(&artifact, &observed, None),
            Err(ReproduceError::ArtifactIntegrity)
        );
    }

    #[test]
    fn missing_dependencies_fail_closed() {
        let missing = ProofArtifactId([9; 32]);
        let artifact = artifact_with(repro("sha256:environment"), vec![missing]);
        let observed = EnvironmentLock::from_repro(&artifact.body.repro).unwrap();
        let available = BTreeSet::new();
        assert_eq!(
            reproduce(&artifact, &observed, Some(&available)),
            Err(ReproduceError::MissingDependencies(vec![missing]))
        );
    }

    #[test]
    fn present_dependencies_allow_reproduce() {
        let dep = ProofArtifactId([1; 32]);
        let artifact = artifact_with(repro("sha256:environment"), vec![dep]);
        let observed = EnvironmentLock::from_repro(&artifact.body.repro).unwrap();
        let available = BTreeSet::from([dep]);
        assert!(reproduce(&artifact, &observed, Some(&available)).is_ok());
    }
}
