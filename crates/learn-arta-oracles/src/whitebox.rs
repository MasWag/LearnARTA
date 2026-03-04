// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Exact white-box equivalence oracle.
//!
//! This oracle compares a target ARTA and a hypothesis ARTA by direct BFS
//! over semantic classes of abstract states `(target_formula, hyp_formula)`.
//! Counterexamples are ordered by shortest word first, then by earlier symbols
//! in the supplied alphabet, then by larger delay classes first. For each
//! popped search state and symbol, the search computes one exact
//! representative delay per guard-membership equivalence class induced by the
//! relevant outgoing intervals in the target and hypothesis formulas. Finite
//! classes use their maximum representable delay, while an unbounded tail class
//! uses its lower bound as a stable finite witness.

use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    hash::Hash,
};

use learn_arta_core::{
    Arta, DagStateFormula, DelayRep, LocationId, MinimalModelKey, StateFormula, TimedWord,
    time::interval::Interval,
};
use learn_arta_traits::EquivalenceOracle;
use thiserror::Error;

const MAX_FINITE_HALF_UNITS: u64 = (u32::MAX - 1) as u64;

/// Errors produced by the exact white-box equivalence oracle.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum WhiteBoxEqOracleError {
    /// The supplied alphabet is empty.
    #[error("alphabet must not be empty")]
    EmptyAlphabet,
}

/// Exact white-box equivalence oracle based on direct BFS over formula pairs.
///
/// Returned counterexamples already use [`DelayRep`], so the learner can refine
/// directly without an additional raw-delay normalization step.
#[derive(Debug, Clone)]
pub struct WhiteBoxEqOracle<A, TargetF = DagStateFormula, HypF = DagStateFormula>
where
    A: Eq + Hash + Clone,
    TargetF: StateFormula<Var = LocationId>,
    HypF: StateFormula<Var = LocationId>,
{
    target: Arta<A, TargetF>,
    alphabet: Vec<A>,
    target_semantic_keys: HashMap<TargetF, MinimalModelKey<LocationId>>,
    hyp_semantic_keys: HashMap<HypF, MinimalModelKey<LocationId>>,
}

impl<A, TargetF, HypF> WhiteBoxEqOracle<A, TargetF, HypF>
where
    A: Eq + Hash + Clone,
    TargetF: StateFormula<Var = LocationId>,
    HypF: StateFormula<Var = LocationId>,
{
    /// Create an exact white-box equivalence oracle over `alphabet`.
    ///
    /// The order of `alphabet` affects tie-breaking among equally short
    /// counterexamples.
    ///
    /// # Errors
    ///
    /// Returns [`WhiteBoxEqOracleError::EmptyAlphabet`] when `alphabet` is empty.
    pub fn try_new(
        target: Arta<A, TargetF>,
        alphabet: Vec<A>,
    ) -> Result<Self, WhiteBoxEqOracleError> {
        if alphabet.is_empty() {
            return Err(WhiteBoxEqOracleError::EmptyAlphabet);
        }

        Ok(Self {
            target,
            alphabet,
            target_semantic_keys: HashMap::new(),
            hyp_semantic_keys: HashMap::new(),
        })
    }

    fn target_semantic_key(&mut self, formula: &TargetF) -> MinimalModelKey<LocationId> {
        if let Some(key) = self.target_semantic_keys.get(formula) {
            return key.clone();
        }

        let key = formula.semantic_key();
        self.target_semantic_keys
            .insert(formula.clone(), key.clone());
        key
    }

    fn hyp_semantic_key(&mut self, formula: &HypF) -> MinimalModelKey<LocationId> {
        if let Some(key) = self.hyp_semantic_keys.get(formula) {
            return key.clone();
        }

        let key = formula.semantic_key();
        self.hyp_semantic_keys.insert(formula.clone(), key.clone());
        key
    }

    fn state_key(&mut self, state: &SearchState<TargetF, HypF>) -> SearchStateKey {
        SearchStateKey {
            target_formula: self.target_semantic_key(&state.target_formula),
            hyp_formula: self.hyp_semantic_key(&state.hyp_formula),
        }
    }
}

