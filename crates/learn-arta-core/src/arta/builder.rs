// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Arta, ArtaError, GuardedTransition, determinism::ensure_bucket_deterministic};
use crate::{
    error::IntervalError,
    location::LocationId,
    state_formula::{DagStateFormula, StateFormula},
    time::interval::Interval,
};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

#[derive(Debug, Clone)]
struct PendingTransition<A, C> {
    loc: LocationId,
    symbol: A,
    guard: Interval,
    target: C,
}

/// Builder for [`Arta`].
///
/// Validation is deferred to [`ArtaBuilder::build`]. Until then the builder may
/// contain references to undeclared locations, invalid intervals, or
/// overlapping guards. Exact duplicate transitions are deduplicated during
/// `build`.
#[derive(Debug, Clone)]
pub struct ArtaBuilder<A, C = DagStateFormula> {
    locations: HashSet<LocationId>,
    init: C,
    accepting: HashSet<LocationId>,
    transitions: Vec<PendingTransition<A, C>>,
}

impl<A, C> ArtaBuilder<A, C>
where
    A: Eq + Hash + Clone,
    C: StateFormula<Var = LocationId>,
{
    /// Create a new builder with the given initial state formula.
    ///
    /// The builder starts with no declared locations, no accepting locations,
    /// and no transitions.
    pub fn new(init: C) -> Self {
        Self {
            locations: HashSet::new(),
            init,
            accepting: HashSet::new(),
            transitions: Vec::new(),
        }
    }

    /// Add one location to the automaton universe.
    ///
    /// Inserting the same location more than once is a no-op.
    pub fn add_location(&mut self, loc: LocationId) -> &mut Self {
        self.locations.insert(loc);
        self
    }

    /// Add multiple locations to the automaton universe.
    ///
    /// Duplicate entries are ignored.
    pub fn add_locations<I>(&mut self, locations: I) -> &mut Self
    where
        I: IntoIterator<Item = LocationId>,
    {
        self.locations.extend(locations);
        self
    }

    /// Mark one location as accepting.
    ///
    /// The location does not need to have been added yet; that is checked when
    /// [`Self::build`] is called.
    pub fn add_accepting(&mut self, loc: LocationId) -> &mut Self {
        self.accepting.insert(loc);
        self
    }

    /// Add one guarded transition.
    ///
    /// Transition validation is deferred to [`Self::build`]. Exact duplicate
    /// transitions are ignored there, while equal guards with different targets
    /// are rejected as non-deterministic.
    pub fn add_transition(
        &mut self,
        loc: LocationId,
        symbol: A,
        guard: Interval,
        target: C,
    ) -> &mut Self {
        self.transitions.push(PendingTransition {
            loc,
            symbol,
            guard,
            target,
        });
        self
    }

    /// Build and validate an [`Arta`].
    ///
    /// This checks:
    /// - all locations referenced by `init`, `accepting`, and transition
    ///   targets are declared
    /// - every interval is valid
    /// - every `(location, symbol)` bucket is deterministic
    ///
    /// On success, transition buckets are stored in deterministic guard order.
    pub fn build(self) -> Result<Arta<A, C>, ArtaError<A>> {
        for var in self.init.vars() {
            if !self.locations.contains(&var) {
                return Err(ArtaError::UnknownLocation { loc: var });
            }
        }

        for loc in &self.accepting {
            if !self.locations.contains(loc) {
                return Err(ArtaError::UnknownLocation { loc: loc.clone() });
            }
        }

        let mut grouped: HashMap<(LocationId, A), Vec<GuardedTransition<C>>> = HashMap::new();
        for (idx, transition) in self.transitions.into_iter().enumerate() {
            transition
                .guard
                .validate()
                .map_err(|source| ArtaError::InvalidInterval {
                    context: format!("transition #{idx} from {}", transition.loc),
                    source,
                })?;

            if !self.locations.contains(&transition.loc) {
                return Err(ArtaError::UnknownLocation {
                    loc: transition.loc,
                });
            }

            for var in transition.target.vars() {
                if !self.locations.contains(&var) {
                    return Err(ArtaError::UnknownLocation { loc: var });
                }
            }

            let entry = grouped
                .entry((transition.loc.clone(), transition.symbol.clone()))
                .or_default();

            if entry.iter().any(|existing| {
                existing.guard == transition.guard && existing.target == transition.target
            }) {
                continue;
            }

            if let Some(existing) = entry.iter().find(|existing| {
                existing.guard == transition.guard && existing.target != transition.target
            }) {
                let witness =
                    transition
                        .guard
                        .pick_witness()
                        .ok_or_else(|| ArtaError::InvalidInterval {
                            context: format!(
                                "transition #{idx} from {} has no representable delay",
                                transition.loc
                            ),
                            source: IntervalError::Empty,
                        })?;
                return Err(ArtaError::NonDeterministic {
                    loc: transition.loc,
                    symbol: transition.symbol,
                    guard1: existing.guard.clone(),
                    guard2: transition.guard,
                    witness,
                });
            }

            entry.push(GuardedTransition {
                guard: transition.guard,
                target: transition.target,
            });
        }

        for ((loc, symbol), edges) in &mut grouped {
            ensure_bucket_deterministic(loc, symbol, edges)?;
        }

        Ok(Arta {
            locations: self.locations,
            accepting: self.accepting,
            init: self.init,
            transitions: grouped,
        })
    }
}
