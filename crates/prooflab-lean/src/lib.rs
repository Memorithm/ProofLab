//! Explicit process boundary to the configured Lean environment.
//!
//! A successful process result is necessary for kernel acceptance but callers
//! must still persist the exact source and environment digests as a proof artifact.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// A verification request for one Lean source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationJob {
    pub source: PathBuf,
}

/// Raw normalized result returned by the Lean process boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelResult {
    pub accepted: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Configured Lean command boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeanKernel {
    lake_binary: PathBuf,
}

impl Default for LeanKernel {
    fn default() -> Self {
        Self {
            lake_binary: PathBuf::from("lake"),
        }
    }
}

impl LeanKernel {
    #[must_use]
    pub fn new(lake_binary: impl Into<PathBuf>) -> Self {
        Self {
            lake_binary: lake_binary.into(),
        }
    }

    /// Verify a Lean file through the pinned Lake environment.
    pub fn verify_file(&self, source: impl AsRef<Path>) -> std::io::Result<KernelResult> {
        let output = Command::new(&self.lake_binary)
            .arg("env")
            .arg("lean")
            .arg(source.as_ref())
            .output()?;

        Ok(KernelResult {
            accepted: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_boundary_uses_lake() {
        assert_eq!(LeanKernel::default().lake_binary, PathBuf::from("lake"));
    }
}
