// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use clap::{Args, ValueEnum};
use learn_arta::{ActiveArtaLearner, ApproxMilpConfig, BasisMinimization};
use learn_arta_core::{
    Arta, DagStateFormula, ParsedArtaJson, TimedWord, read_arta_json_file_document,
    to_arta_json_document_string,
};
use learn_arta_oracles::{ArtaMembershipOracle, CachingMembershipOracle, WhiteBoxEqOracle};
use learn_arta_traits::{EquivalenceOracle, MembershipOracle};
use log::{Level, LevelFilter, debug, info, log_enabled, trace};

use crate::error::CliError;

/// Basis minimization strategy exposed by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum BasisMinimizationArg {
    /// Existing heuristic basis minimization.
    Greedy,
    /// Exact minimum-cardinality basis via MILP, using the native HiGHS backend.
    ///
    /// Requires rebuilding the CLI with `--features milp`.
    #[value(name = "exact-milp")]
    ExactMilp,
    /// Approximate minimum-cardinality basis via bounded MILP, using the native HiGHS backend.
    ///
    /// Requires rebuilding the CLI with `--features milp`.
    #[value(name = "approx-milp")]
    ApproxMilp,
}

#[allow(clippy::derivable_impls)]
impl Default for BasisMinimizationArg {
    fn default() -> Self {
        #[cfg(feature = "milp")]
        {
            Self::ApproxMilp
        }

        #[cfg(not(feature = "milp"))]
        {
            Self::Greedy
        }
    }
}

impl From<BasisMinimizationArg> for BasisMinimization {
    fn from(value: BasisMinimizationArg) -> Self {
        match value {
            BasisMinimizationArg::Greedy => BasisMinimization::Greedy,
            BasisMinimizationArg::ExactMilp => BasisMinimization::ExactMilp,
            BasisMinimizationArg::ApproxMilp => {
                BasisMinimization::ApproxMilp(ApproxMilpConfig::default())
            }
        }
    }
}

impl BasisMinimizationArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Greedy => "greedy",
            Self::ExactMilp => "exact-milp",
            Self::ApproxMilp => "approx-milp",
        }
    }
}

/// Learn an exact hypothesis against a JSON target.
#[derive(Debug, Clone, PartialEq, Args)]
pub(crate) struct LearnArgs {
    /// Path to the target ARTA JSON file.
    pub(crate) input_json: PathBuf,
    /// Write learned JSON output to a file instead of stdout.
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,
    /// Override the default output automaton name.
    #[arg(long)]
    pub(crate) name: Option<String>,
    /// Suppress success diagnostics and emit only the learned JSON payload.
    #[arg(long)]
    pub(crate) quiet: bool,
    /// Emit debug-level learning diagnostics.
    #[arg(long, conflicts_with_all = ["quiet", "trace"])]
    pub(crate) debug: bool,
    /// Emit trace-level learning diagnostics, including hypothesis JSON.
    #[arg(long, conflicts_with_all = ["quiet", "debug"])]
    pub(crate) trace: bool,
    /// Basis minimization strategy.
    ///
    /// `greedy` keeps the existing heuristic; `exact-milp` computes an exact
    /// minimum-cardinality basis for the current observation table via the
    /// native HiGHS MILP backend and may exclude `epsilon`; `approx-milp` uses
    /// the same encoding with bounded gap/time settings and keeps the
    /// incumbent basis if it finds no smaller feasible basis.
    ///
    /// The default is `greedy` in builds without the `milp` feature and
    /// `approx-milp` when the feature is enabled.
    #[arg(long, value_enum, default_value_t = BasisMinimizationArg::default())]
    pub(crate) basis_minimization: BasisMinimizationArg,
    /// Relative MIP gap for `approx-milp`; defaults to `0.01` when that
    /// strategy is selected.
    #[arg(long)]
    pub(crate) basis_mip_gap: Option<f32>,
    /// Time limit in seconds for `approx-milp`; defaults to `5.0` when that
    /// strategy is selected.
    #[arg(long)]
    pub(crate) basis_time_limit_secs: Option<f64>,
}

impl LearnArgs {
    pub(crate) fn log_level_override(&self) -> Option<LevelFilter> {
        if self.trace {
            Some(LevelFilter::Trace)
        } else if self.debug {
            Some(LevelFilter::Debug)
        } else if self.quiet {
            Some(LevelFilter::Warn)
        } else {
            None
        }
    }

