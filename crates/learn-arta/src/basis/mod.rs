// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Basis minimization strategies for cohesive observation tables.

#![allow(dead_code)]

mod approx_milp;
mod config;
mod cover;
mod exact_milp;
mod greedy;
mod traits;

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::time::Instant;

use fixedbitset::FixedBitSet;
use learn_arta_core::TimedWord;
use log::{debug, trace};

use crate::{
    BasisDecomposer, ObservationTable, RowVec, cohesion::BasisWords,
    decomposition::DecompositionError,
};

pub use config::{ApproxMilpConfig, BasisMinimization};
pub use traits::{BasisMinimizationError, BasisMinimizer, BasisReductionPhase};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Obligation {
    positive_column: usize,
    negative_column: usize,
}

#[derive(Debug, Clone)]
struct DistinctRowClass<A> {
    representative: TimedWord<A>,
    row: RowVec,
}

#[derive(Debug)]
struct CollectedDistinctRows<A> {
    classes: Vec<DistinctRowClass<A>>,
    row_to_index: HashMap<RowVec, usize>,
}

#[derive(Debug, Clone)]
struct CoverInstance<A> {
    candidates: Vec<DistinctRowClass<A>>,
    obligations: Vec<Obligation>,
    coverage_by_candidate: Vec<FixedBitSet>,
    coverers_by_obligation: Vec<FixedBitSet>,
    forced_candidates: FixedBitSet,
    projected_basis_candidates: FixedBitSet,
}

#[derive(Debug, Clone)]
struct MinimizationOutcome<A> {
    instance: CoverInstance<A>,
    selected_candidates: Option<FixedBitSet>,
    solve_status: CoverSolveStatus,
    incumbent_size: usize,
    candidate_count_before_presolve: usize,
    obligation_count_before_presolve: usize,
    candidate_count_after_presolve: usize,
    obligation_count_after_presolve: usize,
    component_sizes: Vec<(usize, usize)>,
}

#[derive(Debug, Clone)]
struct CoverComponent {
    candidate_indices: Vec<usize>,
    obligation_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
enum ComponentNode {
    Candidate(usize),
    Obligation(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoverSolveStatus {
    Optimal,
    GapLimited,
    TimeLimited,
    NoImprovement,
}

impl CoverSolveStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Optimal => "optimal",
            Self::GapLimited => "gap-limited",
            Self::TimeLimited => "time-limited",
            Self::NoImprovement => "no-improvement",
        }
    }
}

#[derive(Debug, Clone)]
struct CoverSolveOutcome {
    selected_candidates: Option<FixedBitSet>,
    status: CoverSolveStatus,
}

trait CoverSolver {
    fn solve_bounded_cover<A>(
        &self,
        instance: &CoverInstance<A>,
        incumbent_size: usize,
    ) -> Result<CoverSolveOutcome, BasisMinimizationError>
    where
        A: Eq + Hash + Clone;
}

#[cfg(feature = "milp")]
#[derive(Debug, Clone, Copy, Default)]
struct HighsExactCoverSolver;

#[cfg(feature = "milp")]
#[derive(Debug, Clone, Copy)]
struct HighsApproxCoverSolver {
    config: ApproxMilpConfig,
}

#[derive(Debug, Clone, Copy, Default)]
struct GreedyBasisMinimizer;

#[derive(Debug, Clone, Copy, Default)]
struct ExactMilpBasisMinimizer;

#[derive(Debug, Clone, Copy)]
struct ApproxMilpBasisMinimizer {
    config: ApproxMilpConfig,
}

impl<A> BasisMinimizer<A> for GreedyBasisMinimizer
where
    A: Eq + Hash + Clone,
{
    fn phase(&self) -> BasisReductionPhase {
        BasisReductionPhase::BeforeAdditiveRepairs
    }

    fn minimize_basis(
        &self,
        table: &ObservationTable<A>,
        basis_words: &BasisWords<A>,
    ) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError> {
        maybe_minimize_greedy_basis_words(table, basis_words)
    }
}

impl<A> BasisMinimizer<A> for ExactMilpBasisMinimizer
where
    A: Eq + Hash + Clone,
{
    fn phase(&self) -> BasisReductionPhase {
        BasisReductionPhase::AfterAdditiveRepairs
    }

    fn minimize_basis(
        &self,
        table: &ObservationTable<A>,
        basis_words: &BasisWords<A>,
    ) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError> {
        maybe_minimize_exact_basis_words(table, basis_words)
    }
}

impl<A> BasisMinimizer<A> for ApproxMilpBasisMinimizer
where
    A: Eq + Hash + Clone,
{
    fn phase(&self) -> BasisReductionPhase {
        BasisReductionPhase::AfterAdditiveRepairs
    }

    fn minimize_basis(
        &self,
        table: &ObservationTable<A>,
        basis_words: &BasisWords<A>,
    ) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError> {
        maybe_minimize_approx_basis_words(table, basis_words, self.config)
    }
}

impl<A> BasisMinimizer<A> for BasisMinimization
where
    A: Eq + Hash + Clone,
{
    fn phase(&self) -> BasisReductionPhase {
        match self {
            Self::Greedy => {
                <GreedyBasisMinimizer as BasisMinimizer<A>>::phase(&GreedyBasisMinimizer)
            }
            Self::ExactMilp => {
                <ExactMilpBasisMinimizer as BasisMinimizer<A>>::phase(&ExactMilpBasisMinimizer)
            }
            Self::ApproxMilp(config) => {
                <ApproxMilpBasisMinimizer as BasisMinimizer<A>>::phase(&ApproxMilpBasisMinimizer {
                    config: *config,
                })
            }
        }
    }

    fn minimize_basis(
        &self,
        table: &ObservationTable<A>,
        basis_words: &BasisWords<A>,
    ) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError> {
        match self {
            Self::Greedy => GreedyBasisMinimizer.minimize_basis(table, basis_words),
            Self::ExactMilp => ExactMilpBasisMinimizer.minimize_basis(table, basis_words),
            Self::ApproxMilp(config) => {
                ApproxMilpBasisMinimizer { config: *config }.minimize_basis(table, basis_words)
            }
        }
    }
}

