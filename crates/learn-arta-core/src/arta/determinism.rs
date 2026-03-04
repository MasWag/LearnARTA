// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{ArtaError, GuardedTransition};
use crate::{location::LocationId, state_formula::StateFormula};
use std::hash::Hash;

pub(super) fn ensure_bucket_deterministic<A, C>(
    loc: &LocationId,
    symbol: &A,
    edges: &mut [GuardedTransition<C>],
) -> Result<(), ArtaError<A>>
where
    A: Eq + Hash + Clone,
    C: StateFormula<Var = LocationId>,
{
    edges.sort_by_key(|edge| edge.guard.sort_key());
    for pair in edges.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        if let Some(overlap) = left.guard.intersection(&right.guard)
            && let Some(witness) = overlap.pick_witness()
        {
            return Err(ArtaError::NonDeterministic {
                loc: loc.clone(),
                symbol: symbol.clone(),
                guard1: left.guard.clone(),
                guard2: right.guard.clone(),
                witness,
            });
        }
    }

    Ok(())
}
