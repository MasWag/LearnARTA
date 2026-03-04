// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{io, path::PathBuf};

use learn_arta_core::ArtaJsonError;
use learn_arta_oracles::WhiteBoxEqOracleError;
use thiserror::Error;

/// Errors returned by the LearnARTA CLI.
#[derive(Debug, Error)]
pub(crate) enum CliError {
    /// The input ARTA JSON file could not be read or parsed.
    #[error(transparent)]
    ArtaJson(#[from] ArtaJsonError),
    /// DOT output could not be written to the requested destination.
    #[error("failed to write DOT output to {path}: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// DOT output could not be written to stdout.
    #[error("failed to write DOT output to stdout: {0}")]
    WriteStdout(#[source] io::Error),
    /// Exact learning setup is invalid.
    #[error("invalid exact learning configuration: {reason}")]
    ExactLearningConfiguration { reason: String },
    /// Compare setup is invalid.
    #[error("invalid compare configuration: {reason}")]
    CompareConfiguration { reason: String },
    /// Exact learning failed after setup.
    #[error("exact learning failed: {reason}")]
    ExactLearningFailed { reason: String },
}

impl CliError {
    pub(crate) fn write_file(path: PathBuf, source: io::Error) -> Self {
        Self::WriteFile { path, source }
    }

    pub(crate) fn exact_learning_failed(reason: String) -> Self {
        Self::ExactLearningFailed { reason }
    }

    pub(crate) fn from_whitebox_setup_error(error: WhiteBoxEqOracleError) -> Self {
        match error {
            WhiteBoxEqOracleError::EmptyAlphabet => Self::ExactLearningConfiguration {
                reason: "alphabet must not be empty".to_string(),
            },
            _ => Self::ExactLearningConfiguration {
                reason: error.to_string(),
            },
        }
    }

    pub(crate) fn from_compare_setup_error(error: WhiteBoxEqOracleError) -> Self {
        match error {
            WhiteBoxEqOracleError::EmptyAlphabet => Self::CompareConfiguration {
                reason: "alphabet must not be empty".to_string(),
            },
            _ => Self::CompareConfiguration {
                reason: error.to_string(),
            },
        }
    }
}
