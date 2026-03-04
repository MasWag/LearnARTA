// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Decomposition utilities over basis row vectors.
//!
//! Given basis rows over a fixed set of suffix experiments:
//! - for each suffix column `e`, build the conjunction of all basis rows whose
//!   value at `e` is `⊤` (empty conjunction = `⊤` row),
//! - for a target row `r`, build the disjunction of those conjunctions for the
//!   suffix columns where `r(e) = ⊤` (empty disjunction = `⊥` row/formula).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

use thiserror::Error;

use crate::rowvec::{RowVec, RowVecError};

/// Variable identifier for basis states.
///
/// `BasisVar(i)` refers to the `i`th basis row in the order supplied to
/// [`BasisDecomposer::new`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct BasisVar(pub usize);

/// Canonical positive Boolean formula over [`BasisVar`].
///
/// This is a canonicalized DNF-oriented shape:
/// - `Top`, `Bot`
/// - conjunction term `And(Vec<BasisVar>)`
/// - disjunction `Or(Vec<BasisFormula>)` of canonical terms
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum BasisFormula {
    /// Constant `⊤`.
    Top,
    /// Constant `⊥`.
    Bot,
    /// Conjunction term over basis variables.
    And(Vec<BasisVar>),
    /// Disjunction over canonical terms.
    Or(Vec<BasisFormula>),
}

impl BasisFormula {
    /// Construct `⊤`.
    pub fn top() -> Self {
        Self::Top
    }

    /// Construct `⊥`.
    pub fn bot() -> Self {
        Self::Bot
    }

    /// Construct a basis variable formula.
    pub fn var(v: BasisVar) -> Self {
        Self::and([v])
    }

    /// Construct a canonical conjunction.
    ///
    /// Empty conjunction is canonicalized to `⊤`.
    pub fn and(vars: impl IntoIterator<Item = BasisVar>) -> Self {
        let mut vars: Vec<_> = vars.into_iter().collect();
        vars.sort_unstable();
        vars.dedup();
        if vars.is_empty() {
            Self::Top
        } else {
            Self::And(vars)
        }
    }

    /// Construct a canonical disjunction.
    ///
    /// Rules:
    /// - flatten nested `Or`
    /// - drop `Bot` children
    /// - if any child is `Top`, return `Top`
    /// - sort and deduplicate terms deterministically
    /// - empty disjunction -> `Bot`
    /// - singleton disjunction -> the single term
    pub fn or(terms: impl IntoIterator<Item = BasisFormula>) -> Self {
        let mut flat_terms = Vec::new();
        let mut stack: Vec<_> = terms.into_iter().collect();

        while let Some(term) = stack.pop() {
            match term {
                BasisFormula::Top => return BasisFormula::Top,
                BasisFormula::Bot => {}
                BasisFormula::Or(children) => stack.extend(children),
                BasisFormula::And(vars) => flat_terms.push(BasisFormula::And(vars)),
            }
        }

        if flat_terms.is_empty() {
            return BasisFormula::Bot;
        }

        flat_terms.sort_unstable_by(Self::term_cmp);
        flat_terms.dedup();

        if flat_terms.len() == 1 {
            // safe due len check
            return flat_terms.pop().unwrap_or(BasisFormula::Bot);
        }

        BasisFormula::Or(flat_terms)
    }

    fn term_cmp(left: &Self, right: &Self) -> Ordering {
        fn rank(term: &BasisFormula) -> u8 {
            match term {
                BasisFormula::And(_) => 0,
                BasisFormula::Top => 1,
                BasisFormula::Or(_) => 2,
                BasisFormula::Bot => 3,
            }
        }

        let rank_order = rank(left).cmp(&rank(right));
        if rank_order != Ordering::Equal {
            return rank_order;
        }

        match (left, right) {
            (BasisFormula::And(lhs), BasisFormula::And(rhs)) => lhs.cmp(rhs),
            (BasisFormula::Or(lhs), BasisFormula::Or(rhs)) => lhs.cmp(rhs),
            (BasisFormula::Top, BasisFormula::Top) | (BasisFormula::Bot, BasisFormula::Bot) => {
                Ordering::Equal
            }
            _ => Ordering::Equal,
        }
    }
}

