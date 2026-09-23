//! Thin user/agent entry point over existing `ProofLab` library APIs.
//!
//! Commands expose `reproduce`, cheap `falsify`, `verify-corpus`, `minimize`,
//! `promote` / `formalize`, and read-only PL-2.0 `inspect` without inventing new proof authority. Lean
//! remains the sole `PROVED` backend; CLI success never seals proof status by
//! itself. `inspect` never calls `AcceptedKernel::seal_proof_artifact`.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use prooflab_core::{
    Claim, ClaimBody, ClaimStatus, ConjectureCandidate, EnvironmentLock, EvidenceClaim,
    FormalStatement, FormalizationAuthority, FormalizationMeta, KernelResult, Observation,
    ObservationKind, PromotionAuthority, PromotionMeta, ProofArtifact, ProofObligation, ReproMeta,
    ReproduceOk, RiemannStubEntry, TdiStubEntry, ingest_riemann_stub, ingest_tdi_stub,
    refuse_empirical_proof_seal, refuse_evidence_proof_seal, refuse_riemann_stub_proof_seal,
    refuse_tdi_stub_proof_seal, reproduce as core_reproduce,
};
use prooflab_lean::{
    ExpectedOutcome, ExpectedRemovalOutcome, FalseConjectureReport, LeanKernel,
    MinimizationRunReport, RemovalOutcome, ReproduceOutcome,
    run_controlled_false_conjecture_battery, verify_assumption_minimization, verify_corpus,
};
use serde::Serialize;

/// `ProofLab` agent/UX CLI. Library wrappers only — no new trust authority.
#[derive(Debug, Parser)]
#[command(
    name = "prooflab",
    about = "ProofLab entry point for reproduce / falsify / verify-corpus / minimize / promote / formalize / inspect",
    long_about = "Exposes existing library APIs for agents and local UX.\n\
Lean remains the sole PROVED authority. Cheap falsification may record FALSIFIED only.\n\
Reproduce success is environment/integrity confirmation, not proof status.\n\
Promote/formalize create typed PL-2.0 transition objects only.\n\
Inspect is read-only over PL-2.0 evidence/stub JSON and never seals PROVED."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level CLI commands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Check a stored proof artifact against an observed environment lock.
    ///
    /// Without `--source` / `--formal`, only integrity + lock binding run (core
    /// reproduce). With both, Lean re-verification runs through `LeanKernel`.
    /// Success does not authorize `PROVED`.
    Reproduce {
        /// JSON-encoded [`ProofArtifact`].
        #[arg(long)]
        artifact: PathBuf,
        /// JSON-encoded observed [`EnvironmentLock`].
        #[arg(long)]
        lock: PathBuf,
        /// JSON-encoded [`FormalStatement`] (required with `--source` for Lean re-verify).
        #[arg(long)]
        formal: Option<PathBuf>,
        /// Lean source file bound by the artifact / formal statement.
        #[arg(long)]
        source: Option<PathBuf>,
        /// Lake binary used for Lean re-verification (default: `lake`).
        #[arg(long, default_value = "lake")]
        lake: PathBuf,
    },
    /// Run the PL-1.1 controlled false-conjecture battery (no Lean).
    ///
    /// Records `FALSIFIED` only; never `PROVED`.
    Falsify,
    /// Run the PL-1.0 known-theorem corpus through Lean.
    VerifyCorpus {
        /// Repository root containing `ProofLab/Corpus/`.
        #[arg(long, default_value = ".")]
        repo_root: PathBuf,
        /// Environment digest pinned into corpus `ReproMeta`.
        #[arg(long, default_value = "prooflab-cli-verify-corpus")]
        environment_digest: String,
        /// Optional `ProofLab` revision pin (defaults to `unknown` for local runs).
        #[arg(long, default_value = "unknown")]
        prooflab_revision: String,
        /// Optional JSON observed lock; when set, verification fails closed on drift.
        #[arg(long)]
        lock: Option<PathBuf>,
        /// Lake binary (default: `lake`).
        #[arg(long, default_value = "lake")]
        lake: PathBuf,
    },
    /// Run the PL-1.2 assumption-minimization battery through Lean.
    ///
    /// Rejection is `RemovalRejected` only — never a necessity claim.
    Minimize {
        /// Repository root containing minimization fixtures.
        #[arg(long, default_value = ".")]
        repo_root: PathBuf,
        /// Environment digest pinned into minimization `ReproMeta`.
        #[arg(long, default_value = "prooflab-cli-minimize")]
        environment_digest: String,
        /// Optional `ProofLab` revision pin (defaults to `unknown` for local runs).
        #[arg(long, default_value = "unknown")]
        prooflab_revision: String,
        /// Optional JSON observed lock; when set, verification fails closed on drift.
        #[arg(long)]
        lock: Option<PathBuf>,
        /// Lake binary (default: `lake`).
        #[arg(long, default_value = "lake")]
        lake: PathBuf,
    },
    /// Promote typed evidence into a conjecture candidate with explicit authority.
    ///
    /// Writes a content-addressed `ConjectureCandidate` JSON object. This command
    /// never creates a proof artifact or `PROVED` status.
    Promote {
        /// JSON-encoded content-addressed `Claim`.
        #[arg(long)]
        claim: PathBuf,
        /// One or more JSON-encoded `EvidenceClaim` files.
        #[arg(long, required = true, num_args = 1..)]
        evidence: Vec<PathBuf>,
        /// Candidate statement sketch.
        #[arg(long)]
        statement_sketch: String,
        /// Candidate assumptions; may be repeated.
        #[arg(long = "assumption")]
        assumptions: Vec<String>,
        /// Human or agent authority recording the explicit promotion.
        #[arg(long, value_enum)]
        authority: ActorAuthority,
        /// Stable promoter identifier.
        #[arg(long)]
        promoter_id: String,
        /// Rationale for promoting the evidence.
        #[arg(long)]
        rationale: String,
        /// Destination JSON file for the conjecture candidate.
        #[arg(long)]
        output: PathBuf,
    },
    /// Bind a conjecture to a formal statement with explicit formalization provenance.
    ///
    /// Writes a content-addressed `ProofObligation` JSON object. No Lean process
    /// is invoked; formalization remains untrusted for proof status.
    Formalize {
        /// JSON-encoded `ConjectureCandidate`.
        #[arg(long)]
        conjecture: PathBuf,
        /// JSON-encoded `FormalStatement`.
        #[arg(long)]
        formal: PathBuf,
        /// Human or agent authority recording the formalization.
        #[arg(long, value_enum)]
        authority: ActorAuthority,
        /// Stable formalizer identifier.
        #[arg(long)]
        formalizer_id: String,
        /// Rationale for this conjecture-to-formal-statement binding.
        #[arg(long)]
        rationale: String,
        /// Destination JSON file for the proof obligation.
        #[arg(long)]
        output: PathBuf,
    },
    /// Read-only PL-2.0 evidence / stub-manifest inspect (never seals `PROVED`).
    ///
    /// Deserializes typed evidence-chain JSON or runs label-preserving Riemann/TDI
    /// stub ingest against a fixture manifest. Reports integrity, status/outcome,
    /// and epistemic labels. Does not invoke Lean and never calls
    /// `AcceptedKernel::seal_proof_artifact`.
    Inspect {
        /// Kind of JSON document to inspect.
        #[arg(long, value_enum)]
        kind: InspectKind,
        /// Path to the JSON input.
        #[arg(long)]
        input: PathBuf,
        /// Claim statement used only when ingesting stub manifests (identity pin).
        #[arg(long, default_value = "prooflab-cli-inspect-claim")]
        claim_statement: String,
    },
}