impl<A, TargetF, HypF> EquivalenceOracle for WhiteBoxEqOracle<A, TargetF, HypF>
where
    A: Eq + Hash + Clone,
    TargetF: StateFormula<Var = LocationId>,
    HypF: StateFormula<Var = LocationId>,
{
    type Symbol = A;
    type CounterexampleDelay = DelayRep;
    type Formula = HypF;
    type Error = WhiteBoxEqOracleError;

    fn find_counterexample(
        &mut self,
        hyp: &Arta<Self::Symbol, Self::Formula>,
    ) -> Result<Option<TimedWord<Self::Symbol>>, Self::Error> {
        let alphabet = self.alphabet.clone();
        let initial_state = SearchState {
            target_formula: self.target.init().clone(),
            hyp_formula: hyp.init().clone(),
        };
        if acceptance_mismatch(&self.target, hyp, &initial_state) {
            return Ok(Some(TimedWord::empty()));
        }

        let initial_key = self.state_key(&initial_state);
        let mut queue = VecDeque::from([initial_state.clone()]);
        let mut visited = HashSet::from([initial_key]);
        let mut predecessors = HashMap::new();

        while let Some(current_state) = queue.pop_front() {
            let current_key = self.state_key(&current_state);
            for symbol in &alphabet {
                let delay_representatives = delay_representatives_for_search_state(
                    &current_state,
                    symbol,
                    &self.target,
                    hyp,
                );
                for delay in delay_representatives {
                    let next_state = SearchState {
                        target_formula: self.target.step_formula(
                            &current_state.target_formula,
                            symbol,
                            delay,
                        ),
                        hyp_formula: hyp.step_formula(&current_state.hyp_formula, symbol, delay),
                    };
                    let next_key = self.state_key(&next_state);

                    // Skip already visited states to avoid cycles and redundant checks.
                    if !visited.insert(next_key.clone()) {
                        continue;
                    }

                    predecessors.insert(
                        next_key.clone(),
                        Predecessor {
                            parent_key: current_key.clone(),
                            symbol: symbol.clone(),
                            delay,
                        },
                    );

                    if acceptance_mismatch(&self.target, hyp, &next_state) {
                        return Ok(Some(reconstruct_counterexample(next_key, &predecessors)));
                    }

                    queue.push_back(next_state);
                }
            }
        }

        Ok(None)
    }
}

