//! Trusted, backend-independent data types for `ProofLab`.
//!
//! This crate intentionally contains no theorem prover. It defines immutable
//! claim identity, formal statements, proof artifacts, mathematical lifecycle
//! state, reproducibility metadata, finite descriptive-complexity syntax and
//! evaluation, and exact finite model-comparison game oracles. Proof status is
//! never inferred from reproducibility.

#![forbid(unsafe_code)]

mod bijective_pebble;
mod canonical;
mod cfi;
mod cfi_relational;
mod claim;
mod descriptive;
mod determinism;
mod ef;
mod eso;
mod fo;
mod fo_eval;
mod formal;
mod lfp;
mod lfp_eval;
mod lfp_scope;
mod oblivious_wl;
mod partial_iso;
mod pebble;
mod proof;
mod repro;

pub use bijective_pebble::{
    BijectivePebbleGameError, BijectivePebbleGameResult, solve_bijective_pebble_ordered,
    solve_bijective_pebble_unordered,
};
pub use canonical::{Canonical, CanonicalEncoder, sha256_bytes, sha256_canonical};
pub use cfi::{
    CfiBaseEdge, CfiBaseGraph, CfiError, CfiLinkSide, CfiTwistAssignment, CfiVertex, CubicCfiGraph,
};
pub use cfi_relational::{CFI_EDGE_RELATION, cubic_cfi_as_relational};
pub use claim::{Claim, ClaimBody, ClaimId, ClaimStatus};
pub use descriptive::{
    DescriptiveError, FiniteStructure, FiniteStructureId, OrderedFiniteStructure,
    OrderedFiniteStructureId, RelationInterpretation, RelationSymbol, Vocabulary,
};
pub use determinism::DeterminismLevel;
pub use ef::{EfGameError, EfGameResult, solve_ef_ordered, solve_ef_unordered};
pub use eso::{EsoSentence, EsoValidationError};
pub use fo::{FoAtom, FoFormula, FoValidationError, Variable};
pub use fo_eval::{FoAssignment, FoEvaluationError, evaluate_ordered, evaluate_unordered};
pub use formal::{FormalBackend, FormalStatement, FormalStatementId};
pub use lfp::{LfpAtom, LfpBody, LfpDefinition, LfpValidationError};
pub use lfp_eval::{
    LeastFixedPoint, LfpEvaluationError, evaluate_lfp_ordered, evaluate_lfp_unordered,
};
pub use lfp_scope::LfpScopeError;
pub use oblivious_wl::{
    ObliviousWlComparison, ObliviousWlError, compare_oblivious_wl_ordered,
    compare_oblivious_wl_unordered,
};
pub use pebble::{PebbleGameError, PebbleGameResult, solve_pebble_ordered, solve_pebble_unordered};
pub use proof::{
    KernelReceipt, ProofArtifact, ProofArtifactBody, ProofArtifactError, ProofArtifactId,
};
pub use repro::ReproMeta;