/// Authority labels accepted by PL-2.0 transition commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ActorAuthority {
    Human,
    Agent,
}

impl ActorAuthority {
    const fn promotion(self) -> PromotionAuthority {
        match self {
            Self::Human => PromotionAuthority::Human,
            Self::Agent => PromotionAuthority::Agent,
        }
    }

    const fn formalization(self) -> FormalizationAuthority {
        match self {
            Self::Human => FormalizationAuthority::Human,
            Self::Agent => FormalizationAuthority::Agent,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
        }
    }
}

/// Document kinds accepted by [`Commands::Inspect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum InspectKind {
    /// Content-addressed [`Observation`] JSON.
    Observation,
    /// Content-addressed [`EvidenceClaim`] JSON.
    EvidenceClaim,
    /// Content-addressed [`ConjectureCandidate`] JSON.
    Conjecture,
    /// Content-addressed [`ProofObligation`] JSON.
    Obligation,
    /// Content-addressed [`KernelResult`] JSON (report-only; never seals).
    KernelResult,
    /// Riemann stub fixture manifest (`Vec<RiemannStubEntry>`).
    RiemannStubManifest,
    /// TDI stub fixture manifest (`Vec<TdiStubEntry>`).
    TdiStubManifest,
}

/// Machine-readable CLI outcome envelope.
#[derive(Debug, Clone, Serialize)]
pub struct CliReport {
    pub command: String,
    pub ok: bool,
    pub notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<ReproduceReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub falsify: Option<Vec<FalsifyEntryReport>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corpus: Option<Vec<CorpusEntryReport>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimize: Option<Vec<MinimizeEntryReport>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspect: Option<InspectReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionReport>,
}

/// Summary of one typed PL-2.0 transition written to disk.
#[derive(Debug, Clone, Serialize)]
pub struct TransitionReport {
    pub kind: String,
    pub output: String,
    pub id_hex: String,
    pub status: String,
    pub check_id: bool,
    pub authority: String,
    pub actor_id: String,
}

/// Read-only inspect summary for one evidence-chain or stub-manifest input.
#[derive(Debug, Clone, Serialize)]
pub struct InspectReport {
    pub kind: String,
    pub input: String,
    pub integrity_ok: bool,
    /// Always false: inspect never seals proof artifacts.
    pub sealed_proved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<InspectObjectReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stub_rows: Option<Vec<InspectStubRowReport>>,
}

/// Summary fields for a single typed evidence object.
#[derive(Debug, Clone, Serialize)]
pub struct InspectObjectReport {
    pub id_hex: String,
    pub status: Option<String>,
    pub observation_kind: Option<String>,
    pub evidence_strength: Option<String>,
    pub source_label: Option<String>,
    pub kernel_outcome: Option<String>,
    pub kernel_accepting: Option<bool>,
    pub check_id: bool,
}

/// One stub-manifest row after label-preserving ingest.
#[derive(Debug, Clone, Serialize)]
pub struct InspectStubRowReport {
    pub entry_id: String,
    pub source_label: String,
    pub observation_kind: String,
    pub evidence_strength: String,
    pub status: String,
    pub observation_check_id: bool,
    pub evidence_check_id: bool,
    pub observation_id_hex: String,
    pub evidence_id_hex: String,
    /// Always false for stub ingest paths.
    pub proved: bool,
}

/// Reproduce command summary.
#[derive(Debug, Clone, Serialize)]
pub struct ReproduceReport {
    pub mode: String,
    pub artifact_id_hex: String,
    pub lock_id_hex: String,
    pub lean_reverified: bool,
    pub reverified_artifact_id_hex: Option<String>,
}

/// One falsify battery row.
#[derive(Debug, Clone, Serialize)]
pub struct FalsifyEntryReport {
    pub entry_id: String,
    pub falsified: bool,
    pub proof_search_blocked: bool,
    pub status: Option<String>,
    pub witness_n: Option<u64>,
    pub left_value: Option<u64>,
    pub right_value: Option<u64>,
}

/// One corpus verification row.
#[derive(Debug, Clone, Serialize)]
pub struct CorpusEntryReport {
    pub entry_id: String,
    pub expected: String,
    pub accepted: bool,
    pub produced_proof: bool,
    pub matches_expectation: bool,
}

/// One minimization entry summary.
#[derive(Debug, Clone, Serialize)]
pub struct MinimizeEntryReport {
    pub entry_id: String,
    pub full_accepted: bool,
    pub full_produced_proof: bool,
    pub trials: Vec<MinimizeTrialReport>,
}

