//! PL-2.0 Stage-0 typed scientific evidence plumbing (PL-C15).
//!
//! This module introduces immutable, content-addressed types along the
//! observation → conjecture → obligation → kernel → proof chain. Empirical
//! evidence, solver output and LLM assertions remain untrusted proposers:
//! they may construct observations, evidence claims and conjecture candidates,
//! but they cannot construct a [`crate::ProofArtifact`] or authorize
//! [`ClaimStatus::Proved`].
//!
//! Lean acceptance via an [`AcceptedKernel`] (wrapping a successful
//! [`KernelOutcome::Accepted`]) is the sole path that may seal a proof
//! artifact. Failed, unknown and timeout kernel outcomes stay distinct from
//! acceptance and never upgrade through serde round-trip.
//!
//! Non-claims: this stage defines types and trust-boundary tests only. It does
//! not ingest TDI/Riemann data, does not claim scientific novelty, and does
//! not treat numerical agreement as proof.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::canonical::{Canonical, CanonicalEncoder, sha256_bytes, sha256_canonical};
use crate::{
    ClaimId, ClaimStatus, FormalStatement, FormalStatementId, KernelReceipt, ProofArtifact,
    ProofArtifactError, ProofArtifactId, ReproMeta,
};

const OBSERVATION_DOMAIN: &[u8] = b"prooflab-observation:v1\0";
const EVIDENCE_CLAIM_DOMAIN: &[u8] = b"prooflab-evidence-claim:v1\0";
const CONJECTURE_CANDIDATE_DOMAIN: &[u8] = b"prooflab-conjecture-candidate:v1\0";
const PROOF_OBLIGATION_DOMAIN: &[u8] = b"prooflab-proof-obligation:v1\0";
const KERNEL_RESULT_DOMAIN: &[u8] = b"prooflab-kernel-result:v1\0";

/// Origin class of an empirical or external observation.
///
/// No variant confers proof status. Labels such as `Numerical` or `SolverOutput`
/// are provenance tags only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObservationKind {
    /// Floating-point / finite-sample numerical measurement.
    Numerical,
    /// Symbolic or algebraic experiment output.
    SymbolicExperiment,
    /// External SAT/SMT/CAS/heuristic solver output (untrusted).
    SolverOutput,
    /// Human annotation that is not a kernel certificate.
    ManualAnnotation,
    /// Tiny stub adapter used by PL-2.0 tests (no real bench ingest).
    StubAdapter,
}

impl ObservationKind {
    fn tag(self) -> &'static str {
        match self {
            Self::Numerical => "numerical",
            Self::SymbolicExperiment => "symbolic_experiment",
            Self::SolverOutput => "solver_output",
            Self::ManualAnnotation => "manual_annotation",
            Self::StubAdapter => "stub_adapter",
        }
    }
}

/// Stable identity of an immutable observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObservationId(pub [u8; 32]);

/// Immutable recorded observation. Never a proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub kind: ObservationKind,
    /// Opaque source label (e.g. `stub://pl-2.0/example`). Not a trust token.
    pub source_label: String,
    pub payload_digest: [u8; 32],
    pub parent_ids: Vec<ObservationId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservationBody {
    kind: ObservationKind,
    source_label: String,
    payload_digest: [u8; 32],
    parent_ids: Vec<ObservationId>,
}

impl Observation {
    /// Record an observation from exact payload bytes and provenance parents.
    ///
    /// Parent IDs are sorted and deduplicated. This never yields
    /// [`ClaimStatus::Proved`].
    #[must_use]
    pub fn new(
        kind: ObservationKind,
        source_label: impl Into<String>,
        payload: &[u8],
        mut parent_ids: Vec<ObservationId>,
    ) -> Self {
        parent_ids.sort_unstable();
        parent_ids.dedup();
        let body = ObservationBody {
            kind,
            source_label: source_label.into(),
            payload_digest: sha256_bytes(payload),
            parent_ids,
        };
        Self {
            id: ObservationId(sha256_canonical(OBSERVATION_DOMAIN, &body)),
            kind: body.kind,
            source_label: body.source_label,
            payload_digest: body.payload_digest,
            parent_ids: body.parent_ids,
        }
    }

