//! Content-addressed environment lock and fail-closed drift detection.
//!
//! Contracts are selectively adapted from
//! `Memorithm/SciRust@267af914bd4a72af531c3d22afd222dd62bd3a70`,
//! `sos/sos-repro/src/lock.rs`: pin exact toolchain fields, declare every
//! difference, never silently proceed on mismatch. The field set is
//! `ProofLab`-specific (Lean / mathlib / `ProofLab` revision / environment digest).

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::ReproMeta;
use crate::canonical::{Canonical, CanonicalEncoder, sha256_canonical};

const ENVIRONMENT_LOCK_DOMAIN: &[u8] = b"prooflab-environment-lock:v1\0";

/// Stable identity of an immutable environment lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EnvironmentLockId(pub [u8; 32]);

/// Normative toolchain pins that participate in content addressing.
///
/// Wall-clock times, random IDs and machine paths are intentionally excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentLockBody {
    pub lean_version: String,
    pub mathlib_revision: String,
    pub prooflab_revision: String,
    pub environment_digest: String,
}

/// Content-addressed lock of the trusted formal toolchain environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentLock {
    pub id: EnvironmentLockId,
    pub body: EnvironmentLockBody,
}

/// One localized difference between an expected lock and an observed environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftField {
    pub field: String,
    pub expected: String,
    pub actual: String,
}

/// Structured drift report. Empty means the environments bind for reproduction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DriftReport {
    pub fields: Vec<DriftField>,
}

impl DriftReport {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    #[must_use]
    pub fn binds(&self) -> bool {
        self.is_empty()
    }
}

impl fmt::Display for DriftReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.fields.is_empty() {
            return write!(formatter, "no environment drift");
        }
        write!(formatter, "environment drift:")?;
        for field in &self.fields {
            write!(
                formatter,
                " {} (expected `{}`, actual `{}`);",
                field.field, field.expected, field.actual
            )?;
        }
        Ok(())
    }
}

/// Failure while constructing an environment lock from incomplete pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentLockError {
    MissingField(&'static str),
}

impl fmt::Display for EnvironmentLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => {
                write!(
                    formatter,
                    "required environment lock field is empty: {field}"
                )
            }
        }
    }
}

impl std::error::Error for EnvironmentLockError {}

impl EnvironmentLock {
    /// Build a content-addressed lock from exact normative pins.
    ///
    /// # Errors
    ///
    /// Returns [`EnvironmentLockError::MissingField`] when any required pin is blank.
    pub fn new(
        lean_version: impl Into<String>,
        mathlib_revision: impl Into<String>,
        prooflab_revision: impl Into<String>,
        environment_digest: impl Into<String>,
    ) -> Result<Self, EnvironmentLockError> {
        let body = EnvironmentLockBody {
            lean_version: lean_version.into(),
            mathlib_revision: mathlib_revision.into(),
            prooflab_revision: prooflab_revision.into(),
            environment_digest: environment_digest.into(),
        };
        Self::from_body(body)
    }

    /// Capture a lock from reproducibility metadata already bound to an artifact or job.
    ///
    /// # Errors
    ///
    /// Fails closed when any normative `ReproMeta` field is empty.
    pub fn from_repro(repro: &ReproMeta) -> Result<Self, EnvironmentLockError> {
        Self::new(
            repro.lean_version.clone(),
            repro.mathlib_revision.clone(),
            repro.prooflab_revision.clone(),
            repro.environment_digest.clone(),
        )
    }

    /// Bootstrap lock matching the pinned Lean/mathlib toolchain defaults.
    ///
    /// # Errors
    ///
    /// Fails when `prooflab_revision` or `environment_digest` is empty.
    pub fn bootstrap(
        prooflab_revision: impl Into<String>,
        environment_digest: impl Into<String>,
    ) -> Result<Self, EnvironmentLockError> {
        let mut repro = ReproMeta::bootstrap(environment_digest);
        repro.prooflab_revision = prooflab_revision.into();
        Self::from_repro(&repro)
    }

    fn from_body(body: EnvironmentLockBody) -> Result<Self, EnvironmentLockError> {
        validate_body(&body)?;
        Ok(Self {
            id: EnvironmentLockId(sha256_canonical(ENVIRONMENT_LOCK_DOMAIN, &body)),
            body,
        })
    }

    /// Return whether the stored identifier still matches the lock body.
    #[must_use]
    pub fn check_id(&self) -> bool {
        self.id == EnvironmentLockId(sha256_canonical(ENVIRONMENT_LOCK_DOMAIN, &self.body))
    }