/// One removal trial row.
#[derive(Debug, Clone, Serialize)]
pub struct MinimizeTrialReport {
    pub remove_index: usize,
    pub removed_assumption: String,
    pub outcome: String,
    pub expected: String,
    pub matches_expectation: bool,
    pub produced_proof: bool,
    /// Always false: CLI never claims necessity from rejection.
    pub necessity_claimed: bool,
}

/// Run the parsed CLI and return a process exit code.
#[must_use]
pub fn run(cli: Cli) -> ExitCode {
    match execute(cli) {
        Ok(report) => {
            emit(&report);
            if report.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

/// Execute a parsed CLI invocation without exiting the process.
///
/// # Errors
///
/// Returns a human-readable error string on I/O, decode, library, or contract failures.
pub fn execute(cli: Cli) -> Result<CliReport, String> {
    match cli.command {
        Commands::Reproduce {
            artifact,
            lock,
            formal,
            source,
            lake,
        } => cmd_reproduce(
            &artifact,
            &lock,
            formal.as_deref(),
            source.as_deref(),
            &lake,
        ),
        Commands::Falsify => cmd_falsify(),
        Commands::VerifyCorpus {
            repo_root,
            environment_digest,
            prooflab_revision,
            lock,
            lake,
        } => cmd_verify_corpus(
            &repo_root,
            &environment_digest,
            &prooflab_revision,
            lock.as_deref(),
            &lake,
        ),
        Commands::Minimize {
            repo_root,
            environment_digest,
            prooflab_revision,
            lock,
            lake,
        } => cmd_minimize(
            &repo_root,
            &environment_digest,
            &prooflab_revision,
            lock.as_deref(),
            &lake,
        ),
        Commands::Promote {
            claim,
            evidence,
            statement_sketch,
            assumptions,
            authority,
            promoter_id,
            rationale,
            output,
        } => cmd_promote(&PromoteRequest {
            claim_path: &claim,
            evidence_paths: &evidence,
            statement_sketch: &statement_sketch,
            assumptions: &assumptions,
            authority,
            promoter_id: &promoter_id,
            rationale: &rationale,
            output: &output,
        }),
        Commands::Formalize {
            conjecture,
            formal,
            authority,
            formalizer_id,
            rationale,
            output,
        } => cmd_formalize(
            &conjecture,
            &formal,
            authority,
            &formalizer_id,
            &rationale,
            &output,
        ),
        Commands::Inspect {
            kind,
            input,
            claim_statement,
        } => cmd_inspect(kind, &input, &claim_statement),
    }
}

fn emit(report: &CliReport) {
    // Always emit JSON for agent consumption; humans can pretty-print.
    match serde_json::to_string_pretty(report) {
        Ok(text) => println!("{text}"),
        Err(error) => eprintln!("error: failed to serialize report: {error}"),
    }
}

fn cmd_reproduce(
    artifact_path: &Path,
    lock_path: &Path,
    formal_path: Option<&Path>,
    source_path: Option<&Path>,
    lake: &Path,
) -> Result<CliReport, String> {
    let artifact: ProofArtifact = read_json(artifact_path)?;
    let observed: EnvironmentLock = read_json(lock_path)?;

    match (formal_path, source_path) {
        (None, None) => {
            let ok: ReproduceOk = core_reproduce(&artifact, &observed, None).map_err(err_string)?;
            Ok(CliReport {
                command: "reproduce".into(),
                ok: true,
                notes: vec![
                    "core integrity + lock binding succeeded".into(),
                    "reproduce success is not PROVED authorization".into(),
                ],
                reproduce: Some(ReproduceReport {
                    mode: "core".into(),
                    artifact_id_hex: hex32(&ok.artifact_id.0),
                    lock_id_hex: hex32(&ok.lock_id.0),
                    lean_reverified: false,
                    reverified_artifact_id_hex: None,
                }),
                falsify: None,
                corpus: None,
                minimize: None,
                inspect: None,
                transition: None,
            })
        }
        (Some(formal_path), Some(source_path)) => {
            let formal: FormalStatement = read_json(formal_path)?;
            let kernel = LeanKernel::new(lake);
            let outcome: ReproduceOutcome = kernel
                .reproduce(&artifact, &observed, &formal, source_path, None)
                .map_err(err_string)?;
            let reverified_id = outcome.reverified.as_ref().map(|proof| hex32(&proof.id.0));
            Ok(CliReport {
                command: "reproduce".into(),
                ok: true,
                notes: vec![
                    "Lean re-verification accepted under the observed lock".into(),
                    "reproduce success is not PROVED authorization; only ProofArtifact::new_verified seals proof".into(),
                ],
                reproduce: Some(ReproduceReport {
                    mode: "lean".into(),
                    artifact_id_hex: hex32(&outcome.environment.artifact_id.0),
                    lock_id_hex: hex32(&outcome.environment.lock_id.0),
                    lean_reverified: true,
                    reverified_artifact_id_hex: reverified_id,
                }),
                falsify: None,
                corpus: None,
                minimize: None,
                inspect: None,
                transition: None,
            })
        }
        _ => Err(
            "Lean re-verify requires both --formal and --source; omit both for core-only reproduce"
                .into(),
        ),
    }
}

fn cmd_falsify() -> Result<CliReport, String> {
    let reports = run_controlled_false_conjecture_battery().map_err(err_string)?;
    let entries: Vec<FalsifyEntryReport> = reports.iter().map(falsify_entry).collect();
    let ok = entries
        .iter()
        .all(|entry| entry.falsified && entry.proof_search_blocked);
    let mut notes = vec![
        "PL-1.1 controlled false-conjecture battery (no Lean)".into(),
        "falsification records ClaimStatus::Falsified only; never PROVED".into(),
    ];
    if !ok {
        notes.push("one or more entries failed the falsification gate".into());
    }
    Ok(CliReport {
        command: "falsify".into(),
        ok,
        notes,
        reproduce: None,
        falsify: Some(entries),
        corpus: None,
        minimize: None,
        inspect: None,
        transition: None,
    })
}

fn cmd_verify_corpus(
    repo_root: &Path,
    environment_digest: &str,
    prooflab_revision: &str,
    lock_path: Option<&Path>,
    lake: &Path,
) -> Result<CliReport, String> {
    let mut repro = ReproMeta::bootstrap(environment_digest);
    repro.prooflab_revision = prooflab_revision.into();
    let observed = match lock_path {
        Some(path) => Some(read_json::<EnvironmentLock>(path)?),
        None => None,
    };
    let kernel = LeanKernel::new(lake);
    let results =
        verify_corpus(&kernel, repo_root, &repro, observed.as_ref()).map_err(err_string)?;
    let entries: Vec<CorpusEntryReport> = results
        .iter()
        .map(|(_outcome, report)| CorpusEntryReport {
            entry_id: report.entry_id.into(),
            expected: match report.expected {
                ExpectedOutcome::Accept => "accept".into(),
                ExpectedOutcome::Reject => "reject".into(),
            },
            accepted: report.accepted,
            produced_proof: report.produced_proof,
            matches_expectation: report.matches_expectation,
        })
        .collect();
    let ok = entries.iter().all(|entry| entry.matches_expectation);
    let mut notes = vec![
        "PL-1.0 known-theorem corpus through LeanKernel".into(),
        "matching expected accept/reject measures orchestration only; no novelty claim".into(),
        "only sealed ProofArtifact after Lean acceptance is PROVED evidence".into(),
    ];
    if !ok {
        notes.push("one or more corpus entries missed their expected outcome".into());
    }
    Ok(CliReport {
        command: "verify-corpus".into(),
        ok,
        notes,
        reproduce: None,
        falsify: None,
        corpus: Some(entries),
        minimize: None,
        inspect: None,
        transition: None,
    })
}

fn cmd_minimize(
    repo_root: &Path,
    environment_digest: &str,
    prooflab_revision: &str,
    lock_path: Option<&Path>,
    lake: &Path,
) -> Result<CliReport, String> {
    let mut repro = ReproMeta::bootstrap(environment_digest);
    repro.prooflab_revision = prooflab_revision.into();
    let observed = match lock_path {
        Some(path) => Some(read_json::<EnvironmentLock>(path)?),
        None => None,
    };
    let kernel = LeanKernel::new(lake);
    let reports = verify_assumption_minimization(&kernel, repo_root, &repro, observed.as_ref())
        .map_err(err_string)?;
    let entries: Vec<MinimizeEntryReport> = reports.iter().map(minimize_entry).collect();
    let ok = entries.iter().all(|entry| {
        entry.full_accepted
            && entry.full_produced_proof
            && entry.trials.iter().all(|trial| trial.matches_expectation)
            && entry.trials.iter().all(|trial| !trial.necessity_claimed)
    });
    let mut notes = vec![
        "PL-1.2 assumption minimization through LeanKernel".into(),
        "RemovalRejected is not a necessity certificate".into(),
        "Lean remains the sole PROVED authority".into(),
    ];
    if !ok {
        notes.push("one or more minimization controls missed expectations".into());
    }
    Ok(CliReport {
        command: "minimize".into(),
        ok,
        notes,
        reproduce: None,
        falsify: None,
        corpus: None,
        minimize: Some(entries),
        inspect: None,
        transition: None,
    })
}

#[derive(Clone, Copy)]
#[derive(Clone, Copy)]
struct PromoteRequest<'a> {
    claim_path: &'a Path,
    evidence_paths: &'a [PathBuf],
    statement_sketch: &'a str,
    assumptions: &'a [String],
    authority: ActorAuthority,
    promoter_id: &'a str,
    rationale: &'a str,
    output: &'a Path,
}

fn cmd_promote(request: &PromoteRequest<'_>) -> Result<CliReport, String> {
    let PromoteRequest {
        claim_path,
        evidence_paths,
        statement_sketch,
        assumptions,
        authority,
        promoter_id,
        rationale,
        output,
    } = *request;
    let claim: Claim = read_json(claim_path)?;
    if Claim::new(claim.body.clone()).id != claim.id {
        return Err("claim content address mismatch".into());
    }
    if evidence_paths.is_empty() {
        return Err("promote requires at least one evidence file".into());
    }
    let evidence: Vec<EvidenceClaim> = evidence_paths
        .iter()
        .map(|path| read_json(path))
        .collect::<Result<Vec<_>, String>>()?;
    let refs: Vec<&EvidenceClaim> = evidence.iter().collect();
    let promotion =
        PromotionMeta::new(authority.promotion(), promoter_id, rationale).map_err(err_string)?;
    let candidate = ConjectureCandidate::from_evidence(
        claim.id,
        &refs,
        statement_sketch,
        assumptions.to_vec(),
        promotion,
    )
    .map_err(err_string)?;
    write_json(output, &candidate)?;
    let check_id = candidate.check_id();
    Ok(CliReport {
        command: "promote".into(),
        ok: check_id && candidate.status == ClaimStatus::Conjectured,
        notes: vec![
            "explicit PL-2.0 conjecture promotion; no proof authority".into(),
            "output is a ConjectureCandidate and never PROVED".into(),
        ],
        reproduce: None,
        falsify: None,
        corpus: None,
        minimize: None,
        inspect: None,
        transition: Some(TransitionReport {
            kind: "conjecture".into(),
            output: output.display().to_string(),
            id_hex: hex32(&candidate.id.0),
            status: status_name(candidate.status),
            check_id,
            authority: authority.as_str().into(),
            actor_id: promoter_id.into(),
        }),
    })
}

fn cmd_formalize(
    conjecture_path: &Path,
    formal_path: &Path,
    authority: ActorAuthority,
    formalizer_id: &str,
    rationale: &str,
    output: &Path,
) -> Result<CliReport, String> {
    let conjecture: ConjectureCandidate = read_json(conjecture_path)?;
    let formal: FormalStatement = read_json(formal_path)?;
    let meta = FormalizationMeta::new(authority.formalization(), formalizer_id, rationale)
        .map_err(err_string)?;
    let obligation =
        ProofObligation::from_conjecture(&conjecture, &formal, meta).map_err(err_string)?;
    write_json(output, &obligation)?;
    let check_id = obligation.check_id();
    Ok(CliReport {
        command: "formalize".into(),
        ok: check_id && obligation.status == ClaimStatus::Formalized,
        notes: vec![
            "explicit PL-2.0 formalization provenance; no kernel acceptance implied".into(),
            "output is a ProofObligation and never PROVED".into(),
        ],
        reproduce: None,
        falsify: None,
        corpus: None,
        minimize: None,
        inspect: None,
        transition: Some(TransitionReport {
            kind: "proof-obligation".into(),
            output: output.display().to_string(),
            id_hex: hex32(&obligation.id.0),
            status: status_name(obligation.status),
            check_id,
            authority: authority.as_str().into(),
            actor_id: formalizer_id.into(),
        }),
    })
}

fn cmd_inspect(
    kind: InspectKind,
    input: &Path,
    claim_statement: &str,
) -> Result<CliReport, String> {
    let notes = inspect_notes(kind);
    let inspect = match kind {
        InspectKind::Observation => inspect_observation(input)?,
        InspectKind::EvidenceClaim => inspect_evidence_claim(input)?,
        InspectKind::Conjecture => inspect_conjecture(input)?,
        InspectKind::Obligation => inspect_obligation(input)?,
        InspectKind::KernelResult => inspect_kernel_result(input)?,
        InspectKind::RiemannStubManifest => {
            inspect_stub_manifest(input, claim_statement, StubManifestKind::Riemann)?
        }
        InspectKind::TdiStubManifest => {
            inspect_stub_manifest(input, claim_statement, StubManifestKind::Tdi)?
        }
    };
    let ok = inspect_report_ok(kind, &inspect);
    Ok(CliReport {
        command: "inspect".into(),
        ok,
        notes,
        reproduce: None,
        falsify: None,
        corpus: None,
        minimize: None,
        inspect: Some(inspect),
        transition: None,
    })
}

fn inspect_report_ok(kind: InspectKind, inspect: &InspectReport) -> bool {
    if !inspect.integrity_ok || inspect.sealed_proved {
        return false;
    }
    if let Some(rows) = &inspect.stub_rows
        && (rows.is_empty() || rows.iter().any(|row| row.proved))
    {
        return false;
    }
    if let Some(object) = &inspect.object {
        if object.status.as_deref() == Some("proved") {
            return false;
        }
        if matches!(kind, InspectKind::EvidenceClaim)
            && (object.status.as_deref() != Some("observed") || !object.check_id)
        {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy)]
enum StubManifestKind {
    Riemann,
    Tdi,
}

fn inspect_notes(kind: InspectKind) -> Vec<String> {
    let mut notes = vec![
        "PL-2.0 read-only evidence/stub inspect".into(),
        "inspect never seals PROVED / never calls AcceptedKernel::seal_proof_artifact".into(),
        "Lean remains the sole PROVED authority".into(),
    ];
    if matches!(kind, InspectKind::KernelResult) {
        notes.push(
            "accepting KernelResult still requires AcceptedKernel::seal_proof_artifact to PROVED"
                .into(),
        );
    }
    notes
}

fn empty_object_report(id_hex: String, check_id: bool) -> InspectObjectReport {
    InspectObjectReport {
        id_hex,
        status: None,
        observation_kind: None,
        evidence_strength: None,
        source_label: None,
        kernel_outcome: None,
        kernel_accepting: None,
        check_id,
    }
}

fn inspect_observation(input: &Path) -> Result<InspectReport, String> {
    let obj: Observation = read_json(input)?;
    let check = obj.check_id();
    let _ = refuse_empirical_proof_seal(obj.kind);
    let mut object = empty_object_report(hex32(&obj.id.0), check);
    object.status = Some(status_name(obj.implied_status()));
    object.observation_kind = Some(observation_kind_name(obj.kind));
    object.source_label = Some(obj.source_label.clone());
    Ok(InspectReport {
        kind: "observation".into(),
        input: input.display().to_string(),
        integrity_ok: check,
        sealed_proved: false,
        object: Some(object),
        stub_rows: None,
    })
}

fn inspect_evidence_claim(input: &Path) -> Result<InspectReport, String> {
    let obj: EvidenceClaim = read_json(input)?;
    let check = obj.check_id();
    let _ = refuse_evidence_proof_seal(obj.strength);
    let mut object = empty_object_report(hex32(&obj.id.0), check);
    object.status = Some(status_name(obj.status));
    object.evidence_strength = Some(evidence_strength_name(obj.strength));
    Ok(InspectReport {
        kind: "evidence-claim".into(),
        input: input.display().to_string(),
        integrity_ok: check,
        sealed_proved: false,
        object: Some(object),
        stub_rows: None,
    })
}

fn inspect_conjecture(input: &Path) -> Result<InspectReport, String> {
    let obj: ConjectureCandidate = read_json(input)?;
    let check = obj.check_id();
    let mut object = empty_object_report(hex32(&obj.id.0), check);
    object.status = Some(status_name(obj.status));
    Ok(InspectReport {
        kind: "conjecture".into(),
        input: input.display().to_string(),
        integrity_ok: check,
        sealed_proved: false,
        object: Some(object),
        stub_rows: None,
    })
}

fn inspect_obligation(input: &Path) -> Result<InspectReport, String> {
    let obj: ProofObligation = read_json(input)?;
    let check = obj.check_id();
    let mut object = empty_object_report(hex32(&obj.id.0), check);
    object.status = Some(status_name(obj.status));
    Ok(InspectReport {
        kind: "obligation".into(),
        input: input.display().to_string(),
        integrity_ok: check,
        sealed_proved: false,
        object: Some(object),
        stub_rows: None,
    })
}

fn inspect_kernel_result(input: &Path) -> Result<InspectReport, String> {
    let obj: KernelResult = read_json(input)?;
    let check = obj.check_id();
    let mut object = empty_object_report(hex32(&obj.id.0), check);
    object.kernel_outcome = Some(kernel_outcome_name(&obj.outcome));
    object.kernel_accepting = Some(obj.outcome.is_accepting());
    Ok(InspectReport {
        kind: "kernel-result".into(),
        input: input.display().to_string(),
        integrity_ok: check,
        sealed_proved: false,
        object: Some(object),
        stub_rows: None,
    })
}

fn inspect_stub_manifest(
    input: &Path,
    claim_statement: &str,
    kind: StubManifestKind,
) -> Result<InspectReport, String> {
    let claim = Claim::new(ClaimBody {
        statement: claim_statement.into(),
        assumptions: vec![],
        parents: vec![],
    });
    let rows = match kind {
        StubManifestKind::Riemann => {
            let entries: Vec<RiemannStubEntry> = read_json(input)?;
            entries
                .iter()
                .map(|entry| {
                    let ingest = ingest_riemann_stub(claim.id, entry).map_err(err_string)?;
                    let _ = refuse_riemann_stub_proof_seal(entry.source_label);
                    Ok(stub_row_from_parts(
                        &entry.entry_id,
                        entry.source_label.as_str(),
                        &ingest.observation,
                        &ingest.evidence,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?
        }
        StubManifestKind::Tdi => {
            let entries: Vec<TdiStubEntry> = read_json(input)?;
            entries
                .iter()
                .map(|entry| {
                    let ingest = ingest_tdi_stub(claim.id, entry).map_err(err_string)?;
                    let _ = refuse_tdi_stub_proof_seal(entry.source_label);
                    Ok(stub_row_from_parts(
                        &entry.entry_id,
                        entry.source_label.as_str(),
                        &ingest.observation,
                        &ingest.evidence,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?
        }
    };
    let integrity_ok = rows.iter().all(|row| {
        row.observation_check_id && row.evidence_check_id && row.status == "observed" && !row.proved
    });
    let kind_name = match kind {
        StubManifestKind::Riemann => "riemann-stub-manifest",
        StubManifestKind::Tdi => "tdi-stub-manifest",
    };
    Ok(InspectReport {
        kind: kind_name.into(),
        input: input.display().to_string(),
        integrity_ok,
        sealed_proved: false,
        object: None,
        stub_rows: Some(rows),
    })
}

fn stub_row_from_parts(
    entry_id: &str,
    source_label: &str,
    observation: &Observation,
    evidence: &EvidenceClaim,
) -> InspectStubRowReport {
    InspectStubRowReport {
        entry_id: entry_id.into(),
        source_label: source_label.into(),
        observation_kind: observation_kind_name(observation.kind),
        evidence_strength: evidence_strength_name(evidence.strength),
        status: status_name(evidence.status),
        observation_check_id: observation.check_id(),
        evidence_check_id: evidence.check_id(),
        observation_id_hex: hex32(&observation.id.0),
        evidence_id_hex: hex32(&evidence.id.0),
        proved: evidence.status == ClaimStatus::Proved,
    }
}

fn observation_kind_name(kind: ObservationKind) -> String {
    match kind {
        ObservationKind::Numerical => "numerical".into(),
        ObservationKind::SymbolicExperiment => "symbolic_experiment".into(),
        ObservationKind::SolverOutput => "solver_output".into(),
        ObservationKind::ManualAnnotation => "manual_annotation".into(),
        ObservationKind::StubAdapter => "stub_adapter".into(),
        ObservationKind::Counterexample => "counterexample".into(),
    }
}

fn evidence_strength_name(strength: prooflab_core::EvidenceStrength) -> String {
    match strength {
        prooflab_core::EvidenceStrength::Suggestive => "suggestive".into(),
        prooflab_core::EvidenceStrength::Corroborated => "corroborated".into(),
        prooflab_core::EvidenceStrength::Strong => "strong".into(),
    }
}

fn kernel_outcome_name(outcome: &prooflab_core::KernelOutcome) -> String {
    match outcome {
        prooflab_core::KernelOutcome::Accepted { .. } => "accepted".into(),
        prooflab_core::KernelOutcome::Rejected { .. } => "rejected".into(),
        prooflab_core::KernelOutcome::Unknown { .. } => "unknown".into(),
        prooflab_core::KernelOutcome::Timeout { .. } => "timeout".into(),
    }
}

fn falsify_entry(report: &FalseConjectureReport) -> FalsifyEntryReport {
    FalsifyEntryReport {
        entry_id: report.entry_id.into(),
        falsified: report.falsified,
        proof_search_blocked: report.proof_search_blocked,
        status: report.status.map(status_name),
        witness_n: report.witness_n,
        left_value: report.left_value,
        right_value: report.right_value,
    }
}

fn minimize_entry(report: &MinimizationRunReport) -> MinimizeEntryReport {
    MinimizeEntryReport {
        entry_id: report.entry_id.into(),
        full_accepted: report.full_accepted,
        full_produced_proof: report.full_produced_proof,
        trials: report
            .trials
            .iter()
            .map(|trial| MinimizeTrialReport {
                remove_index: trial.remove_index,
                removed_assumption: trial.removed_assumption.into(),
                outcome: match trial.outcome {
                    RemovalOutcome::Removable => "removable".into(),
                    RemovalOutcome::RemovalRejected => "removal_rejected".into(),
                },
                expected: match trial.expected {
                    ExpectedRemovalOutcome::KernelAccepts => "kernel_accepts".into(),
                    ExpectedRemovalOutcome::KernelRejects => "kernel_rejects".into(),
                },
                matches_expectation: trial.matches_expectation,
                produced_proof: trial.produced_proof,
                necessity_claimed: trial.necessity_claimed,
            })
            .collect(),
    }
}

fn status_name(status: ClaimStatus) -> String {
    match status {
        ClaimStatus::Observed => "observed".into(),
        ClaimStatus::Conjectured => "conjectured".into(),
        ClaimStatus::Falsified => "falsified".into(),
        ClaimStatus::Formalized => "formalized".into(),
        ClaimStatus::Proved => "proved".into(),
        ClaimStatus::Generalized => "generalized".into(),
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(err_string)?;
    fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn err_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use prooflab_core::{
        Claim, ClaimBody, FormalBackend, FormalStatement, KernelReceipt, ProofArtifact, ReproMeta,
        sha256_bytes,
    };
    use std::collections::BTreeSet;

    fn sample_artifact() -> (ProofArtifact, EnvironmentLock, FormalStatement, Vec<u8>) {
        let claim = Claim::new(ClaimBody {
            statement: "n = n".into(),
            assumptions: vec![],
            parents: vec![],
        });
        let source = b"theorem refl : True := by trivial\n".to_vec();
        let formal = FormalStatement::lean4(claim.id, &source, vec!["Mathlib".into()]);
        let mut repro = ReproMeta::bootstrap("sha256:cli-test-env");
        repro.prooflab_revision = "cli-test-revision".into();
        let receipt = KernelReceipt::new(
            FormalBackend::Lean4,
            "lake env lean",
            true,
            Some(0),
            sha256_bytes(b"stdout"),
            sha256_bytes(b"stderr"),
        );
        let artifact =
            ProofArtifact::new_verified(&formal, &source, Vec::new(), repro, receipt).unwrap();
        let lock = EnvironmentLock::from_repro(&artifact.body.repro).unwrap();
        (artifact, lock, formal, source)
    }

    #[test]
    fn falsify_battery_succeeds_without_lean() {
        let report = cmd_falsify().expect("falsify");
        assert!(report.ok);
        assert_eq!(report.command, "falsify");
        let entries = report.falsify.expect("entries");
        assert!(entries.len() >= 4);
        for entry in entries {
            assert!(entry.falsified);
            assert!(entry.proof_search_blocked);
            assert_eq!(entry.status.as_deref(), Some("falsified"));
            assert_ne!(entry.status.as_deref(), Some("proved"));
        }
    }

    #[test]
    fn core_reproduce_round_trips_json_inputs() {
        let tmp =
            std::env::temp_dir().join(format!("prooflab-cli-reproduce-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let (artifact, lock, _formal, _source) = sample_artifact();
        let artifact_path = tmp.join("artifact.json");
        let lock_path = tmp.join("lock.json");
        fs::write(
            &artifact_path,
            serde_json::to_vec_pretty(&artifact).unwrap(),
        )
        .unwrap();
        fs::write(&lock_path, serde_json::to_vec_pretty(&lock).unwrap()).unwrap();

        let report = cmd_reproduce(&artifact_path, &lock_path, None, None, Path::new("lake"))
            .expect("reproduce");
        assert!(report.ok);
        let reproduce = report.reproduce.expect("reproduce section");
        assert_eq!(reproduce.mode, "core");
        assert!(!reproduce.lean_reverified);
        assert_eq!(reproduce.artifact_id_hex, hex32(&artifact.id.0));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn lean_reproduce_requires_both_formal_and_source() {
        let tmp = std::env::temp_dir().join(format!(
            "prooflab-cli-reproduce-partial-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let (artifact, lock, formal, _source) = sample_artifact();
        let artifact_path = tmp.join("artifact.json");
        let lock_path = tmp.join("lock.json");
        let formal_path = tmp.join("formal.json");
        fs::write(
            &artifact_path,
            serde_json::to_vec_pretty(&artifact).unwrap(),
        )
        .unwrap();
        fs::write(&lock_path, serde_json::to_vec_pretty(&lock).unwrap()).unwrap();
        fs::write(&formal_path, serde_json::to_vec_pretty(&formal).unwrap()).unwrap();

        let err = cmd_reproduce(
            &artifact_path,
            &lock_path,
            Some(&formal_path),
            None,
            Path::new("lake"),
        )
        .expect_err("partial lean args");
        assert!(err.contains("--formal") && err.contains("--source"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn drifted_lock_fails_closed_for_core_reproduce() {
        let tmp = std::env::temp_dir().join(format!(
            "prooflab-cli-reproduce-drift-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let (artifact, _lock, _formal, _source) = sample_artifact();
        let drifted = EnvironmentLock::new(
            "v4.33.1",
            "0df444a360eaa60ab8c11dca51a86af692955474",
            "cli-test-revision",
            "sha256:different",
        )
        .unwrap();
        let artifact_path = tmp.join("artifact.json");
        let lock_path = tmp.join("lock.json");
        fs::write(
            &artifact_path,
            serde_json::to_vec_pretty(&artifact).unwrap(),
        )
        .unwrap();
        fs::write(&lock_path, serde_json::to_vec_pretty(&drifted).unwrap()).unwrap();

        let err = cmd_reproduce(
            &artifact_path,
            &lock_path,
            None,
            None,
            Path::new("this-must-not-run"),
        )
        .expect_err("drift");
        assert!(err.contains("drift") || err.contains("environment"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn clap_parses_all_primary_commands() {
        use clap::Parser;
        for args in [
            vec!["prooflab", "falsify"],
            vec![
                "prooflab",
                "reproduce",
                "--artifact",
                "a.json",
                "--lock",
                "l.json",
            ],
            vec!["prooflab", "verify-corpus", "--repo-root", "."],
            vec!["prooflab", "minimize", "--repo-root", "."],
            vec![
                "prooflab",
                "promote",
                "--claim",
                "claim.json",
                "--evidence",
                "evidence.json",
                "--statement-sketch",
                "n = n",
                "--authority",
                "agent",
                "--promoter-id",
                "agent-1",
                "--rationale",
                "candidate promotion",
                "--output",
                "conjecture.json",
            ],
            vec![
                "prooflab",
                "formalize",
                "--conjecture",
                "conjecture.json",
                "--formal",
                "formal.json",
                "--authority",
                "agent",
                "--formalizer-id",
                "agent-2",
                "--rationale",
                "Lean translation",
                "--output",
                "obligation.json",
            ],
            vec![
                "prooflab",
                "inspect",
                "--kind",
                "riemann-stub-manifest",
                "--input",
                "m.json",
            ],
            vec![
                "prooflab",
                "inspect",
                "--kind",
                "observation",
                "--input",
                "o.json",
            ],
        ] {
            Cli::try_parse_from(args).expect("parse");
        }
    }

    #[test]
    fn promote_and_formalize_write_typed_transition_objects() {
        let tmp =
            std::env::temp_dir().join(format!("prooflab-cli-transitions-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let claim = Claim::new(ClaimBody {
            statement: "n = n".into(),
            assumptions: vec!["n : Nat".into()],
            parents: vec![],
        });
        let observation = Observation::stub("stub://cli-promote", b"samples");
        let evidence = EvidenceClaim::from_observations(
            claim.id,
            &[&observation],
            prooflab_core::EvidenceStrength::Corroborated,
        )
        .unwrap();
        let claim_path = tmp.join("claim.json");
        let evidence_path = tmp.join("evidence.json");
        let conjecture_path = tmp.join("conjecture.json");
        fs::write(&claim_path, serde_json::to_vec_pretty(&claim).unwrap()).unwrap();
        fs::write(
            &evidence_path,
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();

        let evidence_paths = [evidence_path];
        let assumptions = ["n : Nat".into()];
        let promote = cmd_promote(&PromoteRequest {
            claim_path: &claim_path,
            evidence_paths: &evidence_paths,
            statement_sketch: "n = n",
            assumptions: &assumptions,
            authority: ActorAuthority::Agent,
            promoter_id: "agent-promoter",
            rationale: "promote a typed evidence claim into a conjecture candidate",
            output: &conjecture_path,
        })
        .expect("promote");
        assert!(promote.ok);
        let conjecture: ConjectureCandidate = read_json(&conjecture_path).unwrap();
        assert!(conjecture.check_id());
        assert_eq!(conjecture.status, ClaimStatus::Conjectured);
        assert_eq!(conjecture.promotion.authority, PromotionAuthority::Agent);

        let source = b"theorem refl_nat (n : Nat) : n = n := rfl\n";
        let formal = FormalStatement::lean4(claim.id, source, vec!["Mathlib".into()]);
        let formal_path = tmp.join("formal.json");
        let obligation_path = tmp.join("obligation.json");
        fs::write(&formal_path, serde_json::to_vec_pretty(&formal).unwrap()).unwrap();
        let formalize = cmd_formalize(
            &conjecture_path,
            &formal_path,
            ActorAuthority::Agent,
            "agent-formalizer",
            "translate the conjecture into the pinned Lean statement",
            &obligation_path,
        )
        .expect("formalize");
        assert!(formalize.ok);
        let obligation: ProofObligation = read_json(&obligation_path).unwrap();
        assert!(obligation.check_id());
        assert_eq!(obligation.status, ClaimStatus::Formalized);
        assert_eq!(
            obligation.formalization.authority,
            FormalizationAuthority::Agent
        );
        assert_ne!(obligation.status, ClaimStatus::Proved);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn inspect_riemann_stub_manifest_never_seals_proved() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../experiments/PL-2.0/fixtures/riemann_stub_manifest.json");
        let report = cmd_inspect(
            InspectKind::RiemannStubManifest,
            &manifest,
            "cli-inspect-riemann-fixture",
        )
        .expect("inspect riemann");
        assert!(report.ok);
        assert_eq!(report.command, "inspect");
        let inspect = report.inspect.expect("inspect section");
        assert!(!inspect.sealed_proved);
        assert!(inspect.integrity_ok);
        let rows = inspect.stub_rows.expect("stub rows");
        assert_eq!(rows.len(), 4);
        for row in &rows {
            assert_eq!(row.status, "observed");
            assert!(!row.proved);
            assert!(row.observation_check_id);
            assert!(row.evidence_check_id);
        }
        let labels: Vec<&str> = rows.iter().map(|r| r.source_label.as_str()).collect();
        assert!(labels.contains(&"numerical"));
        assert!(labels.contains(&"exact"));
        assert!(labels.contains(&"conjecture"));
        assert!(labels.contains(&"formal_asymptotic"));
    }

    #[test]
    fn inspect_tdi_stub_manifest_never_seals_proved() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../experiments/PL-2.0/fixtures/tdi_stub_manifest.json");
        let report = cmd_inspect(
            InspectKind::TdiStubManifest,
            &manifest,
            "cli-inspect-tdi-fixture",
        )
        .expect("inspect tdi");
        assert!(report.ok);
        let inspect = report.inspect.expect("inspect section");
        assert!(!inspect.sealed_proved);
        let rows = inspect.stub_rows.expect("stub rows");
        assert_eq!(rows.len(), 4);
        for row in &rows {
            assert_eq!(row.status, "observed");
            assert!(!row.proved);
        }
    }

    #[test]
    fn inspect_observation_json_reports_integrity_without_sealing() {
        let tmp =
            std::env::temp_dir().join(format!("prooflab-cli-inspect-obs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let obs = Observation::stub("stub://cli-inspect", b"payload");
        let path = tmp.join("obs.json");
        fs::write(&path, serde_json::to_vec_pretty(&obs).unwrap()).unwrap();
        let report = cmd_inspect(InspectKind::Observation, &path, "unused").expect("inspect obs");
        assert!(report.ok);
        let inspect = report.inspect.expect("section");
        assert!(!inspect.sealed_proved);
        let object = inspect.object.expect("object");
        assert!(object.check_id);
        assert_eq!(object.status.as_deref(), Some("observed"));
        assert_eq!(object.observation_kind.as_deref(), Some("stub_adapter"));
        assert_ne!(object.status.as_deref(), Some("proved"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn inspect_evidence_claim_rejects_forged_proved_status() {
        let claim = Claim::new(ClaimBody {
            statement: "cli-inspect-forged".into(),
            assumptions: vec![],
            parents: vec![],
        });
        let obs = Observation::stub("stub://cli-inspect-forged", b"x");
        let evidence = EvidenceClaim::from_observations(
            claim.id,
            &[&obs],
            prooflab_core::EvidenceStrength::Strong,
        )
        .unwrap();
        let mut value = serde_json::to_value(&evidence).unwrap();
        value["status"] = serde_json::json!("Proved");
        let tmp = std::env::temp_dir().join(format!(
            "prooflab-cli-inspect-forged-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("forged.json");
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let report = cmd_inspect(InspectKind::EvidenceClaim, &path, "unused").expect("inspect");
        assert!(!report.ok);
        let inspect = report.inspect.expect("section");
        assert!(!inspect.sealed_proved);
        assert!(!inspect.integrity_ok);
        let object = inspect.object.expect("object");
        assert!(!object.check_id);
        assert_eq!(object.status.as_deref(), Some("proved"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hex32_is_stable_length() {
        let bytes = [0u8; 32];
        assert_eq!(hex32(&bytes).len(), 64);
        let mut set = BTreeSet::new();
        set.insert(hex32(&[1; 32]));
        assert_eq!(set.len(), 1);
    }
}
