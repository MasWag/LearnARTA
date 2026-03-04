// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use learn_arta_core::{
    Arta, DagStateFormula, parse_arta_json_document, read_arta_json_file_document,
};
use learn_arta_oracles::WhiteBoxEqOracle;
use learn_arta_traits::EquivalenceOracle;

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_learn-arta-cli")
}

fn cli_command() -> Command {
    let mut command = Command::new(bin_path());
    command.env_remove("RUST_LOG");
    command.env_remove("RUST_LOG_STYLE");
    command
}

fn cli_command_with_rust_log(level: &str) -> Command {
    let mut command = cli_command();
    command.env("RUST_LOG", level);
    command
}

fn atomic_example_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("atomic-small.json")
}

fn non_atomic_example_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("small.json")
}

fn unique_temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "learn-arta-cli-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn assert_common_learning_summary(stderr: &str) {
    assert!(
        stderr.contains("Number of Equivalence queries: "),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains("Number of Membership queries (with caching): "),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains("Number of Membership queries (without caching): "),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains("Number of Observation table rows: "),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains("Number of Observation table columns: "),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains("Execution Time of Learning: "),
        "stderr was: {stderr}"
    );
}

fn has_log_line(stderr: &str, level: &str, message_fragment: &str) -> bool {
    log_line_index(stderr, level, message_fragment).is_some()
}

fn log_line_index(stderr: &str, level: &str, message_fragment: &str) -> Option<usize> {
    stderr.lines().position(|line| {
        has_human_readable_log_prefix(line, level) && line.contains(message_fragment)
    })
}

fn has_human_readable_log_prefix(line: &str, level: &str) -> bool {
    let Some((prefix, _message)) = line.split_once("] ") else {
        return false;
    };
    let Some(prefix) = prefix.strip_prefix('[') else {
        return false;
    };
    let Some((timestamp, actual_level)) = prefix.rsplit_once(' ') else {
        return false;
    };

    actual_level == level && has_human_readable_timestamp(timestamp)
}

fn has_human_readable_timestamp(timestamp: &str) -> bool {
    if timestamp.len() != 23 {
        return false;
    }

    let bytes = timestamp.as_bytes();

    bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
        && bytes[10] == b' '
        && bytes[11..13].iter().all(u8::is_ascii_digit)
        && bytes[13] == b':'
        && bytes[14..16].iter().all(u8::is_ascii_digit)
        && bytes[16] == b':'
        && bytes[17..19].iter().all(u8::is_ascii_digit)
        && bytes[19] == b'.'
        && bytes[20..23].iter().all(u8::is_ascii_digit)
}

fn has_false_transition(arta: &Arta<String, DagStateFormula>) -> bool {
    arta.transitions()
        .values()
        .flatten()
        .any(|edge| edge.target.semantic_key().clauses().is_empty())
}

#[test]
fn learn_emits_equivalent_hypothesis_json_to_stdout() {
    let target_document =
        read_arta_json_file_document(atomic_example_path()).expect("atomic example should load");

    let output = cli_command()
        .args(["learn", atomic_example_path().to_str().expect("utf-8 path")])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout JSON should be utf-8");
    let hypothesis_document =
        parse_arta_json_document(&stdout).expect("learn output should be valid JSON");
    let mut oracle =
        WhiteBoxEqOracle::try_new(target_document.arta.clone(), target_document.sigma.clone())
            .expect("target should be valid for exact comparison");
    let counterexample = oracle
        .find_counterexample(&hypothesis_document.arta)
        .expect("equivalence query should succeed");

    assert_eq!(counterexample, None);
    assert_eq!(hypothesis_document.name, "atomic-small-hypothesis");
    assert_eq!(hypothesis_document.sigma, target_document.sigma);

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(&stderr, "INFO", "exact learning completed"),
        "stderr was: {stderr}"
    );
    assert_common_learning_summary(&stderr);
    assert!(
        stdout.ends_with('\n'),
        "stdout should end with a trailing newline"
    );
}