    /// Tiny stub adapter helper for PL-2.0 type plumbing tests.
    ///
    /// Does not ingest real TDI/Riemann data.
    #[must_use]
    pub fn stub(source_label: impl Into<String>, payload: &[u8]) -> Self {
        Self::new(ObservationKind::StubAdapter, source_label, payload, vec![])
    }

    /// Return whether the stored content address still matches the fields.
    #[must_use]
    pub fn check_id(&self) -> bool {
        let body = ObservationBody {
            kind: self.kind,
            source_label: self.source_label.clone(),
            payload_digest: self.payload_digest,
            parent_ids: self.parent_ids.clone(),
        };
        self.id == ObservationId(sha256_canonical(OBSERVATION_DOMAIN, &body))
    }

    /// Scientific status implied by an observation alone.
    #[must_use]
    pub const fn implied_status(&self) -> ClaimStatus {
        ClaimStatus::Observed
    }
}

/// Strength of empirical support. Deliberately excludes any `Proved` level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceStrength {
    Suggestive,
    Corroborated,
    Strong,
}

impl EvidenceStrength {
    fn tag(self) -> &'static str {
        match self {
            Self::Suggestive => "suggestive",
            Self::Corroborated => "corroborated",
            Self::Strong => "strong",
        }
    }
}

/// Stable identity of an immutable evidence claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceClaimId(pub [u8; 32]);

/// Evidence that a claim is suggested by observations.
///
/// Always records [`ClaimStatus::Observed`]. Strong numerical evidence still
/// cannot construct a [`ProofArtifact`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceClaim {
    pub id: EvidenceClaimId,
    pub claim_id: ClaimId,
    pub observation_ids: Vec<ObservationId>,
    pub strength: EvidenceStrength,
    /// Fixed to [`ClaimStatus::Observed`] by construction; serde upgrades fail `check_id`.
    pub status: ClaimStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceClaimBody {
    claim_id: ClaimId,
    observation_ids: Vec<ObservationId>,
    strength: EvidenceStrength,
    status: ClaimStatus,
}

/// Failure to construct typed evidence objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    EmptyObservations,
    ObservationIntegrity,
    EmptyEvidence,
    EvidenceIntegrity,
    ConjectureIntegrity,
    ObligationIntegrity,
    FormalStatementIntegrity,
    ClaimMismatch,
    EmptySketch,
    KernelNotAccepted,
    KernelReceiptInconsistent,
    EmpiricalCannotSealProof(&'static str),
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyObservations => {
                write!(formatter, "evidence requires at least one observation")
            }
            Self::ObservationIntegrity => write!(formatter, "observation id mismatch"),
            Self::EmptyEvidence => {
                write!(formatter, "conjecture requires at least one evidence claim")
            }
            Self::EvidenceIntegrity => write!(formatter, "evidence claim id mismatch"),
            Self::ConjectureIntegrity => write!(formatter, "conjecture candidate id mismatch"),
            Self::ObligationIntegrity => write!(formatter, "proof obligation id mismatch"),
            Self::FormalStatementIntegrity => write!(formatter, "formal statement id mismatch"),
            Self::ClaimMismatch => {
                write!(
                    formatter,
                    "formal statement claim_id does not match conjecture"
                )
            }
            Self::EmptySketch => write!(formatter, "conjecture statement sketch is empty"),
            Self::KernelNotAccepted => {
                write!(formatter, "kernel result is not an accepted Lean outcome")
            }
            Self::KernelReceiptInconsistent => {
                write!(
                    formatter,
                    "accepted kernel outcome carries a non-accepting receipt"
                )
            }
            Self::EmpiricalCannotSealProof(kind) => {
                write!(
                    formatter,
                    "empirical or solver evidence ({kind}) cannot seal a proof artifact"
                )
            }
        }
    }
}

impl std::error::Error for EvidenceError {}

