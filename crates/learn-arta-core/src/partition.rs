// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Guard-partition construction from observed delays.
//!
//! This module builds one interval per observed delay from a finite,
//! strictly increasing delay sequence. Delays may share a floor only in the
//! allowed `k, k + 0.5` pattern. The resulting intervals are used during
//! guard inference.

use std::collections::HashMap;

use crate::time::{DelayRep, interval::Interval};
use thiserror::Error;

/// Errors returned by [`infer_guard_intervals_from_delays`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum PartitionError {
    /// Empty delay list.
    #[error("partition input cannot be empty")]
    EmptyInput,
    /// Delay `+∞` is not allowed in partition input.
    #[error("delay at index {index} is +∞; partition input must be finite")]
    InfiniteDelay {
        /// Index of the offending delay.
        index: usize,
    },
    /// Delays are not strictly increasing.
    #[error(
        "delays are not strictly increasing at index {index}: previous {prev} is not less than {next}"
    )]
    NotStrictlyIncreasing {
        /// Index of the right element in the violating pair.
        index: usize,
        /// Previous delay.
        prev: DelayRep,
        /// Next delay.
        next: DelayRep,
    },
    /// Same-floor observations violate the allowed integer-then-half-integer pattern.
    #[error(
        "floor-distinctness violated for floor {floor}: indices {first_index} and {second_index} do not form an allowed integer-then-half-integer pair"
    )]
    NotFloorDistinct {
        /// Duplicated floor value.
        floor: u32,
        /// First index where this floor appears.
        first_index: usize,
        /// Second index where this floor appears.
        second_index: usize,
    },
    /// One step of the interval construction produced an invalid interval.
    #[error("invalid partition interval at index {index}: {details}")]
    InvalidInterval {
        /// Index of interval in the output vector.
        index: usize,
        /// Human-readable invalidity reason.
        details: String,
    },
}

/// Infer guard-partition intervals from an ordered delay sequence.
///
/// Input must be finite, strictly increasing, and satisfy the documented
/// same-floor constraint.
///
/// The returned vector has the same length and order as `delays`.
pub fn infer_guard_intervals_from_delays(
    delays: &[DelayRep],
) -> Result<Vec<Interval>, PartitionError> {
    validate_delays(delays)?;
    let n = delays.len();

    if n == 1 {
        let only = build_interval(0, true, DelayRep::ZERO, false, None)?;
        let out = vec![only];
        ensure_ok_invariants(delays, &out)?;
        return Ok(out);
    }

    let mut out = Vec::with_capacity(n);

    // First interval when there is more than one observed delay.
    let d1 = delays[0];
    let first = if d1.is_integer() {
        build_interval(0, true, DelayRep::ZERO, true, Some(d1))?
    } else {
        build_interval(0, true, DelayRep::ZERO, false, Some(d1.ceil()))?
    };
    out.push(first);

    // Middle intervals depend on the previous delay and the current delay.
    for i in 1..(n - 1) {
        let d_prev = delays[i - 1];
        let d_i = delays[i];

        let interval = match (d_prev.is_integer(), d_i.is_integer()) {
            // Starts just after the previous delay and ends at the current delay.
            (true, true) => build_interval(i, false, d_prev, true, Some(d_i))?,
            // Starts at the next integer after the previous delay and ends at the current delay.
            (false, true) => build_interval(i, true, d_prev.ceil(), true, Some(d_i))?,
            // Starts just after the previous delay and ends just before the next integer above the current delay.
            (true, false) => build_interval(i, false, d_prev, false, Some(d_i.ceil()))?,
            // Starts at the next integer after the previous delay and ends just before the next integer above the current delay.
            (false, false) => build_interval(i, true, d_prev.ceil(), false, Some(d_i.ceil()))?,
        };
        out.push(interval);
    }

    // Final interval when there is more than one observed delay.
    let d_prev = delays[n - 2];
    let last = if d_prev.is_integer() {
        build_interval(n - 1, false, d_prev, false, None)?
    } else {
        build_interval(n - 1, true, d_prev.ceil(), false, None)?
    };
    out.push(last);

    ensure_ok_invariants(delays, &out)?;
    Ok(out)
}