#[cfg(feature = "milp")]
#[test]
fn learn_exact_milp_emits_equivalent_hypothesis_json_to_stdout() {
    let target_document =
        read_arta_json_file_document(atomic_example_path()).expect("atomic example should load");

    let output = cli_command()
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--basis-minimization",
            "exact-milp",
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout JSON should be utf-8");
    let hypothesis_document =
        parse_arta_json_document(&stdout).expect("learn output should be valid JSON");
    let mut oracle =
        WhiteBoxEqOracle::try_new(target_document.arta.clone(), target_document.sigma.clone())
            .expect("target should be valid for exact comparison");
    let counterexample = oracle
        .find_counterexample(&hypothesis_document.arta)
        .expect("equivalence query should succeed");

    assert_eq!(counterexample, None);
    assert_eq!(hypothesis_document.name, "atomic-small-hypothesis");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(&stderr, "INFO", "basis minimization: exact-milp."),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_emits_info_logs_by_default() {
    let target_path = non_atomic_example_path();
    let output = cli_command()
        .args(["learn", target_path.to_str().expect("utf-8 path")])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout JSON should be utf-8");
    parse_arta_json_document(&stdout).expect("learn output should be valid JSON");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    let target_log = format!("target json file: {}", target_path.display());
    let target_log_index =
        log_line_index(&stderr, "INFO", &target_log).expect("target json path should be logged");
    let started_log_index = log_line_index(&stderr, "INFO", "exact learning started.")
        .expect("startup log should be present");
    assert!(target_log_index < started_log_index, "stderr was: {stderr}");
    assert!(
        has_log_line(&stderr, "INFO", "exact learning started."),
        "stderr was: {stderr}"
    );
    assert!(
        has_log_line(
            &stderr,
            "INFO",
            "equivalence query #1 started. hypothesis states:",
        ),
        "stderr was: {stderr}"
    );
    assert!(
        has_log_line(
            &stderr,
            "INFO",
            "equivalence query #1 returned counterexample:"
        ),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_emits_equivalent_hypothesis_json_for_general_target() {
    let target_document = read_arta_json_file_document(non_atomic_example_path())
        .expect("general target example should load");

    let output = cli_command()
        .args([
            "learn",
            non_atomic_example_path().to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout JSON should be utf-8");
    let hypothesis_document =
        parse_arta_json_document(&stdout).expect("learn output should be valid JSON");
    let mut oracle =
        WhiteBoxEqOracle::try_new(target_document.arta.clone(), target_document.sigma.clone())
            .expect("target should be valid for exact comparison");
    let counterexample = oracle
        .find_counterexample(&hypothesis_document.arta)
        .expect("equivalence query should succeed");

    assert_eq!(counterexample, None);
    assert_eq!(hypothesis_document.name, "small-hypothesis");
    assert_eq!(hypothesis_document.sigma, target_document.sigma);
    assert!(!has_false_transition(&hypothesis_document.arta));
}

#[test]
fn learn_logs_greedy_basis_minimization_strategy() {
    let output = cli_command()
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--basis-minimization",
            "greedy",
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(&stderr, "INFO", "basis minimization: greedy."),
        "stderr was: {stderr}"
    );
}

#[cfg(feature = "milp")]
#[test]
fn learn_logs_approx_milp_basis_minimization_strategy() {
    let output = cli_command()
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--basis-minimization",
            "approx-milp",
            "--basis-mip-gap",
            "0.05",
            "--basis-time-limit-secs",
            "1.0",
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(&stderr, "INFO", "basis minimization: approx-milp."),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_debug_emits_table_logs_without_hypothesis_json_trace_logs() {
    let output = cli_command()
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--debug",
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout JSON should be utf-8");
    parse_arta_json_document(&stdout).expect("learn output should be valid JSON");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(
            &stderr,
            "DEBUG",
            "observation table before equivalence query #1: rows="
        ),
        "stderr was: {stderr}"
    );
    assert!(
        !stderr.contains("hypothesis ARTA before equivalence query #1:\n{"),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_trace_emits_hypothesis_json_logs() {
    let output = cli_command()
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--trace",
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout JSON should be utf-8");
    parse_arta_json_document(&stdout).expect("learn output should be valid JSON");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(
            &stderr,
            "DEBUG",
            "observation table before equivalence query #1: rows="
        ),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains("TRACE] hypothesis ARTA before equivalence query #1:\n{"),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains("\"name\": \"atomic-small-hypothesis\""),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_honors_rust_log_debug_without_cli_flags() {
    let output = cli_command_with_rust_log("debug")
        .args(["learn", atomic_example_path().to_str().expect("utf-8 path")])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(
            &stderr,
            "DEBUG",
            "observation table before equivalence query #1: rows="
        ),
        "stderr was: {stderr}"
    );
    assert!(
        !stderr.contains("hypothesis ARTA before equivalence query #1:\n{"),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_honors_rust_log_trace_without_cli_flags() {
    let output = cli_command_with_rust_log("trace")
        .args(["learn", atomic_example_path().to_str().expect("utf-8 path")])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        stderr.contains("TRACE] hypothesis ARTA before equivalence query #1:\n{"),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_quiet_overrides_rust_log_trace() {
    let output = cli_command_with_rust_log("trace")
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--quiet",
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout JSON should be utf-8");
    parse_arta_json_document(&stdout).expect("learn --quiet output should be valid JSON");
    assert!(output.stderr.is_empty(), "stderr was not empty");
}

#[test]
fn learn_debug_overrides_rust_log_error() {
    let output = cli_command_with_rust_log("error")
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--debug",
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(
            &stderr,
            "DEBUG",
            "observation table before equivalence query #1: rows="
        ),
        "stderr was: {stderr}"
    );
    assert!(
        !stderr.contains("hypothesis ARTA before equivalence query #1:\n{"),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_trace_overrides_rust_log_error() {
    let output = cli_command_with_rust_log("error")
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--trace",
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        stderr.contains("TRACE] hypothesis ARTA before equivalence query #1:\n{"),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_quiet_emits_json_to_stdout_without_stderr() {
    let output = cli_command()
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--quiet",
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout JSON should be utf-8");
    let hypothesis_document =
        parse_arta_json_document(&stdout).expect("learn --quiet output should be valid JSON");

    assert_eq!(hypothesis_document.name, "atomic-small-hypothesis");
    assert!(
        stdout.ends_with('\n'),
        "stdout should end with a trailing newline"
    );
    assert!(output.stderr.is_empty(), "stderr was not empty");
}

#[test]
fn learn_accepts_legacy_nrta_json_input() {
    let input_path = unique_temp_path("legacy-nrta.json");
    fs::write(
        &input_path,
        r#"{
          "name":"legacy-overlap",
          "l":["q0","q1","q2"],
          "sigma":["a"],
          "tran":{
            "0":["q0","a","[0,1]","q1"],
            "1":["q0","a","(0,2)","q2"]
          },
          "init":["q0"],
          "accept":["q2"]
        }"#,
    )
    .expect("legacy JSON fixture should be written");

    let output = cli_command()
        .args(["learn", input_path.to_str().expect("utf-8 path"), "--quiet"])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout JSON should be utf-8");
    let hypothesis_document =
        parse_arta_json_document(&stdout).expect("learn output should be valid JSON");

    assert_eq!(hypothesis_document.name, "legacy-overlap-hypothesis");
    assert_eq!(hypothesis_document.sigma, vec!["a".to_string()]);
    assert!(
        stdout.ends_with('\n'),
        "stdout should end with a trailing newline"
    );
    assert!(output.stderr.is_empty(), "stderr was not empty");

    let _ = fs::remove_file(input_path);
}

#[test]
fn learn_quiet_merged_stream_keeps_json_only_output() {
    let output = Command::new("sh")
        .args([
            "-c",
            "env -u RUST_LOG -u RUST_LOG_STYLE \"$1\" learn \"$2\" --quiet 2>&1",
            "sh",
            bin_path(),
            atomic_example_path().to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("shell should run CLI");

    assert!(output.status.success());

    let combined = String::from_utf8(output.stdout).expect("combined output should be utf-8");
    let parsed = parse_arta_json_document(combined.trim_end())
        .expect("merged quiet stream should be a complete JSON document");
    assert_eq!(parsed.name, "atomic-small-hypothesis");
}

#[test]
fn learn_rejects_approx_milp_flags_without_approx_strategy() {
    let output = cli_command()
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--basis-minimization",
            "exact-milp",
            "--basis-mip-gap",
            "0.05",
        ])
        .output()
        .expect("CLI should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(&stderr, "ERROR", "invalid exact learning configuration"),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains("--basis-mip-gap requires --basis-minimization approx-milp"),
        "stderr was: {stderr}"
    );
}

#[cfg(not(feature = "milp"))]
#[test]
fn learn_exact_milp_requires_cli_milp_feature() {
    let output = cli_command()
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--basis-minimization",
            "exact-milp",
        ])
        .output()
        .expect("CLI should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(&stderr, "ERROR", "invalid exact learning configuration"),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains(
            "--basis-minimization exact-milp requires rebuilding learn-arta-cli with --features milp"
        ),
        "stderr was: {stderr}"
    );
}

#[cfg(not(feature = "milp"))]
#[test]
fn learn_approximate_milp_requires_cli_milp_feature() {
    let output = cli_command()
        .args([
            "learn",
            atomic_example_path().to_str().expect("utf-8 path"),
            "--basis-minimization",
            "approx-milp",
        ])
        .output()
        .expect("CLI should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        has_log_line(&stderr, "ERROR", "invalid exact learning configuration"),
        "stderr was: {stderr}"
    );
    assert!(
        stderr.contains(
            "--basis-minimization approx-milp requires rebuilding learn-arta-cli with --features milp"
        ),
        "stderr was: {stderr}"
    );
}

#[test]
fn learn_random_is_rejected_as_unknown_subcommand() {
    let output = cli_command()
        .args([
            "learn-random",
            atomic_example_path().to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(
        stderr.contains("unrecognized subcommand"),
        "stderr was: {stderr}"
    );
    assert!(stderr.contains("learn-random"), "stderr was: {stderr}");
}