impl EvidenceClaim {
    /// Bind validated observations to a claim with an explicit evidence strength.
    ///
    /// # Errors
    ///
    /// Fails when the observation list is empty or any observation fails integrity.
    pub fn from_observations(
        claim_id: ClaimId,
        observations: &[&Observation],
        strength: EvidenceStrength,
    ) -> Result<Self, EvidenceError> {
        if observations.is_empty() {
            return Err(EvidenceError::EmptyObservations);
        }
        let mut observation_ids = Vec::with_capacity(observations.len());
        for observation in observations {
            if !observation.check_id() {
                return Err(EvidenceError::ObservationIntegrity);
            }
            observation_ids.push(observation.id);
        }
        observation_ids.sort_unstable();
        observation_ids.dedup();
        let body = EvidenceClaimBody {
            claim_id,
            observation_ids,
            strength,
            status: ClaimStatus::Observed,
        };
        Ok(Self {
            id: EvidenceClaimId(sha256_canonical(EVIDENCE_CLAIM_DOMAIN, &body)),
            claim_id: body.claim_id,
            observation_ids: body.observation_ids,
            strength: body.strength,
            status: ClaimStatus::Observed,
        })
    }

    /// Return whether identity and fixed status still hold.
    #[must_use]
    pub fn check_id(&self) -> bool {
        if self.status != ClaimStatus::Observed {
            return false;
        }
        let body = EvidenceClaimBody {
            claim_id: self.claim_id,
            observation_ids: self.observation_ids.clone(),
            strength: self.strength,
            status: self.status,
        };
        self.id == EvidenceClaimId(sha256_canonical(EVIDENCE_CLAIM_DOMAIN, &body))
    }
}

/// Stable identity of an immutable conjecture candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConjectureCandidateId(pub [u8; 32]);

/// Candidate mathematical statement motivated by evidence.
///
/// Always records [`ClaimStatus::Conjectured`]. External solvers/LLMs may emit
/// candidates through this type; they never emit `PROVED`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConjectureCandidate {
    pub id: ConjectureCandidateId,
    pub claim_id: ClaimId,
    pub evidence_ids: Vec<EvidenceClaimId>,
    pub statement_sketch: String,
    pub assumptions: Vec<String>,
    pub status: ClaimStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConjectureCandidateBody {
    claim_id: ClaimId,
    evidence_ids: Vec<EvidenceClaimId>,
    statement_sketch: String,
    assumptions: Vec<String>,
    status: ClaimStatus,
}

impl ConjectureCandidate {
    /// Promote evidence into a conjecture candidate (still not a proof).
    ///
    /// # Errors
    ///
    /// Fails on empty evidence, empty sketch, or evidence integrity failure.
    pub fn from_evidence(
        claim_id: ClaimId,
        evidence: &[&EvidenceClaim],
        statement_sketch: impl Into<String>,
        mut assumptions: Vec<String>,
    ) -> Result<Self, EvidenceError> {
        if evidence.is_empty() {
            return Err(EvidenceError::EmptyEvidence);
        }
        let statement_sketch = statement_sketch.into();
        if statement_sketch.trim().is_empty() {
            return Err(EvidenceError::EmptySketch);
        }
        let mut evidence_ids = Vec::with_capacity(evidence.len());
        for item in evidence {
            if !item.check_id() {
                return Err(EvidenceError::EvidenceIntegrity);
            }
            evidence_ids.push(item.id);
        }
        evidence_ids.sort_unstable();
        evidence_ids.dedup();
        assumptions.sort();
        assumptions.dedup();
        let body = ConjectureCandidateBody {
            claim_id,
            evidence_ids,
            statement_sketch,
            assumptions,
            status: ClaimStatus::Conjectured,
        };
        Ok(Self {
            id: ConjectureCandidateId(sha256_canonical(CONJECTURE_CANDIDATE_DOMAIN, &body)),
            claim_id: body.claim_id,
            evidence_ids: body.evidence_ids,
            statement_sketch: body.statement_sketch,
            assumptions: body.assumptions,
            status: ClaimStatus::Conjectured,
        })
    }

    /// Return whether identity and fixed status still hold.
    #[must_use]
    pub fn check_id(&self) -> bool {
        if self.status != ClaimStatus::Conjectured {
            return false;
        }
        let body = ConjectureCandidateBody {
            claim_id: self.claim_id,
            evidence_ids: self.evidence_ids.clone(),
            statement_sketch: self.statement_sketch.clone(),
            assumptions: self.assumptions.clone(),
            status: self.status,
        };
        self.id == ConjectureCandidateId(sha256_canonical(CONJECTURE_CANDIDATE_DOMAIN, &body))
    }
}

/// Stable identity of an immutable proof obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProofObligationId(pub [u8; 32]);

