use serde::{Deserialize, Serialize};

use crate::DeterminismLevel;

/// Minimal reproducibility envelope for a ProofLab computational artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproMeta {
    pub level: DeterminismLevel,
    pub prooflab_revision: String,
    pub lean_version: String,
    pub mathlib_revision: String,
    pub environment_digest: String,
    pub seed: u64,
}

impl ReproMeta {
    #[must_use]
    pub fn bootstrap(environment_digest: impl Into<String>) -> Self {
        Self {
            level: DeterminismLevel::L3,
            prooflab_revision: String::new(),
            lean_version: "v4.33.1".into(),
            mathlib_revision: "0df444a360eaa60ab8c11dca51a86af692955474".into(),
            environment_digest: environment_digest.into(),
            seed: 0,
        }
    }
}
