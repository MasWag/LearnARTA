// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Core types for representing, validating, and serializing LearnARTA models.
//!
//! The `learn-arta-core` crate contains the reusable data model shared by the
//! learner and oracle crates:
//! - exact half-unit delay normalization via [`DelayRep`],
//! - integer-or-infinity guard intervals,
//! - timed words,
//! - positive Boolean state formulas over locations,
//! - ARTA construction and validation,
//! - canonical JSON I/O and DOT rendering.
//!
//! # Example
//!
//! ```
//! use learn_arta_core::time::interval::Interval;
//! use learn_arta_core::{
//!     ArtaBuilder, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId, StateFormula,
//!     TimedWord,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mgr = DagStateFormulaManager::new();
//! let q0 = LocationId::new("q0");
//! let init = DagStateFormula::var(&mgr, q0.clone());
//!
//! let mut builder = ArtaBuilder::new(init);
//! builder.add_location(q0.clone()).add_accepting(q0.clone());
//! builder.add_transition(
//!     q0.clone(),
//!     'a',
//!     Interval::closed(0, 0)?,
//!     DagStateFormula::var(&mgr, q0.clone()),
//! );
//!
//! let arta = builder.build()?;
//! let word = TimedWord::from_vec(vec![('a', DelayRep::from_integer(0))]);
//! assert!(arta.accepts(&word));
//! # Ok(())
//! # }
//! ```

pub mod arta;
pub mod dot;
pub mod error;
pub mod json;
pub mod location;
pub mod normalize;
pub mod partition;
pub mod state_formula;
pub mod time;
pub mod timed_word;

pub use crate::arta::{Arta, ArtaBuilder, ArtaError, GuardedTransition};
pub use crate::dot::DotOptions;
pub use crate::error::TimeError;
pub use crate::json::{
    ArtaJsonError, ParsedArtaJson, parse_arta_json, parse_arta_json_document, read_arta_json_file,
    read_arta_json_file_document, to_arta_json_document_string, to_arta_json_string,
    write_arta_json_document_file, write_arta_json_file,
};
pub use crate::location::LocationId;
pub use crate::normalize::{
    NormalizeHalfInput, try_normalize_delay_half, try_normalize_letter_half,
    try_normalize_word_half,
};
pub use crate::state_formula::{
    DagStateFormula, DagStateFormulaManager, MinimalModelKey, StateFormula,
};
pub use crate::time::DelayRep;
pub use crate::timed_word::{TimedLetter, TimedWord, collect_timed_letters};