#[cfg(feature = "milp")]
impl CoverSolver for HighsExactCoverSolver {
    fn solve_bounded_cover<A>(
        &self,
        instance: &CoverInstance<A>,
        incumbent_size: usize,
    ) -> Result<CoverSolveOutcome, BasisMinimizationError>
    where
        A: Eq + Hash + Clone,
    {
        use good_lp::solvers::SolutionStatus;
        use good_lp::{
            Expression, ResolutionError, Solution, SolverModel, highs, variable, variables,
        };

        let mut selected = instance.forced_candidates.clone();
        if instance.obligations.is_empty() {
            if selected.is_clear() {
                if let Some(candidate_idx) = instance.projected_basis_candidates.ones().next() {
                    selected.insert(candidate_idx);
                } else if !instance.candidates.is_empty() {
                    selected.insert(0);
                }
            }
            return if selected.count_ones(..) < incumbent_size {
                Ok(CoverSolveOutcome {
                    selected_candidates: Some(selected),
                    status: CoverSolveStatus::Optimal,
                })
            } else {
                Ok(CoverSolveOutcome {
                    selected_candidates: None,
                    status: CoverSolveStatus::NoImprovement,
                })
            };
        }

        let forced_count = instance.forced_candidates.count_ones(..);
        if incumbent_size <= forced_count {
            return Ok(CoverSolveOutcome {
                selected_candidates: None,
                status: CoverSolveStatus::NoImprovement,
            });
        }

        let free_candidate_indices = (0..instance.candidates.len())
            .filter(|candidate_idx| !instance.forced_candidates.contains(*candidate_idx))
            .collect::<Vec<_>>();
        if free_candidate_indices.is_empty() {
            let obligation = &instance.obligations[0];
            return Err(BasisMinimizationError::UncoverableObligation {
                positive_column: obligation.positive_column,
                negative_column: obligation.negative_column,
            });
        }

        let max_free_selected = incumbent_size - forced_count - 1;
        let mut vars = variables!();
        let decision_vars = vars.add_vector(variable().binary(), free_candidate_indices.len());
        let objective = decision_vars
            .iter()
            .copied()
            .fold(Expression::from(0.0), |expr, variable| expr + variable);
        let mut model = vars.minimise(objective.clone()).using(highs);
        model = model.with(objective.leq(max_free_selected as f64));

        for (obligation_idx, coverers) in instance.coverers_by_obligation.iter().enumerate() {
            let mut has_coverer = false;
            let constraint_expression = free_candidate_indices
                .iter()
                .enumerate()
                .filter_map(|(free_idx, candidate_idx)| {
                    if coverers.contains(*candidate_idx) {
                        has_coverer = true;
                        Some(decision_vars[free_idx])
                    } else {
                        None
                    }
                })
                .fold(Expression::from(0.0), |expr, variable| expr + variable);

            if !has_coverer {
                let obligation = &instance.obligations[obligation_idx];
                return Err(BasisMinimizationError::UncoverableObligation {
                    positive_column: obligation.positive_column,
                    negative_column: obligation.negative_column,
                });
            }

            model = model.with(constraint_expression.geq(1.0));
        }

        let solution = match model.solve() {
            Ok(solution) => solution,
            Err(ResolutionError::Infeasible) => {
                return Ok(CoverSolveOutcome {
                    selected_candidates: None,
                    status: CoverSolveStatus::NoImprovement,
                });
            }
            Err(error) => {
                return Err(BasisMinimizationError::Solver {
                    reason: error.to_string(),
                });
            }
        };

        match solution.status() {
            SolutionStatus::Optimal => {}
            status => {
                return Err(BasisMinimizationError::Solver {
                    reason: format!(
                        "exact MILP solver returned non-optimal status {}",
                        match status {
                            SolutionStatus::Optimal => "optimal",
                            SolutionStatus::GapLimit => "gap-limited",
                            SolutionStatus::TimeLimit => "time-limited",
                        }
                    ),
                });
            }
        }

        for (free_idx, candidate_idx) in free_candidate_indices.into_iter().enumerate() {
            if solution.value(decision_vars[free_idx]) > 0.5 {
                selected.insert(candidate_idx);
            }
        }

        Ok(CoverSolveOutcome {
            selected_candidates: Some(selected),
            status: CoverSolveStatus::Optimal,
        })
    }
}

#[cfg(feature = "milp")]
impl CoverSolver for HighsApproxCoverSolver {
    fn solve_bounded_cover<A>(
        &self,
        instance: &CoverInstance<A>,
        incumbent_size: usize,
    ) -> Result<CoverSolveOutcome, BasisMinimizationError>
    where
        A: Eq + Hash + Clone,
    {
        use good_lp::solvers::{SolutionStatus, WithMipGap, WithTimeLimit};
        use good_lp::{
            Expression, ResolutionError, Solution, SolverModel, highs, variable, variables,
        };

        self.config.validate()?;

        let mut selected = instance.forced_candidates.clone();
        if instance.obligations.is_empty() {
            if selected.is_clear() {
                if let Some(candidate_idx) = instance.projected_basis_candidates.ones().next() {
                    selected.insert(candidate_idx);
                } else if !instance.candidates.is_empty() {
                    selected.insert(0);
                }
            }
            return if selected.count_ones(..) < incumbent_size {
                Ok(CoverSolveOutcome {
                    selected_candidates: Some(selected),
                    status: CoverSolveStatus::Optimal,
                })
            } else {
                Ok(CoverSolveOutcome {
                    selected_candidates: None,
                    status: CoverSolveStatus::NoImprovement,
                })
            };
        }

        let forced_count = instance.forced_candidates.count_ones(..);
        if incumbent_size <= forced_count {
            return Ok(CoverSolveOutcome {
                selected_candidates: None,
                status: CoverSolveStatus::NoImprovement,
            });
        }

        let free_candidate_indices = (0..instance.candidates.len())
            .filter(|candidate_idx| !instance.forced_candidates.contains(*candidate_idx))
            .collect::<Vec<_>>();
        if free_candidate_indices.is_empty() {
            let obligation = &instance.obligations[0];
            return Err(BasisMinimizationError::UncoverableObligation {
                positive_column: obligation.positive_column,
                negative_column: obligation.negative_column,
            });
        }

        let max_free_selected = incumbent_size - forced_count - 1;
        let mut vars = variables!();
        let decision_vars = vars.add_vector(variable().binary(), free_candidate_indices.len());
        let objective = decision_vars
            .iter()
            .copied()
            .fold(Expression::from(0.0), |expr, variable| expr + variable);
        let mut model = vars.minimise(objective.clone()).using(highs);
        model = model.with(objective.leq(max_free_selected as f64));
        model = model
            .with_mip_gap(self.config.relative_gap)
            .map_err(|error| BasisMinimizationError::InvalidApproxMilpConfig {
                field: "relative_gap",
                reason: error.to_string(),
            })?;
        model = model.with_time_limit(self.config.time_limit.as_secs_f64());

        for (obligation_idx, coverers) in instance.coverers_by_obligation.iter().enumerate() {
            let mut has_coverer = false;
            let constraint_expression = free_candidate_indices
                .iter()
                .enumerate()
                .filter_map(|(free_idx, candidate_idx)| {
                    if coverers.contains(*candidate_idx) {
                        has_coverer = true;
                        Some(decision_vars[free_idx])
                    } else {
                        None
                    }
                })
                .fold(Expression::from(0.0), |expr, variable| expr + variable);

            if !has_coverer {
                let obligation = &instance.obligations[obligation_idx];
                return Err(BasisMinimizationError::UncoverableObligation {
                    positive_column: obligation.positive_column,
                    negative_column: obligation.negative_column,
                });
            }

            model = model.with(constraint_expression.geq(1.0));
        }

        let solution = match model.solve() {
            Ok(solution) => solution,
            Err(ResolutionError::Infeasible) | Err(ResolutionError::Other("NoSolutionFound")) => {
                return Ok(CoverSolveOutcome {
                    selected_candidates: None,
                    status: CoverSolveStatus::NoImprovement,
                });
            }
            Err(error) => {
                return Err(BasisMinimizationError::Solver {
                    reason: error.to_string(),
                });
            }
        };

        let status = match solution.status() {
            SolutionStatus::Optimal => CoverSolveStatus::Optimal,
            SolutionStatus::GapLimit => CoverSolveStatus::GapLimited,
            SolutionStatus::TimeLimit => CoverSolveStatus::TimeLimited,
        };

        for (free_idx, candidate_idx) in free_candidate_indices.into_iter().enumerate() {
            if solution.value(decision_vars[free_idx]) > 0.5 {
                selected.insert(candidate_idx);
            }
        }

        Ok(CoverSolveOutcome {
            selected_candidates: Some(selected),
            status,
        })
    }
}

fn maybe_minimize_greedy_basis_words<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
{
    let Some(redundant_basis_word) = find_redundant_basis_word_for_minimizer(table, basis_words)?
    else {
        return Ok(None);
    };

    let reduced_basis = basis_words
        .iter()
        .filter(|basis_word| *basis_word != &redundant_basis_word)
        .cloned()
        .collect::<Vec<_>>();

    Ok((reduced_basis.len() < basis_words.len()).then_some(reduced_basis))
}

fn maybe_minimize_exact_basis_words<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
{
    #[cfg(feature = "milp")]
    {
        minimize_basis_words_with_cover_solver(
            table,
            basis_words,
            BasisMinimization::ExactMilp,
            ApproxMilpConfig::default(),
            |cover_instance| minimize_cover_instance_exact(cover_instance, HighsExactCoverSolver),
        )
    }

    #[cfg(not(feature = "milp"))]
    {
        let _ = (table, basis_words);
        Err(BasisMinimizationError::MilpBackendUnavailable {
            strategy: BasisMinimization::ExactMilp.as_str(),
        })
    }
}