/// Obligation to verify a formal statement derived from a conjecture.
///
/// Always records [`ClaimStatus::Formalized`]. Creating an obligation does not
/// verify anything and does not seal [`ClaimStatus::Proved`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofObligation {
    pub id: ProofObligationId,
    pub conjecture_id: ConjectureCandidateId,
    pub claim_id: ClaimId,
    pub formal_statement_id: FormalStatementId,
    pub status: ClaimStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProofObligationBody {
    conjecture_id: ConjectureCandidateId,
    claim_id: ClaimId,
    formal_statement_id: FormalStatementId,
    status: ClaimStatus,
}

impl ProofObligation {
    /// Bind a conjecture candidate to an existing formal statement.
    ///
    /// # Errors
    ///
    /// Fails on integrity mismatch or when the formal statement's claim differs
    /// from the conjecture's claim.
    pub fn from_conjecture(
        conjecture: &ConjectureCandidate,
        formal: &FormalStatement,
    ) -> Result<Self, EvidenceError> {
        if !conjecture.check_id() {
            return Err(EvidenceError::ConjectureIntegrity);
        }
        if !formal.check_id() {
            return Err(EvidenceError::FormalStatementIntegrity);
        }
        if formal.claim_id != conjecture.claim_id {
            return Err(EvidenceError::ClaimMismatch);
        }
        let body = ProofObligationBody {
            conjecture_id: conjecture.id,
            claim_id: conjecture.claim_id,
            formal_statement_id: formal.id,
            status: ClaimStatus::Formalized,
        };
        Ok(Self {
            id: ProofObligationId(sha256_canonical(PROOF_OBLIGATION_DOMAIN, &body)),
            conjecture_id: body.conjecture_id,
            claim_id: body.claim_id,
            formal_statement_id: body.formal_statement_id,
            status: ClaimStatus::Formalized,
        })
    }

    /// Return whether identity and fixed status still hold.
    #[must_use]
    pub fn check_id(&self) -> bool {
        if self.status != ClaimStatus::Formalized {
            return false;
        }
        let body = ProofObligationBody {
            conjecture_id: self.conjecture_id,
            claim_id: self.claim_id,
            formal_statement_id: self.formal_statement_id,
            status: self.status,
        };
        self.id == ProofObligationId(sha256_canonical(PROOF_OBLIGATION_DOMAIN, &body))
    }
}

/// Distinct outcomes of a trusted-kernel verification attempt.
///
/// `Rejected`, `Unknown` and `Timeout` are never interchangeable with
/// `Accepted`, including after serialization round-trips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KernelOutcome {
    /// Kernel accepted the proof (receipt must itself report acceptance).
    Accepted { receipt: KernelReceipt },
    /// Kernel explicitly rejected the proof.
    Rejected { receipt: KernelReceipt },
    /// Outcome could not be determined (e.g. malformed process output).
    Unknown { reason: String },
    /// Verification exceeded a time budget.
    Timeout { elapsed_ms: u64 },
}

impl KernelOutcome {
    fn tag(&self) -> &'static str {
        match self {
            Self::Accepted { .. } => "accepted",
            Self::Rejected { .. } => "rejected",
            Self::Unknown { .. } => "unknown",
            Self::Timeout { .. } => "timeout",
        }
    }

    /// Whether this outcome is an accepting kernel result with a consistent receipt.
    #[must_use]
    pub fn is_accepting(&self) -> bool {
        match self {
            Self::Accepted { receipt } => receipt.accepted && receipt.exit_code == Some(0),
            Self::Rejected { .. } | Self::Unknown { .. } | Self::Timeout { .. } => false,
        }
    }
}

/// Stable identity of an immutable kernel result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct KernelResultId(pub [u8; 32]);

/// Content-addressed record of a kernel verification attempt.
///
/// Only [`KernelOutcome::Accepted`] (via [`AcceptedKernel`]) may seal a
/// [`ProofArtifact`]. Other outcomes remain first-class and non-upgradable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelResult {
    pub id: KernelResultId,
    pub obligation_id: ProofObligationId,
    pub formal_statement_id: FormalStatementId,
    pub outcome: KernelOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KernelResultBody {
    obligation_id: ProofObligationId,
    formal_statement_id: FormalStatementId,
    outcome: KernelOutcome,
}

