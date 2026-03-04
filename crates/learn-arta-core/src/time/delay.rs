// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Half-integer delay representation for exact time arithmetic.
//!
//! [`DelayRep`] stores delays as `u32` half-units: the value `n` represents
//! `n / 2.0`.  Even values are integers, odd values are half-integers.
//! The sentinel `u32::MAX` is reserved as [`DelayRep::INFINITY`].

use crate::error::TimeError;

/// Internal representation of a delay as a half-integer.
///
/// A value of `n` represents `n / 2.0` time units:
/// - `n = 0` -> `0.0`
/// - `n = 1` -> `0.5`
/// - `n = 2` -> `1.0`
/// - etc.
///
/// Finite values use `0..u32::MAX`. Use [`DelayRep::INFINITY`] for +inf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DelayRep(pub(super) u32);

impl DelayRep {
    /// Represents positive infinity.
    pub const INFINITY: DelayRep = DelayRep(u32::MAX);

    /// Zero delay.
    pub const ZERO: DelayRep = DelayRep(0);

    /// Construct from a half-integer count directly.
    #[inline]
    pub const fn from_half_units(n: u32) -> Self {
        DelayRep(n)
    }

    /// Construct from an exact non-negative integer value.
    ///
    /// This constructor intentionally uses wrapping arithmetic.
    #[inline]
    pub const fn from_integer(n: u32) -> Self {
        DelayRep(n.wrapping_mul(2))
    }

    /// Construct from `floor + 0.5` for a non-negative integer floor.
    ///
    /// This constructor intentionally uses wrapping arithmetic.
    #[inline]
    pub const fn from_floor_plus_half(n: u32) -> Self {
        DelayRep(n.wrapping_mul(2).wrapping_add(1))
    }

    /// Returns the raw half-unit count.
    #[inline]
    pub const fn half_units(self) -> u32 {
        self.0
    }

    /// Returns `true` if this delay represents +∞.
    #[inline]
    pub fn is_infinity(self) -> bool {
        self == Self::INFINITY
    }

    /// Returns `true` if this delay is an integer value.
    #[inline]
    pub fn is_integer(self) -> bool {
        !self.is_infinity() && (self.0 & 1) == 0
    }

    /// Returns `true` if this delay is a half-integer.
    #[inline]
    pub fn is_half_integer(self) -> bool {
        !self.is_infinity() && (self.0 & 1) == 1
    }

    /// Returns the floor integer value, or `None` if infinite.
    #[inline]
    pub fn floor_int(self) -> Option<u32> {
        if self.is_infinity() {
            None
        } else {
            Some(self.0 / 2)
        }
    }

    /// Returns the ceiling integer value, or `None` if infinite.
    #[inline]
    pub fn ceil_int(self) -> Option<u32> {
        if self.is_infinity() {
            None
        } else {
            Some(self.0.div_ceil(2))
        }
    }

    /// Returns the least integer-valued delay greater than or equal to `self`.
    #[inline]
    pub fn ceil(self) -> DelayRep {
        if self.is_infinity() || self.is_integer() {
            self
        } else {
            DelayRep(self.0 + 1)
        }
    }

    /// Convert a raw non-negative `f64` onto the internal half-unit lattice.
    ///
    /// Finite integer values map exactly, finite non-integers map to
    /// `floor(v) + 0.5`, and `+∞` maps to [`DelayRep::INFINITY`].
    pub fn try_from_f64(v: f64) -> Result<DelayRep, TimeError> {
        if v.is_nan() {
            return Err(TimeError::NaN);
        }
        if v < 0.0 {
            return Err(TimeError::Negative(v));
        }
        if v.is_infinite() {
            return Ok(DelayRep::INFINITY);
        }

        let floor = v.floor();
        let fractional = v - floor;
        let mapped_half_units = if fractional > 0.0 {
            floor * 2.0 + 1.0
        } else {
            floor * 2.0
        };
        if !mapped_half_units.is_finite() || mapped_half_units >= u32::MAX as f64 {
            return Err(TimeError::TooLarge(v));
        }
        Ok(DelayRep(mapped_half_units as u32))
    }

    /// Convert this delay back to `f64`, preserving `+∞`.
    pub fn to_f64(self) -> f64 {
        if self.is_infinity() {
            f64::INFINITY
        } else {
            self.0 as f64 / 2.0
        }
    }
}

impl std::fmt::Display for DelayRep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_infinity() {
            write!(f, "∞")
        } else {
            write!(f, "{}", self.to_f64())
        }
    }
}
