//! Trusted, backend-independent data types for `ProofLab`.
//!
//! This crate intentionally contains no theorem prover. It defines immutable
//! claim identity, formal statements, proof artifacts, mathematical lifecycle
//! state, and reproducibility metadata. Proof status is never inferred from
//! reproducibility.

#![forbid(unsafe_code)]

mod canonical;
mod claim;
mod descriptive;
mod determinism;
mod formal;
mod proof;
mod repro;

pub use canonical::{Canonical, CanonicalEncoder, sha256_bytes, sha256_canonical};
pub use claim::{Claim, ClaimBody, ClaimId, ClaimStatus};
pub use descriptive::{
    DescriptiveError, FiniteStructure, FiniteStructureId, OrderedFiniteStructure,
    OrderedFiniteStructureId, RelationInterpretation, RelationSymbol, Vocabulary,
};
pub use determinism::DeterminismLevel;
pub use formal::{FormalBackend, FormalStatement, FormalStatementId};
pub use proof::{
    KernelReceipt, ProofArtifact, ProofArtifactBody, ProofArtifactError, ProofArtifactId,
};
pub use repro::ReproMeta;