impl KernelResult {
    /// Record a kernel attempt bound to an obligation and formal statement.
    ///
    /// # Errors
    ///
    /// Fails when the obligation or formal statement fails integrity checks, or
    /// when an `Accepted` outcome carries a non-accepting receipt.
    pub fn new(
        obligation: &ProofObligation,
        formal: &FormalStatement,
        outcome: KernelOutcome,
    ) -> Result<Self, EvidenceError> {
        if !obligation.check_id() {
            return Err(EvidenceError::ObligationIntegrity);
        }
        if !formal.check_id() {
            return Err(EvidenceError::FormalStatementIntegrity);
        }
        if obligation.formal_statement_id != formal.id {
            return Err(EvidenceError::ClaimMismatch);
        }
        if matches!(&outcome, KernelOutcome::Accepted { receipt } if !receipt.accepted || receipt.exit_code != Some(0))
        {
            return Err(EvidenceError::KernelReceiptInconsistent);
        }
        let body = KernelResultBody {
            obligation_id: obligation.id,
            formal_statement_id: formal.id,
            outcome,
        };
        Ok(Self {
            id: KernelResultId(sha256_canonical(KERNEL_RESULT_DOMAIN, &body)),
            obligation_id: body.obligation_id,
            formal_statement_id: body.formal_statement_id,
            outcome: body.outcome,
        })
    }

    /// Return whether the stored content address still matches the fields.
    #[must_use]
    pub fn check_id(&self) -> bool {
        let body = KernelResultBody {
            obligation_id: self.obligation_id,
            formal_statement_id: self.formal_statement_id,
            outcome: self.outcome.clone(),
        };
        self.id == KernelResultId(sha256_canonical(KERNEL_RESULT_DOMAIN, &body))
    }

