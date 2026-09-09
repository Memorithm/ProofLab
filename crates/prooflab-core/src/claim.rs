use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable identifier of immutable claim content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClaimId(pub [u8; 32]);

/// Immutable mathematical content whose identity is independent of research status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimBody {
    pub statement: String,
    pub assumptions: Vec<String>,
    pub parents: Vec<ClaimId>,
}

/// Content-addressed claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub id: ClaimId,
    pub body: ClaimBody,
}

/// Mathematical research status. This is deliberately separate from determinism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimStatus {
    Observed,
    Conjectured,
    Falsified,
    Formalized,
    Proved,
    Generalized,
}

impl Claim {
    /// Create a claim and derive its identity from a small canonical encoding.
    ///
    /// Status is not part of the identity: observing, falsifying, or proving the
    /// same immutable proposition must not silently create a different proposition.
    #[must_use]
    pub fn new(body: ClaimBody) -> Self {
        let mut h = Sha256::new();
        h.update(b"prooflab-claim:v1");
        encode_string(&mut h, &body.statement);
        h.update((body.assumptions.len() as u64).to_be_bytes());
        for assumption in &body.assumptions {
            encode_string(&mut h, assumption);
        }
        h.update((body.parents.len() as u64).to_be_bytes());
        for parent in &body.parents {
            h.update(parent.0);
        }
        Self {
            id: ClaimId(h.finalize().into()),
            body,
        }
    }
}

fn encode_string(h: &mut Sha256, value: &str) {
    let bytes = value.as_bytes();
    h.update((bytes.len() as u64).to_be_bytes());
    h.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(statement: &str) -> ClaimBody {
        ClaimBody {
            statement: statement.into(),
            assumptions: vec!["n : Nat".into()],
            parents: vec![],
        }
    }

    #[test]
    fn identity_is_deterministic_and_content_sensitive() {
        let a = Claim::new(body("n = n"));
        let b = Claim::new(body("n = n"));
        let c = Claim::new(body("n + 0 = n"));
        assert_eq!(a.id, b.id);
        assert_ne!(a.id, c.id);
    }

    #[test]
    fn status_is_not_claim_identity() {
        let before = Claim::new(body("n = n")).id;
        let statuses = [
            ClaimStatus::Conjectured,
            ClaimStatus::Formalized,
            ClaimStatus::Proved,
        ];
        let after = Claim::new(body("n = n")).id;
        assert_eq!(before, after);
        assert!(statuses.contains(&ClaimStatus::Proved));
    }
}
