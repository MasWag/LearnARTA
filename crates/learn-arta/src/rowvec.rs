// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Bitset-backed row vectors for observation tables.
//!
//! A row vector is a truth vector over suffix-experiment columns, represented as the set of indices
//! whose value is `⊤`.

use std::fmt;
use std::hash::{Hash, Hasher};

use fixedbitset::FixedBitSet;
use thiserror::Error;

/// Errors produced by [`RowVec`] operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RowVecError {
    /// Index is outside the row bounds.
    #[error("row index out of bounds: index {index}, len {len}")]
    IndexOutOfBounds {
        /// Requested bit index.
        index: usize,
        /// Row width.
        len: usize,
    },
    /// Binary row operations require equal lengths.
    #[error("row length mismatch: left {left}, right {right}")]
    LengthMismatch {
        /// Left-hand row width.
        left: usize,
        /// Right-hand row width.
        right: usize,
    },
}

/// Bitset-backed row vector.
///
/// `len` is the number of columns, and each set bit denotes `⊤`.
#[derive(Clone, Debug)]
pub struct RowVec {
    len: usize,
    bits: FixedBitSet,
}

impl RowVec {
    /// Create a new row of length `len`, initialized to all `⊥`.
    pub fn new(len: usize) -> Self {
        let mut bits = FixedBitSet::with_capacity(len);
        bits.grow(len);
        Self { len, bits }
    }

    /// Create a new row of length `len`, initialized to all `⊤`.
    pub fn top(len: usize) -> Self {
        let mut bits = FixedBitSet::with_capacity(len);
        bits.grow(len);
        bits.set_range(.., true);
        Self { len, bits }
    }

    /// Create a new row of length `len`, initialized to all `⊥`.
    pub fn bot(len: usize) -> Self {
        Self::new(len)
    }

    /// Returns the row length.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the row has no columns.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the bit value at `i`, or `None` when out of bounds.
    pub fn get(&self, i: usize) -> Option<bool> {
        if i >= self.len {
            return None;
        }
        Some(self.bits.contains(i))
    }

    /// Set the bit value at `i`.
    ///
    /// Returns an error when `i` is outside bounds.
    pub fn set(&mut self, i: usize, v: bool) -> Result<(), RowVecError> {
        if i >= self.len {
            return Err(RowVecError::IndexOutOfBounds {
                index: i,
                len: self.len,
            });
        }
        self.bits.set(i, v);
        Ok(())
    }

    /// Extend this row by one column.
    ///
    /// The new column is appended at index `len` and set to `v`.
    pub fn push_bit(&mut self, v: bool) {
        let idx = self.len;
        self.bits.grow(idx + 1);
        if v {
            self.bits.insert(idx);
        }
        self.len += 1;
    }

    /// Iterate the indices whose values are `⊤`.
    pub fn ones(&self) -> impl Iterator<Item = usize> + '_ {
        self.bits.ones().take_while(move |idx| *idx < self.len)
    }

    /// Pointwise conjunction (`∧`) with `other`.
    ///
    /// Returns an error when row lengths differ.
    pub fn and(&self, other: &Self) -> Result<Self, RowVecError> {
        self.ensure_same_len(other)?;
        let mut bits = self.bits.clone();
        bits.intersect_with(&other.bits);
        Ok(Self {
            len: self.len,
            bits,
        })
    }

    /// Pointwise disjunction (`∨`) with `other`.
    ///
    /// Returns an error when row lengths differ.
    pub fn or(&self, other: &Self) -> Result<Self, RowVecError> {
        self.ensure_same_len(other)?;
        let mut bits = self.bits.clone();
        bits.union_with(&other.bits);
        Ok(Self {
            len: self.len,
            bits,
        })
    }

    /// Returns `true` iff all `⊤` entries in `self` are also `⊤` in `other`.
    ///
    /// Returns an error when row lengths differ.
    pub fn is_subset_of(&self, other: &Self) -> Result<bool, RowVecError> {
        self.ensure_same_len(other)?;
        Ok(self.ones().all(|idx| other.bits.contains(idx)))
    }

    /// Convert this row to a boolean vector in index order.
    pub fn to_bools(&self) -> Vec<bool> {
        (0..self.len).map(|idx| self.bits.contains(idx)).collect()
    }

    fn ensure_same_len(&self, other: &Self) -> Result<(), RowVecError> {
        if self.len == other.len {
            return Ok(());
        }
        Err(RowVecError::LengthMismatch {
            left: self.len,
            right: other.len,
        })
    }
}

impl PartialEq for RowVec {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
            && (0..self.len).all(|idx| self.bits.contains(idx) == other.bits.contains(idx))
    }
}

impl Eq for RowVec {}

impl Hash for RowVec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len.hash(state);
        for idx in self.ones() {
            idx.hash(state);
        }
    }
}

impl fmt::Display for RowVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for idx in 0..self.len {
            let bit = if self.bits.contains(idx) { '1' } else { '0' };
            write!(f, "{bit}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn row_from_bools(values: &[bool]) -> RowVec {
        let mut row = RowVec::new(values.len());
        for (idx, value) in values.iter().copied().enumerate() {
            row.set(idx, value).unwrap();
        }
        row
    }

