// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Observation table with closure invariants for the learner.
//!
//! The table stores a Boolean function `T : PrefixSamples × SuffixExperiments -> {⊤, ⊥}` where:
//! - prefix samples are maintained prefix-closed,
//! - suffix experiments are maintained suffix-closed,
//! - each cell `T(prefix_sample, experiment_suffix)` is filled by a membership query
//!   on `prefix_sample.concat(experiment_suffix)`.
//!
//! `ObservationTable::new()` seeds both sets with the empty word.
//! The initial cell for `(empty word, empty word)` is filled lazily on the first closure-maintaining
//! insertion method (`insert_sample_prefixes` or
//! `insert_experiment_suffixes`).

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use learn_arta_core::{collect_timed_letters, time::DelayRep, timed_word::TimedWord};
use learn_arta_traits::MembershipOracle;
use thiserror::Error;

use crate::rowvec::{RowVec, RowVecError};

/// Errors produced by observation-table operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TableError {
    /// Length mismatch between structures that must agree.
    #[error("length mismatch: expected {expected}, found {found}")]
    LengthMismatch {
        /// Expected common row or column count.
        expected: usize,
        /// Encountered row or column count.
        found: usize,
    },
    /// Index is outside valid bounds.
    #[error("index out of bounds: index {index}, len {len}")]
    IndexOutOfBounds {
        /// Requested index.
        index: usize,
        /// Valid upper bound length.
        len: usize,
    },
    /// Generic invariant violation for internal consistency checks.
    #[error("invariant violation: {0}")]
    InvariantViolation(&'static str),
}

/// Errors produced while populating table cells via membership queries.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TableQueryError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    /// Delegated membership query failed.
    #[error("membership query failed: {source}")]
    MembershipQuery {
        /// Wrapped oracle-specific error.
        #[source]
        source: E,
    },
    /// Internal bootstrap row is missing.
    #[error("missing empty-word row during observation-table bootstrap")]
    MissingBootstrapRow,
    /// Failed to assign the bootstrap cell value.
    #[error("failed to fill bootstrap cell T(empty word, empty word): {source}")]
    BootstrapCellUpdate {
        /// Wrapped row-update error.
        #[source]
        source: RowVecError,
    },
    /// Failed to assign a newly queried row cell.
    #[error("failed to set row value at column {column}: {source}")]
    RowCellUpdate {
        /// Column that failed to update.
        column: usize,
        /// Wrapped row-update error.
        #[source]
        source: RowVecError,
    },
    /// Internal row storage is unexpectedly missing.
    #[error("missing row at index {row_index}")]
    MissingRow {
        /// Missing row index.
        row_index: usize,
    },
}

/// Observation table `T : PrefixSamples × SuffixExperiments -> {⊤, ⊥}`.
#[derive(Debug, Clone)]
pub struct ObservationTable<A>
where
    A: Eq + Hash + Clone,
{
    sample_prefix_words: Vec<TimedWord<A>>,
    experiment_suffix_words: Vec<TimedWord<A>>,
    sample_prefix_index: HashMap<TimedWord<A>, usize>,
    experiment_suffix_index: HashMap<TimedWord<A>, usize>,
    rows: Vec<RowVec>,
    bootstrap_pending: bool,
}

