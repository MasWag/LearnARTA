// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Evidence AFA construction from a cohesive observation table.

use log::trace;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

use learn_arta_core::{
    time::DelayRep,
    timed_word::{TimedLetter, TimedWord},
};
use thiserror::Error;

use crate::{
    BasisDecomposer, BasisFormula, DecompositionError, ObservationTable, RowVec,
    cohesion::BasisWords,
};

/// State identifier in an [`EvidenceAfa`].
///
/// Indices follow the deduplicated basis-row order chosen during construction.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct AfaStateId(pub usize);

type Alphabet<A> = Vec<TimedLetter<A>>;
type AlphabetIndex<A> = HashMap<TimedLetter<A>, usize>;

/// Errors produced while constructing an [`EvidenceAfa`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum EvidenceAfaError<A> {
    /// The epsilon prefix is missing from sample prefixes.
    #[error("missing epsilon word in sample prefixes")]
    MissingEpsilonInS,
    /// The epsilon suffix experiment column is missing.
    #[error("missing epsilon column in experiment suffixes")]
    MissingEpsilonColumn,
    /// A required row is missing from the table.
    #[error("missing row for required word")]
    MissingRow {
        /// Word whose row was expected in the observation table.
        word: TimedWord<A>,
    },
    /// A required distinct extension `rep·sigma` is missing from sample prefixes.
    #[error("missing distinct extension row for representative and timed letter")]
    MissingDistinctExtension {
        /// Representative basis word whose extension is required.
        rep: TimedWord<A>,
        /// Timed letter appended to the representative.
        sigma: TimedLetter<A>,
    },
    /// Length mismatch between structures that must agree.
    #[error("length mismatch: expected {expected}, found {found}")]
    LengthMismatch {
        /// Expected common width or state count.
        expected: usize,
        /// Encountered width or state count.
        found: usize,
    },
    /// Any remaining decomposition failure.
    #[error("decomposition error: {0}")]
    Decomposition(DecompositionError),
}

impl<A> From<DecompositionError> for EvidenceAfaError<A> {
    fn from(value: DecompositionError) -> Self {
        match value {
            DecompositionError::LengthMismatch { expected, found } => {
                Self::LengthMismatch { expected, found }
            }
            other => Self::Decomposition(other),
        }
    }
}

/// Evidence AFA derived from basis rows of a cohesive observation table.
///
/// Each state corresponds to one distinct basis row; multiple basis words with
/// the same row are collapsed to the same state.
#[derive(Debug, Clone)]
pub struct EvidenceAfa<A, F>
where
    A: Eq + Hash + Clone,
    F: Clone + Eq + Hash + fmt::Debug + fmt::Display,
{
    basis_rows: Vec<RowVec>,
    representatives: Vec<TimedWord<A>>,
    alphabet: Alphabet<A>,
    initial_formula: F,
    accepting: Vec<bool>,
    transition_table: Vec<Vec<F>>,
    alphabet_index: AlphabetIndex<A>,
}

impl<A, F> EvidenceAfa<A, F>
where
    A: Eq + Hash + Clone,
    F: Clone + Eq + Hash + fmt::Debug + fmt::Display,
{
    /// Number of states in this Evidence AFA.
    pub fn num_states(&self) -> usize {
        self.basis_rows.len()
    }

    /// Iterate states in deterministic index order.
    pub fn states(&self) -> impl Iterator<Item = AfaStateId> + '_ {
        (0..self.num_states()).map(AfaStateId)
    }

    /// Timed-letter alphabet in deterministic order.
    ///
    /// The order matches the observation table's first-occurrence scan.
    pub fn alphabet(&self) -> &[(A, DelayRep)] {
        &self.alphabet
    }

    /// Initial formula.
    pub fn init(&self) -> &F {
        &self.initial_formula
    }

    /// Return whether state `q` is accepting.
    pub fn is_accepting(&self, q: AfaStateId) -> bool {
        self.accepting.get(q.0).copied().unwrap_or(false)
    }

    /// Transition function for one state and timed letter.
    ///
    /// Returns `None` when either the state is out of bounds or `sigma` is not
    /// in the automaton alphabet.
    pub fn transition(&self, q: AfaStateId, sigma: &(A, DelayRep)) -> Option<&F> {
        let alpha_idx = *self.alphabet_index.get(sigma)?;
        self.transition_table.get(q.0)?.get(alpha_idx)
    }

    /// Basis rows used as state representatives.
    pub fn basis_rows(&self) -> &[RowVec] {
        &self.basis_rows
    }

    /// Representative basis words aligned with [`Self::basis_rows`].
    pub fn representatives(&self) -> &[TimedWord<A>] {
        &self.representatives
    }
}

