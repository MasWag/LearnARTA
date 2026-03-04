// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Time domain representation for ARTA.
//!
//! Delays are stored internally as `DelayRep`, a half-integer representation
//! using `u32` half-units: the value `n` represents `n / 2.0`.
//! This allows exact representation of integers (even values) and half-integers
//! (odd values) without floating-point arithmetic.
//!
//! Raw `f64` inputs are normalized onto this lattice: finite integers are kept
//! exact, finite non-integers map to `floor(d) + 0.5`, and `+∞` maps to
//! [`DelayRep::INFINITY`].
//!
//! Finite delays use raw values in `0..u32::MAX`. The sentinel value
//! `u32::MAX` is reserved as [`DelayRep::INFINITY`].

pub mod delay;
pub mod interval;
pub mod normalize;

pub use delay::DelayRep;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeError;
    use proptest::prelude::*;

    #[test]
    fn test_from_f64_zero() {
        let d = DelayRep::try_from_f64(0.0).unwrap();
        assert_eq!(d.floor_int(), Some(0));
        assert!(d.is_integer());
        assert_eq!(d.to_f64(), 0.0);
    }

    #[test]
    fn test_from_f64_one_and_one_point_zero() {
        let d1 = DelayRep::try_from_f64(1_f64).unwrap();
        let d2 = DelayRep::try_from_f64(1.0_f64).unwrap();
        assert!(d1.is_integer());
        assert!(d2.is_integer());
        assert_eq!(d1.floor_int(), Some(1));
        assert_eq!(d2.floor_int(), Some(1));
        assert_eq!(d1.half_units(), 2);
        assert_eq!(d2.half_units(), 2);
    }

    #[test]
    fn test_from_f64_non_integer_maps_to_floor_plus_half() {
        let d = DelayRep::try_from_f64(1.2).unwrap();
        assert_eq!(d.to_f64(), 1.5);
        assert_eq!(d.floor_int(), Some(1));
        assert!(d.is_half_integer());

        let d = DelayRep::try_from_f64(1.999).unwrap();
        assert_eq!(d.to_f64(), 1.5);
        assert_eq!(d.floor_int(), Some(1));
        assert!(d.is_half_integer());
    }

    #[test]
    fn test_ceil_zero_is_unchanged() {
        assert_eq!(DelayRep::ZERO.ceil(), DelayRep::ZERO);
    }

    #[test]
    fn test_ceil_integer_delay_is_unchanged() {
        let d = DelayRep::from_integer(7);
        assert_eq!(d.ceil(), d);
    }

    #[test]
    fn test_ceil_half_integer_rounds_up() {
        assert_eq!(
            DelayRep::from_floor_plus_half(7).ceil(),
            DelayRep::from_integer(8)
        );
    }

    #[test]
    fn test_ceil_infinity_is_unchanged() {
        assert_eq!(DelayRep::INFINITY.ceil(), DelayRep::INFINITY);
    }

    #[test]
    fn test_from_f64_nan() {
        assert_eq!(DelayRep::try_from_f64(f64::NAN), Err(TimeError::NaN));
    }

    #[test]
    fn test_from_f64_negative() {
        assert!(matches!(
            DelayRep::try_from_f64(-0.1),
            Err(TimeError::Negative(_))
        ));
    }

    #[test]
    fn test_from_f64_infinity() {
        let d = DelayRep::try_from_f64(f64::INFINITY).unwrap();
        assert!(d.is_infinity());
    }

    #[test]
    fn test_from_f64_huge_values_behavior() {
        let max_finite_integer = (u32::MAX / 2) as f64;
        let largest_integer = DelayRep::try_from_f64(max_finite_integer).unwrap();
        assert!(largest_integer.is_integer());
        assert_eq!(largest_integer.half_units(), u32::MAX - 1);

        let too_large_integer = max_finite_integer + 1.0;
        assert!(matches!(
            DelayRep::try_from_f64(too_large_integer),
            Err(TimeError::TooLarge(v)) if v == too_large_integer
        ));

        let largest_non_integer = DelayRep::try_from_f64(max_finite_integer - 0.1).unwrap();
        assert!(largest_non_integer.is_half_integer());
        assert_eq!(largest_non_integer.half_units(), u32::MAX - 2);

        let too_large_non_integer = max_finite_integer + 0.25;
        assert!(matches!(
            DelayRep::try_from_f64(too_large_non_integer),
            Err(TimeError::TooLarge(v)) if v == too_large_non_integer
        ));
    }

    #[test]
    fn test_from_floor_plus_half_regular_values() {
        assert_eq!(
            DelayRep::from_floor_plus_half(0),
            DelayRep::from_half_units(1)
        );
        assert_eq!(
            DelayRep::from_floor_plus_half(1),
            DelayRep::from_half_units(3)
        );
        assert_eq!(
            DelayRep::from_floor_plus_half(42),
            DelayRep::from_half_units(85)
        );
    }

    #[test]
    fn test_large_constructor_inputs_wrap_consistently() {
        let largest_finite_floor = (u32::MAX - 2) / 2;
        let largest_finite = DelayRep::from_floor_plus_half(largest_finite_floor);
        assert!(largest_finite.is_half_integer());
        assert_eq!(largest_finite.half_units(), u32::MAX - 2);

        let colliding_floor = u32::MAX / 2;
        assert!(DelayRep::from_floor_plus_half(colliding_floor).is_infinity());

        let overflowing_floor = colliding_floor.saturating_add(1);
        assert_eq!(
            DelayRep::from_floor_plus_half(overflowing_floor),
            DelayRep::from_half_units(1)
        );

        let overflowing_integer = colliding_floor.saturating_add(1);
        assert_eq!(DelayRep::from_integer(overflowing_integer), DelayRep::ZERO);
    }

    proptest! {
        #[test]
        fn prop_try_from_f64_integer_maps_exactly(n in 0u32..10_000u32) {
            let d = DelayRep::try_from_f64(n as f64).unwrap();
            prop_assert!(d.is_integer());
            prop_assert_eq!(d.floor_int(), Some(n));
            prop_assert_eq!(d.half_units(), n * 2);
        }

        #[test]
        fn prop_try_from_f64_non_integer_maps_to_floor_plus_half(
            n in 0u32..10_000u32,
            frac_milli in 1u16..1000u16
        ) {
            let x = n as f64 + f64::from(frac_milli) / 1000.0;
            prop_assume!(x > n as f64 && x < n as f64 + 1.0);
            let d = DelayRep::try_from_f64(x).unwrap();
            prop_assert!(d.is_half_integer());
            prop_assert_eq!(d.floor_int(), Some(n));
            prop_assert_eq!(d.half_units(), n * 2 + 1);
        }

        #[test]
        fn prop_ceil_rounds_to_integer_delay(d in any::<u32>().prop_map(DelayRep::from_half_units)) {
            if d.is_infinity() {
                prop_assert_eq!(d.ceil(), DelayRep::INFINITY);
            } else {
                let ceil = d.ceil();
                prop_assert!(ceil.is_integer());
                prop_assert!(ceil >= d);
                if d.is_integer() {
                    prop_assert_eq!(ceil, d);
                }
            }
        }
    }
}
