//! Finite order-sensitivity search for cubic CFI calibration pairs.
//!
//! PL-DC-4 requires every candidate indistinguishability mechanism to survive
//! explicit order expansion. This module is a falsification tool for that stage
//! gate: callers supply finite families of total orders, the unordered CFI pair
//! is calibrated first, and every supplied order pair is then checked with the
//! existing ordered cross-oracle calibration.
//!
//! Exhausting a caller-supplied finite family is not a proof about arbitrary
//! total orders. A found witness is a concrete counterexample for the exact
//! finite bounds and orders recorded here; absence of a witness is only finite
//! computational evidence for the searched family.

use core::fmt;

use crate::{
    CfiBaseGraph, CfiBijectivePebbleCalibrationError, CfiTwistAssignment, CfiWlCalibrationError,
    FiniteStructureId, OrderedCfiCrossCalibration, OrderedCfiCrossCalibrationConfig,
    OrderedCfiCrossCalibrationError, calibrate_cubic_cfi_bijective_pebble,
    calibrate_cubic_cfi_ordered_cross_oracle, calibrate_cubic_cfi_wl,
};

/// One fully replayable ordered witness found during finite order stress testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiOrderSensitivityWitness {
    left_order_index: usize,
    right_order_index: usize,
    left_order: Vec<u64>,
    right_order: Vec<u64>,
    calibration: OrderedCfiCrossCalibration,
}

impl CfiOrderSensitivityWitness {
    /// Index of the left order in the caller-supplied family.
    #[must_use]
    pub const fn left_order_index(&self) -> usize {
        self.left_order_index
    }

    /// Index of the right order in the caller-supplied family.
    #[must_use]
    pub const fn right_order_index(&self) -> usize {
        self.right_order_index
    }

    /// Exact left total order used by this witness.
    #[must_use]
    pub fn left_order(&self) -> &[u64] {
        &self.left_order
    }

    /// Exact right total order used by this witness.
    #[must_use]
    pub fn right_order(&self) -> &[u64] {
        &self.right_order
    }

    /// Exact ordered cross-oracle calibration for this witness.
    #[must_use]
    pub const fn calibration(&self) -> &OrderedCfiCrossCalibration {
        &self.calibration
    }
}

/// Exhaustive result over one explicitly supplied finite Cartesian product of orders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiOrderSensitivitySearch {
    config: OrderedCfiCrossCalibrationConfig,
    baseline_left_structure: FiniteStructureId,
    baseline_right_structure: FiniteStructureId,
    baseline_wl_distinguished: bool,
    baseline_bijective_duplicator_wins: bool,
    baseline_outcomes_agree: bool,
    left_order_count: usize,
    right_order_count: usize,
    tested_pairs: usize,
    first_wl_change: Option<CfiOrderSensitivityWitness>,
    first_bijective_change: Option<CfiOrderSensitivityWitness>,
    first_ordered_oracle_disagreement: Option<CfiOrderSensitivityWitness>,
}

impl CfiOrderSensitivitySearch {
    /// Independently tunable finite oracle bounds used throughout the search.
    #[must_use]
    pub const fn config(&self) -> OrderedCfiCrossCalibrationConfig {
        self.config
    }

    /// Content identity of the unordered left CFI structure used for the baseline.
    #[must_use]
    pub const fn baseline_left_structure(&self) -> FiniteStructureId {
        self.baseline_left_structure
    }

    /// Content identity of the unordered right CFI structure used for the baseline.
    #[must_use]
    pub const fn baseline_right_structure(&self) -> FiniteStructureId {
        self.baseline_right_structure
    }

    /// Unordered coordinate-wise WL outcome at the configured dimension.
    #[must_use]
    pub const fn baseline_wl_distinguished(&self) -> bool {
        self.baseline_wl_distinguished
    }

    /// Unordered bounded bijective-pebble outcome at the configured resources.
    #[must_use]
    pub const fn baseline_bijective_duplicator_wins(&self) -> bool {
        self.baseline_bijective_duplicator_wins
    }

