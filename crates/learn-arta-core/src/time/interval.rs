// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Interval guards over non-negative time.
//!
//! Endpoints are integers or `+∞`, and membership is evaluated exactly on
//! [`DelayRep`] half-units.

use crate::error::IntervalError;

use super::DelayRep;

const MAX_FINITE_HALF_UNITS: u64 = (u32::MAX - 1) as u64;

/// An interval endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endpoint {
    /// A finite integer endpoint.
    Finite(u32),
    /// Positive infinity.
    Infinity,
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Endpoint::Finite(n) => write!(f, "{n}"),
            Endpoint::Infinity => write!(f, "∞"),
        }
    }
}

/// Internal half-unit interval used for exact overlap and merge operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HalfRange {
    min: u64,
    max: Option<u64>,
}

/// Guard interval with integer endpoints or `+∞`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Interval {
    lower: Endpoint,
    lower_inclusive: bool,
    upper: Endpoint,
    upper_inclusive: bool,
}

impl Interval {
    /// Construct a closed interval `[l, u]`.
    pub fn closed(l: u32, u: u32) -> Result<Self, IntervalError> {
        Self::from_bounds(true, l, true, Some(u))
    }

    /// Construct an open interval `(l, u)`.
    pub fn open(l: u32, u: u32) -> Result<Self, IntervalError> {
        Self::from_bounds(false, l, false, Some(u))
    }

    /// Construct a half-open interval `[l, u)`.
    pub fn left_closed_right_open(l: u32, u: u32) -> Result<Self, IntervalError> {
        Self::from_bounds(true, l, false, Some(u))
    }

    /// Construct a half-open interval `(l, u]`.
    pub fn left_open_right_closed(l: u32, u: u32) -> Result<Self, IntervalError> {
        Self::from_bounds(false, l, true, Some(u))
    }

    /// Construct an interval from explicit bound flags.
    ///
    /// `upper = None` encodes `+∞`. Infinite upper bounds must be open.
    pub fn from_bounds(
        lower_inclusive: bool,
        l: u32,
        upper_inclusive: bool,
        upper: Option<u32>,
    ) -> Result<Self, IntervalError> {
        match upper {
            Some(u) => {
                if l > u {
                    return Err(IntervalError::LowerExceedsUpper { lower: l, upper: u });
                }
                if l == u && !(lower_inclusive && upper_inclusive) {
                    return Err(IntervalError::Empty);
                }

                Ok(Self {
                    lower: Endpoint::Finite(l),
                    lower_inclusive,
                    upper: Endpoint::Finite(u),
                    upper_inclusive,
                })
            }
            None => {
                if upper_inclusive {
                    return Err(IntervalError::InclusiveInfiniteUpper);
                }

                Ok(Self {
                    lower: Endpoint::Finite(l),
                    lower_inclusive,
                    upper: Endpoint::Infinity,
                    upper_inclusive: false,
                })
            }
        }
    }

    /// Validate this interval.
    ///
    /// This re-checks construction invariants and is useful when validating
    /// larger structures that hold intervals.
    pub fn validate(&self) -> Result<(), IntervalError> {
        let lower = match self.lower {
            Endpoint::Finite(v) => v,
            Endpoint::Infinity => return Err(IntervalError::Empty),
        };
        let upper = match self.upper {
            Endpoint::Finite(v) => Some(v),
            Endpoint::Infinity => None,
        };
        Self::from_bounds(self.lower_inclusive, lower, self.upper_inclusive, upper)?;
        if self.pick_witness().is_none() {
            return Err(IntervalError::Empty);
        }
        Ok(())
    }

    /// Returns `true` iff `d` belongs to this interval.
    ///
    /// `DelayRep::INFINITY` is never considered a member.
    pub fn contains(&self, d: DelayRep) -> bool {
        if d.is_infinity() {
            return false;
        }

        let range = self.to_half_range();
        let value = u64::from(d.half_units());
        if value < range.min {
            return false;
        }

        match range.max {
            Some(max) => value <= max,
            None => true,
        }
    }

    /// Returns `true` iff this interval intersects `other`.
    pub fn intersects(&self, other: &Interval) -> bool {
        self.intersection(other).is_some()
    }

    /// Returns `true` iff this interval is disjoint from `other`.
    pub fn is_disjoint(&self, other: &Interval) -> bool {
        !self.intersects(other)
    }

