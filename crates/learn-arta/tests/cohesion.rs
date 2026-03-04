// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::HashSet,
    error::Error,
    fmt::{self, Display, Formatter},
    sync::atomic::{AtomicUsize, Ordering},
};

use learn_arta::{
    BasisMinimization, BasisMinimizationError, BasisMinimizer, BasisReductionPhase, BasisWords,
    CohesionFix, ObservationTable, find_not_basis_closed, find_not_distinct,
    find_not_evidence_closed, find_redundant_basis_word, make_cohesive_step, next_cohesion_fix,
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

#[derive(Debug, Clone, Default)]
struct MockMembershipOracle {
    accepted: HashSet<TimedWord<char>>,
}

impl MockMembershipOracle {
    fn with_accepted(accepted: impl IntoIterator<Item = TimedWord<char>>) -> Self {
        Self {
            accepted: accepted.into_iter().collect(),
        }
    }
}

impl MembershipOracle for MockMembershipOracle {
    type Symbol = char;
    type Error = TestOracleError;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        Ok(self.accepted.contains(w))
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

#[derive(Debug)]
struct TrackingMinimizer {
    phase: BasisReductionPhase,
    selected_basis: Option<Vec<TimedWord<char>>>,
    calls: AtomicUsize,
}

impl TrackingMinimizer {
    fn new(phase: BasisReductionPhase, selected_basis: Option<Vec<TimedWord<char>>>) -> Self {
        Self {
            phase,
            selected_basis,
            calls: AtomicUsize::new(0),
        }
    }
}

impl BasisMinimizer<char> for TrackingMinimizer {
    fn phase(&self) -> BasisReductionPhase {
        self.phase
    }

    fn minimize_basis(
        &self,
        _table: &ObservationTable<char>,
        _basis_words: &BasisWords<char>,
    ) -> Result<Option<Vec<TimedWord<char>>>, BasisMinimizationError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(self.selected_basis.clone())
    }
}

#[test]
fn p_closedness_fix_is_found_first() {
    let e1 = timed_word(&[('e', 1)]);
    let s1 = timed_word(&[('s', 2)]);
    let s1e1 = s1.concat(&e1);

    let mut mq = MockMembershipOracle::with_accepted([TimedWord::empty(), s1e1]);
    let mut table: ObservationTable<char> = ObservationTable::new();
    table.insert_experiment_suffixes(e1, &mut mq).unwrap();
    table.insert_sample_prefixes(s1.clone(), &mut mq).unwrap();

    let basis_words = BasisWords::new_with_epsilon();
    assert_eq!(
        find_not_basis_closed(&table, &basis_words),
        Some(s1.clone())
    );

    let fix = next_cohesion_fix(&table, &basis_words).unwrap();
    assert_eq!(fix, Some(CohesionFix::AddBasisWord(s1)));
}

#[test]
fn minimality_fix_removes_redundant_p() {
    let e1 = timed_word(&[('e', 1)]);
    let p1 = timed_word(&[('p', 3)]);

    let mut mq = MockMembershipOracle::with_accepted([TimedWord::empty()]);
    let mut table: ObservationTable<char> = ObservationTable::new();
    table.insert_experiment_suffixes(e1, &mut mq).unwrap();
    table.insert_sample_prefixes(p1.clone(), &mut mq).unwrap();

    let mut basis_words = BasisWords::new_with_epsilon();
    assert!(basis_words.insert(p1.clone()));

    assert_eq!(find_not_basis_closed(&table, &basis_words), None);
    assert_eq!(
        find_redundant_basis_word(&table, &basis_words),
        Some(p1.clone())
    );

    let fix = next_cohesion_fix(&table, &basis_words).unwrap();
    assert_eq!(fix, Some(CohesionFix::RemoveBasisWord(p1)));
}

#[test]
fn epsilon_is_never_selected_for_minimality_when_evidence_is_still_missing() {
    let p_word = timed_word(&[('a', 6)]);
    let expected = p_word.concat(&p_word);

    let mut mq = MockMembershipOracle::with_accepted([TimedWord::empty(), p_word.clone()]);
    let mut table: ObservationTable<char> = ObservationTable::new();
    table
        .insert_experiment_suffixes(p_word.clone(), &mut mq)
        .unwrap();
    table
        .insert_sample_prefixes(p_word.clone(), &mut mq)
        .unwrap();

    let mut basis_words = BasisWords::new_with_epsilon();
    assert!(basis_words.insert(p_word.clone()));

    assert_eq!(find_not_basis_closed(&table, &basis_words), None);
    assert_eq!(find_redundant_basis_word(&table, &basis_words), None);
    assert_eq!(
        find_not_evidence_closed(&table, &basis_words),
        Some(expected.clone())
    );
    assert_eq!(
        next_cohesion_fix(&table, &basis_words).unwrap(),
        Some(CohesionFix::AddSamplePrefix(expected.clone()))
    );

    assert!(
        make_cohesive_step(
            &mut table,
            &mut basis_words,
            &BasisMinimization::Greedy,
            &mut mq,
        )
        .unwrap()
    );
    assert!(table.sample_prefixes().iter().any(|word| word == &expected));
}

#[test]
fn evidence_closedness_fix_adds_basis_plus_evidence_letter() {
    let e = timed_word(&[('e', 3)]);

    let mut mq = MockMembershipOracle::with_accepted([TimedWord::empty()]);
    let mut table: ObservationTable<char> = ObservationTable::new();
    table
        .insert_experiment_suffixes(e.clone(), &mut mq)
        .unwrap();

    let basis_words = BasisWords::new_with_epsilon();

    assert_eq!(find_not_basis_closed(&table, &basis_words), None);
    assert_eq!(find_redundant_basis_word(&table, &basis_words), None);
    assert_eq!(
        find_not_evidence_closed(&table, &basis_words),
        Some(e.clone())
    );

    let fix = next_cohesion_fix(&table, &basis_words).unwrap();
    assert_eq!(fix, Some(CohesionFix::AddSamplePrefix(e)));
}

#[test]
fn evidence_closedness_uses_letters_from_suffixes_not_full_suffixes() {
    let ab_word = timed_word(&[('a', 1), ('b', 2)]);
    let a_word = timed_word(&[('a', 1)]);
    let b_word = timed_word(&[('b', 2)]);

    let mut mq = MockMembershipOracle::with_accepted([TimedWord::empty()]);
    let mut table: ObservationTable<char> = ObservationTable::new();
    table
        .insert_experiment_suffixes(ab_word.clone(), &mut mq)
        .unwrap();

    let basis_words = BasisWords::new_with_epsilon();

    assert_eq!(find_not_basis_closed(&table, &basis_words), None);
    assert_eq!(find_redundant_basis_word(&table, &basis_words), None);
    assert_eq!(find_not_evidence_closed(&table, &basis_words), Some(a_word));

    let first_fix = next_cohesion_fix(&table, &basis_words).unwrap();
    assert_eq!(
        first_fix,
        Some(CohesionFix::AddSamplePrefix(timed_word(&[('a', 1)])))
    );
    assert_ne!(
        first_fix,
        Some(CohesionFix::AddSamplePrefix(ab_word.clone()))
    );

    table
        .insert_sample_prefixes(timed_word(&[('a', 1)]), &mut mq)
        .unwrap();

    assert_eq!(find_not_evidence_closed(&table, &basis_words), Some(b_word));
    assert_ne!(
        find_not_evidence_closed(&table, &basis_words),
        Some(ab_word)
    );
}

#[test]
fn distinctness_fix_adds_p_concat_sigma() {
    let p_word = timed_word(&[('a', 1)]);
    let expected = p_word.concat(&p_word);

    let mut mq = MockMembershipOracle::with_accepted([p_word.clone()]);
    let mut table: ObservationTable<char> = ObservationTable::new();
    table
        .insert_sample_prefixes(p_word.clone(), &mut mq)
        .unwrap();

    let mut basis_words = BasisWords::new_with_epsilon();
    assert!(basis_words.remove(&TimedWord::empty()));
    assert!(basis_words.insert(p_word.clone()));

    assert_eq!(find_not_basis_closed(&table, &basis_words), None);
    assert_eq!(find_redundant_basis_word(&table, &basis_words), None);
    assert_eq!(find_not_evidence_closed(&table, &basis_words), None);
    assert_eq!(
        find_not_distinct(&table, &basis_words),
        Some(expected.clone())
    );

    let fix = next_cohesion_fix(&table, &basis_words).unwrap();
    assert_eq!(fix, Some(CohesionFix::AddSamplePrefix(expected)));
}

#[test]
fn make_cohesive_step_eventually_reaches_fixpoint() {
    let e = timed_word(&[('e', 5)]);

    let mut mq = MockMembershipOracle::with_accepted([TimedWord::empty()]);
    let mut table: ObservationTable<char> = ObservationTable::new();
    table.insert_experiment_suffixes(e, &mut mq).unwrap();
    let mut basis_words = BasisWords::new_with_epsilon();

    let mut saw_change = false;
    let mut terminated = false;
    for _ in 0..50 {
        let changed = make_cohesive_step(
            &mut table,
            &mut basis_words,
            &BasisMinimization::Greedy,
            &mut mq,
        )
        .unwrap();
        if changed {
            saw_change = true;
            continue;
        }
        terminated = true;
        break;
    }

    assert!(saw_change);
    assert!(terminated);
    assert_eq!(next_cohesion_fix(&table, &basis_words).unwrap(), None);
}

#[test]
fn custom_minimizer_can_run_before_additive_repairs() {
    let p_word = timed_word(&[('a', 6)]);
    let expected_missing_sample = p_word.concat(&p_word);

    let mut mq = MockMembershipOracle::with_accepted([TimedWord::empty(), p_word.clone()]);
    let mut table: ObservationTable<char> = ObservationTable::new();
    table
        .insert_experiment_suffixes(p_word.clone(), &mut mq)
        .unwrap();
    table
        .insert_sample_prefixes(p_word.clone(), &mut mq)
        .unwrap();

    let mut basis_words = BasisWords::new_with_epsilon();
    assert!(basis_words.insert(p_word.clone()));

    let custom_minimizer = TrackingMinimizer::new(
        BasisReductionPhase::BeforeAdditiveRepairs,
        Some(vec![TimedWord::empty()]),
    );

    let changed =
        make_cohesive_step(&mut table, &mut basis_words, &custom_minimizer, &mut mq).unwrap();

    assert!(changed);
    assert_eq!(custom_minimizer.calls.load(Ordering::Relaxed), 1);
    assert!(!basis_words.contains(&p_word));
    assert!(
        !table
            .sample_prefixes()
            .iter()
            .any(|word| word == &expected_missing_sample)
    );
}

#[test]
fn custom_minimizer_can_defer_until_after_additive_repairs() {
    let p_word = timed_word(&[('a', 6)]);
    let expected_sample = p_word.concat(&p_word);

    let mut mq = MockMembershipOracle::with_accepted([TimedWord::empty(), p_word.clone()]);
    let mut table: ObservationTable<char> = ObservationTable::new();
    table
        .insert_experiment_suffixes(p_word.clone(), &mut mq)
        .unwrap();
    table
        .insert_sample_prefixes(p_word.clone(), &mut mq)
        .unwrap();

    let mut basis_words = BasisWords::new_with_epsilon();
    assert!(basis_words.insert(p_word.clone()));

    let custom_minimizer = TrackingMinimizer::new(
        BasisReductionPhase::AfterAdditiveRepairs,
        Some(vec![TimedWord::empty()]),
    );

    let changed =
        make_cohesive_step(&mut table, &mut basis_words, &custom_minimizer, &mut mq).unwrap();

    assert!(changed);
    assert_eq!(custom_minimizer.calls.load(Ordering::Relaxed), 0);
    assert!(basis_words.contains(&TimedWord::empty()));
    assert!(basis_words.contains(&p_word));
    assert!(
        table
            .sample_prefixes()
            .iter()
            .any(|word| word == &expected_sample)
    );
}
