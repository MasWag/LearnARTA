// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use learn_arta_core::read_arta_json_file;

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_learn-arta-cli")
}

fn example_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("small.json")
}

fn expected_dot() -> String {
    read_arta_json_file(example_path())
        .expect("example JSON should load")
        .to_dot_string()
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

#[test]
fn dot_writes_to_stdout_by_default() {
    let output = Command::new(bin_path())
        .args(["dot", example_path().to_str().expect("utf-8 path")])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf-8 stdout"),
        expected_dot()
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn dot_writes_to_requested_output_file() {
    let output_path = unique_temp_path("output.dot");

    let output = Command::new(bin_path())
        .args([
            "dot",
            example_path().to_str().expect("utf-8 path"),
            "--output",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let written = fs::read_to_string(&output_path).expect("DOT output file should exist");
    assert_eq!(written, expected_dot());

    let _ = fs::remove_file(output_path);
}

#[test]
fn dot_reports_missing_input_file() {
    let missing_path = unique_temp_path("missing.json");

    let output = Command::new(bin_path())
        .args(["dot", missing_path.to_str().expect("utf-8 path")])
        .output()
        .expect("CLI should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("utf-8 stderr");
    assert!(stderr.contains("I/O error"), "stderr was: {stderr}");
}

#[test]
fn dot_reports_invalid_json() {
    let invalid_path = unique_temp_path("invalid.json");
    fs::write(&invalid_path, "{").expect("invalid JSON fixture should be written");

    let output = Command::new(bin_path())
        .args(["dot", invalid_path.to_str().expect("utf-8 path")])
        .output()
        .expect("CLI should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("utf-8 stderr");
    assert!(
        stderr.contains("invalid JSON document"),
        "stderr was: {stderr}"
    );

    let _ = fs::remove_file(invalid_path);
}