    /// Compute the overlap between two intervals.
    ///
    /// Returns `None` when there is no representable [`DelayRep`] in the
    /// overlap.
    pub fn intersection(&self, other: &Interval) -> Option<Interval> {
        let overlap = self.overlap_range(other)?;
        let min = overlap.min;
        if min > MAX_FINITE_HALF_UNITS {
            return None;
        }

        let capped_max = match overlap.max {
            Some(max) => Some(max.min(MAX_FINITE_HALF_UNITS)),
            None => Some(MAX_FINITE_HALF_UNITS),
        };
        let effective = HalfRange {
            min,
            max: capped_max,
        };
        Self::from_half_range(effective)
    }

    /// Pick a representable witness delay in this interval.
    ///
    /// Returns `None` when the interval contains no representable finite
    /// [`DelayRep`] value.
    pub fn pick_witness(&self) -> Option<DelayRep> {
        let range = self.to_half_range();
        if range.min > MAX_FINITE_HALF_UNITS {
            return None;
        }

        let max = match range.max {
            Some(upper) => upper.min(MAX_FINITE_HALF_UNITS),
            None => MAX_FINITE_HALF_UNITS,
        };
        if range.min > max {
            return None;
        }

        let half_units = u32::try_from(range.min).ok()?;
        Some(DelayRep::from_half_units(half_units))
    }

    /// Try to merge two overlapping/touching intervals into one interval.
    ///
    /// Returns `None` when the union would not be a single interval.
    pub fn try_merge_adjacent(&self, other: &Interval) -> Option<Interval> {
        let mut left = self.to_half_range();
        let mut right = other.to_half_range();
        if left.min > right.min {
            std::mem::swap(&mut left, &mut right);
        }

        let connected = match left.max {
            None => true,
            Some(left_max) => left_max.saturating_add(1) >= right.min,
        };
        if !connected {
            return None;
        }

        let merged = HalfRange {
            min: left.min,
            max: match (left.max, right.max) {
                (None, _) | (_, None) => None,
                (Some(a), Some(b)) => Some(a.max(b)),
            },
        };
        Self::from_half_range(merged)
    }

    /// Ordering key based on normalized half-unit bounds.
    ///
    /// This is used to sort intervals stably for deterministic ARTA and DOT
    /// output. Infinite upper bounds sort after finite ones.
    pub(crate) fn sort_key(&self) -> (u64, u64) {
        let range = self.to_half_range();
        (range.min, range.max.unwrap_or(u64::MAX))
    }

    /// Return the representable half-unit range covered by this interval.
    ///
    /// The start and end are inclusive half-unit indices. `None` for the end
    /// denotes an interval that extends to `+∞`.
    pub(crate) fn representable_half_range(&self) -> Option<(u32, Option<u32>)> {
        let range = self.to_half_range();
        if range.min > MAX_FINITE_HALF_UNITS {
            return None;
        }

        let start = u32::try_from(range.min).ok()?;
        let end = match range.max {
            Some(max) => {
                let capped = max.min(MAX_FINITE_HALF_UNITS);
                if range.min > capped {
                    return None;
                }
                Some(u32::try_from(capped).ok()?)
            }
            None => None,
        };

        Some((start, end))
    }

    /// Construct an interval from an inclusive representable half-unit range.
    ///
    /// `end = None` denotes an interval that extends to `+∞`.
    pub(crate) fn from_representable_half_range(start: u32, end: Option<u32>) -> Option<Self> {
        let range = HalfRange {
            min: u64::from(start),
            max: end.map(u64::from),
        };
        Self::from_half_range(range)
    }

    /// Finite integer lower endpoint.
    pub fn lower_bound(&self) -> u32 {
        match self.lower {
            Endpoint::Finite(v) => v,
            Endpoint::Infinity => 0,
        }
    }

    /// Finite integer upper endpoint (`None` for `+∞`).
    pub fn upper_bound(&self) -> Option<u32> {
        match self.upper {
            Endpoint::Finite(v) => Some(v),
            Endpoint::Infinity => None,
        }
    }

    fn to_half_range(&self) -> HalfRange {
        let lower = match self.lower {
            Endpoint::Finite(n) => u64::from(n),
            Endpoint::Infinity => {
                return HalfRange {
                    min: u64::MAX,
                    max: Some(0),
                };
            }
        };
        let min = lower * 2 + if self.lower_inclusive { 0 } else { 1 };
        let max = match self.upper {
            Endpoint::Infinity => None,
            Endpoint::Finite(u) => {
                let base = u64::from(u) * 2;
                Some(if self.upper_inclusive {
                    base
                } else {
                    base.saturating_sub(1)
                })
            }
        };

        HalfRange { min, max }
    }

