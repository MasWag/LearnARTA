// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::HashSet,
    error::Error,
    fmt::{self, Display, Formatter},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[cfg(not(feature = "milp"))]
use learn_arta::CohesionStepError;
use learn_arta::{
    ActiveArtaLearner, ApproxMilpConfig, BasisMinimization, BasisMinimizationError, BasisMinimizer,
    BasisReductionPhase, LearnError,
};
use learn_arta_core::{Arta, DagStateFormula, TimeError, TimedWord, try_normalize_word_half};
use learn_arta_traits::{EquivalenceOracle, MembershipOracle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestOracleError;

impl Display for TestOracleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("test oracle error")
    }
}

impl Error for TestOracleError {}

#[derive(Debug, Default)]
struct CountingMembershipOracle {
    query_count: usize,
}

impl MembershipOracle for CountingMembershipOracle {
    type Symbol = char;
    type Error = TestOracleError;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        self.query_count = self.query_count.saturating_add(1);
        Ok(w.len().is_multiple_of(2))
    }
}

#[derive(Debug)]
struct ScriptedEquivalenceOracle {
    responses: Vec<Option<TimedWord<char, f64>>>,
    calls: usize,
}

impl ScriptedEquivalenceOracle {
    fn new(responses: Vec<Option<TimedWord<char, f64>>>) -> Self {
        Self {
            responses,
            calls: 0,
        }
    }
}

impl EquivalenceOracle for ScriptedEquivalenceOracle {
    type Symbol = char;
    type CounterexampleDelay = f64;
    type Formula = DagStateFormula;
    type Error = TestOracleError;

    fn find_counterexample(
        &mut self,
        _hyp: &Arta<Self::Symbol, Self::Formula>,
    ) -> Result<Option<TimedWord<Self::Symbol, Self::CounterexampleDelay>>, Self::Error> {
        let response = self.responses.get(self.calls).cloned().unwrap_or(None);
        self.calls = self.calls.saturating_add(1);
        Ok(response)
    }
}

fn raw_timed_word(letters: &[(char, f64)]) -> TimedWord<char, f64> {
    TimedWord::from_vec(letters.to_vec())
}

#[derive(Debug)]
struct CountingCustomMinimizer {
    calls: Arc<AtomicUsize>,
}

impl BasisMinimizer<char> for CountingCustomMinimizer {
    fn phase(&self) -> BasisReductionPhase {
        BasisReductionPhase::AfterAdditiveRepairs
    }

    fn minimize_basis(
        &self,
        _table: &learn_arta::ObservationTable<char>,
        _basis_words: &learn_arta::BasisWords<char>,
    ) -> Result<Option<Vec<TimedWord<char>>>, BasisMinimizationError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }
}

#[test]
fn learner_happy_path_returns_hypothesis_when_eq_accepts_immediately() {
    let mut learner = ActiveArtaLearner::<char>::new();
    let mut mq = CountingMembershipOracle::default();
    let mut eq = ScriptedEquivalenceOracle::new(vec![None]);

    let hypothesis = learner.learn(&mut mq, &mut eq).unwrap();

    assert!(!hypothesis.locations().is_empty());
    assert_eq!(eq.calls, 1);
    assert_eq!(learner.state().refinement_rounds, 0);
    assert_eq!(learner.state().hypothesis_iterations, 1);
}

#[test]
fn learner_stepwise_refinement_normalizes_and_adds_suffixes() {
    let counterexample = raw_timed_word(&[('a', 1.2), ('b', 2.0)]);
    let normalized_counterexample =
        try_normalize_word_half(&counterexample).expect("counterexample should normalize");

    let mut learner = ActiveArtaLearner::<char>::new();
    let initial_suffixes: HashSet<_> = learner
        .state()
        .observation_table
        .experiment_suffixes()
        .iter()
        .cloned()
        .collect();

    let expected_new_suffixes: HashSet<_> = normalized_counterexample
        .suffixes()
        .into_iter()
        .filter(|suffix| !initial_suffixes.contains(suffix))
        .collect();

    let mut mq = CountingMembershipOracle::default();
    let first_hypothesis = learner.build_hypothesis(&mut mq).unwrap();
    let queries_after_first_hypothesis = mq.query_count;

    assert!(!first_hypothesis.locations().is_empty());
    assert_eq!(learner.state().hypothesis_iterations, 1);
    assert_eq!(learner.state().refinement_rounds, 0);

    learner
        .refine_with_counterexample(&mut mq, &counterexample)
        .unwrap();

    let final_suffixes = learner
        .state()
        .observation_table
        .experiment_suffixes()
        .len();

    assert!(
        final_suffixes >= initial_suffixes.len() + expected_new_suffixes.len(),
        "expected at least {} suffixes, found {}",
        initial_suffixes.len() + expected_new_suffixes.len(),
        final_suffixes
    );
    assert_eq!(learner.state().hypothesis_iterations, 1);
    assert_eq!(learner.state().refinement_rounds, 1);
    assert!(
        mq.query_count > queries_after_first_hypothesis,
        "expected MQ calls to increase during counterexample refinement"
    );

    let second_hypothesis = learner.build_hypothesis(&mut mq).unwrap();

    assert!(!second_hypothesis.locations().is_empty());
    assert_eq!(learner.state().hypothesis_iterations, 2);
    assert_eq!(learner.state().refinement_rounds, 1);
}