    pub(crate) fn run(&self) -> Result<(), CliError> {
        let input_document = read_arta_json_file_document(&self.input_json)?;
        let target = input_document.arta.clone();
        let sigma = input_document.sigma.clone();
        let output_name = output_name(&input_document, self.name.as_deref());
        let basis_minimization = learner_config(
            self.basis_minimization,
            self.basis_mip_gap,
            self.basis_time_limit_secs,
        )
        .map_err(|reason| CliError::ExactLearningConfiguration { reason })?;

        let mut mq = CachingMembershipOracle::new(ArtaMembershipOracle::new(target.clone()));
        let mut eq = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target, sigma.clone())
            .map_err(CliError::from_whitebox_setup_error)?;

        let mut learner = ActiveArtaLearner::<String>::with_minimizer(basis_minimization);
        let learning_started_at = Instant::now();
        log_target_json_file(&self.input_json);
        log_learning_started("exact", self.basis_minimization);
        let mut hypothesis = loop {
            let hypothesis = learner
                .build_hypothesis(&mut mq)
                .map_err(|error| CliError::exact_learning_failed(error.to_string()))?;
            let eq_index = learner.state().hypothesis_iterations;
            log_equivalence_query_started(eq_index, hypothesis.locations().len());
            log_observation_table_dimensions(
                eq_index,
                learner.state().observation_table.sample_prefixes().len(),
                learner
                    .state()
                    .observation_table
                    .experiment_suffixes()
                    .len(),
            );
            log_hypothesis_json(eq_index, &output_name, &sigma, &hypothesis)
                .map_err(|error| CliError::exact_learning_failed(error.to_string()))?;

            let maybe_counterexample = eq
                .find_counterexample(&hypothesis)
                .map_err(|error| CliError::exact_learning_failed(error.to_string()))?;
            log_equivalence_query_result(eq_index, maybe_counterexample.as_ref());

            let Some(counterexample) = maybe_counterexample else {
                break hypothesis;
            };

            learner
                .refine_with_counterexample(&mut mq, &counterexample)
                .map_err(|error| CliError::exact_learning_failed(error.to_string()))?;
        };
        hypothesis.simplify();

        let output_document = ParsedArtaJson {
            name: output_name,
            sigma,
            arta: hypothesis,
        };
        write_output_document(&output_document, self.output.as_ref())?;
        let learning_elapsed = learning_started_at.elapsed();

        info!("exact learning completed: learned hypothesis is equivalent to the target.");
        print_common_summary(
            &output_document.name,
            self.output.as_ref(),
            &learner,
            &mq,
            learning_elapsed,
        );

        Ok(())
    }
}

fn learner_config(
    basis_minimization: BasisMinimizationArg,
    basis_mip_gap: Option<f32>,
    basis_time_limit_secs: Option<f64>,
) -> Result<BasisMinimization, String> {
    if !matches!(basis_minimization, BasisMinimizationArg::ApproxMilp) {
        if basis_mip_gap.is_some() {
            return Err("--basis-mip-gap requires --basis-minimization approx-milp".to_string());
        }
        if basis_time_limit_secs.is_some() {
            return Err(
                "--basis-time-limit-secs requires --basis-minimization approx-milp".to_string(),
            );
        }
    }

    if !cfg!(feature = "milp")
        && matches!(
            basis_minimization,
            BasisMinimizationArg::ExactMilp | BasisMinimizationArg::ApproxMilp
        )
    {
        return Err(format!(
            "--basis-minimization {} requires rebuilding learn-arta-cli with --features milp",
            basis_minimization.as_str()
        ));
    }

    let relative_gap = basis_mip_gap.unwrap_or(ApproxMilpConfig::DEFAULT_RELATIVE_GAP);
    if relative_gap.is_nan() || !relative_gap.is_finite() || relative_gap.is_sign_negative() {
        return Err("basis-mip-gap must be a finite, non-negative number".to_string());
    }

    let time_limit_secs =
        basis_time_limit_secs.unwrap_or(ApproxMilpConfig::DEFAULT_TIME_LIMIT_SECS);
    if !time_limit_secs.is_finite() || time_limit_secs < 0.0 {
        return Err("basis-time-limit-secs must be a finite, non-negative number".to_string());
    }

    Ok(match basis_minimization {
        BasisMinimizationArg::Greedy => BasisMinimization::Greedy,
        BasisMinimizationArg::ExactMilp => BasisMinimization::ExactMilp,
        BasisMinimizationArg::ApproxMilp => BasisMinimization::ApproxMilp(ApproxMilpConfig {
            relative_gap,
            time_limit: Duration::from_secs_f64(time_limit_secs),
        }),
    })
}

