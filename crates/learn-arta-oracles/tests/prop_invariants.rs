// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    cell::Cell,
    collections::HashSet,
    error::Error,
    fmt::{self, Display, Formatter},
    rc::Rc,
};

use learn_arta_core::{
    Arta, ArtaBuilder, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId, StateFormula,
    TimedWord, time::interval::Interval,
};
use learn_arta_oracles::{CachingMembershipOracle, WhiteBoxEqOracle};
use learn_arta_traits::{EquivalenceOracle, MembershipOracle};
use proptest::{collection::vec, prelude::*};

const MAX_GUARD: u32 = 6;
const MAX_WORD_LEN: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestOracleError;

impl Display for TestOracleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("test oracle error")
    }
}

impl Error for TestOracleError {}

#[derive(Clone)]
struct CountingOracle {
    calls: Rc<Cell<usize>>,
}

impl MembershipOracle for CountingOracle {
    type Symbol = char;
    type Error = TestOracleError;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        self.calls.set(self.calls.get().saturating_add(1));
        Ok(word_predicate(w))
    }
}

fn symbol_strategy() -> impl Strategy<Value = char> {
    prop_oneof![Just('a'), Just('b'), Just('c')]
}

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
    vec((symbol_strategy(), finite_delay_strategy()), 0..=max_len).prop_map(TimedWord::from_vec)
}

fn word_predicate(word: &TimedWord<char>) -> bool {
    let len_term = word.len() as u32;
    let delay_term = word
        .iter()
        .map(|(_, delay)| delay.half_units())
        .sum::<u32>();
    let symbol_term = word
        .iter()
        .enumerate()
        .map(|(idx, (symbol, _))| (idx as u32 + 1) * symbol_weight(*symbol))
        .sum::<u32>();

    (len_term + delay_term + symbol_term).is_multiple_of(2)
}

fn symbol_weight(symbol: char) -> u32 {
    match symbol {
        'a' => 1,
        'b' => 3,
        'c' => 5,
        _ => 7,
    }
}

fn rejecting_hypothesis() -> Arta<char, DagStateFormula> {
    let mgr = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let init = DagStateFormula::var(&mgr, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder.add_location(q0);
    builder.build().expect("rejecting hypothesis should build")
}

fn all_delays_guard() -> Interval {
    Interval::from_bounds(true, 0, false, None).expect("[0, ∞) must be valid")
}

fn alternating_one_step_accepting_target() -> Arta<char, DagStateFormula> {
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
        .expect("alternating one-step target should build")
}

fn delay_representatives(max_guard_constant: u32) -> Vec<DelayRep> {
    let mut reps = Vec::with_capacity(max_guard_constant as usize * 2 + 2);
    for integer in 0..=max_guard_constant {
        reps.push(DelayRep::from_integer(integer));
    }
    for floor in 0..=max_guard_constant {
        reps.push(DelayRep::from_floor_plus_half(floor));
    }
    reps
}

fn enumerate_words(
    alphabet: &[char],
    delays: &[DelayRep],
    max_depth: usize,
) -> Vec<TimedWord<char>> {
    let mut words = vec![TimedWord::empty()];
    let mut frontier = vec![TimedWord::empty()];

    for _ in 0..max_depth {
        let mut next_frontier = Vec::new();
        for word in frontier {
            for symbol in alphabet {
                for delay in delays {
                    let mut letters = word.as_slice().to_vec();
                    letters.push((*symbol, *delay));
                    let extended = TimedWord::from_vec(letters);
                    words.push(extended.clone());
                    next_frontier.push(extended);
                }
            }
        }
        frontier = next_frontier;
    }

    words
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    #[test]
    fn prop_caching_wrapper_matches_predicate_and_counts_unique_queries(
        words in vec(timed_word_strategy(MAX_WORD_LEN), 0..=16),
    ) {
        let calls = Rc::new(Cell::new(0));
        let inner = CountingOracle {
            calls: Rc::clone(&calls),
        };
        let mut oracle = CachingMembershipOracle::new(inner);
        let distinct_words = words.iter().cloned().collect::<HashSet<_>>();

        for word in &words {
            prop_assert_eq!(oracle.query(word), Ok(word_predicate(word)));
        }

        prop_assert_eq!(oracle.cache_len(), distinct_words.len());
        prop_assert_eq!(calls.get(), distinct_words.len());
        prop_assert_eq!(oracle.cache_misses(), distinct_words.len());
        prop_assert_eq!(
            oracle.cache_hits(),
            words.len().saturating_sub(distinct_words.len())
        );
    }

}

#[test]
fn repeated_identical_query_only_calls_inner_once() {
    let calls = Rc::new(Cell::new(0));
    let inner = CountingOracle {
        calls: Rc::clone(&calls),
    };
    let mut oracle = CachingMembershipOracle::new(inner);
    let word = TimedWord::from_vec(vec![
        ('a', DelayRep::from_integer(0)),
        ('b', DelayRep::from_floor_plus_half(2)),
    ]);

    let first = oracle.query(&word).expect("counting oracle must not fail");
    let second = oracle.query(&word).expect("counting oracle must not fail");

    assert_eq!(first, second);
    assert_eq!(calls.get(), 1);
    assert_eq!(oracle.cache_hits(), 1);
    assert_eq!(oracle.cache_misses(), 1);
}

#[test]
fn whitebox_counterexample_always_witnesses_a_language_mismatch() {
    let target = alternating_one_step_accepting_target();
    let hypothesis = rejecting_hypothesis();
    let mut oracle = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target.clone(), vec!['a'])
        .expect("white-box oracle should build");

    let counterexample = oracle
        .find_counterexample(&hypothesis)
        .expect("white-box EQ should succeed")
        .expect("mismatch should yield a counterexample");

    assert_ne!(
        target.accepts(&counterexample),
        hypothesis.accepts(&counterexample)
    );
}

#[test]
fn whitebox_none_agrees_with_bounded_exhaustive_search() {
    let target = alternating_one_step_accepting_target();
    let hypothesis = target.clone();
    let alphabet = vec!['a'];
    let mut oracle =
        WhiteBoxEqOracle::<_, DagStateFormula>::try_new(target.clone(), alphabet.clone())
            .expect("white-box oracle should build");

    let counterexample = oracle
        .find_counterexample(&hypothesis)
        .expect("white-box EQ should succeed");
    assert_eq!(counterexample, None);

    let max_guard_constant = target
        .max_guard_constant()
        .max(hypothesis.max_guard_constant());
    let representatives = delay_representatives(max_guard_constant);
    for word in enumerate_words(&alphabet, &representatives, 3) {
        assert_eq!(target.accepts(&word), hypothesis.accepts(&word));
    }
}
