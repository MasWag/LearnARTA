// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Active ARTA learner implementing the end-to-end learning loop.

use std::convert::Infallible;
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

use learn_arta_core::{
    Arta, DagStateFormula, DagStateFormulaManager, NormalizeHalfInput, TimedWord,
    try_normalize_word_half,
};
use learn_arta_traits::{EquivalenceOracle, MembershipOracle};

use crate::{
    basis::{BasisMinimization, BasisMinimizer},
    cohesion::{BasisWords, make_cohesive_step},
    error::LearnError,
    evidence_afa::build_from_cohesive_table,
    observation_table::ObservationTable,
};

/// Mutable learning state tracked across active-learning iterations.
#[derive(Debug, Clone)]
pub struct ActiveArtaLearnerState<A>
where
    A: Eq + Hash + Clone,
{
    /// Observation table over sample prefixes and experiment suffixes.
    pub observation_table: ObservationTable<A>,
    /// Ordered basis words used for cohesion checks and decomposition.
    pub basis_words: BasisWords<A>,
    /// DAG formula manager reused across all hypothesis constructions.
    pub dag_state_formula_manager: Arc<DagStateFormulaManager>,
    /// Number of completed hypothesis/EQ iterations.
    pub hypothesis_iterations: usize,
    /// Number of processed counterexample refinements.
    pub refinement_rounds: usize,
}

impl<A> ActiveArtaLearnerState<A>
where
    A: Eq + Hash + Clone,
{
    fn new() -> Self {
        Self {
            observation_table: ObservationTable::new(),
            basis_words: BasisWords::new_with_epsilon(),
            dag_state_formula_manager: DagStateFormulaManager::new(),
            hypothesis_iterations: 0,
            refinement_rounds: 0,
        }
    }
}

/// Active learner orchestrating cohesion repair, hypothesis construction, and EQ refinement.
#[derive(Debug, Clone)]
pub struct ActiveArtaLearner<A>
where
    A: Eq + Hash + Clone,
{
    state: ActiveArtaLearnerState<A>,
    basis_minimizer: Arc<dyn BasisMinimizer<A>>,
}

