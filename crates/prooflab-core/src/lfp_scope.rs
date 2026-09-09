//! Free-variable scope validation for positive LFP definitions.
//!
//! Tuple parameters are the only first-order variables permitted to remain
//! free in an [`LfpDefinition`]. Auxiliary variables must be bound by ordinary
//! first-order quantifiers in the defining body.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{FoAtom, LfpAtom, LfpBody, LfpDefinition, Variable};

impl LfpDefinition {
    /// Verify that every free first-order variable is one of the LFP tuple parameters.
    ///
    /// # Errors
    ///
    /// Returns [`LfpScopeError::UnexpectedFreeVariables`] when the body contains
    /// free auxiliary variables that are neither tuple parameters nor locally bound.
    pub fn validate_scope(&self) -> Result<(), LfpScopeError> {
        let mut bound = Vec::new();
        let mut free = BTreeSet::new();
        collect_free_variables(self.body(), &mut bound, &mut free);

        let parameters: BTreeSet<_> = self.parameters().iter().copied().collect();
        let unexpected: Vec<_> = free.difference(&parameters).copied().collect();
        if unexpected.is_empty() {
            Ok(())
        } else {
            Err(LfpScopeError::UnexpectedFreeVariables(unexpected))
        }
    }
}

fn collect_free_variables(
    body: &LfpBody,
    bound: &mut Vec<Variable>,
    free: &mut BTreeSet<Variable>,
) {
    match body {
        LfpBody::True | LfpBody::False => {}
        LfpBody::Atom(atom) => collect_atom_variables(atom, bound, free),
        LfpBody::Not(inner) => collect_free_variables(inner, bound, free),
        LfpBody::And(parts) | LfpBody::Or(parts) => {
            for part in parts {
                collect_free_variables(part, bound, free);
            }
        }
        LfpBody::Exists { variable, body } | LfpBody::ForAll { variable, body } => {
            bound.push(*variable);
            collect_free_variables(body, bound, free);
            let popped = bound.pop();
            debug_assert_eq!(popped, Some(*variable));
        }
    }
}

fn collect_atom_variables(atom: &LfpAtom, bound: &[Variable], free: &mut BTreeSet<Variable>) {
    match atom {
        LfpAtom::FirstOrder(atom) => match atom {
            FoAtom::Equal(left, right) | FoAtom::LessThan(left, right) => {
                collect_variable(*left, bound, free);
                collect_variable(*right, bound, free);
            }
            FoAtom::Relation { args, .. } => {
                for &variable in args {
                    collect_variable(variable, bound, free);
                }
            }
        },
        LfpAtom::Recursive(args) => {
            for &variable in args {
                collect_variable(variable, bound, free);
            }
        }
    }
}

fn collect_variable(variable: Variable, bound: &[Variable], free: &mut BTreeSet<Variable>) {
    if !bound.contains(&variable) {
        let _ = free.insert(variable);
    }
}

/// First-order scope failures in an LFP definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LfpScopeError {
    /// Variables other than tuple parameters remain free in the defining body.
    UnexpectedFreeVariables(Vec<Variable>),
}

impl fmt::Display for LfpScopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedFreeVariables(variables) => write!(
                formatter,
                "LFP body contains free non-parameter variables: {variables:?}"
            ),
        }
    }
}

impl Error for LfpScopeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuple_parameters_may_remain_free() {
        let x = Variable(0);
        let definition = LfpDefinition::new(
            vec![x],
            LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Equal(x, x))),
        )
        .unwrap();

        assert_eq!(definition.validate_scope(), Ok(()));
    }

    #[test]
    fn free_auxiliary_variables_fail_closed() {
        let x = Variable(0);
        let z = Variable(2);
        let definition = LfpDefinition::new(
            vec![x],
            LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Equal(x, z))),
        )
        .unwrap();

        assert_eq!(
            definition.validate_scope(),
            Err(LfpScopeError::UnexpectedFreeVariables(vec![z]))
        );
    }

    #[test]
    fn quantified_auxiliary_variables_are_not_free() {
        let x = Variable(0);
        let z = Variable(2);
        let definition = LfpDefinition::new(
            vec![x],
            LfpBody::Exists {
                variable: z,
                body: Box::new(LfpBody::Atom(LfpAtom::FirstOrder(FoAtom::Equal(x, z)))),
            },
        )
        .unwrap();

        assert_eq!(definition.validate_scope(), Ok(()));
    }
}
