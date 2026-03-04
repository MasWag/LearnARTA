// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Caching membership oracle.
//!
//! Only successful query results are cached. If the wrapped oracle returns an
//! error, that error is forwarded and no cache entry is inserted.

use std::collections::HashMap;

use learn_arta_core::timed_word::TimedWord;
use learn_arta_traits::MembershipOracle;

/// A membership oracle that caches successful query results.
pub struct CachingMembershipOracle<O>
where
    O: MembershipOracle,
{
    inner: O,
    // TimedWord already stores DelayRep values, so cache keys are canonical and hashable.
    cache: HashMap<TimedWord<O::Symbol>, bool>,
    cache_hits: usize,
    cache_misses: usize,
}

impl<O> CachingMembershipOracle<O>
where
    O: MembershipOracle,
{
    /// Create a new caching oracle wrapping `inner`.
    ///
    /// Repeated queries for the same timed word will be answered from the cache without
    /// delegating to `inner` again. Errors from `inner` are not cached.
    ///
    /// # Arguments
    ///
    /// * `inner` — the underlying [`MembershipOracle`] to delegate cache misses to.
    pub fn new(inner: O) -> Self {
        Self {
            inner,
            cache: HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    /// The number of cached entries.
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Clear all cached answers while preserving hit/miss counters.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Number of cache hits served so far.
    pub fn cache_hits(&self) -> usize {
        self.cache_hits
    }

    /// Number of cache misses delegated to the inner oracle so far.
    pub fn cache_misses(&self) -> usize {
        self.cache_misses
    }
}

impl<O> MembershipOracle for CachingMembershipOracle<O>
where
    O: MembershipOracle,
{
    type Symbol = O::Symbol;
    type Error = O::Error;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        if let Some(&cached) = self.cache.get(w) {
            self.cache_hits = self.cache_hits.saturating_add(1);
            return Ok(cached);
        }

        self.cache_misses = self.cache_misses.saturating_add(1);
        let result = self.inner.query(w)?;
        self.cache.insert(w.clone(), result);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        collections::HashSet,
        error::Error,
        fmt::{self, Display, Formatter},
        rc::Rc,
    };

    use learn_arta_core::DelayRep;
    use proptest::prelude::*;

    use super::*;

    struct CountingOracle {
        calls: Rc<Cell<usize>>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestOracleError;

    impl Display for TestOracleError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("test oracle error")
        }
    }

    impl Error for TestOracleError {}

    impl MembershipOracle for CountingOracle {
        type Symbol = char;
        type Error = TestOracleError;

        fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
            self.calls.set(self.calls.get().saturating_add(1));
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
    fn repeated_query_hits_cache() {
        let calls = Rc::new(Cell::new(0));
        let inner = CountingOracle {
            calls: Rc::clone(&calls),
        };
        let mut oracle = CachingMembershipOracle::new(inner);
        let word = timed_word(&[('a', 1), ('b', 2)]);

        let first = oracle.query(&word).unwrap();
        let second = oracle.query(&word).unwrap();

        assert_eq!(first, second);
        assert_eq!(calls.get(), 1);
        assert_eq!(oracle.cache_len(), 1);
        assert_eq!(oracle.cache_hits(), 1);
        assert_eq!(oracle.cache_misses(), 1);
    }

    #[test]
    fn different_words_are_cached_separately() {
        let calls = Rc::new(Cell::new(0));
        let inner = CountingOracle {
            calls: Rc::clone(&calls),
        };
        let mut oracle = CachingMembershipOracle::new(inner);
        let first_word = timed_word(&[('a', 1)]);
        let second_word = timed_word(&[('a', 1), ('b', 2)]);

        let first = oracle.query(&first_word).unwrap();
        let second = oracle.query(&second_word).unwrap();

        assert_ne!(first_word, second_word);
        assert_ne!(first, second);
        assert_eq!(calls.get(), 2);
        assert_eq!(oracle.cache_len(), 2);
        assert_eq!(oracle.cache_hits(), 0);
        assert_eq!(oracle.cache_misses(), 2);
    }

    fn delay_strategy() -> impl Strategy<Value = DelayRep> {
        prop_oneof![
            (0u32..=20u32).prop_map(DelayRep::from_half_units),
            Just(DelayRep::INFINITY),
        ]
    }

    fn letter_strategy() -> impl Strategy<Value = (char, DelayRep)> {
        (
            prop_oneof![Just('a'), Just('b'), Just('c')],
            delay_strategy(),
        )
    }

    fn timed_word_strategy() -> impl Strategy<Value = TimedWord<char>> {
        prop::collection::vec(letter_strategy(), 0..=6).prop_map(TimedWord::from_vec)
    }

    proptest! {
        #[test]
        fn cached_wrapper_matches_uncached_answers(words in prop::collection::vec(timed_word_strategy(), 0..=16)) {
            let calls = Rc::new(Cell::new(0));
            let inner = CountingOracle {
                calls: Rc::clone(&calls),
            };
            let mut oracle = CachingMembershipOracle::new(inner);
            let distinct_words = words.iter().cloned().collect::<HashSet<_>>();

            for word in &words {
                let cached_answer = oracle.query(word).unwrap();
                let expected_answer = word.len().is_multiple_of(2);
                prop_assert_eq!(cached_answer, expected_answer);
            }

            prop_assert_eq!(oracle.cache_len(), distinct_words.len());
            prop_assert_eq!(calls.get(), distinct_words.len());
        }
    }
}
