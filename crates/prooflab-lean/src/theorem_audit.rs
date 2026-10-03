use std::collections::BTreeSet;
use std::fmt;

use crate::LeanProcessResult;

const CONTRACT_MARKER: &str = ";theorem_audit=v1;theorem=";
const ALLOWLIST_MARKER: &str = ";allow_axioms=";

/// Content-addressed policy for the declaration whose proof may be promoted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremAuditPolicy {
    theorem_name: String,
    allowed_axioms: BTreeSet<String>,
}

/// Axiom dependencies reported by a fresh Lean elaboration of the target theorem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremAuditReport {
    pub theorem_name: String,
    pub axioms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TheoremAuditError {
    InvalidDeclarationName(String),
    MissingPolicy,
    InvocationMismatch,
    MissingReport(String),
    SorryAxiom,
    ForbiddenAxioms(Vec<String>),
}

impl fmt::Display for TheoremAuditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDeclarationName(name) => {
                write!(
                    formatter,
                    "invalid Lean declaration name in theorem audit: {name}"
                )
            }
            Self::MissingPolicy => {
                write!(formatter, "verification job is missing theorem audit policy")
            }
            Self::InvocationMismatch => write!(
                formatter,
                "theorem audit policy does not match the pinned invocation contract"
            ),
            Self::MissingReport(name) => write!(
                formatter,
                "Lean accepted but emitted no parseable axiom report for {name}"
            ),
            Self::SorryAxiom => write!(formatter, "target theorem depends on forbidden sorryAx"),
            Self::ForbiddenAxioms(axioms) => {
                write!(
                    formatter,
                    "target theorem depends on non-allowlisted axioms: {}",
                    axioms.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for TheoremAuditError {}

impl TheoremAuditPolicy {
    pub fn new(
        theorem_name: impl Into<String>,
        allowed_axioms: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, TheoremAuditError> {
        let theorem_name = theorem_name.into();
        validate_name(&theorem_name)?;
        let mut allowed = BTreeSet::new();
        for axiom in allowed_axioms {
            let axiom = axiom.into();
            validate_name(&axiom)?;
            if axiom == "sorryAx" || axiom.ends_with(".sorryAx") {
                return Err(TheoremAuditError::SorryAxiom);
            }
            allowed.insert(axiom);
        }
        Ok(Self {
            theorem_name,
            allowed_axioms: allowed,
        })
    }

    #[must_use]
    pub fn theorem_name(&self) -> &str {
        &self.theorem_name
    }

    #[must_use]
    pub fn allowed_axioms(&self) -> &BTreeSet<String> {
        &self.allowed_axioms
    }

    pub(crate) fn invocation_contract(&self, base: &str) -> String {
        let allowlist = self
            .allowed_axioms
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{base}{CONTRACT_MARKER}{}{ALLOWLIST_MARKER}{allowlist}",
            self.theorem_name
        )
    }

    pub(crate) fn from_invocation(
        invocation: &str,
        expected_base: &str,
    ) -> Result<Self, TheoremAuditError> {
        let suffix = invocation
            .strip_prefix(expected_base)
            .ok_or(TheoremAuditError::InvocationMismatch)?;
        let value = suffix
            .strip_prefix(CONTRACT_MARKER)
            .ok_or(TheoremAuditError::MissingPolicy)?;
        let (theorem, allowlist) = value
            .split_once(ALLOWLIST_MARKER)
            .ok_or(TheoremAuditError::InvocationMismatch)?;
        if allowlist.contains(';') {
            return Err(TheoremAuditError::InvocationMismatch);
        }
        let axioms = if allowlist.is_empty() {
            Vec::new()
        } else {
            allowlist.split(',').map(str::to_owned).collect()
        };
        Self::new(theorem, axioms)
    }

    pub(crate) fn audited_source(&self, source: &[u8]) -> Vec<u8> {
        let mut audited = Vec::with_capacity(source.len() + self.theorem_name.len() + 64);
        audited.extend_from_slice(source);
        if !source.ends_with(b"\n") {
            audited.push(b'\n');
        }
        audited.extend_from_slice(b"\n#print axioms ");
        audited.extend_from_slice(self.theorem_name.as_bytes());
        audited.push(b'\n');
        audited
    }

    pub(crate) fn assess(
        &self,
        process: &LeanProcessResult,
    ) -> Result<Option<TheoremAuditReport>, TheoremAuditError> {
        if !process.accepted {
            return Ok(None);
        }
        let output = format!("{}\n{}", process.stdout, process.stderr);
        let axioms = parse_axioms(&output, &self.theorem_name)
            .ok_or_else(|| TheoremAuditError::MissingReport(self.theorem_name.clone()))?;
        if axioms
            .iter()
            .any(|axiom| axiom == "sorryAx" || axiom.ends_with(".sorryAx"))
        {
            return Err(TheoremAuditError::SorryAxiom);
        }
        let forbidden = axioms
            .iter()
            .filter(|axiom| !self.allowed_axioms.contains(*axiom))
            .cloned()
            .collect::<Vec<_>>();
        if !forbidden.is_empty() {
            return Err(TheoremAuditError::ForbiddenAxioms(forbidden));
        }
        Ok(Some(TheoremAuditReport {
            theorem_name: self.theorem_name.clone(),
            axioms,
        }))
    }
}

fn validate_name(name: &str) -> Result<(), TheoremAuditError> {
    if name.is_empty()
        || name.len() > 256
        || name.split('.').any(|segment| segment.is_empty())
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'\''))
    {
        return Err(TheoremAuditError::InvalidDeclarationName(name.to_owned()));
    }
    Ok(())
}

