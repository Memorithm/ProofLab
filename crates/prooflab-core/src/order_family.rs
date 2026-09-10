//! Deterministic finite total-order families for PL-DC order stress tests.
//!
//! The constructors in this module generate explicit finite families only. The
//! adjacent-transposition family is a search heuristic; the exhaustive family
//! is exhaustive only when its explicit factorial budget admits every order.
//! Every returned member is a validated permutation of the exact runtime domain
//! `0..domain_size`.

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
    /// Exhaustive enumeration would exceed the caller-declared order budget.
    ExhaustiveLimitExceeded { domain_size: u64, max_orders: usize },
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
                write!(
                    formatter,
                    "order family with {members} members cannot be represented"
                )
            }
            Self::ExhaustiveLimitExceeded {
                domain_size,
                max_orders,
            } => write!(
                formatter,
                "exhaustive order family for domain size {domain_size} exceeds explicit budget {max_orders}"
            ),
        }
    }
}

impl std::error::Error for OrderFamilyError {}

/// Exact all-orders family for one finite runtime carrier.
///
/// Values of this type are created only after the factorial budget check has
/// succeeded and every permutation has been generated. The wrapper preserves
/// the distinction between a genuinely exhaustive small-carrier family and an
/// arbitrary or heuristic `Vec<Vec<u64>>` supplied elsewhere in the research
/// pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExhaustiveOrderFamily {
    domain_size: u64,
    orders: Vec<Vec<u64>>,
}

impl ExhaustiveOrderFamily {
    /// Exact finite carrier size for which every total order was generated.
    #[must_use]
    pub const fn domain_size(&self) -> u64 {
        self.domain_size
    }

    /// Number of total orders in the exhaustive family.
    #[must_use]
    pub const fn order_count(&self) -> usize {
        self.orders.len()
    }

    /// Every total order of the exact finite carrier, in lexicographic order.
    #[must_use]
    pub fn orders(&self) -> &[Vec<u64>] {
        &self.orders
    }

    /// Consume the provenance wrapper and return the exact generated orders.
    #[must_use]
    pub fn into_orders(self) -> Vec<Vec<u64>> {
        self.orders
    }
}

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

/// Enumerate every total order of `0..domain_size` in deterministic lexicographic order.
///
/// Exhaustiveness is permitted only when `domain_size! <= max_orders`. The
/// factorial bound is checked before the family is allocated or any permutation
/// is generated, so callers cannot accidentally turn a bounded PL-DC experiment
/// into an unbounded factorial search. A successful result is exhaustive for the
/// exact finite carrier, including the singleton empty order when `domain_size = 0`.
/// The returned [`ExhaustiveOrderFamily`] retains that provenance explicitly.
///
/// # Errors
///
/// Fails closed when the runtime domain is not addressable, the exhaustive
/// factorial family exceeds `max_orders`, or the exact result vector cannot be
/// reserved on this platform.
pub fn exhaustive_total_order_family(
    domain_size: u64,
    max_orders: usize,
) -> Result<ExhaustiveOrderFamily, OrderFamilyError> {
    let domain_len = usize::try_from(domain_size)
        .map_err(|_| OrderFamilyError::DomainNotAddressable(domain_size))?;
    let members = factorial_with_limit(domain_size, domain_len, max_orders)?;

    let mut orders = Vec::new();
    orders
        .try_reserve_exact(members)
        .map_err(|_| OrderFamilyError::FamilyNotAddressable { members })?;

    let mut current: Vec<u64> = (0..domain_size).collect();
    loop {
        orders.push(current.clone());
        if !next_lexicographic_permutation(&mut current) {
            break;
        }
    }

    debug_assert_eq!(orders.len(), members);
    Ok(ExhaustiveOrderFamily {
        domain_size,
        orders,
    })
}

fn factorial_with_limit(
    domain_size: u64,
    domain_len: usize,
    max_orders: usize,
) -> Result<usize, OrderFamilyError> {
    let mut count = 1usize;
    for factor in 2..=domain_len {
        if count > max_orders / factor {
            return Err(OrderFamilyError::ExhaustiveLimitExceeded {
                domain_size,
                max_orders,
            });
        }
        count *= factor;
    }
    if count > max_orders {
        return Err(OrderFamilyError::ExhaustiveLimitExceeded {
            domain_size,
            max_orders,
        });
    }
    Ok(count)
}

fn next_lexicographic_permutation(values: &mut [u64]) -> bool {
    let Some(pivot) = (0..values.len().saturating_sub(1))
        .rev()
        .find(|&index| values[index] < values[index + 1])
    else {
        return false;
    };

    let successor = ((pivot + 1)..values.len())
        .rev()
        .find(|&index| values[pivot] < values[index])
        .expect("a lexicographic pivot always has a larger suffix element");
    values.swap(pivot, successor);
    values[(pivot + 1)..].reverse();
    true
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

    #[test]
    fn exhaustive_three_element_family_is_complete_and_lexicographic() {
        let family = exhaustive_total_order_family(3, 6).unwrap();
        assert_eq!(family.domain_size(), 3);
        assert_eq!(family.order_count(), 6);
        assert_eq!(
            family.orders(),
            &[
                vec![0, 1, 2],
                vec![0, 2, 1],
                vec![1, 0, 2],
                vec![1, 2, 0],
                vec![2, 0, 1],
                vec![2, 1, 0],
            ]
        );
        for order in family.orders() {
            validate_total_order(3, order).unwrap();
        }
    }

    #[test]
    fn exhaustive_family_requires_explicit_factorial_budget() {
        assert_eq!(
            exhaustive_total_order_family(4, 23),
            Err(OrderFamilyError::ExhaustiveLimitExceeded {
                domain_size: 4,
                max_orders: 23,
            })
        );
        assert_eq!(
            exhaustive_total_order_family(4, 24).unwrap().order_count(),
            24
        );
    }

    #[test]
    fn exhaustive_empty_and_singleton_families_are_exact() {
        assert_eq!(
            exhaustive_total_order_family(0, 1).unwrap().orders(),
            &[Vec::<u64>::new()]
        );
        assert_eq!(
            exhaustive_total_order_family(1, 1).unwrap().orders(),
            &[vec![0]]
        );
        assert_eq!(
            exhaustive_total_order_family(0, 0),
            Err(OrderFamilyError::ExhaustiveLimitExceeded {
                domain_size: 0,
                max_orders: 0,
            })
        );
    }

    #[test]
    fn exhaustive_generation_is_deterministic() {
        let first = exhaustive_total_order_family(5, 120).unwrap();
        let second = exhaustive_total_order_family(5, 120).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn exhaustive_provenance_survives_until_explicit_consumption() {
        let family = exhaustive_total_order_family(3, 6).unwrap();
        assert_eq!(family.domain_size(), 3);
        assert_eq!(family.order_count(), 6);
        assert_eq!(family.into_orders().len(), 6);
    }
}
