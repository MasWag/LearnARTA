// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::traits::StateFormula;

/// Canonical semantic key for a positive Boolean formula.
///
/// The key is the antichain of minimal satisfying variable sets:
/// - `[]` represents `⊥`
/// - `[[]]` represents `⊤`
///
/// Each clause is sorted and deduplicated. The list of clauses is sorted
/// lexicographically and contains no strict supersets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct MinimalModelKey<V> {
    clauses: Vec<Vec<V>>,
}

impl<V> MinimalModelKey<V> {
    /// Borrow the canonical minimal satisfying sets.
    pub fn clauses(&self) -> &[Vec<V>] {
        &self.clauses
    }
}

impl<V> MinimalModelKey<V>
where
    V: Ord,
{
    /// Canonical key for `⊥`.
    pub fn bot() -> Self {
        Self {
            clauses: Vec::new(),
        }
    }

    /// Canonical key for `⊤`.
    pub fn top() -> Self {
        Self {
            clauses: vec![Vec::new()],
        }
    }

    /// Canonical key for a single variable.
    pub fn var(v: V) -> Self {
        Self {
            clauses: vec![vec![v]],
        }
    }

    /// Build a canonical semantic key from raw satisfying clauses.
    pub fn from_clauses(clauses: impl IntoIterator<Item = Vec<V>>) -> Self {
        Self {
            clauses: normalize_clauses(clauses),
        }
    }
}

impl<V> MinimalModelKey<V>
where
    V: Ord + Clone,
{
    pub(super) fn union_all(keys: impl IntoIterator<Item = Self>) -> Self {
        let clauses = keys
            .into_iter()
            .flat_map(|key| key.clauses)
            .collect::<Vec<_>>();
        Self::from_clauses(clauses)
    }

    pub(super) fn intersection_all(keys: impl IntoIterator<Item = Self>) -> Self {
        let mut iter = keys.into_iter();
        let Some(first) = iter.next() else {
            return Self::top();
        };

        let mut current = first.clauses;
        for key in iter {
            if current.is_empty() || key.clauses.is_empty() {
                return Self::bot();
            }

            let mut combined = Vec::new();
            for left in &current {
                for right in &key.clauses {
                    combined.push(merge_sorted_unique(left, right));
                }
            }
            current = normalize_clauses(combined);
        }

        Self { clauses: current }
    }
}

pub(super) fn default_semantic_key<F>(formula: &F) -> MinimalModelKey<F::Var>
where
    F: StateFormula,
    F::Var: Ord,
{
    let mut vars = formula.vars();
    vars.sort();
    vars.dedup();

    let mut assignment = Vec::with_capacity(vars.len());
    let mut clauses = Vec::new();
    enumerate_satisfying_assignments(formula, &vars, 0, &mut assignment, &mut clauses);
    MinimalModelKey::from_clauses(clauses)
}

fn enumerate_satisfying_assignments<F>(
    formula: &F,
    vars: &[F::Var],
    index: usize,
    assignment: &mut Vec<F::Var>,
    clauses: &mut Vec<Vec<F::Var>>,
) where
    F: StateFormula,
    F::Var: Ord,
{
    if index == vars.len() {
        if F::eval_bool(formula, |var| assignment.binary_search(&var).is_ok()) {
            clauses.push(assignment.clone());
        }
        return;
    }

    enumerate_satisfying_assignments(formula, vars, index + 1, assignment, clauses);
    assignment.push(vars[index].clone());
    enumerate_satisfying_assignments(formula, vars, index + 1, assignment, clauses);
    assignment.pop();
}

fn normalize_clauses<V>(clauses: impl IntoIterator<Item = Vec<V>>) -> Vec<Vec<V>>
where
    V: Ord,
{
    let mut normalized = clauses
        .into_iter()
        .map(|mut clause| {
            clause.sort();
            clause.dedup();
            clause
        })
        .collect::<Vec<_>>();

    normalized.sort_by(|lhs, rhs| lhs.len().cmp(&rhs.len()).then_with(|| lhs.cmp(rhs)));
    normalized.dedup();

    let mut minimal: Vec<Vec<V>> = Vec::new();
    for clause in normalized {
        if minimal
            .iter()
            .any(|existing| is_sorted_subset(existing, &clause))
        {
            continue;
        }
        minimal.push(clause);
    }

    minimal.sort();
    minimal
}

fn is_sorted_subset<V>(lhs: &[V], rhs: &[V]) -> bool
where
    V: Ord,
{
    let mut lhs_index = 0usize;
    let mut rhs_index = 0usize;

    while lhs_index < lhs.len() && rhs_index < rhs.len() {
        match lhs[lhs_index].cmp(&rhs[rhs_index]) {
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {
                lhs_index += 1;
                rhs_index += 1;
            }
            std::cmp::Ordering::Greater => rhs_index += 1,
        }
    }

    lhs_index == lhs.len()
}

fn merge_sorted_unique<V>(lhs: &[V], rhs: &[V]) -> Vec<V>
where
    V: Ord + Clone,
{
    let mut merged = Vec::with_capacity(lhs.len() + rhs.len());
    let mut lhs_index = 0usize;
    let mut rhs_index = 0usize;

    while lhs_index < lhs.len() && rhs_index < rhs.len() {
        match lhs[lhs_index].cmp(&rhs[rhs_index]) {
            std::cmp::Ordering::Less => {
                merged.push(lhs[lhs_index].clone());
                lhs_index += 1;
            }
            std::cmp::Ordering::Equal => {
                merged.push(lhs[lhs_index].clone());
                lhs_index += 1;
                rhs_index += 1;
            }
            std::cmp::Ordering::Greater => {
                merged.push(rhs[rhs_index].clone());
                rhs_index += 1;
            }
        }
    }

    while lhs_index < lhs.len() {
        merged.push(lhs[lhs_index].clone());
        lhs_index += 1;
    }
    while rhs_index < rhs.len() {
        merged.push(rhs[rhs_index].clone());
        rhs_index += 1;
    }

    merged
}
