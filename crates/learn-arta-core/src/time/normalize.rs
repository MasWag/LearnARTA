// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Delay and timed-word normalization to the library's half-unit lattice.
//!
//! This module provides the fallible boundary between raw timed words returned
//! by an equivalence oracle and the exact internal `DelayRep` domain used
//! throughout the library. Finite integer delays stay unchanged, finite
//! non-integer delays are snapped to `floor(d) + 0.5`, and infinite delays are
//! preserved.
//!
//! The public `try_normalize_*` functions are the intended entry points. They
//! accept either raw `f64` delays or already-normalized `DelayRep` inputs and
//! return a normalized `DelayRep`-based result or a [`TimeError`] if the raw
//! input is invalid.

use crate::{
    error::TimeError,
    time::DelayRep,
    timed_word::{TimedLetter, TimedWord},
};

/// Input delay values that can be normalized onto the half-unit lattice.
pub trait NormalizeHalfInput {
    /// Convert this delay into the canonical half-unit representation.
    fn try_normalize_half(self) -> Result<DelayRep, TimeError>;
}

impl NormalizeHalfInput for DelayRep {
    fn try_normalize_half(self) -> Result<DelayRep, TimeError> {
        Ok(self)
    }
}

impl NormalizeHalfInput for f64 {
    fn try_normalize_half(self) -> Result<DelayRep, TimeError> {
        DelayRep::try_from_f64(self)
    }
}

/// Normalize one raw delay to the canonical half-unit representation.
///
/// `DelayRep` inputs are returned unchanged. Raw `f64` inputs are validated and
/// mapped onto the internal half-unit lattice.
pub fn try_normalize_delay_half<D>(d: D) -> Result<DelayRep, TimeError>
where
    D: NormalizeHalfInput,
{
    d.try_normalize_half()
}

/// Normalize one timed letter by normalizing its delay component.
///
/// The symbol is preserved and only the delay component is normalized.
pub fn try_normalize_letter_half<A, D>(x: &TimedLetter<A, D>) -> Result<TimedLetter<A>, TimeError>
where
    A: Clone,
    D: NormalizeHalfInput + Clone,
{
    Ok((x.0.clone(), try_normalize_delay_half(x.1.clone())?))
}

/// Normalize a raw timed word by applying half-unit normalization to each delay.
///
/// Symbol order and word length are preserved. This is the normalization
/// boundary used by the learner before counterexample refinement.
pub fn try_normalize_word_half<A, D>(w: &TimedWord<A, D>) -> Result<TimedWord<A>, TimeError>
where
    A: Clone,
    D: NormalizeHalfInput + Clone,
{
    w.iter()
        .map(try_normalize_letter_half)
        .collect::<Result<Vec<_>, _>>()
        .map(TimedWord::from_vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_normalize_delay_half_maps_f64_to_half_unit_lattice() {
        assert_eq!(
            try_normalize_delay_half(0.0_f64),
            Ok(DelayRep::from_integer(0))
        );
        assert_eq!(
            try_normalize_delay_half(1.2_f64),
            Ok(DelayRep::from_floor_plus_half(1))
        );
        assert_eq!(
            try_normalize_delay_half(f64::INFINITY),
            Ok(DelayRep::INFINITY)
        );
    }

    #[test]
    fn try_normalize_delay_half_rejects_invalid_f64_inputs() {
        assert_eq!(try_normalize_delay_half(f64::NAN), Err(TimeError::NaN));
        assert!(matches!(
            try_normalize_delay_half(-0.1_f64),
            Err(TimeError::Negative(_))
        ));
        assert!(matches!(
            try_normalize_delay_half(u32::MAX as f64),
            Err(TimeError::TooLarge(_))
        ));
    }

    #[test]
    fn try_normalize_delay_half_keeps_delay_rep_inputs_unchanged() {
        let cases = [
            DelayRep::from_integer(0),
            DelayRep::from_integer(1),
            DelayRep::from_half_units(3),
            DelayRep::INFINITY,
        ];

        for input in cases {
            assert_eq!(try_normalize_delay_half(input), Ok(input));
        }
    }

    #[test]
    fn try_normalize_letter_half_accepts_raw_f64_letter() {
        let letter = ('a', 1.2_f64);

        assert_eq!(
            try_normalize_letter_half(&letter),
            Ok(('a', DelayRep::from_floor_plus_half(1)))
        );
    }

    #[test]
    fn try_normalize_word_half_preserves_length_symbols_and_normalizes_delays() {
        let w = TimedWord::from_vec(vec![
            ('a', DelayRep::from_integer(0)),
            ('b', DelayRep::from_half_units(3)),
            ('c', DelayRep::from_integer(4)),
            ('a', DelayRep::INFINITY),
        ]);

        let normalized = try_normalize_word_half(&w).expect("DelayRep words should normalize");

        assert_eq!(normalized.len(), w.len());
        let in_symbols: Vec<_> = w.iter().map(|(symbol, _)| *symbol).collect();
        let out_symbols: Vec<_> = normalized.iter().map(|(symbol, _)| *symbol).collect();
        assert_eq!(out_symbols, in_symbols);

        for ((_, in_delay), (_, out_delay)) in w.iter().zip(normalized.iter()) {
            assert_eq!(*out_delay, *in_delay);
        }
    }

    #[test]
    fn try_normalize_word_half_is_idempotent_for_delay_rep_words() {
        let w = TimedWord::from_vec(vec![
            ('a', DelayRep::from_integer(1)),
            ('b', DelayRep::from_half_units(3)),
            ('c', DelayRep::INFINITY),
        ]);

        let once = try_normalize_word_half(&w).expect("DelayRep words should normalize");
        let twice = try_normalize_word_half(&once).expect("normalized words should normalize");
        assert_eq!(twice, once);
    }

    #[test]
    fn try_normalize_word_half_accepts_raw_f64_words() {
        let w = TimedWord::from_vec(vec![('a', 1.2_f64), ('b', 2.0_f64), ('c', f64::INFINITY)]);

        let normalized = try_normalize_word_half(&w).expect("word should normalize");

        assert_eq!(
            normalized,
            TimedWord::from_vec(vec![
                ('a', DelayRep::from_floor_plus_half(1)),
                ('b', DelayRep::from_integer(2)),
                ('c', DelayRep::INFINITY),
            ])
        );
    }

    #[test]
    fn try_normalize_word_half_rejects_invalid_raw_f64_words() {
        let w = TimedWord::from_vec(vec![('a', 0.0_f64), ('b', -0.1_f64)]);

        assert!(matches!(
            try_normalize_word_half(&w),
            Err(TimeError::Negative(_))
        ));
    }
}