fn maybe_minimize_approx_basis_words<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
    approx_milp: ApproxMilpConfig,
) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
{
    #[cfg(feature = "milp")]
    {
        approx_milp.validate()?;
        minimize_basis_words_with_cover_solver(
            table,
            basis_words,
            BasisMinimization::ApproxMilp(approx_milp),
            approx_milp,
            |cover_instance| {
                minimize_cover_instance_approx(
                    cover_instance,
                    HighsApproxCoverSolver {
                        config: approx_milp,
                    },
                )
            },
        )
    }

    #[cfg(not(feature = "milp"))]
    {
        let _ = (table, basis_words, approx_milp);
        Err(BasisMinimizationError::MilpBackendUnavailable {
            strategy: BasisMinimization::ApproxMilp(approx_milp).as_str(),
        })
    }
}

fn minimize_basis_words_with_cover_solver<A, F>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
    strategy: BasisMinimization,
    approx_milp: ApproxMilpConfig,
    minimize_cover: F,
) -> Result<Option<Vec<TimedWord<A>>>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
    F: FnOnce(CoverInstance<A>) -> Result<MinimizationOutcome<A>, BasisMinimizationError>,
{
    let started_at = Instant::now();
    let CollectedDistinctRows {
        classes: distinct_rows,
        row_to_index,
    } = collect_distinct_row_classes(table)?;
    let projected_basis_candidates =
        project_basis_words_to_candidates(table, basis_words, &row_to_index, distinct_rows.len())?;
    let cover_instance = build_cover_instance(distinct_rows, projected_basis_candidates)?;
    let outcome = minimize_cover(cover_instance)?;
    let elapsed = started_at.elapsed();
    let selected_count = outcome
        .selected_candidates
        .as_ref()
        .map(|selected| selected.count_ones(..))
        .unwrap_or(outcome.incumbent_size);

    debug!(
        "{} basis minimization: status {}, candidates {} -> {}, obligations {} -> {}, components {}, incumbent {}, selected {}, relative_gap {}, time_limit {:?}, elapsed {:?}",
        strategy.as_str(),
        outcome.solve_status.as_str(),
        outcome.candidate_count_before_presolve,
        outcome.candidate_count_after_presolve,
        outcome.obligation_count_before_presolve,
        outcome.obligation_count_after_presolve,
        outcome.component_sizes.len(),
        outcome.incumbent_size,
        selected_count,
        approx_milp.relative_gap,
        approx_milp.time_limit,
        elapsed,
    );
    trace!(
        "{} basis minimization component sizes (candidates, obligations): {:?}",
        strategy.as_str(),
        outcome.component_sizes
    );

    let selected_basis =
        selected_basis_words(&outcome.instance, effective_selected_candidates(&outcome));

    if selected_basis.len() < basis_words.len() {
        Ok(Some(selected_basis))
    } else {
        Ok(None)
    }
}

fn find_redundant_basis_word_for_minimizer<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Result<Option<TimedWord<A>>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
{
    for candidate_basis_word in basis_words.iter() {
        if candidate_basis_word.is_empty() {
            continue;
        }

        let other_basis_words = basis_words
            .iter()
            .filter(|basis_word| *basis_word != candidate_basis_word)
            .cloned()
            .collect::<Vec<_>>();
        if other_basis_words.is_empty() {
            continue;
        }

        let mut decomposer = build_decomposer_from_words_for_minimizer(table, other_basis_words)?;
        let mut all_samples_representable = true;
        for sample_prefix in table.sample_prefixes() {
            let row = table
                .row_of(sample_prefix)
                .ok_or(BasisMinimizationError::MissingEpsilonSample)?;
            if !decomposer
                .representable(row)
                .map_err(map_decomposition_error)?
            {
                all_samples_representable = false;
                break;
            }
        }

        if all_samples_representable {
            return Ok(Some(candidate_basis_word.clone()));
        }
    }

    Ok(None)
}

fn build_decomposer_from_words_for_minimizer<A>(
    table: &ObservationTable<A>,
    words: impl IntoIterator<Item = TimedWord<A>>,
) -> Result<BasisDecomposer, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
{
    let mut rows = Vec::new();
    for word in words {
        let row = table
            .row_of(&word)
            .ok_or(BasisMinimizationError::MissingEpsilonSample)?;
        rows.push(row.clone());
    }

    if rows.is_empty() {
        return Err(BasisMinimizationError::EmptyBasisWords);
    }

    BasisDecomposer::new(rows).map_err(map_decomposition_error)
}

fn map_decomposition_error(error: DecompositionError) -> BasisMinimizationError {
    match error {
        DecompositionError::LengthMismatch { expected, found } => {
            BasisMinimizationError::LengthMismatch { expected, found }
        }
        DecompositionError::EmptyBasis => BasisMinimizationError::EmptyBasisWords,
        DecompositionError::IndexOutOfBounds { index, len } => {
            BasisMinimizationError::Decomposition {
                reason: format!("index out of bounds: index {index}, len {len}"),
            }
        }
    }
}

fn collect_distinct_row_classes<A>(
    table: &ObservationTable<A>,
) -> Result<CollectedDistinctRows<A>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
{
    let mut row_to_index = HashMap::<RowVec, usize>::new();
    let mut distinct_rows = Vec::<DistinctRowClass<A>>::new();

    for sample_prefix in table.sample_prefixes() {
        let row = table
            .row_of(sample_prefix)
            .ok_or(BasisMinimizationError::MissingEpsilonSample)?;
        if row_to_index.contains_key(row) {
            continue;
        }

        row_to_index.insert(row.clone(), distinct_rows.len());
        distinct_rows.push(DistinctRowClass {
            representative: sample_prefix.clone(),
            row: row.clone(),
        });
    }

    Ok(CollectedDistinctRows {
        classes: distinct_rows,
        row_to_index,
    })
}

fn project_basis_words_to_candidates<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
    row_to_index: &HashMap<RowVec, usize>,
    candidate_count: usize,
) -> Result<FixedBitSet, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
{
    let mut projected = empty_bitset(candidate_count);

    for basis_word in basis_words.iter() {
        let row = table
            .row_of(basis_word)
            .ok_or(BasisMinimizationError::MissingEpsilonSample)?;
        let candidate_idx = row_to_index
            .get(row)
            .copied()
            .ok_or(BasisMinimizationError::MissingEpsilonSample)?;
        projected.insert(candidate_idx);
    }

    Ok(projected)
}

fn build_cover_instance<A>(
    candidates: Vec<DistinctRowClass<A>>,
    projected_basis_candidates: FixedBitSet,
) -> Result<CoverInstance<A>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
{
    let obligations = collect_obligations(&candidates);
    let mut coverage_by_candidate = Vec::with_capacity(candidates.len());
    let mut coverers_by_obligation = Vec::with_capacity(obligations.len());

    for _ in &obligations {
        coverers_by_obligation.push(empty_bitset(candidates.len()));
    }

    for _ in &candidates {
        coverage_by_candidate.push(empty_bitset(obligations.len()));
    }

    for (candidate_idx, candidate) in candidates.iter().enumerate() {
        for (obligation_idx, obligation) in obligations.iter().enumerate() {
            if candidate_covers_obligation(&candidate.row, obligation) {
                coverage_by_candidate[candidate_idx].insert(obligation_idx);
                coverers_by_obligation[obligation_idx].insert(candidate_idx);
            }
        }
    }

    for (obligation_idx, coverers) in coverers_by_obligation.iter().enumerate() {
        if coverers.is_clear() {
            let obligation = &obligations[obligation_idx];
            return Err(BasisMinimizationError::UncoverableObligation {
                positive_column: obligation.positive_column,
                negative_column: obligation.negative_column,
            });
        }
    }

    Ok(CoverInstance {
        candidates,
        obligations,
        coverage_by_candidate,
        coverers_by_obligation,
        forced_candidates: empty_bitset(projected_basis_candidates.len()),
        projected_basis_candidates,
    })
}

fn collect_obligations<A>(candidates: &[DistinctRowClass<A>]) -> Vec<Obligation> {
    let mut obligation_to_index = HashMap::<Obligation, usize>::new();
    let mut obligations = Vec::new();

    for candidate in candidates {
        for positive_column in candidate.row.ones() {
            for negative_column in 0..candidate.row.len() {
                if candidate.row.get(negative_column) == Some(false) {
                    let obligation = Obligation {
                        positive_column,
                        negative_column,
                    };
                    if obligation_to_index.contains_key(&obligation) {
                        continue;
                    }
                    obligation_to_index.insert(obligation.clone(), obligations.len());
                    obligations.push(obligation);
                }
            }
        }
    }

    obligations
}