fn validate_delays(delays: &[DelayRep]) -> Result<(), PartitionError> {
    if delays.is_empty() {
        return Err(PartitionError::EmptyInput);
    }

    for (idx, d) in delays.iter().copied().enumerate() {
        if d.is_infinity() {
            return Err(PartitionError::InfiniteDelay { index: idx });
        }
    }

    for i in 1..delays.len() {
        let prev = delays[i - 1];
        let next = delays[i];
        if prev >= next {
            return Err(PartitionError::NotStrictlyIncreasing {
                index: i,
                prev,
                next,
            });
        }
    }

    let mut floor_first_observation = HashMap::<u32, (usize, DelayRep)>::new();
    let mut floor_duplicate_seen = HashMap::<u32, ()>::new();
    for (idx, d) in delays.iter().copied().enumerate() {
        let floor = d
            .floor_int()
            .ok_or(PartitionError::InfiniteDelay { index: idx })?;
        if let Some((first_index, first_delay)) = floor_first_observation.get(&floor).copied() {
            let duplicate_allowed = !floor_duplicate_seen.contains_key(&floor)
                && first_delay.is_integer()
                && d.is_half_integer();
            if !duplicate_allowed {
                return Err(PartitionError::NotFloorDistinct {
                    floor,
                    first_index,
                    second_index: idx,
                });
            }
            floor_duplicate_seen.insert(floor, ());
        } else {
            floor_first_observation.insert(floor, (idx, d));
        }
    }

    Ok(())
}

fn build_interval(
    index: usize,
    lower_inclusive: bool,
    lower: DelayRep,
    upper_inclusive: bool,
    upper: Option<DelayRep>,
) -> Result<Interval, PartitionError> {
    let lower = integer_bound(index, "lower", lower)?;
    let upper = upper
        .map(|endpoint| integer_bound(index, "upper", endpoint))
        .transpose()?;
    Interval::from_bounds(lower_inclusive, lower, upper_inclusive, upper).map_err(|details| {
        PartitionError::InvalidInterval {
            index,
            details: details.to_string(),
        }
    })
}

fn integer_bound(index: usize, side: &str, delay: DelayRep) -> Result<u32, PartitionError> {
    if delay.is_infinity() {
        return Err(PartitionError::InvalidInterval {
            index,
            details: format!("{side} bound cannot be +∞"),
        });
    }
    if !delay.is_integer() {
        return Err(PartitionError::InvalidInterval {
            index,
            details: format!("{side} bound must be integer-valued, got {delay}"),
        });
    }

    delay
        .floor_int()
        .ok_or_else(|| PartitionError::InvalidInterval {
            index,
            details: format!("failed to read finite integer {side} bound"),
        })
}

