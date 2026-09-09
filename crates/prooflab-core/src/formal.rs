use serde::{Deserialize, Serialize};

use crate::ClaimId;
use crate::canonical::{Canonical, CanonicalEncoder, sha256_bytes, sha256_canonical};

const FORMAL_STATEMENT_DOMAIN: &[u8] = b"prooflab-formal-statement:v1\0";

/// Stable identifier of an immutable formal statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FormalStatementId(pub [u8; 32]);

/// Formal proof backend named by the statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FormalBackend {
    /// Lean 4 source checked by the configured Lean kernel environment.
    Lean4,
}

/// Content-addressed formalization of a mathematical claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalStatement {
    pub id: FormalStatementId,
    pub claim_id: ClaimId,
    pub backend: FormalBackend,
    pub source_digest: [u8; 32],
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FormalIdentity {
    claim_id: ClaimId,
    backend: FormalBackend,
    source_digest: [u8; 32],
    imports: Vec<String>,
}

impl FormalStatement {
    /// Create a Lean formal statement from exact source bytes.
    ///
    /// Imports are sorted and deduplicated so caller ordering cannot alter the
    /// statement identity.
    #[must_use]
    pub fn lean4(claim_id: ClaimId, source: &[u8], mut imports: Vec<String>) -> Self {
        imports.sort();
        imports.dedup();
        let source_digest = sha256_bytes(source);
        let identity = FormalIdentity {
            claim_id,
            backend: FormalBackend::Lean4,
            source_digest,
            imports: imports.clone(),
        };
        Self {
            id: FormalStatementId(sha256_canonical(FORMAL_STATEMENT_DOMAIN, &identity)),
            claim_id,
            backend: FormalBackend::Lean4,
            source_digest,
            imports,
        }
    }

    /// Return whether the stored content address still matches the fields.
    #[must_use]
    pub fn check_id(&self) -> bool {
        self.id == FormalStatementId(sha256_canonical(FORMAL_STATEMENT_DOMAIN, &self.identity()))
    }

    /// Return whether `source` is exactly the source committed by this statement.
    #[must_use]
    pub fn matches_source(&self, source: &[u8]) -> bool {
        self.source_digest == sha256_bytes(source)
    }

    fn identity(&self) -> FormalIdentity {
        FormalIdentity {
            claim_id: self.claim_id,
            backend: self.backend,
            source_digest: self.source_digest,
            imports: self.imports.clone(),
        }
    }
}

impl Canonical for ClaimId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for FormalStatementId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for FormalBackend {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        let tag = match self {
            Self::Lean4 => 1u8,
        };
        encoder.u64(u64::from(tag));
    }
}

impl Canonical for FormalIdentity {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.claim_id);
        encoder.value(&self.backend);
        encoder.value(&self.source_digest);
        encoder.value(&self.imports);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Claim, ClaimBody};

    fn claim_id() -> ClaimId {
        Claim::new(ClaimBody {
            statement: "n = n".into(),
            assumptions: vec!["n : Nat".into()],
            parents: vec![],
        })
        .id
    }

    #[test]
    fn statement_identity_is_stable_and_import_order_independent() {
        let source = b"theorem refl (n : Nat) : n = n := rfl\n";
        let a = FormalStatement::lean4(
            claim_id(),
            source,
            vec!["Mathlib".into(), "ProofLab.Core".into()],
        );
        let b = FormalStatement::lean4(
            claim_id(),
            source,
            vec!["ProofLab.Core".into(), "Mathlib".into(), "Mathlib".into()],
        );
        assert_eq!(a.id, b.id);
        assert!(a.check_id());
        assert!(a.matches_source(source));
    }

    #[test]
    fn source_change_changes_statement_identity() {
        let a = FormalStatement::lean4(claim_id(), b"theorem a : True := by trivial\n", vec![]);
        let b = FormalStatement::lean4(claim_id(), b"theorem b : True := by trivial\n", vec![]);
        assert_ne!(a.id, b.id);
    }
}