impl fmt::Display for BasisFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BasisFormula::Top => write!(f, "⊤"),
            BasisFormula::Bot => write!(f, "⊥"),
            BasisFormula::And(vars) => {
                write!(f, "(")?;
                for (idx, var) in vars.iter().enumerate() {
                    if idx > 0 {
                        write!(f, " & ")?;
                    }
                    write!(f, "v{}", var.0)?;
                }
                write!(f, ")")
            }
            BasisFormula::Or(terms) => {
                write!(f, "(")?;
                for (idx, term) in terms.iter().enumerate() {
                    if idx > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{term}")?;
                }
                write!(f, ")")
            }
        }
    }
}

/// Errors produced by decomposition operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DecompositionError {
    /// Basis rows are required to define column count.
    #[error("basis cannot be empty")]
    EmptyBasis,
    /// Input row length differs from basis column count.
    #[error("row length mismatch: expected {expected}, found {found}")]
    LengthMismatch {
        /// Expected common row width.
        expected: usize,
        /// Encountered row width.
        found: usize,
    },
    /// Column index is outside valid suffix-experiment bounds.
    #[error("index out of bounds: index {index}, len {len}")]
    IndexOutOfBounds {
        /// Requested suffix column index.
        index: usize,
        /// Total number of suffix columns.
        len: usize,
    },
}

/// Precomputed decomposition helper for a fixed basis rows.
#[derive(Debug)]
pub struct BasisDecomposer {
    column_count: usize,
    basis_rows: Vec<RowVec>,
    // For each suffix column, cache the row-vector meet M_RP(e) of matching basis rows.
    column_meet_rows: Vec<RowVec>,
    // For each suffix column, cache the basis variables that participate in M_RP(e).
    column_meet_vars: Vec<Vec<BasisVar>>,
    closure_cache: HashMap<RowVec, RowVec>,
    formula_cache: HashMap<RowVec, BasisFormula>,
}

impl BasisDecomposer {
    /// Build a decomposer from basis rows in fixed order.
    pub fn new(basis_rows: Vec<RowVec>) -> Result<Self, DecompositionError> {
        if basis_rows.is_empty() {
            return Err(DecompositionError::EmptyBasis);
        }

        let column_count = basis_rows[0].len();
        for row in basis_rows.iter().skip(1) {
            if row.len() != column_count {
                return Err(DecompositionError::LengthMismatch {
                    expected: column_count,
                    found: row.len(),
                });
            }
        }

        let mut column_meet_rows = Vec::with_capacity(column_count);
        let mut column_meet_vars = Vec::with_capacity(column_count);

        for suffix_idx in 0..column_count {
            let mut vars_for_suffix = Vec::new();
            let mut has_term = false;
            let mut column_meet_row = RowVec::top(column_count);

            for (basis_idx, row) in basis_rows.iter().enumerate() {
                if row.get(suffix_idx) == Some(true) {
                    vars_for_suffix.push(BasisVar(basis_idx));
                    if !has_term {
                        column_meet_row = row.clone();
                        has_term = true;
                    } else {
                        column_meet_row =
                            column_meet_row.and(row).map_err(Self::map_rowvec_error)?;
                    }
                }
            }

            if !has_term {
                column_meet_row = RowVec::top(column_count);
            }

            column_meet_vars.push(vars_for_suffix);
            column_meet_rows.push(column_meet_row);
        }

        Ok(Self {
            column_count,
            basis_rows,
            column_meet_rows,
            column_meet_vars,
            closure_cache: HashMap::new(),
            formula_cache: HashMap::new(),
        })
    }

    /// Number of suffix-experiment columns.
    pub fn column_count(&self) -> usize {
        self.column_count
    }

    /// Basis rows in fixed order.
    pub fn basis_rows(&self) -> &[RowVec] {
        &self.basis_rows
    }

