// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Error types for the learning algorithm.

use learn_arta_core::TimeError;
use thiserror::Error;

use crate::{
    CohesionStepError, EvidenceAfaError, HypothesisArtaError, TableError,
    observation_table::TableQueryError,
};

/// Errors that can occur during active ARTA learning.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LearnError<A, MembershipOracleError, EquivalenceOracleError>
where
    MembershipOracleError: std::error::Error + Send + Sync + 'static,
    EquivalenceOracleError: std::error::Error + Send + Sync + 'static,
{
    /// Membership query failed while filling or refining the observation table.
    #[error("membership query failed: {source}")]
    MembershipQuery {
        /// Wrapped oracle error.
        #[source]
        source: TableQueryError<MembershipOracleError>,
    },
    /// Equivalence query failed while validating a hypothesis.
    #[error("equivalence query failed: {source}")]
    EquivalenceQuery {
        /// Wrapped oracle error.
        #[source]
        source: EquivalenceOracleError,
    },
    /// Raw counterexample normalization failed before table refinement.
    #[error("counterexample normalization failed: {source}")]
    CounterexampleNormalization {
        /// Wrapped time-normalization error.
        #[source]
        source: TimeError,
    },
    /// Cohesion repair step failed.
    #[error("cohesion repair failed: {source}")]
    CohesionRepair {
        /// Wrapped cohesion-step error.
        #[source]
        source: CohesionStepError<A, MembershipOracleError>,
    },
    /// Evidence automaton construction failed.
    #[error("failed to build evidence automaton: {source}")]
    EvidenceAutomatonBuild {
        /// Wrapped evidence-construction error.
        #[source]
        source: EvidenceAfaError<A>,
    },
    /// Hypothesis ARTA construction from evidence failed.
    #[error("failed to build hypothesis ARTA: {source}")]
    HypothesisBuild {
        /// Wrapped hypothesis-construction error.
        #[source]
        source: HypothesisArtaError<A>,
    },
    /// Observation table invariants were violated.
    #[error("observation table invariant failure: {source}")]
    TableInvariant {
        /// Wrapped table invariant error.
        #[source]
        source: TableError,
    },
}