fn candidate_covers_obligation(row: &RowVec, obligation: &Obligation) -> bool {
    row.get(obligation.positive_column) == Some(true)
        && row.get(obligation.negative_column) == Some(false)
}

fn minimize_cover_instance_exact<A, S>(
    instance: CoverInstance<A>,
    solver: S,
) -> Result<MinimizationOutcome<A>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
    S: CoverSolver,
{
    let outcome = minimize_cover_instance(instance, solver)?;
    match outcome.solve_status {
        CoverSolveStatus::Optimal | CoverSolveStatus::NoImprovement => Ok(outcome),
        status => Err(BasisMinimizationError::Solver {
            reason: format!(
                "exact MILP solver returned non-optimal status {}",
                status.as_str()
            ),
        }),
    }
}

fn minimize_cover_instance_approx<A, S>(
    instance: CoverInstance<A>,
    solver: S,
) -> Result<MinimizationOutcome<A>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
    S: CoverSolver,
{
    minimize_cover_instance(instance, solver)
}

fn minimize_cover_instance<A, S>(
    instance: CoverInstance<A>,
    solver: S,
) -> Result<MinimizationOutcome<A>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
    S: CoverSolver,
{
    let candidate_count_before_presolve = instance.candidates.len();
    let obligation_count_before_presolve = instance.obligations.len();
    let instance = presolve_cover_instance(instance)?;

    if !covers_all_obligations(&instance, &instance.projected_basis_candidates) {
        return Err(BasisMinimizationError::Solver {
            reason: "projected incumbent basis became infeasible during presolve".to_string(),
        });
    }

    let component_sizes = connected_components(&instance)
        .into_iter()
        .map(|component| {
            (
                component.candidate_indices.len(),
                component.obligation_indices.len(),
            )
        })
        .collect::<Vec<_>>();
    let incumbent_size = instance.projected_basis_candidates.count_ones(..);
    let solve_outcome = minimize_presolved_cover_instance(&instance, &solver)?;
    let candidate_count_after_presolve = instance.candidates.len();
    let obligation_count_after_presolve = instance.obligations.len();

    Ok(MinimizationOutcome {
        instance,
        selected_candidates: solve_outcome.selected_candidates,
        solve_status: solve_outcome.status,
        incumbent_size,
        candidate_count_before_presolve,
        obligation_count_before_presolve,
        candidate_count_after_presolve,
        obligation_count_after_presolve,
        component_sizes,
    })
}

fn presolve_cover_instance<A>(
    mut instance: CoverInstance<A>,
) -> Result<CoverInstance<A>, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
{
    if instance.obligations.is_empty() {
        return Ok(compact_no_obligation_instance(instance));
    }

    loop {
        let mut changed = false;

        loop {
            let mut forced_new = false;
            for obligation_idx in active_obligation_indices(&instance)? {
                if let Some(candidate_idx) =
                    unique_coverer_index(&instance.coverers_by_obligation[obligation_idx])
                    && !instance.forced_candidates.contains(candidate_idx)
                {
                    instance.forced_candidates.insert(candidate_idx);
                    forced_new = true;
                }
            }

            if !forced_new {
                break;
            }
            changed = true;
        }

        let active_obligations = active_obligation_indices(&instance)?;
        if active_obligations.len() != instance.obligations.len() {
            let keep_obligations =
                bitset_from_indices(instance.obligations.len(), active_obligations);
            instance = filter_cover_instance_obligations(instance, &keep_obligations);
            changed = true;
        }

        if instance.obligations.is_empty() {
            instance = compact_no_obligation_instance(instance);
            break;
        }

        let keep_candidates = candidate_keep_mask_for_remaining_obligations(&instance);
        if keep_candidates.count_ones(..) != instance.candidates.len() {
            instance = filter_cover_instance_candidates(instance, &keep_candidates);
            changed = true;
        }

        let (next_instance, candidate_changed) = reduce_dominated_candidates(instance);
        instance = next_instance;
        changed |= candidate_changed;

        let (next_instance, obligation_changed) = reduce_dominated_obligations(instance);
        instance = next_instance;
        changed |= obligation_changed;

        if !changed {
            break;
        }
    }

    Ok(instance)
}

fn compact_no_obligation_instance<A>(instance: CoverInstance<A>) -> CoverInstance<A>
where
    A: Eq + Hash + Clone,
{
    let mut keep_candidates = instance.forced_candidates.clone();

    if keep_candidates.is_clear() {
        if let Some(candidate_idx) = instance.projected_basis_candidates.ones().next() {
            keep_candidates.insert(candidate_idx);
        } else if !instance.candidates.is_empty() {
            keep_candidates.insert(0);
        }
    }

    filter_cover_instance_candidates(instance, &keep_candidates)
}

fn active_obligation_indices<A>(
    instance: &CoverInstance<A>,
) -> Result<Vec<usize>, BasisMinimizationError> {
    let mut active = Vec::new();

    for (obligation_idx, coverers) in instance.coverers_by_obligation.iter().enumerate() {
        let covered_by_forced = coverers
            .ones()
            .any(|candidate_idx| instance.forced_candidates.contains(candidate_idx));
        if covered_by_forced {
            continue;
        }
        if coverers.is_clear() {
            let obligation = &instance.obligations[obligation_idx];
            return Err(BasisMinimizationError::UncoverableObligation {
                positive_column: obligation.positive_column,
                negative_column: obligation.negative_column,
            });
        }
        active.push(obligation_idx);
    }

    Ok(active)
}

fn unique_coverer_index(coverers: &FixedBitSet) -> Option<usize> {
    let mut iter = coverers.ones();
    let first = iter.next()?;
    if iter.next().is_none() {
        Some(first)
    } else {
        None
    }
}

fn candidate_keep_mask_for_remaining_obligations<A>(instance: &CoverInstance<A>) -> FixedBitSet {
    let mut keep_candidates = instance.forced_candidates.clone();

    for candidate_idx in 0..instance.candidates.len() {
        if !instance.coverage_by_candidate[candidate_idx].is_clear() {
            keep_candidates.insert(candidate_idx);
        }
    }

    keep_candidates
}

fn reduce_dominated_candidates<A>(mut instance: CoverInstance<A>) -> (CoverInstance<A>, bool)
where
    A: Eq + Hash + Clone,
{
    let mut keep_candidates = full_bitset(instance.candidates.len());

    for candidate_idx in 0..instance.candidates.len() {
        if instance.forced_candidates.contains(candidate_idx) {
            continue;
        }
        if is_candidate_dominated(&instance, candidate_idx) {
            keep_candidates.set(candidate_idx, false);
        }
    }

    if keep_candidates.count_ones(..) == instance.candidates.len() {
        return (instance, false);
    }

    let mut replacements = vec![None; instance.candidates.len()];
    for (candidate_idx, replacement) in replacements.iter_mut().enumerate() {
        if keep_candidates.contains(candidate_idx) {
            continue;
        }
        *replacement = find_candidate_replacement(&instance, candidate_idx, &keep_candidates);
    }
    apply_projected_candidate_replacements(&mut instance, &replacements);

    (
        filter_cover_instance_candidates(instance, &keep_candidates),
        true,
    )
}

fn is_candidate_dominated<A>(instance: &CoverInstance<A>, candidate_idx: usize) -> bool {
    let coverage = &instance.coverage_by_candidate[candidate_idx];

    (0..instance.candidates.len()).any(|other_candidate_idx| {
        if other_candidate_idx == candidate_idx {
            return false;
        }

        let other_coverage = &instance.coverage_by_candidate[other_candidate_idx];
        coverage.is_subset(other_coverage)
            && (coverage != other_coverage || other_candidate_idx < candidate_idx)
    })
}

