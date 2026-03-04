// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::hash_map::DefaultHasher,
    error::Error,
    fmt::{self, Display, Formatter},
    hash::{Hash, Hasher},
};

use learn_arta::{
    BasisDecomposer, BasisMinimization, BasisWords, ObservationTable, RowVec, make_cohesive_step,
};
use learn_arta_core::{DelayRep, TimedWord};
use learn_arta_traits::MembershipOracle;
use proptest::{collection::vec, prelude::*};

const MAX_WORD_LEN: usize = 6;
const MAX_ROW_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestOracleError;

impl Display for TestOracleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("test oracle error")
    }
}

impl Error for TestOracleError {}

#[derive(Debug, Default)]
struct PureMembershipOracle;

impl MembershipOracle for PureMembershipOracle {
    type Symbol = char;
    type Error = TestOracleError;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        Ok(word_predicate(w))
    }
}

fn symbol_strategy() -> impl Strategy<Value = char> {
    prop_oneof![Just('a'), Just('b'), Just('c')]
}

fn finite_delay_strategy() -> impl Strategy<Value = DelayRep> {
    (0u32..=6u32, any::<bool>()).prop_map(|(floor, is_integer)| {
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

#[derive(Clone, Debug)]
enum TableOp {
    InsertSample(TimedWord<char>),
    InsertSuffix(TimedWord<char>),
}

type CohesionSeed = (
    Vec<TimedWord<char>>,
    Vec<TimedWord<char>>,
    Vec<TimedWord<char>>,
);

fn table_op_strategy(max_len: usize) -> impl Strategy<Value = TableOp> {
    prop_oneof![
        timed_word_strategy(max_len).prop_map(TableOp::InsertSample),
        timed_word_strategy(max_len).prop_map(TableOp::InsertSuffix),
    ]
}

fn row_from_bools_with_set(values: &[bool]) -> RowVec {
    let mut row = RowVec::new(values.len());
    for (idx, value) in values.iter().copied().enumerate() {
        row.set(idx, value)
            .expect("generated index must fit within row length");
    }
    row
}

fn row_from_bools_with_push(values: &[bool]) -> RowVec {
    let mut row = RowVec::new(0);
    for value in values.iter().copied() {
        row.push_bit(value);
    }
    row
}

fn row_hash(row: &RowVec) -> u64 {
    let mut hasher = DefaultHasher::new();
    row.hash(&mut hasher);
    hasher.finish()
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

fn assert_table_matches_oracle(table: &ObservationTable<char>) {
    assert!(table.validate_invariants().is_ok());
    assert!(table.is_prefix_closed());
    assert!(table.is_suffix_closed());
    assert!(table.row_of(&TimedWord::empty()).is_some());
    assert!(table.experiment_suffixes().iter().any(TimedWord::is_empty));

    for row in table.rows_of_all_sample_prefixes() {
        assert_eq!(row.len(), table.experiment_suffixes().len());
    }

    for (row_idx, prefix) in table.sample_prefixes().iter().enumerate() {
        let row = table.try_row(row_idx).unwrap();
        for (col_idx, suffix) in table.experiment_suffixes().iter().enumerate() {
            let expected = word_predicate(&prefix.concat(suffix));
            assert_eq!(row.get(col_idx), Some(expected));
        }
    }
}

fn assert_basis_subset_of_samples(table: &ObservationTable<char>, basis_words: &BasisWords<char>) {
    let epsilon = TimedWord::empty();
    assert!(basis_words.contains(&epsilon));
    for basis_word in basis_words.iter() {
        assert!(table.row_of(basis_word).is_some());
    }
}

fn distinct_bool_vectors_strategy() -> impl Strategy<Value = (Vec<bool>, Vec<bool>)> {
    vec(any::<bool>(), 1..=MAX_ROW_LEN).prop_flat_map(|left| {
        let len = left.len();
        (Just(left.clone()), 0usize..len).prop_map(|(left, flip_idx)| {
            let mut right = left.clone();
            right[flip_idx] = !right[flip_idx];
            (left, right)
        })
    })
}

fn basis_rows_and_row_strategy() -> impl Strategy<Value = (Vec<RowVec>, RowVec)> {
    (0usize..=MAX_ROW_LEN, 1usize..=6usize).prop_flat_map(|(len, basis_size)| {
        (
            vec(vec(any::<bool>(), len), basis_size),
            vec(any::<bool>(), len),
        )
            .prop_map(|(basis, row)| {
                let basis_rows = basis
                    .into_iter()
                    .map(|values| row_from_bools_with_set(&values))
                    .collect::<Vec<_>>();
                let row = row_from_bools_with_set(&row);
                (basis_rows, row)
            })
    })
}

fn cohesion_seed_strategy() -> impl Strategy<Value = CohesionSeed> {
    (
        vec(timed_word_strategy(4), 0..=4),
        vec(timed_word_strategy(4), 0..=4),
        vec(timed_word_strategy(4), 0..=4),
    )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 96,
        .. ProptestConfig::default()
    })]

    #[test]
    fn prop_rowvec_equality_and_hash_are_content_stable(values in vec(any::<bool>(), 0..=MAX_ROW_LEN)) {
        let from_set = row_from_bools_with_set(&values);
        let from_push = row_from_bools_with_push(&values);

        prop_assert_eq!(from_set.clone(), from_push.clone());
        prop_assert_eq!(row_hash(&from_set), row_hash(&from_push));
    }

    #[test]
    fn prop_rowvec_different_content_is_not_equal((left, right) in distinct_bool_vectors_strategy()) {
        let left = row_from_bools_with_set(&left);
        let right = row_from_bools_with_push(&right);

        prop_assert_ne!(left, right);
    }

    #[test]
    fn prop_basis_decomposition_closure_and_representability((basis_rows, row) in basis_rows_and_row_strategy()) {
        let mut decomposer = BasisDecomposer::new(basis_rows).expect("generated basis must be non-empty and rectangular");
        let closure = decomposer.closure_row(&row).expect("generated row length should match");
        let closure_twice = decomposer
            .closure_row(&closure)
            .expect("closure output keeps the same row length");
        let representable = decomposer
            .representable(&row)
            .expect("generated row length should match");

        prop_assert!(row.is_subset_of(&closure).expect("same-length rows"));
        prop_assert_eq!(closure_twice, closure.clone());
        prop_assert_eq!(representable, closure == row);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    #[test]
    fn prop_observation_table_insertions_preserve_invariants(
        ops in vec(table_op_strategy(MAX_WORD_LEN), 1..=10),
    ) {
        let mut table = ObservationTable::new();
        let mut mq = PureMembershipOracle;

        for op in ops {
            match op {
                TableOp::InsertSample(word) => table
                    .insert_sample_prefixes(word, &mut mq)
                    .expect("pure MQ must not fail"),
                TableOp::InsertSuffix(word) => table
                    .insert_experiment_suffixes(word, &mut mq)
                    .expect("pure MQ must not fail"),
            }

            assert_table_matches_oracle(&table);
        }
    }

    #[test]
    fn prop_make_cohesive_step_preserves_table_and_basis_invariants(
        (sample_words, suffix_words, basis_candidates) in cohesion_seed_strategy(),
    ) {
        let mut table = ObservationTable::new();
        let mut mq = PureMembershipOracle;

        for word in sample_words {
            table
                .insert_sample_prefixes(word, &mut mq)
                .expect("pure MQ must not fail");
        }
        for word in suffix_words {
            table
                .insert_experiment_suffixes(word, &mut mq)
                .expect("pure MQ must not fail");
        }
        table
            .insert_sample_prefixes(TimedWord::empty(), &mut mq)
            .expect("pure MQ must not fail");

        let mut basis_words = BasisWords::new_with_epsilon();
        for candidate in basis_candidates {
            if table.row_of(&candidate).is_some() {
                basis_words.insert(candidate);
            }
        }

        assert_table_matches_oracle(&table);
        assert_basis_subset_of_samples(&table, &basis_words);

        for _ in 0..20 {
            let changed = make_cohesive_step(
                &mut table,
                &mut basis_words,
                &BasisMinimization::Greedy,
                &mut mq,
            )
                .expect("pure MQ must not fail");
            assert_table_matches_oracle(&table);
            assert_basis_subset_of_samples(&table, &basis_words);
            if !changed {
                break;
            }
        }
    }
}

#[test]
fn cohesion_loop_terminates_for_bounded_example() {
    let mut table = ObservationTable::new();
    let mut mq = PureMembershipOracle;
    let mut basis_words = BasisWords::new_with_epsilon();

    table
        .insert_sample_prefixes(
            TimedWord::from_vec(vec![
                ('a', DelayRep::from_integer(0)),
                ('b', DelayRep::from_floor_plus_half(1)),
            ]),
            &mut mq,
        )
        .expect("pure MQ must not fail");
    table
        .insert_experiment_suffixes(
            TimedWord::from_vec(vec![
                ('c', DelayRep::from_integer(1)),
                ('a', DelayRep::from_floor_plus_half(0)),
            ]),
            &mut mq,
        )
        .expect("pure MQ must not fail");

    let mut terminated = false;
    for _ in 0..100 {
        let changed = make_cohesive_step(
            &mut table,
            &mut basis_words,
            &BasisMinimization::Greedy,
            &mut mq,
        )
        .expect("pure MQ must not fail");
        assert_table_matches_oracle(&table);
        assert_basis_subset_of_samples(&table, &basis_words);
        if !changed {
            terminated = true;
            break;
        }
    }

    assert!(terminated);
}
