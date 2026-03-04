// SPDX-License-Identifier: Apache-2.0 OR MIT

mod cmd;
mod error;

use std::{io::Write, process::ExitCode};

use clap::{Parser, Subcommand};
use env_logger::{Builder, Env};
use log::{LevelFilter, error};
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    cmd::{compare::CompareArgs, dot::DotArgs, learn::LearnArgs},
    error::CliError,
};

#[derive(Debug, Parser)]
#[command(name = "learn-arta-cli")]
#[command(about = "Auxiliary CLI utilities for LearnARTA")]
#[command(subcommand_required = true, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
enum Commands {
    /// Compare two ARTA JSON files for semantic equivalence.
    Compare(CompareArgs),
    /// Render an ARTA JSON file as DOT.
    Dot(DotArgs),
    /// Learn an exact hypothesis against a target ARTA.
    Learn(LearnArgs),
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    init_logger(&cli);
    run_cli(cli)
}

fn run_cli(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Compare(args) => args.run(),
        Commands::Dot(args) => args.run(),
        Commands::Learn(args) => args.run(),
    }
}

fn init_logger(cli: &Cli) {
    let mut builder = match log_level_override(&cli.command) {
        Some(level) => {
            let mut builder = Builder::new();
            builder.filter_level(level);
            builder
        }
        None => Builder::from_env(Env::default().default_filter_or("info")),
    };

    builder.format(|buf, record| {
        writeln!(
            buf,
            "[{} {}] {}",
            format_log_timestamp(),
            record.level(),
            record.args()
        )
    });
    builder.init();
}

fn log_level_override(command: &Commands) -> Option<LevelFilter> {
    match command {
        Commands::Learn(args) => args.log_level_override(),
        Commands::Compare(_) | Commands::Dot(_) => None,
    }
}