fn find_candidate_replacement<A>(
    instance: &CoverInstance<A>,
    candidate_idx: usize,
    keep_candidates: &FixedBitSet,
) -> Option<usize> {
    let coverage = &instance.coverage_by_candidate[candidate_idx];

    keep_candidates.ones().find(|other_candidate_idx| {
        if *other_candidate_idx == candidate_idx {
            return false;
        }

        let other_coverage = &instance.coverage_by_candidate[*other_candidate_idx];
        coverage.is_subset(other_coverage)
            && (coverage != other_coverage || *other_candidate_idx < candidate_idx)
    })
}

fn apply_projected_candidate_replacements<A>(
    instance: &mut CoverInstance<A>,
    replacements: &[Option<usize>],
) {
    for (candidate_idx, replacement_idx) in replacements.iter().enumerate() {
        if !instance.projected_basis_candidates.contains(candidate_idx) {
            continue;
        }
        if let Some(replacement_idx) = replacement_idx {
            instance.projected_basis_candidates.insert(*replacement_idx);
        }
    }
}

fn reduce_dominated_obligations<A>(instance: CoverInstance<A>) -> (CoverInstance<A>, bool)
where
    A: Eq + Hash + Clone,
{
    let mut keep_obligations = full_bitset(instance.obligations.len());

    for obligation_idx in 0..instance.obligations.len() {
        if is_obligation_dominated(&instance, obligation_idx) {
            keep_obligations.set(obligation_idx, false);
        }
    }

    if keep_obligations.count_ones(..) == instance.obligations.len() {
        return (instance, false);
    }

    (
        filter_cover_instance_obligations(instance, &keep_obligations),
        true,
    )
}

fn is_obligation_dominated<A>(instance: &CoverInstance<A>, obligation_idx: usize) -> bool {
    let coverers = &instance.coverers_by_obligation[obligation_idx];

    (0..instance.obligations.len()).any(|other_obligation_idx| {
        if other_obligation_idx == obligation_idx {
            return false;
        }

        let other_coverers = &instance.coverers_by_obligation[other_obligation_idx];
        other_coverers.is_subset(coverers)
            && (coverers != other_coverers || other_obligation_idx < obligation_idx)
    })
}

fn filter_cover_instance_candidates<A>(
    instance: CoverInstance<A>,
    keep_candidates: &FixedBitSet,
) -> CoverInstance<A>
where
    A: Eq + Hash + Clone,
{
    let mut old_to_new = vec![None; instance.candidates.len()];
    let mut candidates = Vec::new();
    for old_idx in keep_candidates.ones() {
        old_to_new[old_idx] = Some(candidates.len());
        candidates.push(instance.candidates[old_idx].clone());
    }

    let mut forced_candidates = empty_bitset(candidates.len());
    let mut projected_basis_candidates = empty_bitset(candidates.len());
    for old_idx in keep_candidates.ones() {
        let new_idx = old_to_new[old_idx].expect("kept candidate must have a new index");
        if instance.forced_candidates.contains(old_idx) {
            forced_candidates.insert(new_idx);
        }
        if instance.projected_basis_candidates.contains(old_idx) {
            projected_basis_candidates.insert(new_idx);
        }
    }

    let mut coverage_by_candidate = Vec::with_capacity(candidates.len());
    for old_idx in keep_candidates.ones() {
        let mut coverage = empty_bitset(instance.obligations.len());
        for obligation_idx in instance.coverage_by_candidate[old_idx].ones() {
            coverage.insert(obligation_idx);
        }
        coverage_by_candidate.push(coverage);
    }

    let mut coverers_by_obligation = Vec::with_capacity(instance.obligations.len());
    for coverers in &instance.coverers_by_obligation {
        let mut filtered_coverers = empty_bitset(candidates.len());
        for old_idx in coverers.ones() {
            if let Some(new_idx) = old_to_new[old_idx] {
                filtered_coverers.insert(new_idx);
            }
        }
        coverers_by_obligation.push(filtered_coverers);
    }

    CoverInstance {
        candidates,
        obligations: instance.obligations,
        coverage_by_candidate,
        coverers_by_obligation,
        forced_candidates,
        projected_basis_candidates,
    }
}

fn filter_cover_instance_obligations<A>(
    instance: CoverInstance<A>,
    keep_obligations: &FixedBitSet,
) -> CoverInstance<A>
where
    A: Eq + Hash + Clone,
{
    let mut old_to_new = vec![None; instance.obligations.len()];
    let mut obligations = Vec::new();
    for old_idx in keep_obligations.ones() {
        old_to_new[old_idx] = Some(obligations.len());
        obligations.push(instance.obligations[old_idx].clone());
    }

    let mut coverage_by_candidate = Vec::with_capacity(instance.candidates.len());
    for coverage in &instance.coverage_by_candidate {
        let mut filtered_coverage = empty_bitset(obligations.len());
        for old_idx in coverage.ones() {
            if let Some(new_idx) = old_to_new[old_idx] {
                filtered_coverage.insert(new_idx);
            }
        }
        coverage_by_candidate.push(filtered_coverage);
    }

    let mut coverers_by_obligation = Vec::with_capacity(obligations.len());
    for old_idx in keep_obligations.ones() {
        let mut filtered_coverers = empty_bitset(instance.candidates.len());
        for candidate_idx in instance.coverers_by_obligation[old_idx].ones() {
            filtered_coverers.insert(candidate_idx);
        }
        coverers_by_obligation.push(filtered_coverers);
    }

    CoverInstance {
        candidates: instance.candidates,
        obligations,
        coverage_by_candidate,
        coverers_by_obligation,
        forced_candidates: instance.forced_candidates,
        projected_basis_candidates: instance.projected_basis_candidates,
    }
}

fn minimize_presolved_cover_instance<A, S>(
    instance: &CoverInstance<A>,
    solver: &S,
) -> Result<CoverSolveOutcome, BasisMinimizationError>
where
    A: Eq + Hash + Clone,
    S: CoverSolver,
{
    let mut selected = instance.forced_candidates.clone();

    if instance.obligations.is_empty() {
        if selected.is_clear() {
            if let Some(candidate_idx) = instance.projected_basis_candidates.ones().next() {
                selected.insert(candidate_idx);
            } else if !instance.candidates.is_empty() {
                selected.insert(0);
            }
        }
        let incumbent_size = instance.projected_basis_candidates.count_ones(..);
        return if selected.count_ones(..) < incumbent_size {
            Ok(CoverSolveOutcome {
                selected_candidates: Some(selected),
                status: CoverSolveStatus::Optimal,
            })
        } else {
            Ok(CoverSolveOutcome {
                selected_candidates: None,
                status: CoverSolveStatus::NoImprovement,
            })
        };
    }

    let components = connected_components(instance);
    let mut candidates_in_components = empty_bitset(instance.candidates.len());
    let mut any_improvement = false;
    let mut saw_gap_limited = false;
    let mut saw_time_limited = false;

    for component in &components {
        for candidate_idx in &component.candidate_indices {
            candidates_in_components.insert(*candidate_idx);
        }
    }

    for component in &components {
        let component_incumbent_size = component
            .candidate_indices
            .iter()
            .filter(|candidate_idx| {
                instance
                    .projected_basis_candidates
                    .contains(**candidate_idx)
            })
            .count();
        if component_incumbent_size == 0 {
            return Err(BasisMinimizationError::Solver {
                reason: "projected incumbent basis does not cover a presolved component"
                    .to_string(),
            });
        }

        let component_instance = build_component_instance(instance, component);
        let component_outcome =
            solver.solve_bounded_cover(&component_instance, component_incumbent_size)?;
        saw_gap_limited |= matches!(component_outcome.status, CoverSolveStatus::GapLimited);
        saw_time_limited |= matches!(component_outcome.status, CoverSolveStatus::TimeLimited);

        if let Some(component_selection) = component_outcome.selected_candidates {
            any_improvement = true;
            for local_candidate_idx in component_selection.ones() {
                selected.insert(component.candidate_indices[local_candidate_idx]);
            }
        } else {
            for candidate_idx in &component.candidate_indices {
                if instance.projected_basis_candidates.contains(*candidate_idx) {
                    selected.insert(*candidate_idx);
                }
            }
        }
    }

    for candidate_idx in instance.projected_basis_candidates.ones() {
        if !candidates_in_components.contains(candidate_idx) {
            selected.insert(candidate_idx);
        }
    }

    if selected.is_clear() && !instance.candidates.is_empty() {
        selected.insert(0);
    }

    let status = if saw_time_limited {
        CoverSolveStatus::TimeLimited
    } else if saw_gap_limited {
        CoverSolveStatus::GapLimited
    } else if any_improvement {
        CoverSolveStatus::Optimal
    } else {
        CoverSolveStatus::NoImprovement
    };

    Ok(CoverSolveOutcome {
        selected_candidates: any_improvement.then_some(selected),
        status,
    })
}

