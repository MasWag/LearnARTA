// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Cohesion checks and one-step repairs for observation tables.
//!
//! This module applies cohesion checks in the following order:
//! 1) basis-closedness (`Rows(PrefixSamples)` representable by basis rows)
//! 2) basis minimization according to the active minimizer phase
//! 3) evidence-closedness
//! 4) distinctness

use std::hash::Hash;

use learn_arta_core::TimedWord;
use learn_arta_traits::MembershipOracle;
use thiserror::Error;

use crate::{
    BasisDecomposer, ObservationTable,
    basis::{BasisMinimizationError, BasisMinimizer, BasisReductionPhase},
    decomposition::DecompositionError,
    observation_table::TableQueryError,
};

/// Ordered basis words used by cohesion checks.
///
/// Insertion order is preserved and duplicates are suppressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasisWords<A> {
    elems: Vec<TimedWord<A>>,
}

impl<A> BasisWords<A>
where
    A: Eq + Hash + Clone,
{
    /// Construct basis words initialized to the empty word only.
    pub fn new_with_epsilon() -> Self {
        Self {
            elems: vec![TimedWord::empty()],
        }
    }

    /// Iterate basis words in deterministic insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &TimedWord<A>> {
        self.elems.iter()
    }

    /// Return `true` iff `word` is present in basis words.
    ///
    /// Runs in **O(n)** time (linear scan over insertion-ordered elements).
    pub fn contains(&self, word: &TimedWord<A>) -> bool {
        self.elems.iter().any(|w| w == word)
    }

    /// Insert `word` if absent. Returns `true` when inserted.
    ///
    /// Calls [`Self::contains`] first, so also runs in **O(n)** time.
    pub fn insert(&mut self, word: TimedWord<A>) -> bool {
        if self.contains(&word) {
            return false;
        }
        self.elems.push(word);
        true
    }

    /// Remove `word` if present. Returns `true` when removed.
    ///
    /// Runs in **O(n)** time (linear scan followed by a `Vec` remove).
    pub fn remove(&mut self, word: &TimedWord<A>) -> bool {
        if let Some(idx) = self.elems.iter().position(|w| w == word) {
            self.elems.remove(idx);
            return true;
        }
        false
    }

    /// Number of basis words.
    pub fn len(&self) -> usize {
        self.elems.len()
    }

    /// Returns `true` iff there are no basis words.
    pub fn is_empty(&self) -> bool {
        self.elems.is_empty()
    }

    pub(crate) fn replace_with(&mut self, words: Vec<TimedWord<A>>) -> bool {
        if self.elems == words {
            return false;
        }
        self.elems = words;
        true
    }
}

/// A single cohesion repair action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CohesionFix<A> {
    /// Repair basis-closedness by adding a witness sample to basis words.
    AddBasisWord(TimedWord<A>),
    /// Repair minimality by removing a redundant basis word.
    RemoveBasisWord(TimedWord<A>),
    /// Repair evidence-closedness/distinctness by adding a sample prefix.
    AddSamplePrefix(TimedWord<A>),
}

/// Errors produced while checking cohesion conditions.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CohesionCheckError<A> {
    /// Required row is unexpectedly missing from sample prefixes.
    #[error("missing row for a required word")]
    MissingRow {
        /// Word whose row was expected to be present in the observation table.
        word: TimedWord<A>,
    },
    /// Row/decomposition length mismatch.
    #[error("length mismatch: expected {expected}, found {found}")]
    LengthMismatch {
        /// Expected common row width.
        expected: usize,
        /// Encountered row width.
        found: usize,
    },
    /// Attempted to remove epsilon from basis words.
    #[error("attempted to remove epsilon from basis words")]
    AttemptedToRemoveEpsilon,
    /// Basis words are empty, so decomposition-based checks cannot proceed.
    #[error("basis words are empty")]
    EmptyBasisWords,
    /// Unexpected decomposition issue.
    #[error("decomposition error: {0}")]
    Decomposition(String),
}