fn format_log_timestamp() -> String {
    let format =
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]");
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    OffsetDateTime::now_utc()
        .to_offset(offset)
        .format(&format)
        .unwrap_or_else(|_| "1970-01-01 00:00:00.000".to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::{Parser, error::ErrorKind};

    use crate::cmd::learn::BasisMinimizationArg;

    use super::{Cli, Commands, CompareArgs, DotArgs, LearnArgs};

    #[test]
    fn parses_minimal_compare_command() {
        let cli = Cli::try_parse_from(["learn-arta-cli", "compare", "left.json", "right.json"])
            .expect("compare command should parse");

        assert_eq!(
            cli.command,
            Commands::Compare(CompareArgs {
                left_json: PathBuf::from("left.json"),
                right_json: PathBuf::from("right.json"),
            })
        );
    }

    #[test]
    fn compare_command_requires_both_input_paths() {
        let error = Cli::try_parse_from(["learn-arta-cli", "compare", "left.json"])
            .expect_err("missing second input should fail");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn parses_minimal_dot_command() {
        let cli = Cli::try_parse_from(["learn-arta-cli", "dot", "examples/small.json"])
            .expect("dot command should parse");

        assert_eq!(
            cli.command,
            Commands::Dot(DotArgs {
                input_json: PathBuf::from("examples/small.json"),
                output: None,
                unicode: false,
                hide_init_node: false,
            })
        );
    }

    #[test]
    fn parses_dot_command_with_all_flags() {
        let cli = Cli::try_parse_from([
            "learn-arta-cli",
            "dot",
            "examples/small.json",
            "--output",
            "out.dot",
            "--unicode",
            "--hide-init-node",
        ])
        .expect("dot command with flags should parse");

        assert_eq!(
            cli.command,
            Commands::Dot(DotArgs {
                input_json: PathBuf::from("examples/small.json"),
                output: Some(PathBuf::from("out.dot")),
                unicode: true,
                hide_init_node: true,
            })
        );
    }

    #[test]
    fn dot_command_requires_input_path() {
        let error = Cli::try_parse_from(["learn-arta-cli", "dot"])
            .expect_err("missing required input should fail");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn parses_minimal_learn_command() {
        let cli = Cli::try_parse_from(["learn-arta-cli", "learn", "examples/atomic-small.json"])
            .expect("learn command should parse");

        assert_eq!(
            cli.command,
            Commands::Learn(LearnArgs {
                input_json: PathBuf::from("examples/atomic-small.json"),
                output: None,
                name: None,
                quiet: false,
                debug: false,
                trace: false,
                basis_minimization: BasisMinimizationArg::default(),
                basis_mip_gap: None,
                basis_time_limit_secs: None,
            })
        );
    }

    #[test]
    fn parses_learn_command_with_quiet() {
        let cli = Cli::try_parse_from([
            "learn-arta-cli",
            "learn",
            "examples/atomic-small.json",
            "--quiet",
        ])
        .expect("learn command with quiet should parse");

        assert_eq!(
            cli.command,
            Commands::Learn(LearnArgs {
                input_json: PathBuf::from("examples/atomic-small.json"),
                output: None,
                name: None,
                quiet: true,
                debug: false,
                trace: false,
                basis_minimization: BasisMinimizationArg::default(),
                basis_mip_gap: None,
                basis_time_limit_secs: None,
            })
        );
    }

    #[test]
    fn parses_learn_command_with_debug() {
        let cli = Cli::try_parse_from([
            "learn-arta-cli",
            "learn",
            "examples/atomic-small.json",
            "--debug",
        ])
        .expect("learn command with debug should parse");

        assert_eq!(
            cli.command,
            Commands::Learn(LearnArgs {
                input_json: PathBuf::from("examples/atomic-small.json"),
                output: None,
                name: None,
                quiet: false,
                debug: true,
                trace: false,
                basis_minimization: BasisMinimizationArg::default(),
                basis_mip_gap: None,
                basis_time_limit_secs: None,
            })
        );
    }

    #[test]
    fn parses_learn_command_with_trace() {
        let cli = Cli::try_parse_from([
            "learn-arta-cli",
            "learn",
            "examples/atomic-small.json",
            "--trace",
        ])
        .expect("learn command with trace should parse");

        assert_eq!(
            cli.command,
            Commands::Learn(LearnArgs {
                input_json: PathBuf::from("examples/atomic-small.json"),
                output: None,
                name: None,
                quiet: false,
                debug: false,
                trace: true,
                basis_minimization: BasisMinimizationArg::default(),
                basis_mip_gap: None,
                basis_time_limit_secs: None,
            })
        );
    }

    #[test]
    fn parses_learn_command_with_exact_milp_basis_minimization() {
        let cli = Cli::try_parse_from([
            "learn-arta-cli",
            "learn",
            "examples/atomic-small.json",
            "--basis-minimization",
            "exact-milp",
        ])
        .expect("learn command with exact MILP basis minimization should parse");

        assert_eq!(
            cli.command,
            Commands::Learn(LearnArgs {
                input_json: PathBuf::from("examples/atomic-small.json"),
                output: None,
                name: None,
                quiet: false,
                debug: false,
                trace: false,
                basis_minimization: BasisMinimizationArg::ExactMilp,
                basis_mip_gap: None,
                basis_time_limit_secs: None,
            })
        );
    }

    #[test]
    fn parses_learn_command_with_approx_milp_basis_minimization_flags() {
        let cli = Cli::try_parse_from([
            "learn-arta-cli",
            "learn",
            "examples/atomic-small.json",
            "--basis-minimization",
            "approx-milp",
            "--basis-mip-gap",
            "0.05",
            "--basis-time-limit-secs",
            "1.5",
        ])
        .expect("learn command with approximate MILP basis minimization should parse");

        assert_eq!(
            cli.command,
            Commands::Learn(LearnArgs {
                input_json: PathBuf::from("examples/atomic-small.json"),
                output: None,
                name: None,
                quiet: false,
                debug: false,
                trace: false,
                basis_minimization: BasisMinimizationArg::ApproxMilp,
                basis_mip_gap: Some(0.05),
                basis_time_limit_secs: Some(1.5),
            })
        );
    }

    #[test]
    fn rejects_removed_progress_flag_for_learn() {
        let error = Cli::try_parse_from([
            "learn-arta-cli",
            "learn",
            "examples/atomic-small.json",
            "--progress",
        ])
        .expect_err("removed progress flag should fail");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn rejects_conflicting_quiet_and_debug_for_learn() {
        let error = Cli::try_parse_from([
            "learn-arta-cli",
            "learn",
            "examples/atomic-small.json",
            "--quiet",
            "--debug",
        ])
        .expect_err("quiet and debug should conflict");

        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn rejects_conflicting_debug_and_trace_for_learn() {
        let error = Cli::try_parse_from([
            "learn-arta-cli",
            "learn",
            "examples/atomic-small.json",
            "--debug",
            "--trace",
        ])
        .expect_err("debug and trace should conflict");

        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn rejects_removed_learn_random_subcommand() {
        let error = Cli::try_parse_from([
            "learn-arta-cli",
            "learn-random",
            "examples/atomic-small.json",
        ])
        .expect_err("learn-random should be rejected");

        assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
    }
}
