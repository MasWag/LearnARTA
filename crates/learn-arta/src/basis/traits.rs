// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{BasisWords, ObservationTable};
use learn_arta_core::TimedWord;
use std::hash::Hash;
use std::sync::Arc;
use thiserror::Error;

/// Scheduling hint for a [`BasisMinimizer`] during cohesion repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasisReductionPhase {
    /// Run basis reduction after basis-closedness but before additive repairs.
    ///
    /// This allows a minimizer to shrink `P` eagerly, even if evidence- or
    /// distinctness repairs are still pending.
    BeforeAdditiveRepairs,
    /// Run basis reduction only after additive repairs are exhausted.
    ///
    /// This preserves the historical greedy behavior more closely.
    AfterAdditiveRepairs,
}

/// Strategy object used to reduce the current basis during cohesion repair.
pub trait BasisMinimizer<A>: std::fmt::Debug + Send + Sync
where
    A: Eq + Hash + Clone,
{
    /// Return when this minimizer should run within one cohesion-repair step.
    fn phase(&self) -> BasisReductionPhase;

    /// Return a replacement basis when the current table admits a smaller one.
    ///
    /// Returning `Ok(None)` means "keep the current basis". Returned basis words
    /// are expected to preserve representability for the current table, and
    /// their order becomes the learner's new deterministic basis order.
    fn minimize_basis(
        &self,
        table: &ObservationTable<A>,
        basis_words: &BasisWords<A>,
    ) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError>;
}

impl<A, M> BasisMinimizer<A> for Arc<M>
where
    A: Eq + Hash + Clone,
    M: BasisMinimizer<A> + ?Sized,
{
    fn phase(&self) -> BasisReductionPhase {
        self.as_ref().phase()
    }

    fn minimize_basis(
        &self,
        table: &ObservationTable<A>,
        basis_words: &BasisWords<A>,
    ) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError> {
        self.as_ref().minimize_basis(table, basis_words)
    }
}

/// Errors produced while minimizing basis words.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum BasisMinimizationError {
    /// The epsilon sample row was unexpectedly missing from the table.
    #[error("missing required sample prefix while minimizing the basis")]
    MissingEpsilonSample,
    /// Two row-oriented structures had incompatible widths.
    #[error("length mismatch while minimizing basis: expected {expected}, found {found}")]
    LengthMismatch {
        /// Expected common row width.
        expected: usize,
        /// Observed row width.
        found: usize,
    },
    /// Basis minimization was requested for an empty basis.
    #[error("basis words are empty")]
    EmptyBasisWords,
    /// The decomposition helper reported an internal construction error.
    #[error("decomposition error while minimizing the basis: {reason}")]
    Decomposition {
        /// Human-readable reason returned by the decomposition helper.
        reason: String,
    },
    /// No basis-row candidate could satisfy a required positive/negative column split.
    #[error(
        "no candidate row covers obligation requiring e[{positive_column}] = true and e[{negative_column}] = false"
    )]
    UncoverableObligation {
        /// Column that must evaluate to `true`.
        positive_column: usize,
        /// Column that must evaluate to `false`.
        negative_column: usize,
    },
    /// The approximate MILP configuration was invalid before solver invocation.
    #[error("invalid approximate MILP config for {field}: {reason}")]
    InvalidApproxMilpConfig {
        /// Name of the invalid configuration field.
        field: &'static str,
        /// Human-readable validation failure.
        reason: String,
    },
    /// The caller selected a MILP strategy without compiling the `milp` feature.
    #[error(
        "basis minimizer {strategy} requires rebuilding learn-arta with the `milp` cargo feature"
    )]
    MilpBackendUnavailable {
        /// Name of the requested built-in basis minimizer.
        strategy: &'static str,
    },
    /// The external MILP solver failed or returned an unusable status.
    #[error("MILP solver failed: {reason}")]
    Solver {
        /// Human-readable solver failure.
        reason: String,
    },
}
