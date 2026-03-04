// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use clap::Args;
use learn_arta_core::{DotOptions, read_arta_json_file};

use crate::error::CliError;

/// Render an ARTA JSON document as DOT.
#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub(crate) struct DotArgs {
    /// Path to the input ARTA JSON file.
    pub(crate) input_json: PathBuf,
    /// Write DOT output to a file instead of stdout.
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,
    /// Preserve Unicode labels such as `⊤`, `⊥`, and `∞`.
    #[arg(long)]
    pub(crate) unicode: bool,
    /// Omit the plaintext initial-formula annotation node.
    #[arg(long)]
    pub(crate) hide_init_node: bool,
}

impl DotArgs {
    pub(crate) fn run(&self) -> Result<(), CliError> {
        let arta = read_arta_json_file(&self.input_json)?;
        let dot = arta.to_dot_string_with(&DotOptions {
            unicode: self.unicode,
            show_init_node: !self.hide_init_node,
        });

        if let Some(path) = &self.output {
            fs::write(path, dot).map_err(|source| CliError::write_file(path.clone(), source))?;
        } else {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(dot.as_bytes())
                .map_err(CliError::WriteStdout)?;
        }

        Ok(())
    }
}
