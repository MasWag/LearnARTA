// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::BasisMinimizationError;
use std::time::Duration;

/// Configuration for approximate MILP basis minimization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ApproxMilpConfig {
    /// Relative MIP gap tolerated by the approximate solver.
    pub relative_gap: f32,
    /// Wall-clock time limit for a bounded approximate solve.
    pub time_limit: Duration,
}

impl ApproxMilpConfig {
    /// Default relative MIP gap used by the approximate MILP backend.
    pub const DEFAULT_RELATIVE_GAP: f32 = 0.01;
    const DEFAULT_TIME_LIMIT_SECS_INT: u64 = 5;
    /// Default approximate-solver time limit in seconds as a floating-point value.
    pub const DEFAULT_TIME_LIMIT_SECS: f64 = Self::DEFAULT_TIME_LIMIT_SECS_INT as f64;
    /// Default approximate-solver wall-clock time limit.
    pub const DEFAULT_TIME_LIMIT: Duration = Duration::from_secs(Self::DEFAULT_TIME_LIMIT_SECS_INT);

    pub(super) fn validate(self) -> Result<(), BasisMinimizationError> {
        if self.relative_gap.is_nan() {
            return Err(BasisMinimizationError::InvalidApproxMilpConfig {
                field: "relative_gap",
                reason: "must not be NaN".to_string(),
            });
        }
        if self.relative_gap.is_infinite() {
            return Err(BasisMinimizationError::InvalidApproxMilpConfig {
                field: "relative_gap",
                reason: "must be finite".to_string(),
            });
        }
        if self.relative_gap.is_sign_negative() {
            return Err(BasisMinimizationError::InvalidApproxMilpConfig {
                field: "relative_gap",
                reason: "must be greater than or equal to zero".to_string(),
            });
        }

        Ok(())
    }
}

impl Default for ApproxMilpConfig {
    fn default() -> Self {
        Self {
            relative_gap: Self::DEFAULT_RELATIVE_GAP,
            time_limit: Self::DEFAULT_TIME_LIMIT,
        }
    }
}

/// Built-in basis minimizers exposed by the public API.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BasisMinimization {
    /// Preserve the existing greedy "remove the first redundant basis word" behavior.
    Greedy,
    /// Compute a minimum-cardinality basis with an exact MILP set-cover encoding.
    ExactMilp,
    /// Compute a smaller basis with a bounded MILP solve.
    ApproxMilp(ApproxMilpConfig),
}

impl BasisMinimization {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Greedy => "greedy",
            Self::ExactMilp => "exact-milp",
            Self::ApproxMilp(_) => "approx-milp",
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for BasisMinimization {
    fn default() -> Self {
        #[cfg(feature = "milp")]
        {
            Self::ApproxMilp(ApproxMilpConfig::default())
        }

        #[cfg(not(feature = "milp"))]
        {
            Self::Greedy
        }
    }
}
