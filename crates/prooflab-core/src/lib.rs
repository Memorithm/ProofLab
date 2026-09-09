//! Trusted, backend-independent data types for `ProofLab`.
//!
//! This crate intentionally contains no theorem prover. It defines immutable
//! claim identity, formal statements, proof artifacts, mathematical lifecycle
//! state, reproducibility metadata, and finite descriptive-complexity syntax
//! and evaluation. Proof status is never inferred from reproducibility.

#![forbid(unsafe_code)]

mod canonical;
mod claim;
mod descriptive;
mod determinism;
mod eso;
mod fo;
mod fo_eval;
mod formal;
mod lfp;
mod lfp_scope;
mod proof;
mod repro;

pub use canonical::{Canonical, CanonicalEncoder, sha256_bytes, sha256_canonical};
pub use claim::{Claim, ClaimBody, ClaimId, ClaimStatus};
pub use descriptive::{
    DescriptiveError, FiniteStructure, FiniteStructureId, OrderedFiniteStructure,
    OrderedFiniteStructureId, RelationInterpretation, RelationSymbol, Vocabulary,
};
pub use determinism::DeterminismLevel;
pub use eso::{EsoSentence, EsoValidationError};
pub use fo::{FoAtom, FoFormula, FoValidationError, Variable};
pub use fo_eval::{FoAssignment, FoEvaluationError, evaluate_ordered, evaluate_unordered};
pub use formal::{FormalBackend, FormalStatement, FormalStatementId};
pub use lfp::{LfpAtom, LfpBody, LfpDefinition, LfpValidationError};
pub use lfp_scope::LfpScopeError;
pub use proof::{
    KernelReceipt, ProofArtifact, ProofArtifactBody, ProofArtifactError, ProofArtifactId,
};
pub use repro::ReproMeta;
