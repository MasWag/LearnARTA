// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Error types for the LearnARTA library.

use thiserror::Error;

/// Errors related to time values.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum TimeError {
    /// The f64 value is NaN.
    #[error("time value is NaN")]
    NaN,
    /// The f64 value is negative.
    #[error("time value {0} is negative")]
    Negative(f64),
    /// The finite f64 value is too large to encode as a finite `DelayRep`.
    #[error("time value {0} is too large to represent")]
    TooLarge(f64),
}

/// Errors related to interval construction or validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum IntervalError {
    /// The interval string does not match the accepted textual syntax.
    #[error("invalid interval syntax: {0}")]
    InvalidSyntax(String),
    /// The lower bound exceeds the upper bound.
    #[error("invalid interval bounds: lower bound {lower} exceeds upper bound {upper}")]
    LowerExceedsUpper {
        /// Invalid lower endpoint.
        lower: u32,
        /// Invalid upper endpoint.
        upper: u32,
    },
    /// The interval denotes an empty set.
    #[error("interval is empty")]
    Empty,
    /// `+∞` upper bound must be open.
    #[error("invalid interval bounds: +∞ upper bound cannot be inclusive")]
    InclusiveInfiniteUpper,
}
