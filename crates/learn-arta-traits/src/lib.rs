// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared learning and oracle traits for LearnARTA.

use std::{error::Error, hash::Hash};

use learn_arta_core::{Arta, LocationId, NormalizeHalfInput, StateFormula, TimedWord};

/// Timed word type returned by an equivalence oracle before learner-side normalization.
pub type CounterexampleWord<A, D> = TimedWord<A, D>;

/// Result of an equivalence query that may yield a raw counterexample timed word.
pub type CounterexampleQueryResult<A, D, E> = Result<Option<CounterexampleWord<A, D>>, E>;

/// A membership oracle answers "is this timed word accepted?".
pub trait MembershipOracle {
    /// Alphabet symbol type used by the oracle.
    type Symbol: Eq + Hash + Clone;

    /// Oracle-specific error type.
    type Error: Error + Send + Sync + 'static;

    /// Query whether the timed word `w` is accepted by the target language.
    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error>;
}

/// An equivalence oracle answers "is this hypothesis correct?".
pub trait EquivalenceOracle {
    /// Alphabet symbol type used by the oracle.
    type Symbol: Eq + Hash + Clone;

    /// Delay type used in raw counterexamples before learner normalization.
    type CounterexampleDelay: NormalizeHalfInput + Clone;

    /// State-formula representation expected by the hypothesis ARTA.
    type Formula: StateFormula<Var = LocationId>;

    /// Oracle-specific error type.
    type Error: Error + Send + Sync + 'static;

    /// Check whether the hypothesis automaton is equivalent to the target language.
    ///
    /// Returns `Ok(None)` if the hypothesis is equivalent, or `Ok(Some(w))` where `w` is
    /// a counterexample timed word on which the hypothesis and the target disagree.
    /// The learner normalizes the returned delays to the half-unit lattice before
    /// refining its observation table.
    fn find_counterexample(
        &mut self,
        hyp: &Arta<Self::Symbol, Self::Formula>,
    ) -> CounterexampleQueryResult<Self::Symbol, Self::CounterexampleDelay, Self::Error>;
}