#[test]
fn learner_builds_after_single_letter_counterexample_refinement() {
    let counterexample = raw_timed_word(&[('a', 3.0)]);
    let normalized_counterexample =
        try_normalize_word_half(&counterexample).expect("counterexample should normalize");

    let mut learner = ActiveArtaLearner::<char>::new();
    let mut mq = CountingMembershipOracle::default();

    let first_hypothesis = learner.build_hypothesis(&mut mq).unwrap();
    assert!(!first_hypothesis.locations().is_empty());

    learner
        .refine_with_counterexample(&mut mq, &counterexample)
        .unwrap();

    let second_hypothesis = learner.build_hypothesis(&mut mq).unwrap();

    assert!(!second_hypothesis.locations().is_empty());
    assert_eq!(learner.state().hypothesis_iterations, 2);
    assert_eq!(learner.state().refinement_rounds, 1);
    assert!(
        learner
            .state()
            .observation_table
            .sample_prefixes()
            .iter()
            .any(|word| word == &normalized_counterexample)
    );
}

#[test]
fn learner_multiple_refinements_terminates_without_panics() {
    let cex1 = raw_timed_word(&[('a', 0.7), ('b', 1.0)]);
    let cex2 = raw_timed_word(&[('b', 1.9)]);
    let mut learner = ActiveArtaLearner::<char>::new();
    let initial_suffix_len = learner
        .state()
        .observation_table
        .experiment_suffixes()
        .len();

    let mut mq = CountingMembershipOracle::default();
    let mut eq = ScriptedEquivalenceOracle::new(vec![Some(cex1), Some(cex2), None]);

    let hypothesis = learner.learn(&mut mq, &mut eq).unwrap();

    assert!(!hypothesis.locations().is_empty());
    assert_eq!(eq.calls, 3);
    assert_eq!(learner.state().refinement_rounds, 2);
    assert_eq!(learner.state().hypothesis_iterations, 3);
    assert!(
        learner
            .state()
            .observation_table
            .experiment_suffixes()
            .len()
            >= initial_suffix_len
    );
}

#[test]
fn learner_reports_counterexample_normalization_errors() {
    let mut learner = ActiveArtaLearner::<char>::new();
    let mut mq = CountingMembershipOracle::default();

    let error = learner
        .refine_with_counterexample(&mut mq, &raw_timed_word(&[('a', -0.1)]))
        .expect_err("invalid raw counterexample must fail");

    assert!(matches!(
        error,
        LearnError::CounterexampleNormalization {
            source: TimeError::Negative(_)
        }
    ));
}

#[cfg(feature = "milp")]
#[test]
fn learner_builds_with_exact_milp_basis_minimization() {
    let mut learner = ActiveArtaLearner::<char>::with_minimizer(BasisMinimization::ExactMilp);
    let mut mq = CountingMembershipOracle::default();

    let hypothesis = learner.build_hypothesis(&mut mq).unwrap();

    assert!(!hypothesis.locations().is_empty());
}

#[test]
fn learner_uses_feature_appropriate_default_basis_minimization() {
    let mut learner = ActiveArtaLearner::<char>::default();
    let mut mq = CountingMembershipOracle::default();

    #[cfg(feature = "milp")]
    assert!(learner.build_hypothesis(&mut mq).is_ok());

    #[cfg(not(feature = "milp"))]
    assert!(learner.build_hypothesis(&mut mq).is_ok());
}

#[cfg(feature = "milp")]
#[test]
fn learner_builds_with_approx_milp_basis_minimization() {
    let mut learner = ActiveArtaLearner::<char>::with_minimizer(BasisMinimization::ApproxMilp(
        ApproxMilpConfig::default(),
    ));
    let mut mq = CountingMembershipOracle::default();

    let hypothesis = learner.build_hypothesis(&mut mq).unwrap();

    assert!(!hypothesis.locations().is_empty());
}

#[cfg(not(feature = "milp"))]
#[test]
fn learner_reports_exact_milp_backend_unavailable_without_feature() {
    let mut learner = ActiveArtaLearner::<char>::with_minimizer(BasisMinimization::ExactMilp);
    let mut mq = CountingMembershipOracle::default();

    let error = learner
        .build_hypothesis(&mut mq)
        .expect_err("exact MILP should be unavailable without the feature");

    assert!(matches!(
        error,
        LearnError::CohesionRepair {
            source: CohesionStepError::BasisMinimization(
                BasisMinimizationError::MilpBackendUnavailable {
                    strategy: "exact-milp"
                }
            )
        }
    ));
}

#[cfg(not(feature = "milp"))]
#[test]
fn learner_reports_approximate_milp_backend_unavailable_without_feature() {
    let mut learner = ActiveArtaLearner::<char>::with_minimizer(BasisMinimization::ApproxMilp(
        ApproxMilpConfig::default(),
    ));
    let mut mq = CountingMembershipOracle::default();

    let error = learner
        .build_hypothesis(&mut mq)
        .expect_err("approximate MILP should be unavailable without the feature");

    assert!(matches!(
        error,
        LearnError::CohesionRepair {
            source: CohesionStepError::BasisMinimization(
                BasisMinimizationError::MilpBackendUnavailable {
                    strategy: "approx-milp"
                }
            )
        }
    ));
}

#[test]
fn learner_with_custom_basis_minimizer_reports_custom_and_invokes_it() {
    let calls = Arc::new(AtomicUsize::new(0));
    let custom_minimizer = CountingCustomMinimizer {
        calls: Arc::clone(&calls),
    };
    let mut learner = ActiveArtaLearner::<char>::with_minimizer(custom_minimizer);
    let mut mq = CountingMembershipOracle::default();

    let hypothesis = learner.build_hypothesis(&mut mq).unwrap();

    assert!(!hypothesis.locations().is_empty());
    assert!(calls.load(Ordering::Relaxed) > 0);
}