    fn overlap_range(&self, other: &Interval) -> Option<HalfRange> {
        let a = self.to_half_range();
        let b = other.to_half_range();
        let min = a.min.max(b.min);
        let max = match (a.max, b.max) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        };

        if let Some(m) = max
            && min > m
        {
            return None;
        }
        Some(HalfRange { min, max })
    }

    fn from_half_range(range: HalfRange) -> Option<Interval> {
        let (lower_inclusive, lower_u64) = if range.min.is_multiple_of(2) {
            (true, range.min / 2)
        } else {
            (false, (range.min - 1) / 2)
        };
        let lower = u32::try_from(lower_u64).ok()?;

        match range.max {
            None => Interval::from_bounds(lower_inclusive, lower, false, None).ok(),
            Some(max) => {
                let (upper_inclusive, upper_u64) = if max % 2 == 0 {
                    (true, max / 2)
                } else {
                    (false, max.div_ceil(2))
                };
                let upper = u32::try_from(upper_u64).ok()?;
                Interval::from_bounds(lower_inclusive, lower, upper_inclusive, Some(upper)).ok()
            }
        }
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let left = if self.lower_inclusive { '[' } else { '(' };
        let right = if self.upper_inclusive { ']' } else { ')' };
        write!(f, "{left}{},{}{right}", self.lower, self.upper)
    }
}

impl std::str::FromStr for Interval {
    type Err = IntervalError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let trimmed = input.trim();
        if trimmed.len() < 5 {
            return Err(IntervalError::InvalidSyntax(trimmed.to_string()));
        }

        let lower_inclusive = match trimmed.as_bytes().first().copied() {
            Some(b'[') => true,
            Some(b'(') => false,
            _ => return Err(IntervalError::InvalidSyntax(trimmed.to_string())),
        };
        let upper_inclusive = match trimmed.as_bytes().last().copied() {
            Some(b']') => true,
            Some(b')') => false,
            _ => return Err(IntervalError::InvalidSyntax(trimmed.to_string())),
        };

        let body = &trimmed[1..trimmed.len() - 1];
        let (lower_raw, upper_raw) = body
            .split_once(',')
            .ok_or_else(|| IntervalError::InvalidSyntax(trimmed.to_string()))?;

        let lower = lower_raw
            .trim()
            .parse::<u32>()
            .map_err(|_| IntervalError::InvalidSyntax(trimmed.to_string()))?;

        let upper = match upper_raw.trim() {
            "+" | "∞" => None,
            finite => Some(
                finite
                    .parse::<u32>()
                    .map_err(|_| IntervalError::InvalidSyntax(trimmed.to_string()))?,
            ),
        };

