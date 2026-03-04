// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Exact membership oracle backed by a concrete ARTA target.

use std::{convert::Infallible, hash::Hash};

use learn_arta_core::{Arta, DagStateFormula, LocationId, StateFormula, TimedWord};
use learn_arta_traits::MembershipOracle;

/// Exact membership oracle that delegates queries to a concrete target [`Arta`].
///
/// This adapter owns the target automaton and answers each membership query by
/// evaluating the ARTA semantics directly via [`Arta::accepts`].
#[derive(Debug, Clone)]
pub struct ArtaMembershipOracle<A, C = DagStateFormula>
where
    A: Eq + Hash + Clone,
    C: StateFormula<Var = LocationId>,
{
    target: Arta<A, C>,
}

impl<A, C> ArtaMembershipOracle<A, C>
where
    A: Eq + Hash + Clone,
    C: StateFormula<Var = LocationId>,
{
    /// Create a new infallible exact membership oracle over `target`.
    pub fn new(target: Arta<A, C>) -> Self {
        Self { target }
    }
}

impl<A, C> MembershipOracle for ArtaMembershipOracle<A, C>
where
    A: Eq + Hash + Clone,
    C: StateFormula<Var = LocationId>,
{
    type Symbol = A;
    type Error = Infallible;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        Ok(self.target.accepts(w))
    }
}