fn parse_axioms(output: &str, theorem_name: &str) -> Option<Vec<String>> {
    let no_axioms = format!("'{theorem_name}' does not depend on any axioms");
    if output.contains(&no_axioms) {
        return Some(Vec::new());
    }
    let marker = format!("'{theorem_name}' depends on axioms:");
    let tail = output.split(&marker).nth(1)?;
    let start = tail.find('[')? + 1;
    let end = tail[start..].find(']')? + start;
    let body = &tail[start..end];
    if body.trim().is_empty() {
        return Some(Vec::new());
    }
    Some(
        body.split(',')
            .map(|axiom| axiom.trim().to_owned())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LeanProcessLimits, ProcessTermination};
    use prooflab_core::sha256_bytes;

    fn accepted(stdout: &str) -> LeanProcessResult {
        LeanProcessResult {
            accepted: true,
            exit_code: Some(0),
            termination: ProcessTermination::Exited,
            limits: LeanProcessLimits::default(),
            stdout: stdout.into(),
            stderr: String::new(),
            stdout_digest: sha256_bytes(stdout.as_bytes()),
            stderr_digest: sha256_bytes(&[]),
        }
    }

    #[test]
    fn policy_round_trips_through_invocation() {
        let policy = TheoremAuditPolicy::new("ProofLab.good", ["Quot.sound", "propext"]).unwrap();
        let invocation = policy.invocation_contract("base");
        assert_eq!(
            TheoremAuditPolicy::from_invocation(&invocation, "base").unwrap(),
            policy
        );
    }

    #[test]
    fn sorry_and_unlisted_axioms_fail_closed() {
        let policy = TheoremAuditPolicy::new("good", ["propext"]).unwrap();
        assert_eq!(
            policy.assess(&accepted("'good' depends on axioms: [sorryAx]")),
            Err(TheoremAuditError::SorryAxiom)
        );
        assert_eq!(
            policy.assess(&accepted("'good' depends on axioms: [bad]")),
            Err(TheoremAuditError::ForbiddenAxioms(vec!["bad".into()]))
        );
    }

    #[test]
    fn explicit_allowlist_is_enforced() {
        let policy = TheoremAuditPolicy::new("good", ["propext"]).unwrap();
        let report = policy
            .assess(&accepted("'good' depends on axioms: [propext]"))
            .unwrap()
            .unwrap();
        assert_eq!(report.axioms, vec!["propext"]);
    }
}