impl<A> ObservationTable<A>
where
    A: Eq + Hash + Clone,
{
    /// Create a table seeded with the empty word for both prefix samples and suffix experiments.
    ///
    /// The bootstrap cell for `(empty word, empty word)` is filled lazily on
    /// the first closure insertion.
    pub fn new() -> Self {
        let empty = TimedWord::empty();
        let mut sample_prefix_index = HashMap::new();
        sample_prefix_index.insert(empty.clone(), 0);

        let mut experiment_suffix_index = HashMap::new();
        experiment_suffix_index.insert(empty.clone(), 0);

        Self {
            sample_prefix_words: vec![empty.clone()],
            experiment_suffix_words: vec![empty],
            sample_prefix_index,
            experiment_suffix_index,
            rows: vec![RowVec::bot(1)],
            bootstrap_pending: true,
        }
    }

    /// Read-only view of prefix samples.
    pub fn sample_prefixes(&self) -> &[TimedWord<A>] {
        &self.sample_prefix_words
    }

    /// Read-only view of suffix experiments.
    pub fn experiment_suffixes(&self) -> &[TimedWord<A>] {
        &self.experiment_suffix_words
    }

    /// Fallible accessor for row `T[s_idx]`.
    pub fn try_row(&self, s_idx: usize) -> Result<&RowVec, TableError> {
        self.rows.get(s_idx).ok_or(TableError::IndexOutOfBounds {
            index: s_idx,
            len: self.rows.len(),
        })
    }

    /// Access row for a sample word when present in prefix samples.
    pub fn row_of(&self, sample_word: &TimedWord<A>) -> Option<&RowVec> {
        self.sample_prefix_index
            .get(sample_word)
            .and_then(|idx| self.rows.get(*idx))
    }

    /// Return `true` iff `word` is present in prefix samples.
    pub(crate) fn contains_sample_prefix(&self, word: &TimedWord<A>) -> bool {
        self.sample_prefix_index.contains_key(word)
    }

    /// Ensure `word` and all of its prefixes are in prefix samples, and fill any new rows by MQ.
    ///
    /// Prefix insertion order is deterministic: `ε`, first letter, ..., full word.
    pub fn insert_sample_prefixes<O>(
        &mut self,
        word: TimedWord<A>,
        mq: &mut O,
    ) -> Result<(), TableQueryError<O::Error>>
    where
        O: MembershipOracle<Symbol = A>,
    {
        self.ensure_bootstrapped(mq)?;

        for prefix in word.prefixes() {
            self.insert_prefix_word(prefix, mq)?;
        }

        Ok(())
    }

    /// Ensure `word` and all of its suffixes are in suffix experiments, and fill any new columns
    /// by MQ.
    ///
    /// Suffix insertion order follows [`TimedWord::suffixes`]: full word first,
    /// empty word last.
    pub fn insert_experiment_suffixes<O>(
        &mut self,
        word: TimedWord<A>,
        mq: &mut O,
    ) -> Result<(), TableQueryError<O::Error>>
    where
        O: MembershipOracle<Symbol = A>,
    {
        self.ensure_bootstrapped(mq)?;

        for suffix in word.suffixes() {
            self.insert_suffix_word(suffix, mq)?;
        }

        Ok(())
    }

    /// Clone rows for a subset of prefix-sample indices.
    pub fn rows_of_indices(
        &self,
        idxs: impl IntoIterator<Item = usize>,
    ) -> Result<Vec<RowVec>, TableError> {
        idxs.into_iter()
            .map(|idx| self.try_row(idx).cloned())
            .collect()
    }

    /// Clone all rows for prefix samples.
    pub fn rows_of_all_sample_prefixes(&self) -> Vec<RowVec> {
        self.rows.clone()
    }

    /// Collect all timed letters appearing in prefix samples or suffix experiments.
    ///
    /// The returned [`HashSet`] is unordered. Callers that need deterministic
    /// iteration should derive their own stable ordering from the table.
    pub fn timed_letters(&self) -> HashSet<(A, DelayRep)> {
        collect_timed_letters(
            self.sample_prefix_words
                .iter()
                .chain(self.experiment_suffix_words.iter()),
        )
    }

    /// Collect timed letters in deterministic first-occurrence order.
    ///
    /// The scan order is all prefix samples in insertion order, followed by all
    /// suffix experiments in insertion order. Each timed letter appears at most
    /// once in the returned vector.
    pub(crate) fn ordered_timed_letters(&self) -> Vec<(A, DelayRep)> {
        let mut seen = HashSet::new();
        let mut ordered = Vec::new();

        for word in self
            .sample_prefix_words
            .iter()
            .chain(self.experiment_suffix_words.iter())
        {
            for letter in word.iter() {
                let timed_letter = letter.clone();
                if seen.insert(timed_letter.clone()) {
                    ordered.push(timed_letter);
                }
            }
        }

        ordered
    }

    /// Collect timed letters from suffix experiments in deterministic
    /// first-occurrence order.
    ///
    /// The scan order is the suffix experiments in insertion order, scanning
    /// each word left-to-right. Each timed letter appears at most once in the
    /// returned vector.
    pub(crate) fn ordered_experiment_timed_letters(&self) -> Vec<(A, DelayRep)> {
        let mut seen = HashSet::new();
        let mut ordered = Vec::new();

        for word in &self.experiment_suffix_words {
            for letter in word.iter() {
                let timed_letter = letter.clone();
                if seen.insert(timed_letter.clone()) {
                    ordered.push(timed_letter);
                }
            }
        }

        ordered
    }

    /// Return `true` iff prefix samples are prefix-closed.
    pub fn is_prefix_closed(&self) -> bool {
        self.sample_prefix_words.iter().all(|word| {
            word.prefixes()
                .into_iter()
                .all(|prefix| self.sample_prefix_index.contains_key(&prefix))
        })
    }

    /// Return `true` iff suffix experiments are suffix-closed.
    pub fn is_suffix_closed(&self) -> bool {
        self.experiment_suffix_words.iter().all(|word| {
            word.suffixes()
                .into_iter()
                .all(|suffix| self.experiment_suffix_index.contains_key(&suffix))
        })
    }

    /// Validate shape/index/closure invariants.
    ///
    /// This checks row widths, index maps, and prefix/suffix closure.
    pub fn validate_invariants(&self) -> Result<(), TableError> {
        if self.sample_prefix_words.len() != self.rows.len() {
            return Err(TableError::LengthMismatch {
                expected: self.sample_prefix_words.len(),
                found: self.rows.len(),
            });
        }

        for row in &self.rows {
            if row.len() != self.experiment_suffix_words.len() {
                return Err(TableError::LengthMismatch {
                    expected: self.experiment_suffix_words.len(),
                    found: row.len(),
                });
            }
        }

        if self.sample_prefix_index.len() != self.sample_prefix_words.len() {
            return Err(TableError::LengthMismatch {
                expected: self.sample_prefix_words.len(),
                found: self.sample_prefix_index.len(),
            });
        }

        if self.experiment_suffix_index.len() != self.experiment_suffix_words.len() {
            return Err(TableError::LengthMismatch {
                expected: self.experiment_suffix_words.len(),
                found: self.experiment_suffix_index.len(),
            });
        }

        for (idx, word) in self.sample_prefix_words.iter().enumerate() {
            if self.sample_prefix_index.get(word).copied() != Some(idx) {
                return Err(TableError::InvariantViolation(
                    "sample_prefix_index does not match order of sample prefixes",
                ));
            }
        }

        for (idx, word) in self.experiment_suffix_words.iter().enumerate() {
            if self.experiment_suffix_index.get(word).copied() != Some(idx) {
                return Err(TableError::InvariantViolation(
                    "experiment_suffix_index does not match order of experiment suffixes",
                ));
            }
        }

        if !self.is_prefix_closed() {
            return Err(TableError::InvariantViolation(
                "sample prefixes are not prefix-closed",
            ));
        }

        if !self.is_suffix_closed() {
            return Err(TableError::InvariantViolation(
                "experiment suffixes are not suffix-closed",
            ));
        }

        Ok(())
    }

    fn ensure_bootstrapped<O>(&mut self, mq: &mut O) -> Result<(), TableQueryError<O::Error>>
    where
        O: MembershipOracle<Symbol = A>,
    {
        if !self.bootstrap_pending {
            return Ok(());
        }

        let row = self
            .rows
            .get_mut(0)
            .ok_or(TableQueryError::MissingBootstrapRow)?;
        let query_word = self.sample_prefix_words[0].concat(&self.experiment_suffix_words[0]);
        let value = mq
            .query(&query_word)
            .map_err(|source| TableQueryError::MembershipQuery { source })?;
        row.set(0, value)
            .map_err(|source| TableQueryError::BootstrapCellUpdate { source })?;
        self.bootstrap_pending = false;

        Ok(())
    }

    fn insert_prefix_word<O>(
        &mut self,
        word: TimedWord<A>,
        mq: &mut O,
    ) -> Result<(), TableQueryError<O::Error>>
    where
        O: MembershipOracle<Symbol = A>,
    {
        if self.sample_prefix_index.contains_key(&word) {
            return Ok(());
        }

        let mut row = RowVec::bot(self.experiment_suffix_words.len());
        for (col_idx, suffix) in self.experiment_suffix_words.iter().enumerate() {
            let query_word = word.concat(suffix);
            let value = mq
                .query(&query_word)
                .map_err(|source| TableQueryError::MembershipQuery { source })?;
            row.set(col_idx, value)
                .map_err(|source| TableQueryError::RowCellUpdate {
                    column: col_idx,
                    source,
                })?;
        }

        let idx = self.sample_prefix_words.len();
        self.sample_prefix_words.push(word.clone());
        self.sample_prefix_index.insert(word, idx);
        self.rows.push(row);

        Ok(())
    }

    fn insert_suffix_word<O>(
        &mut self,
        word: TimedWord<A>,
        mq: &mut O,
    ) -> Result<(), TableQueryError<O::Error>>
    where
        O: MembershipOracle<Symbol = A>,
    {
        if self.experiment_suffix_index.contains_key(&word) {
            return Ok(());
        }

        let col_idx = self.experiment_suffix_words.len();
        self.experiment_suffix_words.push(word.clone());
        self.experiment_suffix_index.insert(word.clone(), col_idx);

        for (row_idx, prefix) in self.sample_prefix_words.iter().enumerate() {
            let query_word = prefix.concat(&word);
            let value = mq
                .query(&query_word)
                .map_err(|source| TableQueryError::MembershipQuery { source })?;
            let row = self
                .rows
                .get_mut(row_idx)
                .ok_or(TableQueryError::MissingRow { row_index: row_idx })?;
            row.push_bit(value);
        }

        Ok(())
    }
}

