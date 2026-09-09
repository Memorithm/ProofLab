//! Trusted, backend-independent data types for ProofLab.
//!
//! This crate intentionally contains no theorem prover. It defines immutable
//! claim identity, mathematical lifecycle state, and reproducibility metadata.
//! Proof status is never inferred from reproducibility.

#![forbid(unsafe_code)]

mod claim;
mod determinism;
mod repro;

pub use claim::{Claim, ClaimBody, ClaimId, ClaimStatus};
pub use determinism::DeterminismLevel;
pub use repro::ReproMeta;
