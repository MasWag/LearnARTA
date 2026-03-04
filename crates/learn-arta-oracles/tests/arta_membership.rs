// SPDX-License-Identifier: Apache-2.0 OR MIT

use learn_arta_core::{
    Arta, ArtaBuilder, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId, StateFormula,
    TimedWord, time::interval::Interval,
};
use learn_arta_oracles::ArtaMembershipOracle;
use learn_arta_traits::MembershipOracle;
use proptest::{collection::vec, prelude::*};

const MAX_GUARD: u32 = 4;
const MAX_WORD_LEN: usize = 6;

fn finite_delay_strategy() -> impl Strategy<Value = DelayRep> {
    (0u32..=MAX_GUARD, any::<bool>()).prop_map(|(floor, is_integer)| {
        if is_integer {
            DelayRep::from_integer(floor)
        } else {
            DelayRep::from_floor_plus_half(floor)
        }
    })
}

fn timed_word_strategy(max_len: usize) -> impl Strategy<Value = TimedWord<char>> {
    vec(
        (prop_oneof![Just('a'), Just('b')], finite_delay_strategy()),
        0..=max_len,
    )
    .prop_map(TimedWord::from_vec)
}

fn all_delays_guard() -> Interval {
    Interval::from_bounds(true, 0, false, None).expect("[0, ∞) must be valid")
}

fn rejecting_target() -> Arta<char, DagStateFormula> {
    let manager = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let init = DagStateFormula::var(&manager, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder.add_location(q0);
    builder.build().expect("rejecting target should build")
}

fn empty_word_accepting_target() -> Arta<char, DagStateFormula> {
    let manager = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let init = DagStateFormula::var(&manager, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder.add_location(q0.clone()).add_accepting(q0);
    builder
        .build()
        .expect("empty-word accepting target should build")
}

fn one_step_accepting_target() -> Arta<char, DagStateFormula> {
    let manager = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let q1 = LocationId::new("q1");
    let init = DagStateFormula::var(&manager, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder
        .add_location(q0.clone())
        .add_location(q1.clone())
        .add_accepting(q1.clone())
        .add_transition(
            q0,
            'a',
            all_delays_guard(),
            DagStateFormula::var(&manager, q1),
        );
    builder.build().expect("one-step target should build")
}

#[test]
fn empty_word_query_matches_accepts() {
    let target = empty_word_accepting_target();
    let mut oracle = ArtaMembershipOracle::new(target.clone());
    let word = TimedWord::empty();

    assert_eq!(oracle.query(&word), Ok(target.accepts(&word)));
    assert!(target.accepts(&word));
}

#[test]
fn non_empty_query_matches_accepts() {
    let target = one_step_accepting_target();
    let mut oracle = ArtaMembershipOracle::new(target.clone());
    let word = TimedWord::from_vec(vec![('a', DelayRep::from_floor_plus_half(1))]);

    assert_eq!(oracle.query(&word), Ok(target.accepts(&word)));
    assert!(target.accepts(&word));
}

#[test]
fn accepting_and_rejecting_targets_behave_as_expected() {
    let accepting = empty_word_accepting_target();
    let rejecting = rejecting_target();
    let word = TimedWord::empty();

    let mut accepting_oracle = ArtaMembershipOracle::new(accepting);
    let mut rejecting_oracle = ArtaMembershipOracle::new(rejecting);

    assert_eq!(accepting_oracle.query(&word), Ok(true));
    assert_eq!(rejecting_oracle.query(&word), Ok(false));
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    #[test]
    fn prop_membership_oracle_matches_target_acceptance(word in timed_word_strategy(MAX_WORD_LEN)) {
        let target = one_step_accepting_target();
        let mut oracle = ArtaMembershipOracle::new(target.clone());

        prop_assert_eq!(oracle.query(&word), Ok(target.accepts(&word)));
    }
}