impl<A> Default for ObservationTable<A>
where
    A: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        error::Error,
        fmt::{self, Display, Formatter},
    };

    use learn_arta_core::DelayRep;

    use super::*;

    #[derive(Default)]
    struct MockMembershipOracle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestOracleError;

    impl Display for TestOracleError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("test oracle error")
        }
    }

    impl Error for TestOracleError {}

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
    fn ensure_in_s_makes_prefix_closed_set() {
        let mut table = ObservationTable::new();
        let mut mq = MockMembershipOracle;

        let word = timed_word(&[('a', 2), ('b', 3), ('c', 4)]);
        table.insert_sample_prefixes(word.clone(), &mut mq).unwrap();

        let expected = word.prefixes();
        assert_eq!(table.sample_prefixes(), expected.as_slice());
        assert_eq!(table.sample_prefixes().len(), 4);
        assert_eq!(
            table.sample_prefixes().iter().collect::<HashSet<_>>().len(),
            table.sample_prefixes().len()
        );
        assert!(table.is_prefix_closed());
        assert!(table.validate_invariants().is_ok());
    }

    #[test]
    fn ensure_in_e_makes_suffix_closed_set() {
        let mut table = ObservationTable::new();
        let mut mq = MockMembershipOracle;

        let word = timed_word(&[('a', 2), ('b', 3), ('c', 4)]);
        table
            .insert_experiment_suffixes(word.clone(), &mut mq)
            .unwrap();

        let mut expected = vec![TimedWord::empty()];
        expected.extend(
            word.suffixes()
                .into_iter()
                .filter(|suffix| !suffix.is_empty()),
        );

        assert_eq!(table.experiment_suffixes(), expected.as_slice());
        assert_eq!(table.experiment_suffixes().len(), 4);
        assert_eq!(
            table
                .experiment_suffixes()
                .iter()
                .collect::<HashSet<_>>()
                .len(),
            table.experiment_suffixes().len()
        );
        assert!(table.is_suffix_closed());
        assert!(table.validate_invariants().is_ok());
    }

    #[test]
    fn table_cells_are_mq_filled_for_s_concat_e() {
        let mut table = ObservationTable::new();
        let mut mq = MockMembershipOracle;

        let s_word = timed_word(&[('a', 2), ('b', 3), ('c', 4)]);
        let e_word = timed_word(&[('b', 1), ('a', 2), ('b', 5)]);

        table.insert_sample_prefixes(s_word, &mut mq).unwrap();
        table.insert_experiment_suffixes(e_word, &mut mq).unwrap();

        for row in table.rows_of_all_sample_prefixes() {
            assert_eq!(row.len(), table.experiment_suffixes().len());
        }

        for (row_idx, prefix) in table.sample_prefixes().iter().enumerate() {
            let row = table.try_row(row_idx).unwrap();
            for (col_idx, suffix) in table.experiment_suffixes().iter().enumerate() {
                let expected = prefix.concat(suffix).len().is_multiple_of(2);
                assert_eq!(row.get(col_idx), Some(expected));
            }
        }

        assert!(table.validate_invariants().is_ok());
    }

    #[test]
    fn duplicate_inserts_are_idempotent_and_order_stable() {
        let s_word = timed_word(&[('a', 2), ('b', 3), ('c', 4)]);
        let e_word = timed_word(&[('b', 1), ('a', 2), ('b', 5)]);

        let mut table_a = ObservationTable::new();
        let mut mq_a = MockMembershipOracle;
        table_a
            .insert_sample_prefixes(s_word.clone(), &mut mq_a)
            .unwrap();
        table_a
            .insert_experiment_suffixes(e_word.clone(), &mut mq_a)
            .unwrap();
        let s_len_before = table_a.sample_prefixes().len();
        let e_len_before = table_a.experiment_suffixes().len();
        table_a
            .insert_sample_prefixes(s_word.clone(), &mut mq_a)
            .unwrap();
        table_a
            .insert_experiment_suffixes(e_word.clone(), &mut mq_a)
            .unwrap();
        assert_eq!(table_a.sample_prefixes().len(), s_len_before);
        assert_eq!(table_a.experiment_suffixes().len(), e_len_before);

        let mut table_b = ObservationTable::new();
        let mut mq_b = MockMembershipOracle;
        table_b
            .insert_experiment_suffixes(e_word.clone(), &mut mq_b)
            .unwrap();
        table_b
            .insert_sample_prefixes(s_word.clone(), &mut mq_b)
            .unwrap();

        let s_a: HashSet<_> = table_a.sample_prefixes().iter().cloned().collect();
        let s_b: HashSet<_> = table_b.sample_prefixes().iter().cloned().collect();
        let e_a: HashSet<_> = table_a.experiment_suffixes().iter().cloned().collect();
        let e_b: HashSet<_> = table_b.experiment_suffixes().iter().cloned().collect();

        assert_eq!(s_a, s_b);
        assert_eq!(e_a, e_b);

        assert!(
            table_a
                .rows_of_all_sample_prefixes()
                .iter()
                .all(|row| row.len() == table_a.experiment_suffixes().len())
        );
        assert!(
            table_b
                .rows_of_all_sample_prefixes()
                .iter()
                .all(|row| row.len() == table_b.experiment_suffixes().len())
        );
        assert!(table_a.validate_invariants().is_ok());
        assert!(table_b.validate_invariants().is_ok());
    }

    #[test]
    fn rows_of_indices_is_fallible_for_oob() {
        let table: ObservationTable<char> = ObservationTable::new();

        let rows = table.rows_of_indices([0]).unwrap();
        assert_eq!(rows.len(), 1);

        assert_eq!(
            table.rows_of_indices([1]),
            Err(TableError::IndexOutOfBounds { index: 1, len: 1 })
        );
    }

    #[test]
    fn contains_sample_prefix_tracks_inserted_words() {
        let mut table = ObservationTable::new();
        let mut mq = MockMembershipOracle;
        let word = timed_word(&[('a', 2), ('b', 3)]);
        let missing = timed_word(&[('c', 4)]);

        assert!(table.contains_sample_prefix(&TimedWord::empty()));
        assert!(!table.contains_sample_prefix(&word));
        assert!(!table.contains_sample_prefix(&missing));

        table.insert_sample_prefixes(word.clone(), &mut mq).unwrap();

        for prefix in word.prefixes() {
            assert!(table.contains_sample_prefix(&prefix));
        }
        assert!(!table.contains_sample_prefix(&missing));
    }

    #[test]
    fn ordered_timed_letters_uses_first_occurrence_across_table_words() {
        let mut table = ObservationTable::new();
        let mut mq = MockMembershipOracle;
        let sample_word = timed_word(&[('a', 2), ('b', 3)]);
        let experiment_word = timed_word(&[('b', 3), ('c', 4), ('a', 2)]);

        table
            .insert_sample_prefixes(sample_word.clone(), &mut mq)
            .unwrap();
        table
            .insert_experiment_suffixes(experiment_word, &mut mq)
            .unwrap();

        assert_eq!(
            table.ordered_timed_letters(),
            vec![
                ('a', DelayRep::from_half_units(2)),
                ('b', DelayRep::from_half_units(3)),
                ('c', DelayRep::from_half_units(4)),
            ]
        );
    }

    #[test]
    fn ordered_experiment_timed_letters_uses_only_suffix_experiments() {
        let mut table = ObservationTable::new();
        let mut mq = MockMembershipOracle;
        let sample_word = timed_word(&[('x', 9), ('a', 2)]);
        let experiment_word = timed_word(&[('b', 3), ('c', 4), ('b', 3), ('a', 2)]);

        table.insert_sample_prefixes(sample_word, &mut mq).unwrap();
        table
            .insert_experiment_suffixes(experiment_word, &mut mq)
            .unwrap();

        assert_eq!(
            table.ordered_experiment_timed_letters(),
            vec![
                ('b', DelayRep::from_half_units(3)),
                ('c', DelayRep::from_half_units(4)),
                ('a', DelayRep::from_half_units(2)),
            ]
        );
    }
}