    /// Whether the two unordered finite oracles had matching distinguishability polarity.
    #[must_use]
    pub const fn baseline_outcomes_agree(&self) -> bool {
        self.baseline_outcomes_agree
    }

    /// Number of left orders supplied to the finite search.
    #[must_use]
    pub const fn left_order_count(&self) -> usize {
        self.left_order_count
    }

    /// Number of right orders supplied to the finite search.
    #[must_use]
    pub const fn right_order_count(&self) -> usize {
        self.right_order_count
    }

    /// Number of order pairs actually evaluated.
    #[must_use]
    pub const fn tested_pairs(&self) -> usize {
        self.tested_pairs
    }

    /// First supplied order pair whose WL outcome differs from the unordered baseline.
    #[must_use]
    pub const fn first_wl_change(&self) -> Option<&CfiOrderSensitivityWitness> {
        self.first_wl_change.as_ref()
    }

    /// First supplied order pair whose bijective-game outcome differs from the unordered baseline.
    #[must_use]
    pub const fn first_bijective_change(&self) -> Option<&CfiOrderSensitivityWitness> {
        self.first_bijective_change.as_ref()
    }

    /// First supplied order pair on which the two ordered finite oracles disagree.
    #[must_use]
    pub const fn first_ordered_oracle_disagreement(&self) -> Option<&CfiOrderSensitivityWitness> {
        self.first_ordered_oracle_disagreement.as_ref()
    }

    /// Whether any supplied order pair changed at least one finite-oracle outcome.
    #[must_use]
    pub const fn found_order_sensitivity(&self) -> bool {
        self.first_wl_change.is_some() || self.first_bijective_change.is_some()
    }
}

/// Failure while exhaustively checking one finite family of CFI order expansions.
#[derive(Debug)]
pub enum CfiOrderSensitivityError {
    /// A finite search without left orders is undefined.
    EmptyLeftOrderFamily,
    /// A finite search without right orders is undefined.
    EmptyRightOrderFamily,
    /// The Cartesian-product size cannot be represented by this platform.
    CandidatePairCountOverflow,
    /// The unordered WL baseline failed closed.
    WlBaseline(CfiWlCalibrationError),
    /// The unordered bijective-pebble baseline failed closed.
    BijectiveBaseline(CfiBijectivePebbleCalibrationError),
    /// Independent unordered adapters produced inconsistent content identities.
    BaselineStructureIdentityMismatch,
    /// One ordered cross-oracle calibration failed closed.
    Ordered(OrderedCfiCrossCalibrationError),
}

impl fmt::Display for CfiOrderSensitivityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLeftOrderFamily => formatter.write_str("left CFI order family is empty"),
            Self::EmptyRightOrderFamily => formatter.write_str("right CFI order family is empty"),
            Self::CandidatePairCountOverflow => {
                formatter.write_str("CFI order-family Cartesian product overflows usize")
            }
            Self::WlBaseline(error) => {
                write!(formatter, "unordered CFI WL baseline failed: {error}")
            }
            Self::BijectiveBaseline(error) => write!(
                formatter,
                "unordered CFI bijective-pebble baseline failed: {error}"
            ),
            Self::BaselineStructureIdentityMismatch => formatter.write_str(
                "unordered CFI baseline adapters produced different structure identities",
            ),
            Self::Ordered(error) => write!(formatter, "ordered CFI calibration failed: {error}"),
        }
    }
}

impl std::error::Error for CfiOrderSensitivityError {}

