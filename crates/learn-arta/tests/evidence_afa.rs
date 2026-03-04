// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::HashSet,
    error::Error,
    fmt::{self, Display, Formatter},
};

use learn_arta::{
    AfaStateId, BasisDecomposer, BasisWords, EvidenceAfaError, ObservationTable,
    build_from_cohesive_table,
};
use learn_arta_core::{DelayRep, TimedWord};
use learn_arta_traits::MembershipOracle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestOracleError;

impl Display for TestOracleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("test oracle error")
    }
}

impl Error for TestOracleError {}

#[derive(Debug, Default)]
struct MockMembershipOracle;

impl MembershipOracle for MockMembershipOracle {
    type Symbol = char;
    type Error = TestOracleError;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        Ok(w.len().is_multiple_of(2))
    }
}

fn timed_word(letters: &[(char, u32)]) -> TimedWord<char> {
    TimedWord::from_vec(
        letters
            .iter()
            .map(|(symbol, half_units)| (*symbol, DelayRep::from_half_units(*half_units)))
            .collect(),
    )
}

#[test]
fn build_from_cohesive_table_constructs_expected_evidence_afa() {
    let sigma_a = ('a', DelayRep::from_half_units(1));
    let sigma_b = ('b', DelayRep::from_half_units(2));
    let a_word = TimedWord::from_vec(vec![sigma_a]);
    let b_word = TimedWord::from_vec(vec![sigma_b]);
    let bb_word = TimedWord::from_vec(vec![sigma_b, sigma_b]);
    let ba_word = TimedWord::from_vec(vec![sigma_b, sigma_a]);

    let mut mq = MockMembershipOracle;
    let mut table: ObservationTable<char> = ObservationTable::new();
    table
        .insert_experiment_suffixes(a_word.clone(), &mut mq)
        .unwrap();
    table
        .insert_sample_prefixes(b_word.clone(), &mut mq)
        .unwrap();
    table
        .insert_sample_prefixes(a_word.clone(), &mut mq)
        .unwrap();
    table.insert_sample_prefixes(bb_word, &mut mq).unwrap();
    table.insert_sample_prefixes(ba_word, &mut mq).unwrap();

    let mut basis_words = BasisWords::new_with_epsilon();
    assert!(basis_words.insert(b_word));

    let evidence_afa = build_from_cohesive_table(&table, &basis_words).unwrap();
    assert_eq!(
        evidence_afa.basis_rows().len(),
        evidence_afa.representatives().len()
    );
    assert!(evidence_afa.num_states() >= 2);

    let epsilon_column = table
        .experiment_suffixes()
        .iter()
        .position(TimedWord::is_empty)
        .unwrap();
    for state in evidence_afa.states() {
        let expected_accepting = evidence_afa.basis_rows()[state.0]
            .get(epsilon_column)
            .unwrap();
        assert_eq!(evidence_afa.is_accepting(state), expected_accepting);
    }

    let mut decomposer = BasisDecomposer::new(evidence_afa.basis_rows().to_vec()).unwrap();
    let epsilon_row = table.row_of(&TimedWord::empty()).unwrap();
    let expected_init = decomposer.decompose_formula(epsilon_row).unwrap();
    assert_eq!(evidence_afa.init(), &expected_init);

    for state in evidence_afa.states() {
        let representative = &evidence_afa.representatives()[state.0];
        for sigma in evidence_afa.alphabet() {
            let mut extension = representative.clone();
            extension.push(*sigma);
            let extension_row = table.row_of(&extension).unwrap();
            let expected = decomposer.decompose_formula(extension_row).unwrap();
            let actual = evidence_afa.transition(state, sigma).unwrap();
            assert_eq!(actual, &expected);
        }
    }

    let alphabet_set: HashSet<_> = evidence_afa.alphabet().iter().cloned().collect();
    assert_eq!(alphabet_set, table.timed_letters());
}

#[test]
fn build_returns_missing_distinct_extension_when_required_successor_is_absent() {
    let a_word = timed_word(&[('a', 1)]);
    let b_word = timed_word(&[('b', 2)]);

    let mut mq = MockMembershipOracle;
    let mut table: ObservationTable<char> = ObservationTable::new();
    table.insert_experiment_suffixes(a_word, &mut mq).unwrap();
    table
        .insert_sample_prefixes(b_word.clone(), &mut mq)
        .unwrap();

    let mut basis_words = BasisWords::new_with_epsilon();
    assert!(basis_words.insert(b_word));

    let err = build_from_cohesive_table(&table, &basis_words).unwrap_err();
    assert!(matches!(
        err,
        EvidenceAfaError::MissingDistinctExtension { .. }
    ));
}

#[test]
fn transition_returns_none_for_unknown_letter_or_state() {
    let sigma_a = ('a', DelayRep::from_half_units(1));
    let sigma_b = ('b', DelayRep::from_half_units(2));
    let a_word = TimedWord::from_vec(vec![sigma_a]);
    let b_word = TimedWord::from_vec(vec![sigma_b]);
    let bb_word = TimedWord::from_vec(vec![sigma_b, sigma_b]);
    let ba_word = TimedWord::from_vec(vec![sigma_b, sigma_a]);

    let mut mq = MockMembershipOracle;
    let mut table: ObservationTable<char> = ObservationTable::new();
    table
        .insert_experiment_suffixes(a_word.clone(), &mut mq)
        .unwrap();
    table
        .insert_sample_prefixes(b_word.clone(), &mut mq)
        .unwrap();
    table.insert_sample_prefixes(a_word, &mut mq).unwrap();
    table.insert_sample_prefixes(bb_word, &mut mq).unwrap();
    table.insert_sample_prefixes(ba_word, &mut mq).unwrap();

    let mut basis_words = BasisWords::new_with_epsilon();
    assert!(basis_words.insert(b_word));
    let evidence_afa = build_from_cohesive_table(&table, &basis_words).unwrap();

    assert!(
        evidence_afa
            .transition(AfaStateId(0), &('x', DelayRep::from_half_units(9)),)
            .is_none()
    );
    assert!(
        evidence_afa
            .transition(AfaStateId(evidence_afa.num_states() + 1), &sigma_a)
            .is_none()
    );
}