fn output_name(document: &ParsedArtaJson, requested: Option<&str>) -> String {
    if let Some(name) = requested {
        return name.to_string();
    }

    if document.name.is_empty() {
        "hypothesis".to_string()
    } else {
        format!("{}-hypothesis", document.name)
    }
}

fn write_output_document(
    document: &ParsedArtaJson,
    output: Option<&PathBuf>,
) -> Result<(), CliError> {
    let json = to_arta_json_document_string(document)?;

    if let Some(path) = output {
        fs::write(path, json).map_err(|source| CliError::write_file(path.clone(), source))?;
    } else {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(json.as_bytes())
            .map_err(CliError::WriteStdout)?;
        stdout.write_all(b"\n").map_err(CliError::WriteStdout)?;
        stdout.flush().map_err(CliError::WriteStdout)?;
    }

    Ok(())
}

fn print_common_summary(
    output_name: &str,
    output: Option<&PathBuf>,
    learner: &ActiveArtaLearner<String>,
    mq: &CachingMembershipOracle<ArtaMembershipOracle<String>>,
    learning_elapsed: Duration,
) {
    let (membership_queries_with_caching, membership_queries_without_caching) =
        membership_query_counts(mq);
    let observation_table = &learner.state().observation_table;
    let observation_table_rows = observation_table.sample_prefixes().len();
    let observation_table_columns = observation_table.experiment_suffixes().len();

    info!("output name: {output_name}");
    match output {
        Some(path) => info!("output: {}", path.display()),
        None => info!("output: stdout"),
    }
    info!(
        "Number of Equivalence queries: {}",
        learner.state().hypothesis_iterations
    );
    info!("Number of Membership queries (with caching): {membership_queries_with_caching}");
    info!("Number of Membership queries (without caching): {membership_queries_without_caching}");
    info!("Number of Observation table rows: {observation_table_rows}");
    info!("Number of Observation table columns: {observation_table_columns}");
    info!(
        "Execution Time of Learning: {}",
        format_duration(learning_elapsed)
    );
}

fn membership_query_counts<O>(mq: &CachingMembershipOracle<O>) -> (usize, usize)
where
    O: MembershipOracle,
{
    let membership_queries_with_caching = mq.cache_misses();
    let membership_queries_without_caching = mq.cache_hits().saturating_add(mq.cache_misses());
    (
        membership_queries_with_caching,
        membership_queries_without_caching,
    )
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs_f64();
    if seconds >= 1.0 {
        return format!("{seconds:.3}s");
    }

    let milliseconds = seconds * 1_000.0;
    if milliseconds >= 1.0 {
        return format!("{milliseconds:.3}ms");
    }

    let microseconds = seconds * 1_000_000.0;
    if microseconds >= 1.0 {
        return format!("{microseconds:.3}us");
    }

    format!("{}ns", duration.as_nanos())
}

fn log_learning_started(mode: &str, basis_minimization: BasisMinimizationArg) {
    info!(
        "{mode} learning started. basis minimization: {}.",
        basis_minimization.as_str()
    );
}

fn log_target_json_file(input_json: &Path) {
    info!("target json file: {}", input_json.display());
}

fn log_equivalence_query_started(index: usize, state_count: usize) {
    info!("equivalence query #{index} started. hypothesis states: {state_count}.");
}

fn log_observation_table_dimensions(index: usize, row_count: usize, column_count: usize) {
    debug!(
        "observation table before equivalence query #{index}: rows={row_count}, columns={column_count}"
    );
}

