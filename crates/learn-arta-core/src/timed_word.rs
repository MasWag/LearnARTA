// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Timed words and helpers.
//!
//! A timed word is a finite sequence of timed letters `(a, d)`, where `a` is
//! an alphabet symbol and `d` is a delay.
//!
//! This module also provides:
//! - `collect_timed_letters(...)` for collecting `Phi(L)`: all timed letters
//!   that appear in a set of timed words.
//! - `TimedWord::prefixes()` for enumerating `pref(w)`.
//! - `TimedWord::suffixes()` for enumerating `suff(w)`.

use std::collections::HashSet;
use std::hash::Hash;

use crate::time::DelayRep;

/// A single timed letter `(a, d)` in a timed word.
pub type TimedLetter<A, D = DelayRep> = (A, D);

/// A timed word: a finite sequence of timed letters `(a, d)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TimedWord<A, D = DelayRep> {
    letters: Vec<TimedLetter<A, D>>,
}

impl<A, D> TimedWord<A, D> {
    /// Construct the empty timed word.
    ///
    /// # Examples
    ///
    /// ```
    /// use learn_arta_core::timed_word::TimedWord;
    ///
    /// let w = TimedWord::<char>::empty();
    /// assert!(w.is_empty());
    /// ```
    pub fn empty() -> Self {
        TimedWord {
            letters: Vec::new(),
        }
    }

    /// Construct a timed word from an owned vector of timed letters.
    ///
    /// The input vector order is preserved.
    pub fn from_vec(vec: Vec<TimedLetter<A, D>>) -> Self {
        TimedWord { letters: vec }
    }

    /// Return the number of timed letters in this word.
    pub fn len(&self) -> usize {
        self.letters.len()
    }

    /// Return `true` iff this timed word is empty.
    pub fn is_empty(&self) -> bool {
        self.letters.is_empty()
    }

    /// Return an iterator over timed letters.
    pub fn iter(&self) -> std::slice::Iter<'_, TimedLetter<A, D>> {
        self.letters.iter()
    }

    /// Borrow the underlying slice of timed letters.
    pub fn as_slice(&self) -> &[TimedLetter<A, D>] {
        &self.letters
    }

    /// Append one timed letter to this word.
    ///
    /// # Arguments
    ///
    /// * `letter` - the `(symbol, delay)` pair to append.
    pub fn push(&mut self, letter: TimedLetter<A, D>) {
        self.letters.push(letter);
    }

    /// Return a new timed word with one timed letter appended.
    ///
    /// This does not mutate `self`; it clones the existing letters into a new
    /// word and then appends `letter`.
    pub fn append_letter(&self, letter: TimedLetter<A, D>) -> TimedWord<A, D>
    where
        A: Clone,
        D: Clone,
    {
        let mut letters = Vec::with_capacity(self.letters.len() + 1);
        letters.extend_from_slice(&self.letters);
        letters.push(letter);
        TimedWord { letters }
    }

    /// Concatenate two timed words: `self . other`.
    ///
    /// # Arguments
    ///
    /// * `other` - the right-hand timed word.
    pub fn concat(&self, other: &TimedWord<A, D>) -> TimedWord<A, D>
    where
        A: Clone,
        D: Clone,
    {
        let mut letters = Vec::with_capacity(self.letters.len() + other.letters.len());
        letters.extend_from_slice(&self.letters);
        letters.extend_from_slice(&other.letters);
        TimedWord { letters }
    }

    /// Return all prefixes of this timed word, including the empty word.
    ///
    /// The return order is deterministic and follows increasing prefix length
    /// `k = 0..=len`:
    /// - first element is the empty word (`k = 0`)
    /// - last element is the full word (`k = len`)
    ///
    /// # Examples
    ///
    /// ```
    /// use learn_arta_core::{DelayRep, TimedWord};
    ///
    /// let w = TimedWord::from_vec(vec![
    ///     ('a', DelayRep::from_half_units(1)),
    ///     ('b', DelayRep::from_half_units(2)),
    /// ]);
    /// let prefixes = w.prefixes();
    ///
    /// assert_eq!(prefixes.len(), 3);
    /// assert!(prefixes[0].is_empty());
    /// assert_eq!(prefixes[2].as_slice(), w.as_slice());
    /// ```
    pub fn prefixes(&self) -> Vec<TimedWord<A, D>>
    where
        A: Clone,
        D: Clone,
    {
        let mut out = Vec::with_capacity(self.letters.len() + 1);
        for end in 0..=self.letters.len() {
            out.push(TimedWord::from_vec(self.letters[..end].to_vec()));
        }
        out
    }

    /// Return all suffixes of this timed word, including the empty word.
    ///
    /// The return order is deterministic and follows increasing start index
    /// `i = 0..=len`:
    /// - first element is the full word (`i = 0`)
    /// - last element is the empty word (`i = len`)
    ///
    /// # Examples
    ///
    /// ```
    /// use learn_arta_core::{DelayRep, TimedWord};
    ///
    /// let w = TimedWord::from_vec(vec![
    ///     ('a', DelayRep::from_half_units(1)),
    ///     ('b', DelayRep::from_half_units(2)),
    /// ]);
    /// let suffixes = w.suffixes();
    ///
    /// assert_eq!(suffixes.len(), 3);
    /// assert_eq!(suffixes[0].as_slice(), w.as_slice());
    /// assert!(suffixes[2].is_empty());
    /// ```
    pub fn suffixes(&self) -> Vec<TimedWord<A, D>>
    where
        A: Clone,
        D: Clone,
    {
        let mut out = Vec::with_capacity(self.letters.len() + 1);
        for start in 0..=self.letters.len() {
            out.push(TimedWord::from_vec(self.letters[start..].to_vec()));
        }
        out
    }
}