    /// Itemized differences from this lock (expected) to `observed` (actual).
    ///
    /// An empty report means the environments bind. Differences are listed in
    /// deterministic field order and never collapsed into a boolean alone.
    #[must_use]
    pub fn compare(&self, observed: &Self) -> DriftReport {
        let mut fields = Vec::new();
        push_drift(
            &mut fields,
            "lean_version",
            &self.body.lean_version,
            &observed.body.lean_version,
        );
        push_drift(
            &mut fields,
            "mathlib_revision",
            &self.body.mathlib_revision,
            &observed.body.mathlib_revision,
        );
        push_drift(
            &mut fields,
            "prooflab_revision",
            &self.body.prooflab_revision,
            &observed.body.prooflab_revision,
        );
        push_drift(
            &mut fields,
            "environment_digest",
            &self.body.environment_digest,
            &observed.body.environment_digest,
        );
        DriftReport { fields }
    }

    /// Whether this lock pins the identical normative environment as `observed`.
    #[must_use]
    pub fn binds(&self, observed: &Self) -> bool {
        self.compare(observed).binds()
    }
}

fn validate_body(body: &EnvironmentLockBody) -> Result<(), EnvironmentLockError> {
    for (name, value) in [
        ("lean_version", body.lean_version.as_str()),
        ("mathlib_revision", body.mathlib_revision.as_str()),
        ("prooflab_revision", body.prooflab_revision.as_str()),
        ("environment_digest", body.environment_digest.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(EnvironmentLockError::MissingField(name));
        }
    }
    Ok(())
}

fn push_drift(fields: &mut Vec<DriftField>, field: &str, expected: &str, actual: &str) {
    if expected != actual {
        fields.push(DriftField {
            field: field.into(),
            expected: expected.into(),
            actual: actual.into(),
        });
    }
}

impl Canonical for EnvironmentLockId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for EnvironmentLockBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.lean_version);
        encoder.value(&self.mathlib_revision);
        encoder.value(&self.prooflab_revision);
        encoder.value(&self.environment_digest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock(digest: &str) -> EnvironmentLock {
        EnvironmentLock::new(
            "v4.33.1",
            "0df444a360eaa60ab8c11dca51a86af692955474",
            "deadbeef",
            digest,
        )
        .expect("lock")
    }

    #[test]
    fn lock_identity_is_deterministic_and_content_addressed() {
        let a = lock("sha256:env-a");
        let b = lock("sha256:env-a");
        let c = lock("sha256:env-b");
        assert_eq!(a, b);
        assert!(a.check_id());
        assert_ne!(a.id, c.id);
    }

    #[test]
    fn identical_locks_bind_with_no_drift() {
        let a = lock("sha256:env");
        let b = lock("sha256:env");
        assert!(a.binds(&b));
        assert!(a.compare(&b).is_empty());
    }

    #[test]
    fn drift_is_itemized_and_localized() {
        let expected = lock("sha256:env");
        let observed = EnvironmentLock::new(
            "v4.34.0",
            "0df444a360eaa60ab8c11dca51a86af692955474",
            "cafebabe",
            "sha256:other",
        )
        .expect("lock");
        let report = expected.compare(&observed);
        assert!(!expected.binds(&observed));
        assert_eq!(
            report.fields,
            vec![
                DriftField {
                    field: "lean_version".into(),
                    expected: "v4.33.1".into(),
                    actual: "v4.34.0".into(),
                },
                DriftField {
                    field: "prooflab_revision".into(),
                    expected: "deadbeef".into(),
                    actual: "cafebabe".into(),
                },
                DriftField {
                    field: "environment_digest".into(),
                    expected: "sha256:env".into(),
                    actual: "sha256:other".into(),
                },
            ]
        );
    }

    #[test]
    fn empty_pins_fail_closed() {
        assert_eq!(
            EnvironmentLock::new("v4.33.1", "mathlib", "", "env"),
            Err(EnvironmentLockError::MissingField("prooflab_revision"))
        );
        assert_eq!(
            EnvironmentLock::from_repro(&ReproMeta::bootstrap("env")),
            Err(EnvironmentLockError::MissingField("prooflab_revision"))
        );
    }

    #[test]
    fn from_repro_round_trips_normative_fields() {
        let mut repro = ReproMeta::bootstrap("sha256:environment");
        repro.prooflab_revision = "f062f108".into();
        let lock = EnvironmentLock::from_repro(&repro).expect("lock");
        assert_eq!(lock.body.lean_version, repro.lean_version);
        assert_eq!(lock.body.mathlib_revision, repro.mathlib_revision);
        assert_eq!(lock.body.prooflab_revision, repro.prooflab_revision);
        assert_eq!(lock.body.environment_digest, repro.environment_digest);
    }
}