fn log_equivalence_query_result<A, D>(index: usize, counterexample: Option<&TimedWord<A, D>>)
where
    A: std::fmt::Display,
    D: std::fmt::Display,
{
    match counterexample {
        Some(counterexample) => {
            info!("equivalence query #{index} returned counterexample: {counterexample}");
        }
        None => info!("equivalence query #{index} returned no counterexample."),
    }
}

fn log_hypothesis_json(
    eq_index: usize,
    output_name: &str,
    sigma: &[String],
    hypothesis: &Arta<String, DagStateFormula>,
) -> Result<(), learn_arta_core::ArtaJsonError> {
    if !log_enabled!(Level::Trace) {
        return Ok(());
    }

    let json = to_arta_json_document_string(&ParsedArtaJson {
        name: output_name.to_string(),
        sigma: sigma.to_vec(),
        arta: hypothesis.clone(),
    })?;
    trace!("hypothesis ARTA before equivalence query #{eq_index}:\n{json}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, convert::Infallible, rc::Rc};

    use learn_arta_core::DelayRep;

    use super::*;

    struct CountingOracle {
        calls: Rc<Cell<usize>>,
    }

    impl MembershipOracle for CountingOracle {
        type Symbol = char;
        type Error = Infallible;

        fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
            Ok(w.len().is_multiple_of(2))
        }
    }

    #[test]
    fn membership_query_counts_treat_cache_misses_as_with_caching_total() {
        let calls = Rc::new(Cell::new(0));
        let inner = CountingOracle {
            calls: Rc::clone(&calls),
        };
        let mut mq = CachingMembershipOracle::new(inner);
        let word = TimedWord::from_vec(vec![('a', DelayRep::from_integer(0))]);

        assert!(mq.query(&word).is_ok());
        assert!(mq.query(&word).is_ok());

        let (with_caching, without_caching) = membership_query_counts(&mq);

        assert_eq!(calls.get(), 1);
        assert_eq!(mq.cache_hits(), 1);
        assert_eq!(mq.cache_misses(), 1);
        assert_eq!(with_caching, 1);
        assert_eq!(without_caching, 2);
    }

    #[test]
    #[cfg(feature = "milp")]
    fn learner_config_uses_conservative_approx_milp_defaults() {
        let config = learner_config(BasisMinimizationArg::ApproxMilp, None, None)
            .expect("approx-milp config should use defaults");

        assert_eq!(
            config,
            BasisMinimization::ApproxMilp(ApproxMilpConfig::default())
        );
    }

    #[test]
    fn learner_config_rejects_approx_flags_without_approx_strategy() {
        let error = learner_config(BasisMinimizationArg::ExactMilp, Some(0.1), None)
            .expect_err("approx-only flags must be rejected for non-approx strategies");

        assert!(error.contains("--basis-mip-gap requires --basis-minimization approx-milp"));
    }

    #[test]
    #[cfg(feature = "milp")]
    fn learner_config_rejects_invalid_approx_gap_values() {
        let error = learner_config(BasisMinimizationArg::ApproxMilp, Some(f32::NAN), Some(1.0))
            .expect_err("NaN gaps must be rejected");

        assert!(error.contains("basis-mip-gap must be a finite, non-negative number"));
    }

    #[test]
    #[cfg(feature = "milp")]
    fn learner_config_rejects_invalid_approx_time_limit_values() {
        let error = learner_config(BasisMinimizationArg::ApproxMilp, Some(0.1), Some(-1.0))
            .expect_err("negative time limits must be rejected");

        assert!(error.contains("basis-time-limit-secs must be a finite, non-negative number"));
    }

    #[test]
    #[cfg(not(feature = "milp"))]
    fn learner_config_rejects_exact_milp_without_feature() {
        let error = learner_config(BasisMinimizationArg::ExactMilp, None, None)
            .expect_err("exact MILP should be unavailable without the feature");

        assert!(error.contains(
            "--basis-minimization exact-milp requires rebuilding learn-arta-cli with --features milp"
        ));
    }

    #[test]
    #[cfg(not(feature = "milp"))]
    fn learner_config_rejects_approximate_milp_without_feature() {
        let error = learner_config(BasisMinimizationArg::ApproxMilp, None, None)
            .expect_err("approximate MILP should be unavailable without the feature");

        assert!(error.contains(
            "--basis-minimization approx-milp requires rebuilding learn-arta-cli with --features milp"
        ));
    }
}