fn connected_components<A>(instance: &CoverInstance<A>) -> Vec<CoverComponent> {
    let mut components = Vec::new();
    let mut seen_candidates = empty_bitset(instance.candidates.len());
    let mut seen_obligations = empty_bitset(instance.obligations.len());

    for start_candidate_idx in 0..instance.candidates.len() {
        if instance.forced_candidates.contains(start_candidate_idx)
            || instance.coverage_by_candidate[start_candidate_idx].is_clear()
            || seen_candidates.contains(start_candidate_idx)
        {
            continue;
        }

        let mut queue = VecDeque::new();
        let mut candidate_indices = Vec::new();
        let mut obligation_indices = Vec::new();

        seen_candidates.insert(start_candidate_idx);
        queue.push_back(ComponentNode::Candidate(start_candidate_idx));

        while let Some(node) = queue.pop_front() {
            match node {
                ComponentNode::Candidate(candidate_idx) => {
                    candidate_indices.push(candidate_idx);
                    for obligation_idx in instance.coverage_by_candidate[candidate_idx].ones() {
                        if seen_obligations.contains(obligation_idx) {
                            continue;
                        }
                        seen_obligations.insert(obligation_idx);
                        queue.push_back(ComponentNode::Obligation(obligation_idx));
                    }
                }
                ComponentNode::Obligation(obligation_idx) => {
                    obligation_indices.push(obligation_idx);
                    for candidate_idx in instance.coverers_by_obligation[obligation_idx].ones() {
                        if instance.forced_candidates.contains(candidate_idx)
                            || seen_candidates.contains(candidate_idx)
                        {
                            continue;
                        }
                        seen_candidates.insert(candidate_idx);
                        queue.push_back(ComponentNode::Candidate(candidate_idx));
                    }
                }
            }
        }

        candidate_indices.sort_unstable();
        obligation_indices.sort_unstable();

        components.push(CoverComponent {
            candidate_indices,
            obligation_indices,
        });
    }

    for start_obligation_idx in 0..instance.obligations.len() {
        if seen_obligations.contains(start_obligation_idx) {
            continue;
        }

        let mut queue = VecDeque::new();
        let mut candidate_indices = Vec::new();
        let mut obligation_indices = Vec::new();

        seen_obligations.insert(start_obligation_idx);
        queue.push_back(ComponentNode::Obligation(start_obligation_idx));

        while let Some(node) = queue.pop_front() {
            match node {
                ComponentNode::Candidate(candidate_idx) => {
                    candidate_indices.push(candidate_idx);
                    for obligation_idx in instance.coverage_by_candidate[candidate_idx].ones() {
                        if seen_obligations.contains(obligation_idx) {
                            continue;
                        }
                        seen_obligations.insert(obligation_idx);
                        queue.push_back(ComponentNode::Obligation(obligation_idx));
                    }
                }
                ComponentNode::Obligation(obligation_idx) => {
                    obligation_indices.push(obligation_idx);
                    for candidate_idx in instance.coverers_by_obligation[obligation_idx].ones() {
                        if instance.forced_candidates.contains(candidate_idx)
                            || seen_candidates.contains(candidate_idx)
                        {
                            continue;
                        }
                        seen_candidates.insert(candidate_idx);
                        queue.push_back(ComponentNode::Candidate(candidate_idx));
                    }
                }
            }
        }

        candidate_indices.sort_unstable();
        obligation_indices.sort_unstable();

        components.push(CoverComponent {
            candidate_indices,
            obligation_indices,
        });
    }

    components
}

fn build_component_instance<A>(
    instance: &CoverInstance<A>,
    component: &CoverComponent,
) -> CoverInstance<A>
where
    A: Eq + Hash + Clone,
{
    let keep_candidates = bitset_from_indices(
        instance.candidates.len(),
        component.candidate_indices.iter().copied(),
    );
    let keep_obligations = bitset_from_indices(
        instance.obligations.len(),
        component.obligation_indices.iter().copied(),
    );

    let component_instance = filter_cover_instance_candidates(instance.clone(), &keep_candidates);
    filter_cover_instance_obligations(component_instance, &keep_obligations)
}

fn covers_all_obligations<A>(
    instance: &CoverInstance<A>,
    selected_candidates: &FixedBitSet,
) -> bool {
    instance.coverers_by_obligation.iter().all(|coverers| {
        coverers
            .ones()
            .any(|candidate_idx| selected_candidates.contains(candidate_idx))
    })
}

fn selected_basis_words<A>(
    instance: &CoverInstance<A>,
    selected_candidates: &FixedBitSet,
) -> Vec<TimedWord<A>>
where
    A: Eq + Hash + Clone,
{
    selected_candidates
        .ones()
        .map(|candidate_idx| instance.candidates[candidate_idx].representative.clone())
        .collect()
}

fn effective_selected_candidates<A>(outcome: &MinimizationOutcome<A>) -> &FixedBitSet
where
    A: Eq + Hash + Clone,
{
    outcome
        .selected_candidates
        .as_ref()
        .unwrap_or(&outcome.instance.projected_basis_candidates)
}

fn empty_bitset(len: usize) -> FixedBitSet {
    let mut bitset = FixedBitSet::with_capacity(len);
    bitset.grow(len);
    bitset
}

fn full_bitset(len: usize) -> FixedBitSet {
    let mut bitset = empty_bitset(len);
    for idx in 0..len {
        bitset.insert(idx);
    }
    bitset
}

