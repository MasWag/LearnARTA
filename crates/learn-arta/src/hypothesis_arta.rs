// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hypothesis ARTA construction from an evidence AFA (Algorithm 3).

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;

use learn_arta_core::{
    Arta, ArtaBuilder, ArtaError, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId,
    StateFormula,
    partition::{PartitionError, infer_guard_intervals_from_delays},
};
use thiserror::Error;

use crate::{AfaStateId, BasisFormula, BasisVar, EvidenceAfa};

/// Errors produced while converting an evidence AFA into a hypothesis ARTA.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HypothesisArtaError<A> {
    /// The evidence automaton is missing a transition for one of its own alphabet letters.
    #[error(
        "missing evidence transition for state {state_index}, symbol index {symbol_index}, delay {delay}"
    )]
    MissingDelta {
        /// Evidence-state index whose transition is missing.
        state_index: usize,
        /// Index of the symbol in the evidence alphabet.
        symbol_index: usize,
        /// Delay representative for the missing transition lookup.
        delay: DelayRep,
    },
    /// Guard inference failed for a compressed delay list.
    #[error(
        "guard inference failed for state {state_index}, symbol index {symbol_index}: {source}"
    )]
    GuardInferenceFailed {
        /// Evidence-state index being converted.
        state_index: usize,
        /// Index of the symbol in the evidence alphabet.
        symbol_index: usize,
        /// Underlying partition/guard inference failure.
        #[source]
        source: PartitionError,
    },
    /// ARTA builder validation failed.
    #[error("failed to build hypothesis ARTA: {source}")]
    ArtaBuildFailed {
        /// Underlying ARTA construction failure.
        #[source]
        source: ArtaError<A>,
    },
}

/// Map evidence-state index `i` to hypothesis location `q{i}`.
///
/// This stable naming keeps JSON and DOT output diff-friendly.
pub fn evidence_state_to_location_id(state: AfaStateId) -> LocationId {
    LocationId::new(format!("q{}", state.0))
}

/// Convert a basis formula into a DAG state formula over hypothesis locations.
///
/// Each `BasisVar(i)` is rewritten to location `q{i}`.
pub fn convert_basis_formula_to_dag_state_formula(
    formula: &BasisFormula,
    mgr: &Arc<DagStateFormulaManager>,
) -> DagStateFormula {
    match formula {
        BasisFormula::Top => DagStateFormula::top(mgr),
        BasisFormula::Bot => DagStateFormula::bot(mgr),
        BasisFormula::And(vars) => DagStateFormula::and(
            mgr,
            vars.iter()
                .copied()
                .map(|var| DagStateFormula::var(mgr, basis_var_to_location_id(var))),
        ),
        BasisFormula::Or(terms) => DagStateFormula::or(
            mgr,
            terms
                .iter()
                .map(|term| convert_basis_formula_to_dag_state_formula(term, mgr)),
        ),
    }
}

