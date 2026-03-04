// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::Arta;
use crate::{
    error::TimeError, location::LocationId, state_formula::StateFormula, time::DelayRep,
    timed_word::TimedWord,
};
use std::hash::Hash;

impl<A, C> Arta<A, C>
where
    A: Eq + Hash + Clone,
    C: StateFormula<Var = LocationId>,
{
    /// Compute one execution step from a location on timed letter `(sym, d)`.
    ///
    /// Returns the unique enabled target for `(loc, sym, d)`. If there is no
    /// enabled transition, returns `⊥` according to the library's
    /// partial-transition semantics.
    pub fn step_location(&self, loc: LocationId, sym: &A, d: DelayRep) -> C {
        if let Some(edges) = self.transitions.get(&(loc, sym.clone())) {
            for edge in edges {
                if edge.guard.contains(d) {
                    return edge.target.clone();
                }
            }
        }
        C::bot(self.init.manager())
    }

    /// Compute one execution step from a formula on timed letter `(sym, d)`.
    ///
    /// This applies [`Self::step_location`] homomorphically via substitution.
    pub fn step_formula(&self, phi: &C, sym: &A, d: DelayRep) -> C {
        C::substitute(phi.manager(), phi, |loc| self.step_location(loc, sym, d))
    }

    /// Run the automaton semantics from `phi` over timed word `w`.
    ///
    /// This iterates [`Self::step_formula`] left-to-right over the timed letters.
    pub fn run_from(&self, phi: &C, w: &TimedWord<A>) -> C {
        let mut current = phi.clone();
        for (sym, delay) in w.iter() {
            current = self.step_formula(&current, sym, *delay);
        }
        current
    }

    /// Evaluate a state formula under the accepting-location valuation.
    pub fn eval(&self, phi: &C) -> bool {
        C::eval_bool(phi, |loc| self.accepting.contains(&loc))
    }

    /// Decide whether timed word `w` is accepted by this ARTA.
    pub fn accepts(&self, w: &TimedWord<A>) -> bool {
        self.eval(&self.run_from(&self.init, w))
    }

    /// Convenience wrapper over [`Self::accepts`] using raw `f64` delays.
    ///
    /// Finite non-integer delays are normalized to `floor(d) + 0.5` before the
    /// acceptance check runs.
    pub fn accepts_f64(&self, w: &[(A, f64)]) -> Result<bool, TimeError> {
        let mut letters = Vec::with_capacity(w.len());
        for (symbol, delay) in w {
            letters.push((symbol.clone(), DelayRep::try_from_f64(*delay)?));
        }
        Ok(self.accepts(&TimedWord::from_vec(letters)))
    }
}