fn ensure_ok_invariants(delays: &[DelayRep], intervals: &[Interval]) -> Result<(), PartitionError> {
    for (index, (delay, interval)) in delays.iter().copied().zip(intervals.iter()).enumerate() {
        if !interval.contains(delay) {
            return Err(PartitionError::InvalidInterval {
                index,
                details: format!("interval {interval} does not contain delay {delay}"),
            });
        }
    }

    for i in 0..intervals.len().saturating_sub(1) {
        if !intervals[i].is_disjoint(&intervals[i + 1]) {
            return Err(PartitionError::InvalidInterval {
                index: i,
                details: format!(
                    "adjacent intervals overlap: {} and {}",
                    intervals[i],
                    intervals[i + 1]
                ),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn d_int(n: u32) -> DelayRep {
        DelayRep::from_integer(n)
    }

    fn d_half(n: u32) -> DelayRep {
        DelayRep::from_floor_plus_half(n)
    }

    fn p(li: bool, l: DelayRep, ui: bool, u: Option<DelayRep>) -> Interval {
        let lower = l.floor_int().expect("expected finite integer lower bound");
        let upper = u.map(|bound| {
            bound
                .floor_int()
                .expect("expected finite integer upper bound")
        });
        Interval::from_bounds(li, lower, ui, upper).expect("valid expected partition interval")
    }

    fn assert_universal_ok_invariants(delays: &[DelayRep], intervals: &[Interval]) {
        assert_eq!(intervals.len(), delays.len());
        for (i, (delay, interval)) in delays.iter().copied().zip(intervals.iter()).enumerate() {
            assert!(
                interval.contains(delay),
                "I[{i}]={interval} must contain d[{i}]={delay}"
            );
        }
        for i in 0..intervals.len().saturating_sub(1) {
            assert!(
                intervals[i].is_disjoint(&intervals[i + 1]),
                "adjacent intervals overlap: I[{i}]={} and I[{}]={}",
                intervals[i],
                i + 1,
                intervals[i + 1]
            );
        }
    }

    #[test]
    fn precondition_empty_input() {
        let err = infer_guard_intervals_from_delays(&[]).unwrap_err();
        assert_eq!(err, PartitionError::EmptyInput);
    }

    #[test]
    fn precondition_not_strictly_increasing() {
        let delays = [d_int(1), d_int(1), d_int(2)];
        let err = infer_guard_intervals_from_delays(&delays).unwrap_err();
        assert!(matches!(
            err,
            PartitionError::NotStrictlyIncreasing { index: 1, .. }
        ));
    }

    #[test]
    fn precondition_allows_integer_then_half_integer_same_floor() {
        let delays = [d_int(1), d_half(1)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![
            p(true, d_int(0), true, Some(d_int(1))),
            p(false, d_int(1), false, None),
        ];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn precondition_rejects_infinity() {
        let delays = [d_int(0), DelayRep::INFINITY];
        let err = infer_guard_intervals_from_delays(&delays).unwrap_err();
        assert!(matches!(err, PartitionError::InfiniteDelay { index: 1 }));
    }

    #[test]
    fn n1_returns_zero_to_infinity() {
        let delays = [d_half(3)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![p(true, d_int(0), false, None)];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn n2_first_delay_integer() {
        let delays = [d_int(2), d_half(5)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![
            p(true, d_int(0), true, Some(d_int(2))),
            p(false, d_int(2), false, None),
        ];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn n2_first_delay_non_integer() {
        let delays = [d_half(2), d_int(5)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![
            p(true, d_int(0), false, Some(d_int(3))),
            p(true, d_int(3), false, None),
        ];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn repeated_floor_is_allowed_across_middle_and_last_cases() {
        let delays = [d_int(1), d_half(1), d_int(2)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![
            p(true, d_int(0), true, Some(d_int(1))),
            p(false, d_int(1), false, Some(d_int(2))),
            p(true, d_int(2), false, None),
        ];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn middle_case_prev_integer_curr_integer() {
        let delays = [d_int(0), d_int(2), d_int(4)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![
            p(true, d_int(0), true, Some(d_int(0))),
            p(false, d_int(0), true, Some(d_int(2))),
            p(false, d_int(2), false, None),
        ];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn middle_case_prev_non_integer_curr_integer() {
        let delays = [d_half(0), d_int(2), d_half(4)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![
            p(true, d_int(0), false, Some(d_int(1))),
            p(true, d_int(1), true, Some(d_int(2))),
            p(false, d_int(2), false, None),
        ];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn middle_case_prev_integer_curr_non_integer() {
        let delays = [d_int(0), d_half(2), d_half(5)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![
            p(true, d_int(0), true, Some(d_int(0))),
            p(false, d_int(0), false, Some(d_int(3))),
            p(true, d_int(3), false, None),
        ];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn middle_case_prev_non_integer_curr_non_integer() {
        let delays = [d_half(0), d_half(2), d_half(5)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![
            p(true, d_int(0), false, Some(d_int(1))),
            p(true, d_int(1), false, Some(d_int(3))),
            p(true, d_int(3), false, None),
        ];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn middle_case_prev_non_integer_curr_integer_can_be_singleton() {
        let delays = [d_half(0), d_int(1), d_int(2)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        let expected = vec![
            p(true, d_int(0), false, Some(d_int(1))),
            p(true, d_int(1), true, Some(d_int(1))),
            p(false, d_int(1), false, None),
        ];
        assert_eq!(got, expected);
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn last_case_prev_integer() {
        let delays = [d_int(1), d_int(3)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        assert_eq!(
            got[1],
            p(false, d_int(1), false, None),
            "last interval must start just after the previous observed delay"
        );
        assert_universal_ok_invariants(&delays, &got);
    }

    #[test]
    fn last_case_prev_non_integer() {
        let delays = [d_half(1), d_int(5)];
        let got = infer_guard_intervals_from_delays(&delays).unwrap();
        assert_eq!(
            got[1],
            p(true, d_int(2), false, None),
            "last interval must start at ceil(previous delay) when that delay is non-integer"
        );
        assert_universal_ok_invariants(&delays, &got);
    }

    fn construct_delays(first_half_units: u32, gaps: &[u32]) -> Vec<DelayRep> {
        let mut half_units = Vec::with_capacity(gaps.len() + 1);
        let mut current = first_half_units;
        half_units.push(current);
        for gap in gaps {
            current += *gap;
            half_units.push(current);
        }

        half_units
            .into_iter()
            .map(DelayRep::from_half_units)
            .collect()
    }

    fn valid_by_construction_delays_strategy() -> impl Strategy<Value = Vec<DelayRep>> {
        (1usize..=8).prop_flat_map(|len| {
            let gaps_len = len.saturating_sub(1);
            (0u32..60u32, prop::collection::vec(1u32..=4u32, gaps_len))
                .prop_map(|(first_half_units, gaps)| construct_delays(first_half_units, &gaps))
        })
    }

    proptest! {
        #[test]
        fn prop_ok_results_satisfy_universal_invariants(
            delays in valid_by_construction_delays_strategy()
        ) {
            match infer_guard_intervals_from_delays(&delays) {
                Ok(intervals) => {
                    prop_assert_eq!(intervals.len(), delays.len());
                    for (delay, interval) in delays.iter().copied().zip(intervals.iter()) {
                        prop_assert!(interval.contains(delay));
                    }
                    for i in 0..intervals.len().saturating_sub(1) {
                        prop_assert!(intervals[i].is_disjoint(&intervals[i + 1]));
                    }
                }
                Err(other) => prop_assert!(false, "unexpected error for generated input: {other:?}"),
            }
        }
    }
}
