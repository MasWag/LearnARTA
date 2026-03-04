// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::BTreeSet,
    io::{self, Write},
    path::PathBuf,
};

use clap::Args;
use learn_arta_core::{DagStateFormula, TimedWord, read_arta_json_file_document};
use learn_arta_oracles::WhiteBoxEqOracle;
use learn_arta_traits::EquivalenceOracle;

use crate::error::CliError;

/// Compare two ARTA JSON documents for semantic equivalence.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub(crate) struct CompareArgs {
    /// Path to the left ARTA JSON file.
    pub(crate) left_json: PathBuf,
    /// Path to the right ARTA JSON file.
    pub(crate) right_json: PathBuf,
}

impl CompareArgs {
    pub(crate) fn run(&self) -> Result<(), CliError> {
        let left_document = read_arta_json_file_document(&self.left_json)?;
        let right_document = read_arta_json_file_document(&self.right_json)?;
        let left_sigma = normalize_sigma(&left_document.sigma);
        let right_sigma = normalize_sigma(&right_document.sigma);

        if left_sigma != right_sigma {
            return write_compare_output(&format!(
                "different\nreason: alphabet mismatch\nleft_sigma: {}\nright_sigma: {}\n",
                format_sigma_set(&left_sigma),
                format_sigma_set(&right_sigma),
            ));
        }

        let left = left_document.arta;
        let right = right_document.arta;
        let empty_word = TimedWord::<String>::empty();
        let left_accepts_empty = left.accepts(&empty_word);
        let right_accepts_empty = right.accepts(&empty_word);

        if left_accepts_empty != right_accepts_empty {
            return write_compare_output(&difference_output(
                &empty_word,
                left_accepts_empty,
                right_accepts_empty,
            ));
        }

        if left_sigma.is_empty() {
            return write_compare_output("equivalent\n");
        }

        let sigma_vec = left_sigma.iter().cloned().collect();
        let mut eq = WhiteBoxEqOracle::<_, DagStateFormula>::try_new(left.clone(), sigma_vec)
            .map_err(CliError::from_compare_setup_error)?;
        let maybe_counterexample = eq
            .find_counterexample(&right)
            .map_err(CliError::from_compare_setup_error)?;

        match maybe_counterexample {
            Some(witness) => write_compare_output(&difference_output(
                &witness,
                left.accepts(&witness),
                right.accepts(&witness),
            )),
            None => write_compare_output("equivalent\n"),
        }
    }
}

fn normalize_sigma(sigma: &[String]) -> BTreeSet<String> {
    sigma.iter().cloned().collect()
}

fn format_sigma_set(sigma: &BTreeSet<String>) -> String {
    let mut out = String::from("[");
    for (index, symbol) in sigma.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("{symbol:?}"));
    }
    out.push(']');
    out
}

fn difference_output(
    witness: &TimedWord<String>,
    left_accepts: bool,
    right_accepts: bool,
) -> String {
    format!(
        "different\nwitness: {witness}\nleft_accepts: {left_accepts}\nright_accepts: {right_accepts}\n"
    )
}

fn write_compare_output(output: &str) -> Result<(), CliError> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(output.as_bytes())
        .map_err(CliError::WriteStdout)
}