/// Exhaustively search one explicit finite Cartesian product of total orders for
/// finite CFI order sensitivity.
///
/// The unordered WL and bijective-pebble outcomes are evaluated first with the
/// exact resource parameters contained in `config`. Every `left_orders ×
/// right_orders` pair is then evaluated with the existing ordered cross-oracle
/// calibrator. The first observed change for each oracle and the first ordered
/// cross-oracle disagreement are retained as replayable witnesses.
///
/// This function deliberately does not generate orders, sample randomly, or
/// infer anything about unsearched total orders.
///
/// # Errors
///
/// Fails closed for empty order families, an unrepresentable Cartesian-product
/// size, baseline construction/oracle failures, inconsistent baseline identities,
/// or any malformed order / ordered-oracle failure encountered during the sweep.
pub fn search_cubic_cfi_order_sensitivity(
    base: &CfiBaseGraph,
    left_twists: &CfiTwistAssignment,
    right_twists: &CfiTwistAssignment,
    left_orders: &[Vec<u64>],
    right_orders: &[Vec<u64>],
    config: OrderedCfiCrossCalibrationConfig,
) -> Result<CfiOrderSensitivitySearch, CfiOrderSensitivityError> {
    if left_orders.is_empty() {
        return Err(CfiOrderSensitivityError::EmptyLeftOrderFamily);
    }
    if right_orders.is_empty() {
        return Err(CfiOrderSensitivityError::EmptyRightOrderFamily);
    }
    let expected_pairs = left_orders
        .len()
        .checked_mul(right_orders.len())
        .ok_or(CfiOrderSensitivityError::CandidatePairCountOverflow)?;

    let wl = calibrate_cubic_cfi_wl(base, left_twists, right_twists, config.wl_dimension())
        .map_err(CfiOrderSensitivityError::WlBaseline)?;
    let bijective = calibrate_cubic_cfi_bijective_pebble(
        base,
        left_twists,
        right_twists,
        config.pebble_pairs(),
        config.rounds(),
    )
    .map_err(CfiOrderSensitivityError::BijectiveBaseline)?;

    if wl.left_structure() != bijective.left_structure()
        || wl.right_structure() != bijective.right_structure()
    {
        return Err(CfiOrderSensitivityError::BaselineStructureIdentityMismatch);
    }

    let baseline_wl_distinguished = wl.distinguished();
    let baseline_bijective_duplicator_wins = bijective.duplicator_wins();
    let baseline_outcomes_agree =
        baseline_wl_distinguished != baseline_bijective_duplicator_wins;

    let mut tested_pairs = 0usize;
    let mut first_wl_change = None;
    let mut first_bijective_change = None;
    let mut first_ordered_oracle_disagreement = None;

    for (left_order_index, left_order) in left_orders.iter().enumerate() {
        for (right_order_index, right_order) in right_orders.iter().enumerate() {
            let calibration = calibrate_cubic_cfi_ordered_cross_oracle(
                base,
                left_twists,
                right_twists,
                left_order.clone(),
                right_order.clone(),
                config,
            )
            .map_err(CfiOrderSensitivityError::Ordered)?;
            tested_pairs += 1;

            let make_witness = || CfiOrderSensitivityWitness {
                left_order_index,
                right_order_index,
                left_order: left_order.clone(),
                right_order: right_order.clone(),
                calibration: calibration.clone(),
            };

            if first_wl_change.is_none()
                && calibration.wl_distinguished() != baseline_wl_distinguished
            {
                first_wl_change = Some(make_witness());
            }
            if first_bijective_change.is_none()
                && calibration.bijective_duplicator_wins() != baseline_bijective_duplicator_wins
            {
                first_bijective_change = Some(make_witness());
            }
            if first_ordered_oracle_disagreement.is_none() && !calibration.outcomes_agree() {
                first_ordered_oracle_disagreement = Some(make_witness());
            }
        }
    }

    debug_assert_eq!(tested_pairs, expected_pairs);

    Ok(CfiOrderSensitivitySearch {
        config,
        baseline_left_structure: wl.left_structure(),
        baseline_right_structure: wl.right_structure(),
        baseline_wl_distinguished,
        baseline_bijective_duplicator_wins,
        baseline_outcomes_agree,
        left_order_count: left_orders.len(),
        right_order_count: right_orders.len(),
        tested_pairs,
        first_wl_change,
        first_bijective_change,
        first_ordered_oracle_disagreement,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k4() -> CfiBaseGraph {
        CfiBaseGraph::new(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]).unwrap()
    }

    fn natural_order(size: u64) -> Vec<u64> {
        (0..size).collect()
    }

    fn reverse_order(size: u64) -> Vec<u64> {
        (0..size).rev().collect()
    }

    #[test]
    fn empty_families_fail_before_running_oracles() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let config = OrderedCfiCrossCalibrationConfig::new(2, 1, 0);
        let natural = natural_order(40);

        assert!(matches!(
            search_cubic_cfi_order_sensitivity(
                &base,
                &twists,
                &twists,
                &[],
                core::slice::from_ref(&natural),
                config,
            ),
            Err(CfiOrderSensitivityError::EmptyLeftOrderFamily)
        ));
        assert!(matches!(
            search_cubic_cfi_order_sensitivity(
                &base,
                &twists,
                &twists,
                core::slice::from_ref(&natural),
                &[],
                config,
            ),
            Err(CfiOrderSensitivityError::EmptyRightOrderFamily)
        ));
    }

    #[test]
    fn supplied_cartesian_product_is_exhausted_deterministically() {
        let base = k4();
        let even = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let odd =
            CfiTwistAssignment::new(&base, vec![true, false, false, false, false, false]).unwrap();
        let left_orders = vec![natural_order(40), reverse_order(40)];
        let right_orders = vec![natural_order(40)];
        let config = OrderedCfiCrossCalibrationConfig::new(2, 1, 0);

        let first = search_cubic_cfi_order_sensitivity(
            &base,
            &even,
            &odd,
            &left_orders,
            &right_orders,
            config,
        )
        .unwrap();
        let second = search_cubic_cfi_order_sensitivity(
            &base,
            &even,
            &odd,
            &left_orders,
            &right_orders,
            config,
        )
        .unwrap();

        assert_eq!(first, second);
        assert_eq!(first.left_order_count(), 2);
        assert_eq!(first.right_order_count(), 1);
        assert_eq!(first.tested_pairs(), 2);
        assert_ne!(
            first.baseline_left_structure(),
            first.baseline_right_structure()
        );
    }

    #[test]
    fn any_recorded_witness_contains_the_exact_replay_orders() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let left_orders = vec![natural_order(40), reverse_order(40)];
        let right_orders = vec![reverse_order(40), natural_order(40)];
        let config = OrderedCfiCrossCalibrationConfig::new(2, 1, 0);

        let search = search_cubic_cfi_order_sensitivity(
            &base,
            &twists,
            &twists,
            &left_orders,
            &right_orders,
            config,
        )
        .unwrap();

        for witness in [
            search.first_wl_change(),
            search.first_bijective_change(),
            search.first_ordered_oracle_disagreement(),
        ]
        .into_iter()
        .flatten()
        {
            assert_eq!(
                witness.left_order(),
                left_orders[witness.left_order_index()]
            );
            assert_eq!(
                witness.right_order(),
                right_orders[witness.right_order_index()]
            );
            let replay = calibrate_cubic_cfi_ordered_cross_oracle(
                &base,
                &twists,
                &twists,
                witness.left_order().to_vec(),
                witness.right_order().to_vec(),
                config,
            )
            .unwrap();
            assert_eq!(witness.calibration(), &replay);
        }
    }

    #[test]
    fn malformed_order_fails_closed() {
        let base = k4();
        let twists = CfiTwistAssignment::new(&base, vec![false; 6]).unwrap();
        let malformed = vec![0; 40];
        let config = OrderedCfiCrossCalibrationConfig::new(2, 1, 0);

        assert!(matches!(
            search_cubic_cfi_order_sensitivity(
                &base,
                &twists,
                &twists,
                core::slice::from_ref(&malformed),
                core::slice::from_ref(&malformed),
                config,
            ),
            Err(CfiOrderSensitivityError::Ordered(_))
        ));
    }
}