/// Build an Evidence AFA from a cohesive observation table and basis words.
///
/// Basis words whose rows are identical are merged into one evidence state. The
/// function expects a cohesive table; if required rows or extensions are
/// missing, it returns a structured error instead of silently guessing.
pub fn build_from_cohesive_table<A>(
    table: &ObservationTable<A>,
    basis_words: &BasisWords<A>,
) -> Result<EvidenceAfa<A, BasisFormula>, EvidenceAfaError<A>>
where
    A: Eq + Hash + Clone + std::fmt::Display,
{
    let mut row_to_state = HashMap::<RowVec, AfaStateId>::new();
    let mut basis_rows = Vec::new();
    let mut representatives = Vec::new();

    for basis_word in basis_words.iter() {
        trace!("Processing basis word: {}", basis_word);
        let row = table
            .row_of(basis_word)
            .ok_or_else(|| EvidenceAfaError::MissingRow {
                word: basis_word.clone(),
            })?;
        if row_to_state.contains_key(row) {
            continue;
        }
        let state_id = AfaStateId(basis_rows.len());
        row_to_state.insert(row.clone(), state_id);
        basis_rows.push(row.clone());
        representatives.push(basis_word.clone());
    }

    if basis_rows.len() != representatives.len() {
        return Err(EvidenceAfaError::LengthMismatch {
            expected: basis_rows.len(),
            found: representatives.len(),
        });
    }

    let mut decomposer = BasisDecomposer::new(basis_rows.clone())?;
    let alphabet = table.ordered_timed_letters();
    let alphabet_index = alphabet
        .iter()
        .cloned()
        .enumerate()
        .map(|(idx, letter)| (letter, idx))
        .collect();

    let epsilon_column = table
        .experiment_suffixes()
        .iter()
        .position(TimedWord::is_empty)
        .ok_or(EvidenceAfaError::MissingEpsilonColumn)?;

    let epsilon = TimedWord::empty();
    let initial_row = table
        .row_of(&epsilon)
        .ok_or(EvidenceAfaError::MissingEpsilonInS)?;
    let initial_formula = decomposer.decompose_formula(initial_row)?;

    let mut accepting = Vec::with_capacity(basis_rows.len());
    for row in &basis_rows {
        let value = row
            .get(epsilon_column)
            .ok_or(EvidenceAfaError::LengthMismatch {
                expected: epsilon_column + 1,
                found: row.len(),
            })?;
        accepting.push(value);
    }

    let mut transition_table = Vec::with_capacity(basis_rows.len());
    for representative in &representatives {
        let mut transitions = Vec::with_capacity(alphabet.len());
        for sigma in &alphabet {
            let extension = representative.append_letter(sigma.clone());
            let extension_row = table.row_of(&extension).ok_or_else(|| {
                EvidenceAfaError::MissingDistinctExtension {
                    rep: representative.clone(),
                    sigma: sigma.clone(),
                }
            })?;
            transitions.push(decomposer.decompose_formula(extension_row)?);
        }
        transition_table.push(transitions);
    }

    Ok(EvidenceAfa {
        basis_rows,
        representatives,
        alphabet,
        initial_formula,
        accepting,
        transition_table,
        alphabet_index,
    })
}