    #[test]
    fn new_has_no_ones() {
        let row = RowVec::new(5);
        assert_eq!(row.len(), 5);
        assert!(!row.is_empty());
        assert!(row.ones().next().is_none());
        assert_eq!(row.to_bools(), vec![false, false, false, false, false]);
    }

    #[test]
    fn top_has_all_ones() {
        let row = RowVec::top(5);
        assert_eq!(row.len(), 5);
        assert_eq!(row.to_bools(), vec![true, true, true, true, true]);
    }

    #[test]
    fn bot_has_no_ones() {
        let row = RowVec::bot(4);
        assert_eq!(row.len(), 4);
        assert_eq!(row.to_bools(), vec![false, false, false, false]);
    }

    #[test]
    fn set_get_works() {
        let mut row = RowVec::new(4);
        assert_eq!(row.get(0), Some(false));
        assert_eq!(row.get(3), Some(false));
        assert_eq!(row.get(4), None);

        row.set(1, true).unwrap();
        row.set(3, true).unwrap();
        row.set(1, false).unwrap();

        assert_eq!(row.get(0), Some(false));
        assert_eq!(row.get(1), Some(false));
        assert_eq!(row.get(3), Some(true));
        assert_eq!(row.to_string(), "0001");
    }

    #[test]
    fn set_out_of_bounds_returns_error() {
        let mut row = RowVec::new(2);
        assert_eq!(
            row.set(2, true),
            Err(RowVecError::IndexOutOfBounds { index: 2, len: 2 })
        );
    }

    #[test]
    fn push_bit_extends_row() {
        let mut row = RowVec::new(2);
        row.set(0, true).unwrap();
        row.push_bit(false);
        row.push_bit(true);

        assert_eq!(row.len(), 4);
        assert_eq!(row.to_bools(), vec![true, false, false, true]);
    }

    #[test]
    fn and_or_match_pointwise_semantics() {
        let left = row_from_bools(&[true, false, true, false]);
        let right = row_from_bools(&[true, true, false, false]);

        let and = left.and(&right).unwrap();
        let or = left.or(&right).unwrap();

        assert_eq!(and.to_bools(), vec![true, false, false, false]);
        assert_eq!(or.to_bools(), vec![true, true, true, false]);
    }

    #[test]
    fn length_mismatch_is_an_error() {
        let left = RowVec::new(2);
        let right = RowVec::new(3);

        assert_eq!(
            left.and(&right),
            Err(RowVecError::LengthMismatch { left: 2, right: 3 })
        );
        assert_eq!(
            left.or(&right),
            Err(RowVecError::LengthMismatch { left: 2, right: 3 })
        );
        assert_eq!(
            left.is_subset_of(&right),
            Err(RowVecError::LengthMismatch { left: 2, right: 3 })
        );
    }

    proptest! {
        #[test]
        fn prop_and_or_match_pointwise(
            pairs in proptest::collection::vec((any::<bool>(), any::<bool>()), 0usize..=256)
        ) {
            let left_values: Vec<bool> = pairs.iter().map(|(l, _)| *l).collect();
            let right_values: Vec<bool> = pairs.iter().map(|(_, r)| *r).collect();

            let left = row_from_bools(&left_values);
            let right = row_from_bools(&right_values);

            let and = left.and(&right).unwrap();
            let or = left.or(&right).unwrap();

            let expected_and: Vec<bool> = left_values
                .iter()
                .zip(right_values.iter())
                .map(|(l, r)| *l && *r)
                .collect();
            let expected_or: Vec<bool> = left_values
                .iter()
                .zip(right_values.iter())
                .map(|(l, r)| *l || *r)
                .collect();

            prop_assert_eq!(and.to_bools(), expected_and);
            prop_assert_eq!(or.to_bools(), expected_or);
        }

        #[test]
        fn prop_subset_matches_pointwise_implication(
            pairs in proptest::collection::vec((any::<bool>(), any::<bool>()), 0usize..=256)
        ) {
            let left_values: Vec<bool> = pairs.iter().map(|(l, _)| *l).collect();
            let right_values: Vec<bool> = pairs.iter().map(|(_, r)| *r).collect();

            let left = row_from_bools(&left_values);
            let right = row_from_bools(&right_values);

            let expected = left_values
                .iter()
                .zip(right_values.iter())
                .all(|(l, r)| !*l || *r);

            prop_assert_eq!(left.is_subset_of(&right).unwrap(), expected);
        }

        #[test]
        fn prop_ones_matches_true_indices(values in proptest::collection::vec(any::<bool>(), 0usize..=256)) {
            let row = row_from_bools(&values);

            let from_ones: Vec<usize> = row.ones().collect();
            let expected: Vec<usize> = values
                .iter()
                .enumerate()
                .filter_map(|(idx, value)| value.then_some(idx))
                .collect();

            prop_assert_eq!(from_ones, expected);
        }
    }
}