    /// Safe accessor for the column-meet row `M_RP(e)`.
    pub fn try_column_meet_row(&self, suffix_idx: usize) -> Result<&RowVec, DecompositionError> {
        self.column_meet_rows
            .get(suffix_idx)
            .ok_or(DecompositionError::IndexOutOfBounds {
                index: suffix_idx,
                len: self.column_count,
            })
    }

    /// Safe accessor for the column-meet formula `M_RP(e)`.
    ///
    /// This is the conjunction of all basis variables whose rows are `⊤` in
    /// column `suffix_idx`.
    pub fn try_column_meet_formula(
        &self,
        suffix_idx: usize,
    ) -> Result<BasisFormula, DecompositionError> {
        let vars =
            self.column_meet_vars
                .get(suffix_idx)
                .ok_or(DecompositionError::IndexOutOfBounds {
                    index: suffix_idx,
                    len: self.column_count,
                })?;
        Ok(BasisFormula::and(vars.iter().copied()))
    }

    /// Compute `b_RP(r)` as a row-vector closure.
    ///
    /// The result is the pointwise `∨` of all cached column meets for columns
    /// where `r` is `⊤`.
    pub fn closure_row(&mut self, r: &RowVec) -> Result<RowVec, DecompositionError> {
        self.ensure_row_len(r)?;

        if let Some(cached) = self.closure_cache.get(r) {
            return Ok(cached.clone());
        }

        let mut closure = RowVec::new(self.column_count);
        for suffix_idx in r.ones() {
            let column_meet_row = self.try_column_meet_row(suffix_idx)?;
            closure = closure
                .or(column_meet_row)
                .map_err(Self::map_rowvec_error)?;
        }

        self.closure_cache.insert(r.clone(), closure.clone());
        Ok(closure)
    }

    /// Compute `b_RP(r)` as a positive basis formula.
    ///
    /// The returned formula is canonical and uses [`BasisVar`] indices into
    /// [`Self::basis_rows`].
    pub fn decompose_formula(&mut self, r: &RowVec) -> Result<BasisFormula, DecompositionError> {
        self.ensure_row_len(r)?;

        if let Some(cached) = self.formula_cache.get(r) {
            return Ok(cached.clone());
        }

        let mut terms = Vec::new();
        for suffix_idx in r.ones() {
            terms.push(self.try_column_meet_formula(suffix_idx)?);
        }

        let formula = BasisFormula::or(terms);
        self.formula_cache.insert(r.clone(), formula.clone());
        Ok(formula)
    }

    /// Return whether `r` is representable, i.e. `b_RP(r) == r`.
    ///
    /// This is the basis-closedness predicate used by the learner.
    pub fn representable(&mut self, r: &RowVec) -> Result<bool, DecompositionError> {
        Ok(self.closure_row(r)? == *r)
    }

    fn ensure_row_len(&self, r: &RowVec) -> Result<(), DecompositionError> {
        if r.len() == self.column_count {
            return Ok(());
        }
        Err(DecompositionError::LengthMismatch {
            expected: self.column_count,
            found: r.len(),
        })
    }

