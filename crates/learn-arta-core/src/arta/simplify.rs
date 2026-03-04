// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Arta, GuardedTransition};
use crate::{
    location::LocationId,
    state_formula::{MinimalModelKey, StateFormula},
};
use std::{collections::HashMap, hash::Hash};

impl<A, C> Arta<A, C>
where
    A: Eq + Hash + Clone,
    C: StateFormula<Var = LocationId>,
{
    /// Simplify this ARTA in place without changing acceptance semantics.
    ///
    /// This opt-in pass:
    /// - rewrites the initial formula and transition targets to minimal
    ///   positive Boolean formulas using semantic keys;
    /// - removes transitions whose targets simplify to `⊥`; and
    /// - merges touching guard intervals whose simplified targets are equal.
    ///
    /// Missing transitions already denote `⊥` in [`Self::step_location`], so
    /// removing simplified-`⊥` transitions does not change the language.
    ///
    /// LearnARTA does not apply this automatically during construction,
    /// evaluation, or JSON serialization; callers must invoke it explicitly.
    pub fn simplify(&mut self) {
        self.init = simplify_formula(&self.init);

        let transitions = std::mem::take(&mut self.transitions);
        let mut simplified = HashMap::with_capacity(transitions.len());

        for (key, edges) in transitions {
            let edges = simplify_transition_bucket(edges);
            if !edges.is_empty() {
                simplified.insert(key, edges);
            }
        }

        self.transitions = simplified;
    }
}

fn simplify_formula<C>(formula: &C) -> C
where
    C: StateFormula<Var = LocationId>,
{
    let key = formula.semantic_key();
    formula_from_semantic_key(formula.manager(), &key)
}

fn formula_from_semantic_key<C>(mgr: &C::Manager, key: &MinimalModelKey<LocationId>) -> C
where
    C: StateFormula<Var = LocationId>,
{
    match key.clauses() {
        [] => C::bot(mgr),
        [clause] => formula_from_clause(mgr, clause),
        clauses => C::or(
            mgr,
            clauses
                .iter()
                .map(|clause| formula_from_clause(mgr, clause)),
        ),
    }
}

fn formula_from_clause<C>(mgr: &C::Manager, clause: &[LocationId]) -> C
where
    C: StateFormula<Var = LocationId>,
{
    match clause {
        [] => C::top(mgr),
        [location] => C::var(mgr, location.clone()),
        _ => C::and(
            mgr,
            clause.iter().cloned().map(|location| C::var(mgr, location)),
        ),
    }
}

fn simplify_transition_bucket<C>(mut edges: Vec<GuardedTransition<C>>) -> Vec<GuardedTransition<C>>
where
    C: StateFormula<Var = LocationId>,
{
    edges.sort_by_key(|edge| edge.guard.sort_key());

    let bot_key = MinimalModelKey::bot();
    let mut simplified_edges: Vec<GuardedTransition<C>> = Vec::with_capacity(edges.len());
    let mut simplified_keys: Vec<MinimalModelKey<LocationId>> = Vec::with_capacity(edges.len());

    for edge in edges {
        let target_key = edge.target.semantic_key();
        if target_key == bot_key {
            continue;
        }

        let target = formula_from_semantic_key(edge.target.manager(), &target_key);
        if let (Some(previous), Some(previous_key)) =
            (simplified_edges.last_mut(), simplified_keys.last())
            && previous_key == &target_key
            && let Some(merged) = previous.guard.try_merge_adjacent(&edge.guard)
        {
            previous.guard = merged;
            continue;
        }

        simplified_keys.push(target_key);
        simplified_edges.push(GuardedTransition {
            guard: edge.guard,
            target,
        });
    }

    simplified_edges
}