    /// Typestate gate: only a consistent accepting outcome yields [`AcceptedKernel`].
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError::KernelNotAccepted`] for rejected/unknown/timeout
    /// outcomes or integrity failure.
    pub fn into_accepted(self) -> Result<AcceptedKernel, EvidenceError> {
        if !self.check_id() {
            return Err(EvidenceError::KernelNotAccepted);
        }
        if !self.outcome.is_accepting() {
            return Err(EvidenceError::KernelNotAccepted);
        }
        Ok(AcceptedKernel { inner: self })
    }
}

/// Typestate witness that a [`KernelResult`] is an accepting Lean outcome.
///
/// Constructible only via [`KernelResult::into_accepted`]. This is the only
/// evidence-layer type that may call [`AcceptedKernel::seal_proof_artifact`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedKernel {
    inner: KernelResult,
}

impl AcceptedKernel {
    /// Borrow the underlying kernel result.
    #[must_use]
    pub fn result(&self) -> &KernelResult {
        &self.inner
    }

    /// Seal a [`ProofArtifact`] from this accepted kernel result.
    ///
    /// Delegates to [`ProofArtifact::new_verified`]. Empirical types cannot
    /// reach this method without first obtaining a genuine accepting kernel
    /// outcome.
    ///
    /// # Errors
    ///
    /// Propagates [`ProofArtifactError`] from the verified constructor.
    pub fn seal_proof_artifact(
        &self,
        formal: &FormalStatement,
        proof_source: &[u8],
        dependencies: Vec<ProofArtifactId>,
        repro: ReproMeta,
    ) -> Result<ProofArtifact, ProofArtifactError> {
        let KernelOutcome::Accepted { receipt } = &self.inner.outcome else {
            // Unreachable by typestate; keep fail-closed.
            return Err(ProofArtifactError::KernelRejected);
        };
        if self.inner.formal_statement_id != formal.id {
            return Err(ProofArtifactError::FormalStatementIntegrity);
        }
        ProofArtifact::new_verified(formal, proof_source, dependencies, repro, receipt.clone())
    }
}

/// Explicit deny path: empirical / solver objects cannot seal proof artifacts.
///
/// Prefer relying on the type system (no conversion exists). This helper exists
/// so tests and adapters can document the trust boundary as a runtime error.
///
/// # Errors
///
/// Always returns [`EvidenceError::EmpiricalCannotSealProof`].
pub fn refuse_empirical_proof_seal(kind: ObservationKind) -> Result<ProofArtifact, EvidenceError> {
    Err(EvidenceError::EmpiricalCannotSealProof(kind.tag()))
}

/// Explicit deny path for evidence claims.
///
/// # Errors
///
/// Always returns [`EvidenceError::EmpiricalCannotSealProof`].
pub fn refuse_evidence_proof_seal(
    strength: EvidenceStrength,
) -> Result<ProofArtifact, EvidenceError> {
    Err(EvidenceError::EmpiricalCannotSealProof(strength.tag()))
}

// --- Canonical encodings ---------------------------------------------------

impl Canonical for ObservationId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for ObservationBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str(self.kind.tag());
        encoder.value(&self.source_label);
        encoder.value(&self.payload_digest);
        encoder.value(&self.parent_ids);
    }
}

impl Canonical for EvidenceClaimId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for EvidenceClaimBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.claim_id);
        encoder.value(&self.observation_ids);
        encoder.str(self.strength.tag());
        encoder.str(status_tag(self.status));
    }
}

impl Canonical for ConjectureCandidateId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for ConjectureCandidateBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.claim_id);
        encoder.value(&self.evidence_ids);
        encoder.value(&self.statement_sketch);
        encoder.value(&self.assumptions);
        encoder.str(status_tag(self.status));
    }
}

impl Canonical for ProofObligationId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for ProofObligationBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.conjecture_id);
        encoder.value(&self.claim_id);
        encoder.value(&self.formal_statement_id);
        encoder.str(status_tag(self.status));
    }
}

impl Canonical for KernelResultId {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(&self.0);
    }
}

impl Canonical for KernelOutcome {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str(self.tag());
        match self {
            Self::Accepted { receipt } | Self::Rejected { receipt } => {
                encoder.value(receipt);
            }
            Self::Unknown { reason } => encoder.value(reason),
            Self::Timeout { elapsed_ms } => encoder.value(elapsed_ms),
        }
    }
}

impl Canonical for KernelResultBody {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.value(&self.obligation_id);
        encoder.value(&self.formal_statement_id);
        encoder.value(&self.outcome);
    }
}

fn status_tag(status: ClaimStatus) -> &'static str {
    match status {
        ClaimStatus::Observed => "observed",
        ClaimStatus::Conjectured => "conjectured",
        ClaimStatus::Falsified => "falsified",
        ClaimStatus::Formalized => "formalized",
        ClaimStatus::Proved => "proved",
        ClaimStatus::Generalized => "generalized",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Claim, ClaimBody, FormalBackend, FormalStatement};

    fn claim() -> Claim {
        Claim::new(ClaimBody {
            statement: "n + 0 = n".into(),
            assumptions: vec!["n : Nat".into()],
            parents: vec![],
        })
    }

    fn formal_for(claim: &Claim) -> FormalStatement {
        FormalStatement::lean4(
            claim.id,
            b"theorem add_zero (n : Nat) : n + 0 = n := Nat.add_zero n\n",
            vec!["Mathlib".into()],
        )
    }

    fn repro() -> ReproMeta {
        let mut repro = ReproMeta::bootstrap("sha256:environment");
        repro.prooflab_revision = "pl20deadbeef0123456789abcdef01234567".into();
        repro
    }

    fn accepting_receipt() -> KernelReceipt {
        KernelReceipt::new(
            FormalBackend::Lean4,
            "lake env lean",
            true,
            Some(0),
            sha256_bytes(b"stdout-ok"),
            sha256_bytes(b"stderr-empty"),
        )
    }

    fn rejecting_receipt() -> KernelReceipt {
        KernelReceipt::new(
            FormalBackend::Lean4,
            "lake env lean",
            false,
            Some(1),
            sha256_bytes(b"stdout-fail"),
            sha256_bytes(b"stderr-fail"),
        )
    }

    fn pipeline() -> (
        Claim,
        Observation,
        EvidenceClaim,
        ConjectureCandidate,
        FormalStatement,
        ProofObligation,
    ) {
        let claim = claim();
        let observation = Observation::stub("stub://pl-2.0/nat-add-zero", b"n+0=n samples");
        let evidence =
            EvidenceClaim::from_observations(claim.id, &[&observation], EvidenceStrength::Strong)
                .expect("evidence");
        let conjecture = ConjectureCandidate::from_evidence(
            claim.id,
            &[&evidence],
            "n + 0 = n",
            vec!["n : Nat".into()],
        )
        .expect("conjecture");
        let formal = formal_for(&claim);
        let obligation =
            ProofObligation::from_conjecture(&conjecture, &formal).expect("obligation");
        (claim, observation, evidence, conjecture, formal, obligation)
    }

    #[test]
    fn observation_identity_is_content_addressed_and_observed_only() {
        let a = Observation::new(ObservationKind::Numerical, "stub://a", b"payload", vec![]);
        let b = Observation::new(ObservationKind::Numerical, "stub://a", b"payload", vec![]);
        let c = Observation::new(
            ObservationKind::SolverOutput,
            "stub://a",
            b"payload",
            vec![],
        );
        assert_eq!(a.id, b.id);
        assert_ne!(a.id, c.id);
        assert!(a.check_id());
        assert_eq!(a.implied_status(), ClaimStatus::Observed);
        assert_ne!(a.implied_status(), ClaimStatus::Proved);
    }

    #[test]
    fn strong_evidence_cannot_imply_proved() {
        let claim = claim();
        let observation = Observation::stub("stub://strong", b"lots of samples");
        let evidence =
            EvidenceClaim::from_observations(claim.id, &[&observation], EvidenceStrength::Strong)
                .unwrap();
        assert_eq!(evidence.status, ClaimStatus::Observed);
        assert_ne!(evidence.status, ClaimStatus::Proved);
        assert!(evidence.check_id());
        assert_eq!(
            refuse_evidence_proof_seal(EvidenceStrength::Strong),
            Err(EvidenceError::EmpiricalCannotSealProof("strong"))
        );
        assert_eq!(
            refuse_empirical_proof_seal(ObservationKind::Numerical),
            Err(EvidenceError::EmpiricalCannotSealProof("numerical"))
        );
        assert_eq!(
            refuse_empirical_proof_seal(ObservationKind::SolverOutput),
            Err(EvidenceError::EmpiricalCannotSealProof("solver_output"))
        );
    }

    #[test]
    fn conjecture_and_obligation_preserve_provenance_ids() {
        let (_claim, observation, evidence, conjecture, formal, obligation) = pipeline();
        assert!(conjecture.evidence_ids.contains(&evidence.id));
        assert_eq!(obligation.conjecture_id, conjecture.id);
        assert_eq!(obligation.formal_statement_id, formal.id);
        assert_eq!(conjecture.status, ClaimStatus::Conjectured);
        assert_eq!(obligation.status, ClaimStatus::Formalized);
        assert_ne!(obligation.status, ClaimStatus::Proved);
        assert!(observation.check_id());
    }

    #[test]
    fn rejected_unknown_timeout_cannot_become_accepted() {
        let (_claim, _obs, _ev, _cj, formal, obligation) = pipeline();

        let rejected = KernelResult::new(
            &obligation,
            &formal,
            KernelOutcome::Rejected {
                receipt: rejecting_receipt(),
            },
        )
        .unwrap();
        assert!(!rejected.outcome.is_accepting());
        assert_eq!(
            rejected.clone().into_accepted(),
            Err(EvidenceError::KernelNotAccepted)
        );

        let unknown = KernelResult::new(
            &obligation,
            &formal,
            KernelOutcome::Unknown {
                reason: "garbled stderr".into(),
            },
        )
        .unwrap();
        assert_eq!(
            unknown.into_accepted(),
            Err(EvidenceError::KernelNotAccepted)
        );

        let timeout = KernelResult::new(
            &obligation,
            &formal,
            KernelOutcome::Timeout { elapsed_ms: 30_000 },
        )
        .unwrap();
        assert_eq!(
            timeout.into_accepted(),
            Err(EvidenceError::KernelNotAccepted)
        );
    }

    #[test]
    fn accepted_kernel_can_seal_proof_artifact() {
        let (_claim, _obs, _ev, _cj, formal, obligation) = pipeline();
        let result = KernelResult::new(
            &obligation,
            &formal,
            KernelOutcome::Accepted {
                receipt: accepting_receipt(),
            },
        )
        .unwrap();
        let accepted = result.into_accepted().expect("accepted");
        let artifact = accepted
            .seal_proof_artifact(&formal, b"by Nat.add_zero", vec![], repro())
            .expect("sealed");
        assert!(artifact.check_id());
        assert_eq!(artifact.body.claim_id, formal.claim_id);
    }

    #[test]
    fn inconsistent_accepted_outcome_is_rejected_at_construction() {
        let (_claim, _obs, _ev, _cj, formal, obligation) = pipeline();
        let err = KernelResult::new(
            &obligation,
            &formal,
            KernelOutcome::Accepted {
                receipt: rejecting_receipt(),
            },
        );
        assert_eq!(err, Err(EvidenceError::KernelReceiptInconsistent));
    }

    #[test]
    fn serde_round_trip_cannot_upgrade_kernel_outcome_to_accepted() {
        let (_claim, _obs, _ev, _cj, formal, obligation) = pipeline();
        let rejected = KernelResult::new(
            &obligation,
            &formal,
            KernelOutcome::Rejected {
                receipt: rejecting_receipt(),
            },
        )
        .unwrap();
        let bytes = serde_json::to_vec(&rejected).expect("serialize");
        let restored: KernelResult = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(restored, rejected);
        assert!(!restored.outcome.is_accepting());
        assert_eq!(
            restored.clone().into_accepted(),
            Err(EvidenceError::KernelNotAccepted)
        );

        // Adapter-loss style tampering: rewrite outcome to Accepted while keeping
        // the old content-addressed id. Integrity must fail closed.
        let mut forged = rejected.clone();
        forged.outcome = KernelOutcome::Accepted {
            receipt: accepting_receipt(),
        };
        assert!(!forged.check_id());
        assert_eq!(
            forged.into_accepted(),
            Err(EvidenceError::KernelNotAccepted)
        );

        // Direct status upgrade on evidence via serde must invalidate check_id.
        let (_c, observation, evidence, conjecture, _f, obligation) = pipeline();
        let mut ev_json = serde_json::to_value(&evidence).unwrap();
        ev_json["status"] = serde_json::json!("Proved");
        let tampered_evidence: EvidenceClaim = serde_json::from_value(ev_json).unwrap();
        assert!(!tampered_evidence.check_id());
        assert_ne!(tampered_evidence.status, ClaimStatus::Observed);

        let mut cj_json = serde_json::to_value(&conjecture).unwrap();
        cj_json["status"] = serde_json::json!("Proved");
        let tampered_conjecture: ConjectureCandidate = serde_json::from_value(cj_json).unwrap();
        assert!(!tampered_conjecture.check_id());

        let mut ob_json = serde_json::to_value(&obligation).unwrap();
        ob_json["status"] = serde_json::json!("Proved");
        let tampered_obligation: ProofObligation = serde_json::from_value(ob_json).unwrap();
        assert!(!tampered_obligation.check_id());

        // Timeout remains distinct after round-trip.
        let timeout = KernelResult::new(
            &obligation,
            &formal,
            KernelOutcome::Timeout { elapsed_ms: 12 },
        )
        .unwrap();
        let timeout_rt: KernelResult =
            serde_json::from_slice(&serde_json::to_vec(&timeout).unwrap()).unwrap();
        assert!(matches!(
            timeout_rt.outcome,
            KernelOutcome::Timeout { elapsed_ms: 12 }
        ));
        assert_eq!(
            timeout_rt.into_accepted(),
            Err(EvidenceError::KernelNotAccepted)
        );
        let _ = observation;
    }

    #[test]
    fn rejected_kernel_cannot_seal_via_proof_artifact_constructor() {
        let formal = formal_for(&claim());
        let err =
            ProofArtifact::new_verified(&formal, b"sorry", vec![], repro(), rejecting_receipt());
        assert_eq!(err, Err(ProofArtifactError::KernelRejected));
    }

    #[test]
    fn pipeline_statuses_never_skip_to_proved_without_kernel() {
        let (_claim, observation, evidence, conjecture, _formal, obligation) = pipeline();
        assert_eq!(observation.implied_status(), ClaimStatus::Observed);
        assert_eq!(evidence.status, ClaimStatus::Observed);
        assert_eq!(conjecture.status, ClaimStatus::Conjectured);
        assert_eq!(obligation.status, ClaimStatus::Formalized);
        for status in [
            evidence.status,
            conjecture.status,
            obligation.status,
            observation.implied_status(),
        ] {
            assert_ne!(status, ClaimStatus::Proved);
        }
    }
}