fn bitset_from_indices(len: usize, indices: impl IntoIterator<Item = usize>) -> FixedBitSet {
    let mut bitset = empty_bitset(len);
    for idx in indices {
        bitset.insert(idx);
    }
    bitset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, Default)]
    struct BruteForceExactCoverSolver;

    impl CoverSolver for BruteForceExactCoverSolver {
        fn solve_bounded_cover<A>(
            &self,
            instance: &CoverInstance<A>,
            incumbent_size: usize,
        ) -> Result<CoverSolveOutcome, BasisMinimizationError>
        where
            A: Eq + Hash + Clone,
        {
            let forced_count = instance.forced_candidates.count_ones(..);
            let mut selected = instance.forced_candidates.clone();

            if instance.obligations.is_empty() {
                if selected.is_clear() {
                    if let Some(candidate_idx) = instance.projected_basis_candidates.ones().next() {
                        selected.insert(candidate_idx);
                    } else if !instance.candidates.is_empty() {
                        selected.insert(0);
                    }
                }
                return if selected.count_ones(..) < incumbent_size {
                    Ok(CoverSolveOutcome {
                        selected_candidates: Some(selected),
                        status: CoverSolveStatus::Optimal,
                    })
                } else {
                    Ok(CoverSolveOutcome {
                        selected_candidates: None,
                        status: CoverSolveStatus::NoImprovement,
                    })
                };
            }

            if incumbent_size <= forced_count {
                return Ok(CoverSolveOutcome {
                    selected_candidates: None,
                    status: CoverSolveStatus::NoImprovement,
                });
            }

            let free_candidate_indices = (0..instance.candidates.len())
                .filter(|candidate_idx| !instance.forced_candidates.contains(*candidate_idx))
                .collect::<Vec<_>>();
            let max_additional = incumbent_size - forced_count - 1;
            let free_count = free_candidate_indices.len();
            let mut best = None;

            for mask in 0usize..(1usize << free_count) {
                let selected_count = mask.count_ones() as usize;
                if selected_count > max_additional {
                    continue;
                }

                let mut candidate_selection = instance.forced_candidates.clone();
                for (free_idx, candidate_idx) in free_candidate_indices.iter().enumerate() {
                    if (mask & (1usize << free_idx)) != 0 {
                        candidate_selection.insert(*candidate_idx);
                    }
                }

                if !covers_all_obligations(instance, &candidate_selection) {
                    continue;
                }

                let is_better = best
                    .as_ref()
                    .map(|best_selection: &FixedBitSet| {
                        candidate_selection.count_ones(..) < best_selection.count_ones(..)
                    })
                    .unwrap_or(true);
                if is_better {
                    best = Some(candidate_selection);
                }
            }

            let status = if best.is_some() {
                CoverSolveStatus::Optimal
            } else {
                CoverSolveStatus::NoImprovement
            };

            Ok(CoverSolveOutcome {
                selected_candidates: best,
                status,
            })
        }
    }

    #[derive(Debug, Clone)]
    struct MockCoverSolver {
        status: CoverSolveStatus,
        selected_candidate_indices: Option<Vec<usize>>,
    }

    impl CoverSolver for MockCoverSolver {
        fn solve_bounded_cover<A>(
            &self,
            instance: &CoverInstance<A>,
            _incumbent_size: usize,
        ) -> Result<CoverSolveOutcome, BasisMinimizationError>
        where
            A: Eq + Hash + Clone,
        {
            Ok(CoverSolveOutcome {
                selected_candidates: self.selected_candidate_indices.as_ref().map(|indices| {
                    bitset_from_indices(instance.candidates.len(), indices.iter().copied())
                }),
                status: self.status,
            })
        }
    }

    fn row_from_bits(bits: &str) -> RowVec {
        let mut row = RowVec::new(bits.len());
        for (index, bit) in bits.chars().enumerate() {
            row.set(index, bit == '1')
                .expect("generated row bit index must stay in bounds");
        }
        row
    }

    fn test_classes(rows: &[&str], epsilon_index: usize) -> Vec<DistinctRowClass<char>> {
        rows.iter()
            .enumerate()
            .map(|(index, bits)| DistinctRowClass {
                representative: if index == epsilon_index {
                    TimedWord::empty()
                } else {
                    TimedWord::from_vec(vec![(
                        'a',
                        learn_arta_core::DelayRep::from_integer(index as u32),
                    )])
                },
                row: row_from_bits(bits),
            })
            .collect()
    }

    fn projected_basis_for_all_candidates(candidate_count: usize) -> FixedBitSet {
        full_bitset(candidate_count)
    }

    fn build_test_instance(rows: &[&str], epsilon_index: usize) -> CoverInstance<char> {
        let classes = test_classes(rows, epsilon_index);
        build_cover_instance(classes, projected_basis_for_all_candidates(rows.len()))
            .expect("test cover instance should build")
    }

    fn manual_instance(
        candidate_coverages: &[&[usize]],
        obligation_count: usize,
        projected_basis_candidates: &[usize],
    ) -> CoverInstance<char> {
        let candidates = candidate_coverages
            .iter()
            .enumerate()
            .map(|(index, _)| DistinctRowClass {
                representative: if index == 0 {
                    TimedWord::empty()
                } else {
                    TimedWord::from_vec(vec![(
                        'a',
                        learn_arta_core::DelayRep::from_integer(index as u32),
                    )])
                },
                row: row_from_bits(&"0".repeat(obligation_count.max(1))),
            })
            .collect::<Vec<_>>();
        let obligations = (0..obligation_count)
            .map(|obligation_idx| Obligation {
                positive_column: obligation_idx,
                negative_column: obligation_idx + obligation_count,
            })
            .collect::<Vec<_>>();
        let mut coverage_by_candidate = Vec::with_capacity(candidate_coverages.len());
        let mut coverers_by_obligation =
            vec![empty_bitset(candidate_coverages.len()); obligation_count];

        for coverage in candidate_coverages {
            coverage_by_candidate.push(bitset_from_indices(
                obligation_count,
                coverage.iter().copied(),
            ));
        }
        for (candidate_idx, coverage) in candidate_coverages.iter().enumerate() {
            for obligation_idx in *coverage {
                coverers_by_obligation[*obligation_idx].insert(candidate_idx);
            }
        }

        CoverInstance {
            candidates,
            obligations,
            coverage_by_candidate,
            coverers_by_obligation,
            forced_candidates: empty_bitset(candidate_coverages.len()),
            projected_basis_candidates: bitset_from_indices(
                candidate_coverages.len(),
                projected_basis_candidates.iter().copied(),
            ),
        }
    }

    fn greedy_selection_size(rows: &[RowVec]) -> usize {
        let mut selected = (0..rows.len()).collect::<Vec<_>>();

        let is_representable = |basis_indices: &[usize], row: &RowVec| {
            let basis_rows = basis_indices
                .iter()
                .map(|basis_idx| rows[*basis_idx].clone())
                .collect::<Vec<_>>();
            let mut decomposer = crate::BasisDecomposer::new(basis_rows)
                .expect("test greedy basis must stay non-empty and rectangular");
            decomposer
                .representable(row)
                .expect("test rows must stay rectangular")
        };

        let mut cursor = 0;
        while cursor < selected.len() {
            let candidate_idx = selected[cursor];
            if candidate_idx == 0 {
                cursor += 1;
                continue;
            }

            let remaining = selected
                .iter()
                .copied()
                .filter(|selected_idx| *selected_idx != candidate_idx)
                .collect::<Vec<_>>();
            let all_representable = rows.iter().all(|row| is_representable(&remaining, row));
            if all_representable {
                selected = remaining;
                cursor = 0;
            } else {
                cursor += 1;
            }
        }

        selected.len()
    }

    fn selected_candidate_count<A>(outcome: &MinimizationOutcome<A>) -> Option<usize> {
        outcome
            .selected_candidates
            .as_ref()
            .map(|selected| selected.count_ones(..))
    }

    fn effective_selected_candidate_count<A>(outcome: &MinimizationOutcome<A>) -> usize
    where
        A: Eq + Hash + Clone,
    {
        effective_selected_candidates(outcome).count_ones(..)
    }

    #[test]
    fn obligation_construction_and_coverage_match_manual_example() {
        let instance = build_test_instance(&["101", "110"], 0);

        assert_eq!(
            instance.obligations,
            vec![
                Obligation {
                    positive_column: 0,
                    negative_column: 1,
                },
                Obligation {
                    positive_column: 2,
                    negative_column: 1,
                },
                Obligation {
                    positive_column: 0,
                    negative_column: 2,
                },
                Obligation {
                    positive_column: 1,
                    negative_column: 2,
                },
            ]
        );
        assert!(instance.coverage_by_candidate[0].contains(0));
        assert!(instance.coverage_by_candidate[0].contains(1));
        assert!(!instance.coverage_by_candidate[0].contains(2));
        assert!(!instance.coverage_by_candidate[0].contains(3));
        assert!(instance.coverage_by_candidate[1].contains(2));
        assert!(instance.coverage_by_candidate[1].contains(3));
        assert!(!instance.coverage_by_candidate[1].contains(0));
        assert!(!instance.coverage_by_candidate[1].contains(1));
    }

    #[cfg(feature = "milp")]
    #[test]
    fn exact_solver_reports_no_improvement_when_basis_is_already_minimum() {
        let outcome = minimize_cover_instance_exact(
            build_test_instance(&["100", "010", "001"], 0),
            HighsExactCoverSolver,
        )
        .expect("exact minimization should succeed");

        assert_eq!(outcome.incumbent_size, 3);
        assert_eq!(outcome.solve_status, CoverSolveStatus::NoImprovement);
        assert_eq!(selected_candidate_count(&outcome), None);
    }

    #[cfg(feature = "milp")]
    #[test]
    fn exact_solver_can_exclude_epsilon_from_the_selected_basis() {
        let outcome = minimize_cover_instance_exact(
            build_test_instance(&["00", "01"], 0),
            HighsExactCoverSolver,
        )
        .expect("exact minimization should succeed");
        let selected_words =
            selected_basis_words(&outcome.instance, effective_selected_candidates(&outcome));

        assert_eq!(effective_selected_candidate_count(&outcome), 1);
        assert_eq!(selected_words.len(), 1);
        assert!(!selected_words[0].is_empty());
    }

    #[cfg(feature = "milp")]
    #[test]
    fn exact_solver_reports_no_improvement_for_singleton_no_obligation_basis() {
        let outcome =
            minimize_cover_instance_exact(build_test_instance(&["00"], 0), HighsExactCoverSolver)
                .expect("exact minimization should succeed");

        assert_eq!(outcome.incumbent_size, 1);
        assert_eq!(outcome.solve_status, CoverSolveStatus::NoImprovement);
        assert!(outcome.selected_candidates.is_none());
    }

    #[cfg(feature = "milp")]
    #[test]
    fn exact_solver_can_strictly_improve_on_greedy_removal_order() {
        let rows = ["001", "010", "011", "100", "110"];
        let greedy_rows = rows
            .iter()
            .map(|row| row_from_bits(row))
            .collect::<Vec<_>>();
        let greedy_size = greedy_selection_size(&greedy_rows);
        let outcome =
            minimize_cover_instance_exact(build_test_instance(&rows, 0), HighsExactCoverSolver)
                .expect("exact minimization should succeed");

        assert_eq!(greedy_size, 4);
        let selected_count = effective_selected_candidate_count(&outcome);
        assert_eq!(selected_count, 3);
        assert!(selected_count <= greedy_size);
    }

    #[test]
    fn presolve_drops_dominated_candidates_and_remaps_projected_basis() {
        let instance = manual_instance(&[&[0], &[0, 1], &[1]], 2, &[0, 2]);

        let instance = presolve_cover_instance(instance).expect("presolve should succeed");

        assert_eq!(instance.candidates.len(), 1);
        assert_eq!(instance.projected_basis_candidates.count_ones(..), 1);
        assert!(instance.projected_basis_candidates.contains(0));
    }

    #[test]
    fn presolve_drops_duplicate_and_superset_obligations() {
        let (instance, changed) = reduce_dominated_obligations(manual_instance(
            &[&[0, 1, 2], &[0, 1, 2], &[1, 2]],
            3,
            &[0, 1, 2],
        ));

        assert!(changed);
        assert_eq!(
            instance.obligations,
            vec![Obligation {
                positive_column: 0,
                negative_column: 3,
            }]
        );
    }

    #[test]
    fn connected_components_split_independent_subproblems() {
        let instance = manual_instance(
            &[&[0, 1], &[0, 2], &[1, 2], &[3, 4], &[3, 5], &[4, 5]],
            6,
            &[0, 1, 2, 3, 4, 5],
        );
        let components = connected_components(&instance);

        assert_eq!(components.len(), 2);
        assert_eq!(components[0].candidate_indices.len(), 3);
        assert_eq!(components[0].obligation_indices.len(), 3);
        assert_eq!(components[1].candidate_indices.len(), 3);
        assert_eq!(components[1].obligation_indices.len(), 3);
    }

    #[cfg(feature = "milp")]
    #[test]
    fn strict_incumbent_bound_reports_no_improvement_when_optimum_matches_incumbent() {
        let instance = build_test_instance(&["100", "010", "001"], 0);
        let outcome = minimize_cover_instance_exact(instance.clone(), BruteForceExactCoverSolver)
            .expect("brute-force minimization should succeed");

        assert_eq!(outcome.incumbent_size, 3);
        assert!(outcome.selected_candidates.is_none());

        let no_improvement = HighsExactCoverSolver
            .solve_bounded_cover(&outcome.instance, outcome.incumbent_size)
            .expect("bounded solve should succeed");
        assert_eq!(no_improvement.status, CoverSolveStatus::NoImprovement);
        assert!(no_improvement.selected_candidates.is_none());
    }

    #[test]
    fn exact_minimization_rejects_non_optimal_solver_status() {
        let error = minimize_cover_instance_exact(
            manual_instance(&[&[0, 1], &[0, 2], &[1, 2]], 3, &[0, 1, 2]),
            MockCoverSolver {
                status: CoverSolveStatus::GapLimited,
                selected_candidate_indices: Some(vec![0, 1]),
            },
        )
        .expect_err("exact minimization must reject non-optimal solver statuses");

        assert!(matches!(
            error,
            BasisMinimizationError::Solver { reason }
            if reason.contains("gap-limited")
        ));
    }

    #[test]
    fn approximate_minimization_accepts_gap_limited_improvement() {
        let outcome = minimize_cover_instance_approx(
            manual_instance(&[&[0, 1], &[0, 2], &[1, 2]], 3, &[0, 1, 2]),
            MockCoverSolver {
                status: CoverSolveStatus::GapLimited,
                selected_candidate_indices: Some(vec![0, 1]),
            },
        )
        .expect("approximate minimization should accept bounded solver improvements");

        assert_eq!(outcome.solve_status, CoverSolveStatus::GapLimited);
        assert_eq!(selected_candidate_count(&outcome), Some(2));
    }

    #[test]
    fn approximate_minimization_keeps_incumbent_when_solver_has_no_improvement() {
        let outcome = minimize_cover_instance_approx(
            build_test_instance(&["100", "010", "001"], 0),
            MockCoverSolver {
                status: CoverSolveStatus::NoImprovement,
                selected_candidate_indices: None,
            },
        )
        .expect("approximate minimization should keep the incumbent basis");

        assert_eq!(outcome.solve_status, CoverSolveStatus::NoImprovement);
        assert!(outcome.selected_candidates.is_none());
    }

    #[cfg(feature = "milp")]
    #[test]
    fn exhaustive_small_instances_match_bruteforce_minimum() {
        let all_rows = ["000", "001", "010", "011", "100", "101", "110", "111"];

        for mask in 1usize..(1usize << all_rows.len()) {
            let rows = all_rows
                .iter()
                .enumerate()
                .filter_map(|(row_idx, row)| {
                    if (mask & (1usize << row_idx)) != 0 {
                        Some(*row)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let instance = build_test_instance(&rows, 0);
            let exact = minimize_cover_instance_exact(instance.clone(), HighsExactCoverSolver)
                .expect("HiGHS exact minimization should succeed");
            let brute = minimize_cover_instance_exact(instance, BruteForceExactCoverSolver)
                .expect("brute-force exact minimization should succeed");

            assert_eq!(
                selected_candidate_count(&exact).unwrap_or(exact.incumbent_size),
                selected_candidate_count(&brute).unwrap_or(brute.incumbent_size),
                "row subset: {:?}",
                rows
            );
            if let Some(selected_candidates) = exact.selected_candidates.as_ref() {
                assert!(covers_all_obligations(&exact.instance, selected_candidates));
            } else {
                assert!(covers_all_obligations(
                    &exact.instance,
                    &exact.instance.projected_basis_candidates
                ));
            }
        }
    }

    #[cfg(not(feature = "milp"))]
    #[test]
    fn exact_strategy_reports_backend_unavailable_without_feature() {
        let minimizer = BasisMinimization::ExactMilp;

        let error = minimizer
            .minimize_basis(
                &ObservationTable::<char>::new(),
                &BasisWords::new_with_epsilon(),
            )
            .expect_err("exact MILP should be unavailable without the feature");

        assert!(matches!(
            error,
            BasisMinimizationError::MilpBackendUnavailable {
                strategy: "exact-milp"
            }
        ));
    }

    #[cfg(not(feature = "milp"))]
    #[test]
    fn approximate_strategy_reports_backend_unavailable_without_feature() {
        let minimizer = BasisMinimization::ApproxMilp(ApproxMilpConfig::default());

        let error = minimizer
            .minimize_basis(
                &ObservationTable::<char>::new(),
                &BasisWords::new_with_epsilon(),
            )
            .expect_err("approximate MILP should be unavailable without the feature");

        assert!(matches!(
            error,
            BasisMinimizationError::MilpBackendUnavailable {
                strategy: "approx-milp"
            }
        ));
    }
}
