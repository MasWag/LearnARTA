// SPDX-License-Identifier: Apache-2.0 OR MIT

use learn_arta_core::{
    Arta, ArtaBuilder, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId, StateFormula,
    TimedWord, time::interval::Interval,
};
use learn_arta_oracles::{WhiteBoxEqOracle, WhiteBoxEqOracleError};
use learn_arta_traits::EquivalenceOracle;

fn all_delays_guard() -> Interval {
    Interval::from_bounds(true, 0, false, None).expect("[0, ∞) must be valid")
}

fn guarded_accepting_target(guard: Interval) -> Arta<char, DagStateFormula> {
    let mgr = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let q1 = LocationId::new("q1");
    let init = DagStateFormula::var(&mgr, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder
        .add_location(q0.clone())
        .add_location(q1.clone())
        .add_accepting(q1.clone())
        .add_transition(q0, 'a', guard, DagStateFormula::var(&mgr, q1));
    builder.build().expect("guarded target should build")
}

fn guarded_accepting_target_with_guards(guards: &[Interval]) -> Arta<char, DagStateFormula> {
    let mgr = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let q1 = LocationId::new("q1");
    let init = DagStateFormula::var(&mgr, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder
        .add_location(q0.clone())
        .add_location(q1.clone())
        .add_accepting(q1.clone());

    for guard in guards {
        builder.add_transition(
            q0.clone(),
            'a',
            guard.clone(),
            DagStateFormula::var(&mgr, q1.clone()),
        );
    }

    builder.build().expect("multi-guard target should build")
}

fn rejecting_hypothesis() -> Arta<char, DagStateFormula> {
    let mgr = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let init = DagStateFormula::var(&mgr, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder.add_location(q0);
    builder.build().expect("rejecting hypothesis should build")
}

fn accepting_target() -> Arta<char, DagStateFormula> {
    let mgr = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let init = DagStateFormula::var(&mgr, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder.add_location(q0.clone()).add_accepting(q0);
    builder.build().expect("accepting target should build")
}

fn alternating_initial_target() -> Arta<char, DagStateFormula> {
    let mgr = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let q1 = LocationId::new("q1");
    let init = DagStateFormula::or(
        &mgr,
        [
            DagStateFormula::var(&mgr, q0.clone()),
            DagStateFormula::var(&mgr, q1.clone()),
        ],
    );
    let mut builder = ArtaBuilder::new(init);
    builder
        .add_location(q0)
        .add_location(q1.clone())
        .add_accepting(q1);
    builder
        .build()
        .expect("alternating initial target should build")
}

fn alternating_transition_target() -> Arta<char, DagStateFormula> {
    let mgr = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let q1 = LocationId::new("q1");
    let q2 = LocationId::new("q2");
    let init = DagStateFormula::var(&mgr, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder
        .add_location(q0.clone())
        .add_location(q1.clone())
        .add_location(q2.clone())
        .add_accepting(q1.clone())
        .add_transition(
            q0,
            'a',
            all_delays_guard(),
            DagStateFormula::or(
                &mgr,
                [
                    DagStateFormula::var(&mgr, q1),
                    DagStateFormula::var(&mgr, q2),
                ],
            ),
        );
    builder
        .build()
        .expect("alternating transition target should build")
}

fn syntactically_growing_target() -> Arta<char, DagStateFormula> {
    let mgr = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let q1 = LocationId::new("q1");
    let init = DagStateFormula::var(&mgr, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder
        .add_location(q0.clone())
        .add_location(q1.clone())
        .add_transition(
            q0.clone(),
            'a',
            all_delays_guard(),
            DagStateFormula::or(
                &mgr,
                [
                    DagStateFormula::var(&mgr, q0.clone()),
                    DagStateFormula::and(
                        &mgr,
                        [
                            DagStateFormula::var(&mgr, q0),
                            DagStateFormula::var(&mgr, q1.clone()),
                        ],
                    ),
                ],
            ),
        )
        .add_transition(
            q1.clone(),
            'a',
            all_delays_guard(),
            DagStateFormula::var(&mgr, q1),
        );
    builder
        .build()
        .expect("syntactically growing target should build")
}

fn semantically_equivalent_stable_hypothesis() -> Arta<char, DagStateFormula> {
    let mgr = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let init = DagStateFormula::var(&mgr, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder.add_location(q0.clone()).add_transition(
        q0.clone(),
        'a',
        all_delays_guard(),
        DagStateFormula::var(&mgr, q0),
    );
    builder.build().expect("stable hypothesis should build")
}

#[test]
fn accepts_alternating_initial_target_and_finds_empty_word_mismatch() {
    let target = alternating_initial_target();
    let hypothesis = rejecting_hypothesis();
    let mut oracle = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target, vec!['a'])
        .expect("oracle construction should succeed for alternating initial target");

    let counterexample = oracle
        .find_counterexample(&hypothesis)
        .expect("equivalence query should succeed");

    assert_eq!(counterexample, Some(TimedWord::empty()));
}

#[test]
fn accepts_alternating_transition_target_and_finds_one_step_counterexample() {
    let target = alternating_transition_target();
    let hypothesis = rejecting_hypothesis();
    let mut oracle = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target.clone(), vec!['a'])
        .expect("oracle construction should succeed for alternating transition target");

    let counterexample = oracle
        .find_counterexample(&hypothesis)
        .expect("equivalence query should succeed")
        .expect("one-step mismatch should produce a witness");

    assert_eq!(counterexample.len(), 1);
    assert_eq!(counterexample.as_slice()[0].0, 'a');
    assert_eq!(counterexample.as_slice()[0].1, DelayRep::ZERO);
    assert_ne!(
        target.accepts(&counterexample),
        hypothesis.accepts(&counterexample)
    );
}

#[test]
fn returns_none_for_equivalent_alternating_target_and_hypothesis() {
    let target = alternating_transition_target();
    let mut oracle = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target.clone(), vec!['a'])
        .expect("oracle construction should succeed");

    let counterexample = oracle
        .find_counterexample(&target)
        .expect("equivalence query should succeed");

    assert_eq!(counterexample, None);
}

#[test]
fn open_lower_boundary_prefers_larger_finite_witness() {
    let target = guarded_accepting_target(
        Interval::left_open_right_closed(0, 1).expect("(0, 1] must be valid"),
    );
    let hypothesis = rejecting_hypothesis();
    let mut oracle = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target.clone(), vec!['a'])
        .expect("oracle construction should succeed");

    let counterexample = oracle
        .find_counterexample(&hypothesis)
        .expect("equivalence query should succeed")
        .expect("open lower boundary must still find a witness");

    assert_eq!(counterexample.len(), 1);
    assert_eq!(counterexample.as_slice()[0].0, 'a');
    assert_eq!(counterexample.as_slice()[0].1, DelayRep::from_integer(1));
    assert_ne!(
        target.accepts(&counterexample),
        hypothesis.accepts(&counterexample)
    );
}

#[test]
fn closed_singleton_boundary_keeps_integer_witness() {
    let target = guarded_accepting_target(Interval::closed(1, 1).expect("[1, 1] must be valid"));
    let hypothesis = rejecting_hypothesis();
    let mut oracle = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target.clone(), vec!['a'])
        .expect("oracle construction should succeed");

    let counterexample = oracle
        .find_counterexample(&hypothesis)
        .expect("equivalence query should succeed")
        .expect("closed singleton must produce a witness");

    assert_eq!(counterexample.len(), 1);
    assert_eq!(counterexample.as_slice()[0].0, 'a');
    assert_eq!(counterexample.as_slice()[0].1, DelayRep::from_integer(1));
    assert_ne!(
        target.accepts(&counterexample),
        hypothesis.accepts(&counterexample)
    );
}

#[test]
fn infinite_upper_bound_uses_finite_tail_representative() {
    let target = guarded_accepting_target(
        Interval::from_bounds(true, 2, false, None).expect("[2, ∞) must be valid"),
    );
    let hypothesis = rejecting_hypothesis();
    let mut oracle = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target.clone(), vec!['a'])
        .expect("oracle construction should succeed");

    let counterexample = oracle
        .find_counterexample(&hypothesis)
        .expect("equivalence query should succeed")
        .expect("infinite guard must produce a witness");

    assert_eq!(counterexample.len(), 1);
    assert_eq!(counterexample.as_slice()[0].0, 'a');
    assert_eq!(counterexample.as_slice()[0].1, DelayRep::from_integer(2));
    assert_ne!(
        target.accepts(&counterexample),
        hypothesis.accepts(&counterexample)
    );
}

#[test]
fn prefers_larger_finite_delay_class_before_smaller_one() {
    let target = guarded_accepting_target_with_guards(&[
        Interval::closed(0, 0).expect("[0, 0] must be valid"),
        Interval::closed(2, 2).expect("[2, 2] must be valid"),
    ]);
    let hypothesis = rejecting_hypothesis();
    let mut oracle = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target.clone(), vec!['a'])
        .expect("oracle construction should succeed");

    let counterexample = oracle
        .find_counterexample(&hypothesis)
        .expect("equivalence query should succeed")
        .expect("descending finite preference must produce a witness");

    assert_eq!(counterexample.len(), 1);
    assert_eq!(counterexample.as_slice()[0].0, 'a');
    assert_eq!(counterexample.as_slice()[0].1, DelayRep::from_integer(2));
    assert_ne!(
        target.accepts(&counterexample),
        hypothesis.accepts(&counterexample)
    );
}

#[test]
fn rejects_empty_alphabet() {
    let error =
        WhiteBoxEqOracle::<_, DagStateFormula>::try_new(accepting_target(), Vec::<char>::new())
            .expect_err("empty alphabet must be rejected");

    assert_eq!(error, WhiteBoxEqOracleError::EmptyAlphabet);
}

#[test]
fn semantic_dedup_terminates_on_equivalent_formula_growth() {
    let target = syntactically_growing_target();
    let hypothesis = semantically_equivalent_stable_hypothesis();
    let mut oracle = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target.clone(), vec!['a'])
        .expect("oracle construction should succeed");

    let counterexample = oracle
        .find_counterexample(&hypothesis)
        .expect("equivalence query should succeed");

    assert_eq!(counterexample, None);

    for length in 0..=4 {
        let word = TimedWord::from_vec(vec![('a', DelayRep::ZERO); length]);
        assert_eq!(target.accepts(&word), hypothesis.accepts(&word));
    }
}