#[derive(Debug, Clone)]
struct SearchState<TargetF, HypF>
where
    TargetF: StateFormula<Var = LocationId>,
    HypF: StateFormula<Var = LocationId>,
{
    target_formula: TargetF,
    hyp_formula: HypF,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SearchStateKey {
    target_formula: MinimalModelKey<LocationId>,
    hyp_formula: MinimalModelKey<LocationId>,
}

#[derive(Debug, Clone)]
struct Predecessor<A>
where
    A: Eq + Hash + Clone,
{
    parent_key: SearchStateKey,
    symbol: A,
    delay: DelayRep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HalfUnitRange {
    start: u64,
    end: Option<u64>,
}

impl HalfUnitRange {
    #[cfg(test)]
    fn contains_half_units(&self, half_units: u64) -> bool {
        if half_units < self.start {
            return false;
        }

        match self.end {
            Some(end) => half_units <= end,
            None => true,
        }
    }

    fn descending_preference_delay(&self) -> Option<DelayRep> {
        let half_units = match self.end {
            Some(end) => end,
            None => self.start,
        };
        let half_units = u32::try_from(half_units).ok()?;
        Some(DelayRep::from_half_units(half_units))
    }
}

fn delay_representatives_for_search_state<A, TargetF, HypF>(
    state: &SearchState<TargetF, HypF>,
    symbol: &A,
    target: &Arta<A, TargetF>,
    hyp: &Arta<A, HypF>,
) -> Vec<DelayRep>
where
    A: Eq + Hash + Clone,
    TargetF: StateFormula<Var = LocationId>,
    HypF: StateFormula<Var = LocationId>,
{
    let mut guard_ranges = Vec::new();
    collect_guard_ranges_from_formula(&state.target_formula, symbol, target, &mut guard_ranges);
    collect_guard_ranges_from_formula(&state.hyp_formula, symbol, hyp, &mut guard_ranges);

    partition_half_unit_ranges(&guard_ranges)
        .into_iter()
        .rev()
        .filter_map(|range| range.descending_preference_delay())
        .collect()
}

fn collect_guard_ranges_from_formula<A, F>(
    formula: &F,
    symbol: &A,
    arta: &Arta<A, F>,
    guard_ranges: &mut Vec<HalfUnitRange>,
) where
    A: Eq + Hash + Clone,
    F: StateFormula<Var = LocationId>,
{
    for location in formula.vars() {
        if let Some(edges) = arta.outgoing(&location, symbol) {
            guard_ranges.extend(
                edges
                    .iter()
                    .filter_map(|edge| interval_to_half_unit_range(&edge.guard)),
            );
        }
    }
}

fn interval_to_half_unit_range(interval: &Interval) -> Option<HalfUnitRange> {
    let lower_bound = interval.lower_bound();
    let start = u64::from(lower_bound) * 2
        + if interval.contains(DelayRep::from_integer(lower_bound)) {
            0
        } else {
            1
        };
    if start > MAX_FINITE_HALF_UNITS {
        return None;
    }

    let end = match interval.upper_bound() {
        Some(upper_bound) => {
            let inclusive_offset = if interval.contains(DelayRep::from_integer(upper_bound)) {
                0
            } else {
                1
            };
            let raw_end = u64::from(upper_bound) * 2;
            let end = raw_end.saturating_sub(inclusive_offset);
            Some(end.min(MAX_FINITE_HALF_UNITS))
        }
        None => None,
    };

    if matches!(end, Some(end) if end < start) {
        return None;
    }

    Some(HalfUnitRange { start, end })
}

fn partition_half_unit_ranges(guard_ranges: &[HalfUnitRange]) -> Vec<HalfUnitRange> {
    if guard_ranges.is_empty() {
        return vec![HalfUnitRange {
            start: 0,
            end: None,
        }];
    }

    let mut boundaries = BTreeSet::from([0u64]);
    for range in guard_ranges {
        boundaries.insert(range.start);
        if let Some(end) = range.end {
            boundaries.insert(end.saturating_add(1));
        }
    }

    let ordered_boundaries: Vec<_> = boundaries.into_iter().collect();
    let mut partition = Vec::new();

    for window in ordered_boundaries.windows(2) {
        let start = window[0];
        if start > MAX_FINITE_HALF_UNITS {
            break;
        }

        let end = window[1].saturating_sub(1).min(MAX_FINITE_HALF_UNITS);
        if start <= end {
            partition.push(HalfUnitRange {
                start,
                end: Some(end),
            });
        }
    }

    if let Some(&last_boundary) = ordered_boundaries.last()
        && last_boundary <= MAX_FINITE_HALF_UNITS
    {
        partition.push(HalfUnitRange {
            start: last_boundary,
            end: None,
        });
    }

    partition
}

fn acceptance_mismatch<A, TargetF, HypF>(
    target: &Arta<A, TargetF>,
    hyp: &Arta<A, HypF>,
    state: &SearchState<TargetF, HypF>,
) -> bool
where
    A: Eq + Hash + Clone,
    TargetF: StateFormula<Var = LocationId>,
    HypF: StateFormula<Var = LocationId>,
{
    target.eval(&state.target_formula) != hyp.eval(&state.hyp_formula)
}

fn reconstruct_counterexample<A>(
    terminal_key: SearchStateKey,
    predecessors: &HashMap<SearchStateKey, Predecessor<A>>,
) -> TimedWord<A>
where
    A: Eq + Hash + Clone,
{
    let mut current_key = terminal_key;
    let mut reversed_letters = Vec::new();

    while let Some(predecessor) = predecessors.get(&current_key) {
        reversed_letters.push((predecessor.symbol.clone(), predecessor.delay));
        current_key = predecessor.parent_key.clone();
    }

    reversed_letters.reverse();
    TimedWord::from_vec(reversed_letters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use learn_arta_core::time::interval::Interval;

    fn ranges_from_intervals(intervals: &[Interval]) -> Vec<HalfUnitRange> {
        intervals
            .iter()
            .filter_map(interval_to_half_unit_range)
            .collect()
    }

    fn membership_signature(ranges: &[HalfUnitRange], delay: DelayRep) -> Vec<bool> {
        let half_units = u64::from(delay.half_units());
        ranges
            .iter()
            .map(|range| range.contains_half_units(half_units))
            .collect()
    }

    #[test]
    fn partition_defaults_to_zero_when_no_guards_exist() {
        let partition = partition_half_unit_ranges(&[]);
        assert_eq!(
            partition,
            vec![HalfUnitRange {
                start: 0,
                end: None,
            }]
        );
        assert_eq!(
            partition
                .into_iter()
                .filter_map(|range| range.descending_preference_delay())
                .collect::<Vec<_>>(),
            vec![DelayRep::ZERO]
        );
    }

    #[test]
    fn partition_cells_are_sorted_non_empty_and_deterministic() {
        let guard_ranges = ranges_from_intervals(&[
            Interval::left_closed_right_open(0, 1).expect("valid interval"),
            Interval::closed(1, 3).expect("valid interval"),
            Interval::from_bounds(true, 5, false, None).expect("valid interval"),
        ]);

        let first = partition_half_unit_ranges(&guard_ranges);
        let second = partition_half_unit_ranges(&guard_ranges);

        assert_eq!(first, second);
        assert!(!first.is_empty());
        assert!(first.iter().all(|range| match range.end {
            Some(end) => range.start <= end,
            None => true,
        }));
        assert!(first.windows(2).all(|pair| pair[0].start < pair[1].start));
    }

    #[test]
    fn descending_delay_preference_uses_upper_bound_for_finite_cells_and_start_for_tail() {
        let partition = [
            HalfUnitRange {
                start: 1,
                end: Some(2),
            },
            HalfUnitRange {
                start: 3,
                end: Some(6),
            },
            HalfUnitRange {
                start: 8,
                end: None,
            },
        ];

        let representatives: Vec<_> = partition
            .iter()
            .rev()
            .filter_map(HalfUnitRange::descending_preference_delay)
            .collect();

        assert_eq!(
            representatives,
            vec![
                DelayRep::from_half_units(8),
                DelayRep::from_half_units(6),
                DelayRep::from_half_units(2),
            ]
        );
    }

    #[test]
    fn descending_representatives_stay_inside_same_partition_cell() {
        let guard_ranges = ranges_from_intervals(&[
            Interval::left_open_right_closed(0, 1).expect("valid interval"),
            Interval::closed(1, 1).expect("valid interval"),
            Interval::from_bounds(true, 2, false, None).expect("valid interval"),
        ]);
        let partition = partition_half_unit_ranges(&guard_ranges);

        for cell in partition {
            let first = cell
                .descending_preference_delay()
                .expect("partition cell must have a representative");
            let second_half_units = match cell.end {
                Some(_) => cell.start,
                None => cell.start.saturating_add(1).min(MAX_FINITE_HALF_UNITS),
            };
            let second = DelayRep::from_half_units(
                u32::try_from(second_half_units).expect("cell witness must fit into DelayRep"),
            );

            assert_eq!(
                membership_signature(&guard_ranges, first),
                membership_signature(&guard_ranges, second)
            );
        }
    }

    #[test]
    fn adjacent_partition_cells_change_membership_pattern() {
        let guard_ranges = ranges_from_intervals(&[
            Interval::left_open_right_closed(0, 1).expect("valid interval"),
            Interval::closed(1, 1).expect("valid interval"),
            Interval::from_bounds(true, 2, false, None).expect("valid interval"),
        ]);
        let partition = partition_half_unit_ranges(&guard_ranges);
        let representatives: Vec<_> = partition
            .iter()
            .rev()
            .filter_map(HalfUnitRange::descending_preference_delay)
            .collect();

        for pair in representatives.windows(2) {
            assert_ne!(
                membership_signature(&guard_ranges, pair[0]),
                membership_signature(&guard_ranges, pair[1])
            );
        }
    }
}