    fn map_rowvec_error(err: RowVecError) -> DecompositionError {
        match err {
            RowVecError::IndexOutOfBounds { index, len } => {
                DecompositionError::IndexOutOfBounds { index, len }
            }
            RowVecError::LengthMismatch { left, right } => DecompositionError::LengthMismatch {
                expected: left,
                found: right,
            },
        }
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

    fn row_from_bits(bits: &str) -> RowVec {
        let mut row = RowVec::new(bits.len());
        for (idx, c) in bits.chars().enumerate() {
            if c == '1' {
                row.set(idx, true).unwrap();
            }
        }
        row
    }

    #[test]
    fn decomposition_rows_match_handcrafted_example() {
        let basis_row0 = row_from_bits("110");
        let basis_row1 = row_from_bits("011");
        let mut decomp =
            BasisDecomposer::new(vec![basis_row0.clone(), basis_row1.clone()]).unwrap();

        assert_eq!(decomp.try_column_meet_row(0).unwrap(), &basis_row0);
        assert_eq!(decomp.try_column_meet_row(2).unwrap(), &basis_row1);
        assert_eq!(
            decomp.try_column_meet_row(1).unwrap(),
            &row_from_bits("010")
        );

        let closure = decomp.closure_row(&row_from_bits("101")).unwrap();
        assert_eq!(closure, row_from_bits("111"));
        assert!(decomp.representable(&row_from_bits("010")).unwrap());
    }

    #[test]
    fn formula_generation_matches_handcrafted_example() {
        let basis_row0 = row_from_bits("110");
        let basis_row1 = row_from_bits("011");
        let mut decomp = BasisDecomposer::new(vec![basis_row0, basis_row1]).unwrap();

        assert_eq!(
            decomp.try_column_meet_formula(1).unwrap().to_string(),
            "(v0 & v1)"
        );
        assert_eq!(
            decomp.decompose_formula(&row_from_bits("010")).unwrap(),
            decomp.try_column_meet_formula(1).unwrap()
        );
    }

    #[test]
    fn two_row_basis_representability_matches_manual_example() {
        let mut decomp =
            BasisDecomposer::new(vec![row_from_bits("10"), row_from_bits("11")]).unwrap();

        assert!(decomp.representable(&row_from_bits("10")).unwrap());
        assert!(decomp.representable(&row_from_bits("11")).unwrap());
        assert!(decomp.representable(&row_from_bits("00")).unwrap());
        assert!(!decomp.representable(&row_from_bits("01")).unwrap());
    }

    #[test]
    fn three_row_basis_representability_matches_manual_example() {
        let mut decomp = BasisDecomposer::new(vec![
            row_from_bits("101"),
            row_from_bits("010"),
            row_from_bits("011"),
        ])
        .unwrap();

        for bits in ["111", "101", "011", "001", "010", "000"] {
            assert!(
                decomp.representable(&row_from_bits(bits)).unwrap(),
                "{bits}"
            );
        }

        for bits in ["100", "110"] {
            assert!(
                !decomp.representable(&row_from_bits(bits)).unwrap(),
                "{bits}"
            );
        }
    }

    #[test]
    fn constructor_rejects_empty_basis() {
        assert!(matches!(
            BasisDecomposer::new(Vec::new()),
            Err(DecompositionError::EmptyBasis)
        ));
    }

    #[test]
    fn constructor_rejects_mixed_lengths() {
        let err = BasisDecomposer::new(vec![RowVec::new(3), RowVec::new(2)]).unwrap_err();
        assert_eq!(
            err,
            DecompositionError::LengthMismatch {
                expected: 3,
                found: 2
            }
        );
    }

    #[test]
    fn safe_accessors_return_out_of_bounds_error() {
        let decomp =
            BasisDecomposer::new(vec![row_from_bits("110"), row_from_bits("011")]).unwrap();
        assert_eq!(
            decomp.try_column_meet_row(3),
            Err(DecompositionError::IndexOutOfBounds { index: 3, len: 3 })
        );
        assert_eq!(
            decomp.try_column_meet_formula(3),
            Err(DecompositionError::IndexOutOfBounds { index: 3, len: 3 })
        );
    }

    prop_compose! {
        fn basis_rows_and_row_strategy()
            (len in 0usize..=64, basis_size in 1usize..=6)
            (
                basis in proptest::collection::vec(proptest::collection::vec(any::<bool>(), len), basis_size),
                row in proptest::collection::vec(any::<bool>(), len),
            ) -> (Vec<RowVec>, RowVec) {
                let basis_rows = basis
                    .into_iter()
                    .map(|values| row_from_bools(&values))
                    .collect();
                let r = row_from_bools(&row);
                (basis_rows, r)
            }
    }

    proptest! {
        #[test]
        fn prop_closure_is_extensive_and_idempotent((basis_rows, r) in basis_rows_and_row_strategy()) {
            let mut decomp = BasisDecomposer::new(basis_rows).unwrap();

            let closure = decomp.closure_row(&r).unwrap();
            prop_assert!(r.is_subset_of(&closure).unwrap());

            let closure_twice = decomp.closure_row(&closure).unwrap();
            prop_assert_eq!(closure_twice, closure);
        }
    }
}