/// Errors produced while applying a cohesion step.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CohesionStepError<A, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    /// Cohesion condition check failed.
    #[error(transparent)]
    Check(#[from] CohesionCheckError<A>),
    /// Observation-table population failed while applying a fix.
    #[error(transparent)]
    TableQuery(#[from] TableQueryError<E>),
    /// Basis minimization failed while selecting a new basis.
    #[error(transparent)]
    BasisMinimization(#[from] BasisMinimizationError),
}

/// Return the first applicable cohesion fix in the standard greedy check order.
///
/// This helper preserves the repository's original greedy witness-selection
/// order and does not consult custom [`BasisMinimizer`] implementations.
pub fn next_cohesion_fix<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Result<Option<CohesionFix<A>>, CohesionCheckError<A>>
where
    A: Eq + Hash + Clone,
{
    if let Some(w) = checked_find_not_basis_closed(table, basis_words)? {
        return Ok(Some(CohesionFix::AddBasisWord(w)));
    }
    if let Some(w) = checked_find_redundant_basis_word(table, basis_words)? {
        return Ok(Some(CohesionFix::RemoveBasisWord(w)));
    }
    if let Some(w) = find_not_evidence_closed(table, basis_words) {
        return Ok(Some(CohesionFix::AddSamplePrefix(w)));
    }
    if let Some(w) = find_not_distinct(table, basis_words) {
        return Ok(Some(CohesionFix::AddSamplePrefix(w)));
    }
    Ok(None)
}

/// Public witness finder for non basis-closedness.
///
/// Returns `None` when no witness exists or when an internal consistency error
/// occurs. Use [`next_cohesion_fix`] to receive structured errors.
pub fn find_not_basis_closed<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Option<TimedWord<A>>
where
    A: Eq + Hash + Clone,
{
    checked_find_not_basis_closed(table, basis_words)
        .ok()
        .flatten()
}

/// Public witness finder for a redundant basis word.
///
/// Returns `None` when no witness exists or when an internal consistency error
/// occurs. Use [`next_cohesion_fix`] to receive structured errors.
pub fn find_redundant_basis_word<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Option<TimedWord<A>>
where
    A: Eq + Hash + Clone,
{
    checked_find_redundant_basis_word(table, basis_words)
        .ok()
        .flatten()
}

/// Public witness finder for non evidence-closedness.
///
/// Returns a missing one-letter sample prefix obtained by appending a timed
/// letter from the experiment suffixes to a basis word.
pub fn find_not_evidence_closed<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Option<TimedWord<A>>
where
    A: Eq + Hash + Clone,
{
    for basis_word in basis_words.iter() {
        for evidence_letter in table.ordered_experiment_timed_letters() {
            let witness = basis_word.append_letter(evidence_letter);
            if !table.contains_sample_prefix(&witness) {
                return Some(witness);
            }
        }
    }
    None
}

/// Public witness finder for non distinctness.
///
/// Returns a missing one-letter extension of some basis word when the same
/// extension exists for at least one sample prefix.
pub fn find_not_distinct<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Option<TimedWord<A>>
where
    A: Eq + Hash + Clone,
{
    let ordered_letters = table.ordered_timed_letters();
    for sigma in ordered_letters {
        let has_sample_successor = table.sample_prefixes().iter().any(|sample_prefix| {
            let successor = sample_prefix.append_letter(sigma.clone());
            table.contains_sample_prefix(&successor)
        });
        if !has_sample_successor {
            continue;
        }

        for basis_word in basis_words.iter() {
            let candidate = basis_word.append_letter(sigma.clone());
            if !table.contains_sample_prefix(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Apply one cohesion fix to `(table, basis_words)`.
pub fn apply_fix<A, O>(
    table: &mut ObservationTable<A>,
    basis_words: &mut BasisWords<A>,
    fix: CohesionFix<A>,
    mq: &mut O,
) -> Result<(), TableQueryError<O::Error>>
where
    A: Eq + Hash + Clone,
    O: MembershipOracle<Symbol = A>,
{
    match fix {
        CohesionFix::AddBasisWord(word) => {
            if !table.contains_sample_prefix(&word) {
                table.insert_sample_prefixes(word.clone(), mq)?;
            }
            basis_words.insert(word);
        }
        CohesionFix::RemoveBasisWord(word) => {
            if word.is_empty() {
                return Ok(());
            }
            basis_words.remove(&word);
        }
        CohesionFix::AddSamplePrefix(word) => {
            table.insert_sample_prefixes(word, mq)?;
        }
    }
    Ok(())
}

/// Check and apply at most one cohesion fix using the provided basis minimizer.
///
/// Returns `true` iff sample prefixes or basis words changed.
pub fn make_cohesive_step<A, O, M>(
    table: &mut ObservationTable<A>,
    basis_words: &mut BasisWords<A>,
    basis_minimizer: &M,
    mq: &mut O,
) -> Result<bool, CohesionStepError<A, O::Error>>
where
    A: Eq + Hash + Clone,
    M: BasisMinimizer<A> + ?Sized,
    O: MembershipOracle<Symbol = A>,
{
    if let Some(word) = checked_find_not_basis_closed(table, basis_words)? {
        return apply_single_fix(table, basis_words, CohesionFix::AddBasisWord(word), mq);
    }

    if matches!(
        basis_minimizer.phase(),
        BasisReductionPhase::BeforeAdditiveRepairs
    ) && let Some(new_basis_words) = basis_minimizer.minimize_basis(table, basis_words)?
    {
        let changed = basis_words.replace_with(new_basis_words);
        return Ok(changed);
    }

    if let Some(word) = find_not_evidence_closed(table, basis_words) {
        return apply_single_fix(table, basis_words, CohesionFix::AddSamplePrefix(word), mq);
    }

    if let Some(word) = find_not_distinct(table, basis_words) {
        return apply_single_fix(table, basis_words, CohesionFix::AddSamplePrefix(word), mq);
    }

    if matches!(
        basis_minimizer.phase(),
        BasisReductionPhase::AfterAdditiveRepairs
    ) && let Some(new_basis_words) = basis_minimizer.minimize_basis(table, basis_words)?
    {
        let changed = basis_words.replace_with(new_basis_words);
        return Ok(changed);
    }

    Ok(false)
}

fn apply_single_fix<A, O>(
    table: &mut ObservationTable<A>,
    basis_words: &mut BasisWords<A>,
    fix: CohesionFix<A>,
    mq: &mut O,
) -> Result<bool, CohesionStepError<A, O::Error>>
where
    A: Eq + Hash + Clone,
    O: MembershipOracle<Symbol = A>,
{
    let sample_count_before = table.sample_prefixes().len();
    let basis_count_before = basis_words.len();
    apply_fix(table, basis_words, fix, mq)?;
    let changed = table.sample_prefixes().len() != sample_count_before
        || basis_words.len() != basis_count_before;
    Ok(changed)
}

fn checked_find_not_basis_closed<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Result<Option<TimedWord<A>>, CohesionCheckError<A>>
where
    A: Eq + Hash + Clone,
{
    let mut decomposer = build_decomposer_from_words(table, basis_words.iter().cloned())?;
    for sample_prefix in table.sample_prefixes() {
        let row = table
            .row_of(sample_prefix)
            .ok_or_else(|| CohesionCheckError::MissingRow {
                word: sample_prefix.clone(),
            })?;
        if !decomposer
            .representable(row)
            .map_err(map_decomposition_error)?
        {
            return Ok(Some(sample_prefix.clone()));
        }
    }
    Ok(None)
}

fn checked_find_redundant_basis_word<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Result<Option<TimedWord<A>>, CohesionCheckError<A>>
where
    A: Eq + Hash + Clone,
{
    for candidate_basis_word in basis_words.iter() {
        if candidate_basis_word.is_empty() {
            continue;
        }

        let other_basis_words = basis_words
            .iter()
            .filter(|word| *word != candidate_basis_word)
            .cloned()
            .collect::<Vec<_>>();
        if other_basis_words.is_empty() {
            continue;
        }

        let mut decomposer = build_decomposer_from_words(table, other_basis_words)?;
        let mut all_samples_representable = true;
        for sample_prefix in table.sample_prefixes() {
            let row =
                table
                    .row_of(sample_prefix)
                    .ok_or_else(|| CohesionCheckError::MissingRow {
                        word: sample_prefix.clone(),
                    })?;
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

fn build_decomposer_from_words<A>(
    table: &ObservationTable<A>,
    words: impl IntoIterator<Item = TimedWord<A>>,
) -> Result<BasisDecomposer, CohesionCheckError<A>>
where
    A: Eq + Hash + Clone,
{
    let mut rows = Vec::new();
    for word in words {
        let row = table
            .row_of(&word)
            .ok_or_else(|| CohesionCheckError::MissingRow { word })?;
        rows.push(row.clone());
    }

    if rows.is_empty() {
        return Err(CohesionCheckError::EmptyBasisWords);
    }

    BasisDecomposer::new(rows).map_err(map_decomposition_error)
}

fn map_decomposition_error<A>(err: DecompositionError) -> CohesionCheckError<A> {
    match err {
        DecompositionError::LengthMismatch { expected, found } => {
            CohesionCheckError::LengthMismatch { expected, found }
        }
        DecompositionError::EmptyBasis => CohesionCheckError::EmptyBasisWords,
        DecompositionError::IndexOutOfBounds { index, len } => CohesionCheckError::Decomposition(
            format!("index out of bounds: index {index}, len {len}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        error::Error,
        fmt::{self, Display, Formatter},
    };

    use super::*;
    use learn_arta_core::DelayRep;

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

    #[test]
    fn exact_strategy_defers_basis_minimization_until_additive_repairs_are_quiet() {
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

        let basis_minimizer = crate::basis::BasisMinimization::ExactMilp;
        let changed =
            make_cohesive_step(&mut table, &mut basis_words, &basis_minimizer, &mut mq).unwrap();

        assert!(changed);
        assert!(table.contains_sample_prefix(&expected));
        assert!(basis_words.contains(&TimedWord::empty()));
        assert!(basis_words.contains(&p_word));
        assert_eq!(basis_words.len(), 2);
    }

    #[test]
    fn approximate_strategy_defers_basis_minimization_until_additive_repairs_are_quiet() {
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

        let basis_minimizer =
            crate::basis::BasisMinimization::ApproxMilp(crate::basis::ApproxMilpConfig::default());
        let changed =
            make_cohesive_step(&mut table, &mut basis_words, &basis_minimizer, &mut mq).unwrap();

        assert!(changed);
        assert!(table.contains_sample_prefix(&expected));
        assert!(basis_words.contains(&TimedWord::empty()));
        assert!(basis_words.contains(&p_word));
        assert_eq!(basis_words.len(), 2);
    }
}