impl<A> EvidenceAfa<A, BasisFormula>
where
    A: Eq + Hash + Clone,
{
    /// Construct the hypothesis ARTA corresponding to this evidence AFA.
    ///
    /// Consecutive equal-target evidence delays are compressed before guard
    /// inference, so the resulting ARTA preserves evidence behavior while using
    /// fewer transitions.
    pub fn to_hypothesis_arta(
        &self,
        mgr: &Arc<DagStateFormulaManager>,
    ) -> Result<Arta<A, DagStateFormula>, HypothesisArtaError<A>> {
        let init = convert_basis_formula_to_dag_state_formula(self.init(), mgr);
        let mut builder = ArtaBuilder::new(init);

        for state in self.states() {
            builder.add_location(evidence_state_to_location_id(state));
            if self.is_accepting(state) {
                builder.add_accepting(evidence_state_to_location_id(state));
            }
        }

        let (symbols, delays_by_symbol) = collect_symbol_delay_lists(self.alphabet());
        for state in self.states() {
            let source = evidence_state_to_location_id(state);
            for (symbol_index, symbol) in symbols.iter().enumerate() {
                let Some(delays) = delays_by_symbol.get(symbol) else {
                    continue;
                };
                if delays.is_empty() {
                    continue;
                }

                let compressed =
                    self.compress_change_points(state, symbol, symbol_index, delays)?;
                let compressed_delays: Vec<_> =
                    compressed.iter().map(|(delay, _)| *delay).collect();
                let partition_intervals = infer_guard_intervals_from_delays(&compressed_delays)
                    .map_err(|source| HypothesisArtaError::GuardInferenceFailed {
                        state_index: state.0,
                        symbol_index,
                        source,
                    })?;

                for (interval_index, guard) in partition_intervals.into_iter().enumerate() {
                    let target = convert_basis_formula_to_dag_state_formula(
                        &compressed[interval_index].1,
                        mgr,
                    );
                    builder.add_transition(source.clone(), symbol.clone(), guard, target);
                }
            }
        }

        builder
            .build()
            .map_err(|source| HypothesisArtaError::ArtaBuildFailed { source })
    }

    fn compress_change_points(
        &self,
        state: AfaStateId,
        symbol: &A,
        symbol_index: usize,
        delays: &[DelayRep],
    ) -> Result<Vec<(DelayRep, BasisFormula)>, HypothesisArtaError<A>> {
        let mut resolved = Vec::with_capacity(delays.len());

        for delay in delays.iter().copied() {
            let sigma = (symbol.clone(), delay);
            let target = self.transition(state, &sigma).cloned().ok_or(
                HypothesisArtaError::MissingDelta {
                    state_index: state.0,
                    symbol_index,
                    delay,
                },
            )?;

            resolved.push((delay, target));
        }

        Ok(compress_resolved_change_points(resolved))
    }
}

fn compress_resolved_change_points<T>(points: Vec<(DelayRep, T)>) -> Vec<(DelayRep, T)>
where
    T: Eq,
{
    let mut compressed = Vec::with_capacity(points.len());

    for (delay, target) in points {
        if let Some((kept_delay, kept_target)) = compressed.last_mut()
            && *kept_target == target
        {
            *kept_delay = delay;
            continue;
        }

        compressed.push((delay, target));
    }

    compressed
}

fn basis_var_to_location_id(var: BasisVar) -> LocationId {
    evidence_state_to_location_id(AfaStateId(var.0))
}

fn collect_symbol_delay_lists<A>(alphabet: &[(A, DelayRep)]) -> (Vec<A>, HashMap<A, Vec<DelayRep>>)
where
    A: Eq + Hash + Clone,
{
    let mut symbols = Vec::new();
    let mut seen_symbols = HashSet::new();
    let mut delays_by_symbol = HashMap::<A, Vec<DelayRep>>::new();

    for (symbol, delay) in alphabet {
        if seen_symbols.insert(symbol.clone()) {
            symbols.push(symbol.clone());
        }
        delays_by_symbol
            .entry(symbol.clone())
            .or_default()
            .push(*delay);
    }

    for delays in delays_by_symbol.values_mut() {
        delays.sort_unstable();
    }

    (symbols, delays_by_symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compress_change_points_keeps_last_delay_for_equal_target_run() {
        let q0 = BasisFormula::var(BasisVar(0));
        let q1 = BasisFormula::var(BasisVar(1));

        let compressed = compress_resolved_change_points(vec![
            (DelayRep::from_integer(1), q0.clone()),
            (DelayRep::from_floor_plus_half(2), q1.clone()),
            (DelayRep::from_integer(4), q1),
            (DelayRep::from_integer(5), q0.clone()),
        ]);

        assert_eq!(
            compressed,
            vec![
                (DelayRep::from_integer(1), q0),
                (DelayRep::from_integer(4), BasisFormula::var(BasisVar(1))),
                (DelayRep::from_integer(5), BasisFormula::var(BasisVar(0))),
            ]
        );
    }
}
