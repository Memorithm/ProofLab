//! Reproducibility taxonomy adapted from the SciRust/SOS `sos-core` design.
//!
//! Upstream reference: Memorithm/SciRust@267af914bd4a72af531c3d22afd222dd62bd3a70,
//! `sos/sos-core/src/determinism.rs`.

use serde::{Deserialize, Serialize};

/// Reproducibility strength, from weakest (`L0`) to strongest (`L3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeterminismLevel {
    /// Non-deterministic but recorded.
    L0,
    /// Statistically reproducible.
    L1,
    /// Numerically reproducible within a declared certificate/tolerance.
    L2,
    /// Bit-reproducible for the declared environment/contract.
    L3,
}

impl DeterminismLevel {
    /// Propagate the weakest level along a dependency edge.
    #[must_use]
    pub fn meet(self, other: Self) -> Self {
        self.min(other)
    }

    /// Propagate over an entire dependency set. `L3` is the empty identity.
    #[must_use]
    pub fn min_over<I: IntoIterator<Item = Self>>(levels: I) -> Self {
        levels.into_iter().fold(Self::L3, Self::meet)
    }
}

#[cfg(test)]
mod tests {
    use super::DeterminismLevel::{L0, L1, L2, L3};
    use super::*;

    #[test]
    fn meet_propagates_the_weakest_level() {
        assert_eq!(L3.meet(L1), L1);
        assert_eq!(L2.meet(L0), L0);
        assert_eq!(DeterminismLevel::min_over([L3, L2, L3]), L2);
    }
}