impl<A> ActiveArtaLearner<A>
where
    A: Eq + Hash + Clone + std::fmt::Display,
{
    /// Create a learner with empty state and the default built-in basis minimizer.
    ///
    /// The default minimizer depends on enabled cargo features: with `milp` it
    /// is approximate MILP, otherwise it falls back to the greedy minimizer.
    pub fn new() -> Self {
        Self::with_minimizer(BasisMinimization::default())
    }

    /// Create a learner using the provided basis minimizer.
    pub fn with_minimizer<M>(basis_minimizer: M) -> Self
    where
        M: BasisMinimizer<A> + 'static,
    {
        Self {
            state: ActiveArtaLearnerState::new(),
            basis_minimizer: Arc::new(basis_minimizer),
        }
    }

    /// Read-only access to current learner state.
    pub fn state(&self) -> &ActiveArtaLearnerState<A> {
        &self.state
    }

    /// Mutable access to learner state.
    pub fn state_mut(&mut self) -> &mut ActiveArtaLearnerState<A> {
        &mut self.state
    }

    /// Build the next hypothesis from the current observation table state.
    ///
    /// This bootstraps the initial table row lazily, repairs cohesion to a
    /// fixpoint, constructs the evidence automaton, and converts it into the
    /// next hypothesis ARTA. Callers that need custom stopping policies can
    /// alternate this method with [`Self::refine_with_counterexample`].
    pub fn build_hypothesis<MQ>(
        &mut self,
        mq: &mut MQ,
    ) -> Result<Arta<A>, LearnError<A, MQ::Error, Infallible>>
    where
        MQ: MembershipOracle<Symbol = A>,
    {
        // Bootstrap T(ε, ε) via closure-preserving insertion.
        self.state
            .observation_table
            .insert_sample_prefixes(TimedWord::empty(), mq)
            .map_err(|source| LearnError::MembershipQuery { source })?;
        self.validate_table()?;

        while make_cohesive_step(
            &mut self.state.observation_table,
            &mut self.state.basis_words,
            self.basis_minimizer.as_ref(),
            mq,
        )
        .map_err(|source| LearnError::CohesionRepair { source })?
        {
            self.validate_table()?;
        }
        self.validate_table()?;

        let evidence =
            build_from_cohesive_table(&self.state.observation_table, &self.state.basis_words)
                .map_err(|source| LearnError::EvidenceAutomatonBuild { source })?;

        let hypothesis = evidence
            .to_hypothesis_arta(&self.state.dag_state_formula_manager)
            .map_err(|source| LearnError::HypothesisBuild { source })?;

        self.state.hypothesis_iterations = self.state.hypothesis_iterations.saturating_add(1);

        Ok(hypothesis)
    }

    /// Refine the current observation table with an equivalence-oracle counterexample.
    ///
    /// The learner normalizes the counterexample to the half-unit lattice before
    /// inserting all suffixes into the experiment set. Callers can use this to
    /// drive learning loops with policies external to the core learner.
    pub fn refine_with_counterexample<MQ, D>(
        &mut self,
        mq: &mut MQ,
        counterexample: &TimedWord<A, D>,
    ) -> Result<(), LearnError<A, MQ::Error, Infallible>>
    where
        MQ: MembershipOracle<Symbol = A>,
        D: NormalizeHalfInput + Clone,
    {
        let normalized_counterexample = try_normalize_word_half(counterexample)
            .map_err(|source| LearnError::CounterexampleNormalization { source })?;
        self.state.refinement_rounds = self.state.refinement_rounds.saturating_add(1);

        self.state
            .observation_table
            .insert_experiment_suffixes(normalized_counterexample, mq)
            .map_err(|source| LearnError::MembershipQuery { source })?;
        self.validate_table()?;

        Ok(())
    }

    /// Run active learning until equivalence succeeds.
    ///
    /// This is the exact, unbounded learner loop. Callers that want bounded or
    /// best-effort behavior must drive iterations explicitly with
    /// [`Self::build_hypothesis`] and [`Self::refine_with_counterexample`].
    pub fn learn<MQ, EQ>(
        &mut self,
        mq: &mut MQ,
        eq: &mut EQ,
    ) -> Result<Arta<A>, LearnError<A, MQ::Error, EQ::Error>>
    where
        MQ: MembershipOracle<Symbol = A>,
        EQ: EquivalenceOracle<Symbol = A, Formula = DagStateFormula>,
    {
        loop {
            let hypothesis = self
                .build_hypothesis(mq)
                .map_err(Self::promote_infallible_eq_error)?;

            let maybe_counterexample = eq
                .find_counterexample(&hypothesis)
                .map_err(|source| LearnError::EquivalenceQuery { source })?;

            let Some(counterexample) = maybe_counterexample else {
                return Ok(hypothesis);
            };

            self.refine_with_counterexample(mq, &counterexample)
                .map_err(Self::promote_infallible_eq_error)?;
        }
    }

    #[cfg(debug_assertions)]
    fn validate_table<MembershipOracleError, EquivalenceOracleError>(
        &self,
    ) -> Result<(), LearnError<A, MembershipOracleError, EquivalenceOracleError>>
    where
        MembershipOracleError: Error + Send + Sync + 'static,
        EquivalenceOracleError: Error + Send + Sync + 'static,
    {
        self.state
            .observation_table
            .validate_invariants()
            .map_err(|source| LearnError::TableInvariant { source })
    }

    #[cfg(not(debug_assertions))]
    fn validate_table<MembershipOracleError, EquivalenceOracleError>(
        &self,
    ) -> Result<(), LearnError<A, MembershipOracleError, EquivalenceOracleError>>
    where
        MembershipOracleError: Error + Send + Sync + 'static,
        EquivalenceOracleError: Error + Send + Sync + 'static,
    {
        // These checks are internal consistency assertions for observation-table
        // maintenance. Keep them in debug/test builds, but skip them in release
        // builds where they dominate benchmark time.
        Ok(())
    }

    fn promote_infallible_eq_error<MembershipOracleError, EquivalenceOracleError>(
        error: LearnError<A, MembershipOracleError, Infallible>,
    ) -> LearnError<A, MembershipOracleError, EquivalenceOracleError>
    where
        MembershipOracleError: Error + Send + Sync + 'static,
        EquivalenceOracleError: Error + Send + Sync + 'static,
    {
        match error {
            LearnError::MembershipQuery { source } => LearnError::MembershipQuery { source },
            LearnError::EquivalenceQuery { source } => match source {},
            LearnError::CounterexampleNormalization { source } => {
                LearnError::CounterexampleNormalization { source }
            }
            LearnError::CohesionRepair { source } => LearnError::CohesionRepair { source },
            LearnError::EvidenceAutomatonBuild { source } => {
                LearnError::EvidenceAutomatonBuild { source }
            }
            LearnError::HypothesisBuild { source } => LearnError::HypothesisBuild { source },
            LearnError::TableInvariant { source } => LearnError::TableInvariant { source },
        }
    }
}

impl<A> Default for ActiveArtaLearner<A>
where
    A: Eq + Hash + Clone + std::fmt::Display,
{
    fn default() -> Self {
        Self::new()
    }
}