impl<A: std::fmt::Display, D: std::fmt::Display> std::fmt::Display for TimedWord<A, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, (symbol, delay)) in self.letters.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "({}, {})", symbol, delay)?;
        }
        write!(f, "]")
    }
}

/// Collect `Phi(L)`: all timed letters that appear in a set of timed words.
///
/// The result is a set, so duplicate `(symbol, delay)` pairs are collapsed.
///
/// # Arguments
///
/// * `words` - iterator over timed words.
pub fn collect_timed_letters<'a, A, D, I>(words: I) -> HashSet<(A, D)>
where
    A: Eq + Hash + Clone + 'a,
    D: Eq + Hash + Clone + 'a,
    I: IntoIterator<Item = &'a TimedWord<A, D>>,
{
    let mut seen = HashSet::new();
    for w in words {
        seen.extend(w.iter().cloned());
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn delay_strategy() -> impl Strategy<Value = DelayRep> {
        prop_oneof![
            (0u32..=20u32).prop_map(DelayRep::from_half_units),
            Just(DelayRep::INFINITY),
        ]
    }

    fn letter_strategy() -> impl Strategy<Value = TimedLetter<char>> {
        (
            prop_oneof![Just('a'), Just('b'), Just('c')],
            delay_strategy(),
        )
    }

    fn timed_word_strategy() -> impl Strategy<Value = TimedWord<char>> {
        prop::collection::vec(letter_strategy(), 0..=6).prop_map(TimedWord::from_vec)
    }

    fn timed_words_strategy() -> impl Strategy<Value = Vec<TimedWord<char>>> {
        prop::collection::vec(timed_word_strategy(), 0..=6)
    }

    fn word_from_pairs(pairs: &[(char, u32)]) -> TimedWord<char> {
        TimedWord::from_vec(
            pairs
                .iter()
                .map(|(symbol, half_units)| (*symbol, DelayRep::from_half_units(*half_units)))
                .collect(),
        )
    }

    #[test]
    fn suffixes_empty_word() {
        let w = TimedWord::<char>::empty();
        let suffixes = w.suffixes();

        assert_eq!(suffixes, vec![TimedWord::<char>::empty()]);
    }

    #[test]
    fn prefixes_empty_word() {
        let w = TimedWord::<char>::empty();
        let prefixes = w.prefixes();

        assert_eq!(prefixes, vec![TimedWord::<char>::empty()]);
    }

    #[test]
    fn prefixes_single_letter_word() {
        let w = word_from_pairs(&[('a', 3)]);
        let prefixes = w.prefixes();

        assert_eq!(prefixes, vec![TimedWord::empty(), w.clone()]);
    }

    #[test]
    fn prefixes_multi_letter_word_in_order() {
        let l1 = ('a', DelayRep::from_half_units(1));
        let l2 = ('b', DelayRep::from_half_units(2));
        let l3 = ('a', DelayRep::from_half_units(5));
        let w = TimedWord::from_vec(vec![l1, l2, l3]);

        let expected = vec![
            TimedWord::empty(),
            TimedWord::from_vec(vec![l1]),
            TimedWord::from_vec(vec![l1, l2]),
            TimedWord::from_vec(vec![l1, l2, l3]),
        ];

        assert_eq!(w.prefixes(), expected);
    }

    #[test]
    fn suffixes_single_letter_word() {
        let w = word_from_pairs(&[('a', 3)]);
        let suffixes = w.suffixes();

        assert_eq!(suffixes, vec![w.clone(), TimedWord::empty()]);
    }

    #[test]
    fn append_letter_returns_extended_copy() {
        let original = word_from_pairs(&[('a', 1), ('b', 4)]);
        let letter = ('c', DelayRep::from_half_units(7));
        let extended = original.append_letter(letter);

        assert_eq!(
            original.as_slice(),
            &[
                ('a', DelayRep::from_half_units(1)),
                ('b', DelayRep::from_half_units(4)),
            ],
        );
        assert_eq!(
            extended.as_slice(),
            &[
                ('a', DelayRep::from_half_units(1)),
                ('b', DelayRep::from_half_units(4)),
                ('c', DelayRep::from_half_units(7)),
            ],
        );
    }

    #[test]
    fn suffixes_multi_letter_word_in_order() {
        let l1 = ('a', DelayRep::from_half_units(1));
        let l2 = ('b', DelayRep::from_half_units(2));
        let l3 = ('a', DelayRep::from_half_units(5));
        let w = TimedWord::from_vec(vec![l1, l2, l3]);

        let expected = vec![
            TimedWord::from_vec(vec![l1, l2, l3]),
            TimedWord::from_vec(vec![l2, l3]),
            TimedWord::from_vec(vec![l3]),
            TimedWord::empty(),
        ];

        assert_eq!(w.suffixes(), expected);
    }

    #[test]
    fn generic_f64_word_supports_concat_prefixes_and_suffixes() {
        let left = TimedWord::from_vec(vec![('a', 1.2_f64)]);
        let right = TimedWord::from_vec(vec![('b', f64::INFINITY)]);
        let combined = left.concat(&right);

        assert_eq!(combined.as_slice(), &[('a', 1.2), ('b', f64::INFINITY)]);
        assert_eq!(combined.prefixes().len(), 3);
        assert_eq!(combined.suffixes().len(), 3);
        assert_eq!(combined.prefixes()[1].as_slice(), &[('a', 1.2)]);
        assert_eq!(combined.suffixes()[1].as_slice(), &[('b', f64::INFINITY)]);
    }

    proptest! {
        #[test]
        fn prop_every_prefix_matches_slice_and_reconstructs(w in timed_word_strategy()) {
            let prefixes = w.prefixes();
            prop_assert_eq!(prefixes.len(), w.len() + 1);

            for (end, prefix) in prefixes.iter().enumerate() {
                prop_assert_eq!(prefix.as_slice(), &w.as_slice()[..end]);

                let suffix = TimedWord::from_vec(w.as_slice()[end..].to_vec());
                let reconstructed = prefix.concat(&suffix);
                prop_assert_eq!(reconstructed.as_slice(), w.as_slice());
            }
        }

        #[test]
        fn prop_prefixes_include_empty_and_full_exactly_once(w in timed_word_strategy()) {
            let prefixes = w.prefixes();
            let full_count = prefixes.iter().filter(|prefix| *prefix == &w).count();
            let empty_count = prefixes.iter().filter(|prefix| prefix.is_empty()).count();

            prop_assert_eq!(full_count, 1);
            prop_assert_eq!(empty_count, 1);

            prop_assert!(prefixes.first().is_some_and(TimedWord::is_empty));
            prop_assert_eq!(prefixes.last(), Some(&w));
        }

        #[test]
        fn prop_every_suffix_matches_slice_and_reconstructs(w in timed_word_strategy()) {
            let suffixes = w.suffixes();
            prop_assert_eq!(suffixes.len(), w.len() + 1);

            for (start, suffix) in suffixes.iter().enumerate() {
                prop_assert_eq!(suffix.as_slice(), &w.as_slice()[start..]);

                let prefix = TimedWord::from_vec(w.as_slice()[..start].to_vec());
                let reconstructed = prefix.concat(suffix);
                prop_assert_eq!(reconstructed.as_slice(), w.as_slice());
            }
        }

        #[test]
        fn prop_suffixes_include_full_and_empty_exactly_once(w in timed_word_strategy()) {
            let suffixes = w.suffixes();
            let full_count = suffixes.iter().filter(|suffix| *suffix == &w).count();
            let empty_count = suffixes.iter().filter(|suffix| suffix.is_empty()).count();

            prop_assert_eq!(full_count, 1);
            prop_assert_eq!(empty_count, 1);

            prop_assert_eq!(suffixes.first(), Some(&w));
            prop_assert!(suffixes.last().is_some_and(TimedWord::is_empty));
        }

        #[test]
        fn prop_collect_timed_letters_matches_exact_flattened_letters(words in timed_words_strategy()) {
            let expected: HashSet<_> = words
                .iter()
                .flat_map(|word| word.iter().cloned())
                .collect();
            let actual = collect_timed_letters(words.iter());

            prop_assert_eq!(actual, expected);
        }

        #[test]
        fn prop_append_letter_matches_clone_then_push(
            w in timed_word_strategy(),
            sigma in letter_strategy()
        ) {
            let mut expected = w.clone();
            expected.push(sigma);

            let actual = w.append_letter(sigma);
            prop_assert_eq!(actual, expected);
        }
    }
}
