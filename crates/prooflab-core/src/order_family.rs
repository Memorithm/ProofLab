//! Deterministic finite total-order families for PL-DC order stress tests.
//!
//! The constructors in this module generate explicit finite families only. They
//! are search heuristics, not surrogates for quantification over all total
//! orders. Every returned member is a validated permutation of the exact runtime
//! domain `0..domain_size`.

use core::fmt;
use std::collections::BTreeSet;

/// Failure while validating or generating a finite total-order family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderFamilyError {
    /// The runtime domain cannot be represented as an explicit vector length.
    DomainNotAddressable(u64),
    /// The reference order length differs from the declared runtime domain.
    WrongCardinality { expected: u64, actual: usize },
    /// The reference order contains an element outside `0..domain_size`.
    ElementOutOfDomain { element: u64, domain_size: u64 },
    /// The reference order repeats one domain element.
    DuplicateElement(u64),
    /// The generated family cannot be reserved on this platform.
    FamilyNotAddressable { members: usize },
}

impl fmt::Display for OrderFamilyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DomainNotAddressable(domain_size) => write!(
                formatter,
                "order-family domain size {domain_size} cannot be indexed on this platform"
            ),
            Self::WrongCardinality { expected, actual } => write!(
                formatter,
                "order has cardinality {actual}, expected runtime domain size {expected}"
            ),
            Self::ElementOutOfDomain {
                element,
                domain_size,
            } => write!(
                formatter,
                "order element {element} is outside runtime domain 0..{domain_size}"
            ),
            Self::DuplicateElement(element) => {
                write!(formatter, "order contains duplicate element {element}")
            }
            Self::FamilyNotAddressable { members } => {
                write!(formatter, "order family with {members} members cannot be represented")
            }
        }
    }
}

impl std::error::Error for OrderFamilyError {}

/// Validate one explicit runtime-sized total order.
///
/// # Errors
///
/// Fails closed if `domain_size` cannot be represented on this platform or if
/// `order` is not a permutation of the exact domain `0..domain_size`.
pub fn validate_total_order(domain_size: u64, order: &[u64]) -> Result<(), OrderFamilyError> {
    let expected = usize::try_from(domain_size)
        .map_err(|_| OrderFamilyError::DomainNotAddressable(domain_size))?;
    if order.len() != expected {
        return Err(OrderFamilyError::WrongCardinality {
            expected: domain_size,
            actual: order.len(),
        });
    }

    let mut seen = BTreeSet::new();
    for &element in order {
        if element >= domain_size {
            return Err(OrderFamilyError::ElementOutOfDomain {
                element,
                domain_size,
            });
        }
        if !seen.insert(element) {
            return Err(OrderFamilyError::DuplicateElement(element));
        }
    }

    Ok(())
}

/// Build the deterministic Kendall-radius-one neighborhood of a reference order.
///
/// The returned family contains the reference order first, followed by the
/// orders obtained by swapping positions `(0,1)`, `(1,2)`, ... in that order.
/// For a domain of size `n >= 1`, the family therefore contains exactly `n`
/// members. For the empty runtime domain the only member is the empty order;
/// callers whose structure substrate forbids empty domains remain responsible
/// for enforcing that stronger invariant.
///
/// This finite local neighborhood is intended to falsify order robustness
/// cheaply. Failure to find a witness in it says nothing about unsearched orders.
///
/// # Errors
///
/// Fails closed if `reference` is not an exact total order of `0..domain_size`
/// or if the output family cannot be represented on this platform.
pub fn adjacent_transposition_order_family(
    domain_size: u64,
    reference: &[u64],
) -> Result<Vec<Vec<u64>>, OrderFamilyError> {
    validate_total_order(domain_size, reference)?;

    let members = reference.len().max(1);
    let mut family = Vec::new();
    family
        .try_reserve_exact(members)
        .map_err(|_| OrderFamilyError::FamilyNotAddressable { members })?;
    family.push(reference.to_vec());

    for position in 0..reference.len().saturating_sub(1) {
        let mut neighbor = reference.to_vec();
        neighbor.swap(position, position + 1);
        family.push(neighbor);
    }

    Ok(family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_one_family_has_reference_then_adjacent_swaps() {
        let reference = vec![2, 0, 3, 1];
        let family = adjacent_transposition_order_family(4, &reference).unwrap();

        assert_eq!(
            family,
            vec![
                vec![2, 0, 3, 1],
                vec![0, 2, 3, 1],
                vec![2, 3, 0, 1],
                vec![2, 0, 1, 3],
            ]
        );
    }

    #[test]
    fn every_generated_member_remains_a_total_order() {
        let family = adjacent_transposition_order_family(5, &[4, 1, 3, 0, 2]).unwrap();
        assert_eq!(family.len(), 5);
        for order in &family {
            validate_total_order(5, order).unwrap();
        }
    }

    #[test]
    fn singleton_and_empty_domains_have_one_member() {
        assert_eq!(
            adjacent_transposition_order_family(1, &[0]).unwrap(),
            vec![vec![0]]
        );
        assert_eq!(
            adjacent_transposition_order_family(0, &[]).unwrap(),
            vec![Vec::<u64>::new()]
        );
    }

    #[test]
    fn malformed_reference_orders_fail_closed() {
        assert_eq!(
            adjacent_transposition_order_family(3, &[0, 1]),
            Err(OrderFamilyError::WrongCardinality {
                expected: 3,
                actual: 2,
            })
        );
        assert_eq!(
            adjacent_transposition_order_family(3, &[0, 1, 3]),
            Err(OrderFamilyError::ElementOutOfDomain {
                element: 3,
                domain_size: 3,
            })
        );
        assert_eq!(
            adjacent_transposition_order_family(3, &[0, 1, 1]),
            Err(OrderFamilyError::DuplicateElement(1))
        );
    }

    #[test]
    fn generation_is_deterministic() {
        let reference = vec![3, 0, 2, 1];
        let first = adjacent_transposition_order_family(4, &reference).unwrap();
        let second = adjacent_transposition_order_family(4, &reference).unwrap();
        assert_eq!(first, second);
    }
}
