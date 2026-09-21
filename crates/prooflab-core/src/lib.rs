//! Trusted, backend-independent data types for `ProofLab`.
//!
//! This crate intentionally contains no theorem prover. It defines immutable
//! claim identity, formal statements, proof artifacts, mathematical lifecycle
//! state, reproducibility metadata, typed scientific evidence (PL-2.0 / PL-C15), label-preserving Riemann/TDI stub adapters,
//! finite descriptive-complexity syntax and evaluation, and exact finite
//! model-comparison game oracles. Proof status is never inferred from
//! reproducibility or empirical evidence.

#![forbid(unsafe_code)]

mod bench_adapter;
mod bijective_pebble;
mod canonical;
mod cfi;
mod cfi_bijective_calibration;
mod cfi_order_sensitivity;
mod cfi_ordered_bijective_calibration;
mod cfi_ordered_cross_calibration;
mod cfi_ordered_wl_calibration;
mod cfi_relational;
mod cfi_wl_calibration;
mod claim;
mod descriptive;
mod determinism;
mod ef;
mod env_lock;
mod eso;
mod eso_eval;
mod evidence;
mod falsify;
mod fo;
mod fo_eval;
mod fo_order_invariance;
mod formal;
mod graph_lfp_controls;
mod hamiltonian_eso;
mod lfp;
mod lfp_eval;
mod lfp_scope;
mod oblivious_wl;
mod order_family;
mod parity_ef;
mod partial_iso;
mod pebble;
mod proof;
mod repro;
mod reproduce;
mod verification;

pub use bench_adapter::{
    BenchAdapterError, BenchSourceLabel, RiemannStubEntry, RiemannStubIngest, TdiStubEntry,
    TdiStubIngest, ingest_riemann_stub, ingest_tdi_stub, refuse_label_upgrade,
    refuse_riemann_stub_proof_seal, refuse_tdi_stub_proof_seal,
};
pub use bijective_pebble::{
    BijectivePebbleGameError, BijectivePebbleGameResult, solve_bijective_pebble_ordered,
    solve_bijective_pebble_unordered,
};
pub use canonical::{Canonical, CanonicalEncoder, sha256_bytes, sha256_canonical};
pub use cfi::{
    CfiBaseEdge, CfiBaseGraph, CfiError, CfiLinkSide, CfiTwistAssignment, CfiVertex, CubicCfiGraph,
};
pub use cfi_bijective_calibration::{
    CfiBijectivePebbleCalibration, CfiBijectivePebbleCalibrationError,
    calibrate_cubic_cfi_bijective_pebble,
};
pub use cfi_order_sensitivity::{
    CfiOrderSensitivityError, CfiOrderSensitivitySearch, CfiOrderSensitivityWitness,
    search_cubic_cfi_order_sensitivity,
};
pub use cfi_ordered_bijective_calibration::{
    OrderedCfiBijectivePebbleCalibration, calibrate_cubic_cfi_bijective_pebble_ordered,
};
pub use cfi_ordered_cross_calibration::{
    OrderedCfiCrossCalibration, OrderedCfiCrossCalibrationConfig, OrderedCfiCrossCalibrationError,
    calibrate_cubic_cfi_ordered_cross_oracle,
};
pub use cfi_ordered_wl_calibration::{OrderedCfiWlCalibration, calibrate_cubic_cfi_wl_ordered};
pub use cfi_relational::{CFI_EDGE_RELATION, cubic_cfi_as_relational};
pub use cfi_wl_calibration::{CfiWlCalibration, CfiWlCalibrationError, calibrate_cubic_cfi_wl};
pub use claim::{Claim, ClaimBody, ClaimId, ClaimStatus};
pub use descriptive::{
    DescriptiveError, FiniteStructure, FiniteStructureId, OrderedFiniteStructure,
    OrderedFiniteStructureId, RelationInterpretation, RelationSymbol, Vocabulary,
};
pub use determinism::DeterminismLevel;
pub use ef::{EfGameError, EfGameResult, solve_ef_ordered, solve_ef_unordered};
pub use env_lock::{
    DriftField, DriftReport, EnvironmentLock, EnvironmentLockBody, EnvironmentLockError,
    EnvironmentLockId,
};
pub use eso::{EsoSentence, EsoValidationError};
pub use eso_eval::{
    EsoEvaluation, EsoEvaluationBudget, EsoEvaluationError, evaluate_eso_ordered,
    evaluate_eso_ordered_bounded, evaluate_eso_unordered, evaluate_eso_unordered_bounded,
};
pub use evidence::{
    AcceptedKernel, ConjectureCandidate, ConjectureCandidateId, EvidenceClaim, EvidenceClaimId,
    EvidenceError, EvidenceStrength, KernelOutcome, KernelResult, KernelResultId, Observation,
    ObservationId, ObservationKind, PromotionAuthority, PromotionMeta, ProofObligation,
    ProofObligationId, empirical_auto_upgrade_refused, refuse_auto_upgrade_from_empirical,
    refuse_empirical_proof_seal, refuse_evidence_proof_seal,
};
pub use falsify::{
    CheapClaimShape, CounterexampleWitness, CounterexampleWitnessId, FalsificationOutcome,
    FalsificationRecord, FalsificationRecordId, FalsifyError, NatAtom, NatBinOp, NatExpr,
    refuse_proof_search, try_falsify,
};
pub use fo::{FoAtom, FoFormula, FoValidationError, Variable};
pub use fo_eval::{FoAssignment, FoEvaluationError, evaluate_ordered, evaluate_unordered};
pub use fo_order_invariance::{
    FoOrderInvarianceCheck, FoOrderInvarianceError, FoOrderInvarianceWitness,
    check_fo_order_invariance,
};
pub use formal::{FormalBackend, FormalStatement, FormalStatementId};
pub use graph_lfp_controls::{
    LFP_GRAPH_EDGE_RELATION, directed_reachability_lfp, is_strongly_connected_via_lfp,
};
pub use hamiltonian_eso::{
    HAMILTONIAN_EDGE_RELATION, HAMILTONIAN_ORDER_WITNESS, directed_hamiltonian_cycle_eso,
};
pub use lfp::{LfpAtom, LfpBody, LfpDefinition, LfpValidationError};
pub use lfp_eval::{
    LeastFixedPoint, LfpEvaluationBudget, LfpEvaluationError, evaluate_lfp_ordered,
    evaluate_lfp_ordered_bounded, evaluate_lfp_unordered, evaluate_lfp_unordered_bounded,
};
pub use lfp_scope::LfpScopeError;
pub use oblivious_wl::{
    ObliviousWlComparison, ObliviousWlError, compare_oblivious_wl_ordered,
    compare_oblivious_wl_unordered,
};
pub use order_family::{
    ExhaustiveOrderFamily, OrderFamilyError, adjacent_transposition_order_family,
    exhaustive_total_order_family, validate_total_order,
};
pub use parity_ef::{ParityEfCalibration, calibrate_parity_pure_equality};
pub use pebble::{PebbleGameError, PebbleGameResult, solve_pebble_ordered, solve_pebble_unordered};
pub use proof::{
    KernelReceipt, ProofArtifact, ProofArtifactBody, ProofArtifactError, ProofArtifactId,
};
pub use repro::ReproMeta;
pub use reproduce::{ReproduceError, ReproduceOk, lock_for_artifact, reproduce};
pub use verification::{VerificationJob, VerificationJobError, VerificationJobId};