        Self::from_bounds(lower_inclusive, lower, upper_inclusive, upper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn expected_contains(iv: &Interval, d: DelayRep) -> bool {
        if d.is_infinity() {
            return false;
        }

        let value = u64::from(d.half_units());
        let lower = match iv.lower {
            Endpoint::Finite(n) => {
                let n2 = u64::from(n) * 2;
                if iv.lower_inclusive { n2 } else { n2 + 1 }
            }
            Endpoint::Infinity => return false,
        };
        let upper = match iv.upper {
            Endpoint::Infinity => None,
            Endpoint::Finite(n) => {
                let n2 = u64::from(n) * 2;
                Some(if iv.upper_inclusive { n2 } else { n2 - 1 })
            }
        };

        if value < lower {
            return false;
        }
        match upper {
            Some(max) => value <= max,
            None => true,
        }
    }

    fn interval_strategy() -> impl Strategy<Value = Interval> {
        (
            any::<bool>(),
            0u32..40u32,
            any::<bool>(),
            prop_oneof![Just(None), (0u32..40u32).prop_map(Some)],
        )
            .prop_filter_map("valid interval", |(li, l, ui, upper)| {
                Interval::from_bounds(li, l, ui, upper).ok()
            })
    }

    #[test]
    fn contains_open_interval_boundaries() {
        let iv = Interval::open(5, 7).unwrap();
        assert!(!iv.contains(DelayRep::from_integer(5)));
        assert!(iv.contains(DelayRep::from_half_units(11)));
        assert!(iv.contains(DelayRep::from_integer(6)));
        assert!(!iv.contains(DelayRep::from_integer(7)));
    }

    #[test]
    fn contains_single_point_interval_only_integer_point() {
        let iv = Interval::closed(3, 3).unwrap();
        assert!(iv.contains(DelayRep::from_integer(3)));
        assert!(!iv.contains(DelayRep::from_half_units(5)));
        assert!(!iv.contains(DelayRep::from_half_units(7)));
    }

    #[test]
    fn constructors_reject_empty_equal_endpoints() {
        assert_eq!(Interval::open(4, 4), Err(IntervalError::Empty));
        assert_eq!(
            Interval::left_open_right_closed(4, 4),
            Err(IntervalError::Empty)
        );
        assert_eq!(
            Interval::left_closed_right_open(4, 4),
            Err(IntervalError::Empty)
        );
    }

    #[test]
    fn constructors_reject_invalid_bounds() {
        assert_eq!(
            Interval::closed(7, 4),
            Err(IntervalError::LowerExceedsUpper { lower: 7, upper: 4 })
        );
        assert_eq!(
            Interval::from_bounds(true, 1, true, None),
            Err(IntervalError::InclusiveInfiniteUpper)
        );
    }

    #[test]
    fn intersects_and_disjoint_boundary_behavior() {
        let a = Interval::closed(0, 1).unwrap();
        let b = Interval::left_open_right_closed(1, 2).unwrap(); // (1,2]
        let c = Interval::closed(1, 2).unwrap();

        assert!(a.is_disjoint(&b));
        assert!(!a.intersects(&b));
        assert!(a.intersects(&c));
        assert!(!a.is_disjoint(&c));
    }

    #[test]
    fn merge_adjacent_or_overlapping_intervals() {
        let a = Interval::left_closed_right_open(0, 1).unwrap(); // [0,1)
        let b = Interval::left_closed_right_open(1, 2).unwrap(); // [1,2)
        let merged = a.try_merge_adjacent(&b).unwrap();
        assert_eq!(merged.to_string(), "[0,2)");

        let c = Interval::left_closed_right_open(0, 1).unwrap(); // [0,1)
        let d = Interval::left_open_right_closed(1, 2).unwrap(); // (1,2]
        assert!(c.try_merge_adjacent(&d).is_none());
    }

    #[test]
    fn display_is_stable_and_compact() {
        assert_eq!(
            Interval::left_closed_right_open(2, 4).unwrap().to_string(),
            "[2,4)"
        );
        assert_eq!(
            Interval::from_bounds(false, 5, false, None)
                .unwrap()
                .to_string(),
            "(5,∞)"
        );
        assert_eq!(Interval::closed(0, 0).unwrap().to_string(), "[0,0]");
    }

    #[test]
    fn from_str_accepts_ascii_plus_and_unicode_infinity() {
        let ascii: Interval = "[5,+)".parse().unwrap();
        let unicode: Interval = "(1,∞)".parse().unwrap();

        assert_eq!(ascii, Interval::from_bounds(true, 5, false, None).unwrap());
        assert_eq!(
            unicode,
            Interval::from_bounds(false, 1, false, None).unwrap()
        );
    }

    #[test]
    fn from_str_rejects_malformed_inputs() {
        assert_eq!(
            "[a,1)".parse::<Interval>(),
            Err(IntervalError::InvalidSyntax("[a,1)".to_string()))
        );
        assert_eq!(
            "0,1".parse::<Interval>(),
            Err(IntervalError::InvalidSyntax("0,1".to_string()))
        );
    }

    #[test]
    fn infinity_delay_is_not_contained() {
        let iv = Interval::from_bounds(true, 0, false, None).unwrap();
        assert!(!iv.contains(DelayRep::INFINITY));
    }

    proptest! {
        #[test]
        fn prop_contains_matches_half_range_interpretation(
            iv in interval_strategy(),
            d in prop_oneof![
                (0u32..160u32).prop_map(DelayRep::from_half_units),
                Just(DelayRep::INFINITY),
            ],
        ) {
            prop_assert_eq!(iv.contains(d), expected_contains(&iv, d));
        }

        #[test]
        fn prop_intersects_is_symmetric(
            a in interval_strategy(),
            b in interval_strategy(),
        ) {
            prop_assert_eq!(a.intersects(&b), b.intersects(&a));
        }

        #[test]
        fn prop_disjoint_is_negation_of_intersects(
            a in interval_strategy(),
            b in interval_strategy(),
        ) {
            prop_assert_eq!(a.is_disjoint(&b), !a.intersects(&b));
        }
    }
}
