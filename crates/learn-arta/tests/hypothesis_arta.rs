// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

use learn_arta::{
    AfaStateId, BasisWords, ObservationTable, build_from_cohesive_table,
    convert_basis_formula_to_dag_state_formula, evidence_state_to_location_id,
};
use learn_arta_core::{DagStateFormulaManager, DelayRep, TimedWord};
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
struct EvenLengthOracle;

impl MembershipOracle for EvenLengthOracle {
    type Symbol = char;
    type Error = TestOracleError;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        Ok(w.len().is_multiple_of(2))
    }
}

#[derive(Debug, Default)]
struct AlwaysTrueOracle;

impl MembershipOracle for AlwaysTrueOracle {
    type Symbol = char;
    type Error = TestOracleError;

    fn query(&mut self, _w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        Ok(true)
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
fn to_hypothesis_arta_preserves_evidence_transitions_on_phi() {
    let sigma_a = ('a', DelayRep::from_half_units(1));
    let sigma_b = ('b', DelayRep::from_half_units(2));
    let a_word = TimedWord::from_vec(vec![sigma_a]);
    let b_word = TimedWord::from_vec(vec![sigma_b]);
    let bb_word = TimedWord::from_vec(vec![sigma_b, sigma_b]);
    let ba_word = TimedWord::from_vec(vec![sigma_b, sigma_a]);

    let mut mq = EvenLengthOracle;
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

    let evidence = build_from_cohesive_table(&table, &basis_words).unwrap();
    let mgr = DagStateFormulaManager::new();
    let hypothesis = evidence.to_hypothesis_arta(&mgr).unwrap();

    for state in evidence.states() {
        let source = evidence_state_to_location_id(state);
        for sigma in evidence.alphabet() {
            let expected_basis = evidence.transition(state, sigma).unwrap();
            let expected = convert_basis_formula_to_dag_state_formula(expected_basis, &mgr);
            let actual = hypothesis.step_location(source.clone(), &sigma.0, sigma.1);
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn to_hypothesis_arta_compresses_consecutive_equal_targets() {
    let mut mq = AlwaysTrueOracle;
    let mut table: ObservationTable<char> = ObservationTable::new();
    table
        .insert_sample_prefixes(timed_word(&[('a', 1)]), &mut mq)
        .unwrap();
    table
        .insert_sample_prefixes(timed_word(&[('a', 3)]), &mut mq)
        .unwrap();
    table
        .insert_sample_prefixes(timed_word(&[('a', 5)]), &mut mq)
        .unwrap();

    let basis_words = BasisWords::new_with_epsilon();
    let evidence = build_from_cohesive_table(&table, &basis_words).unwrap();
    let mgr = DagStateFormulaManager::new();
    let hypothesis = evidence.to_hypothesis_arta(&mgr).unwrap();

    let raw_delay_count = evidence
        .alphabet()
        .iter()
        .filter(|(symbol, _)| *symbol == 'a')
        .count();
    let source = evidence_state_to_location_id(AfaStateId(0));
    let transitions = hypothesis.outgoing(&source, &'a').unwrap();

    assert!(transitions.len() < raw_delay_count);
    assert_eq!(transitions.len(), 1);
}
