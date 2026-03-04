// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_learn-arta-cli")
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

fn write_temp_json(name: &str, json: &str) -> PathBuf {
    let path = unique_temp_path(name);
    fs::write(&path, json).expect("fixture JSON should be written");
    path
}

fn rejecting_a_json() -> &'static str {
    r#"{
  "name": "rejecting-a",
  "l": ["q0"],
  "sigma": ["a"],
  "tran": {},
  "init": ["q0"],
  "accept": []
}
"#
}

fn empty_sigma_rejecting_json() -> &'static str {
    r#"{
  "name": "empty-reject",
  "l": ["q0"],
  "sigma": [],
  "tran": {},
  "init": ["q0"],
  "accept": []
}
"#
}

fn empty_sigma_accepting_json() -> &'static str {
    r#"{
  "name": "empty-accept",
  "l": ["q0"],
  "sigma": [],
  "tran": {},
  "init": ["q0"],
  "accept": ["q0"]
}
"#
}

#[test]
fn compare_reports_equivalent_inputs() {
    let output = Command::new(bin_path())
        .args([
            "compare",
            atomic_example_path().to_str().expect("utf-8 path"),
            atomic_example_path().to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf-8 stdout"),
        "equivalent\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn compare_reports_language_difference_with_witness() {
    let rejecting_path = write_temp_json("rejecting-a.json", rejecting_a_json());

    let output = Command::new(bin_path())
        .args([
            "compare",
            atomic_example_path().to_str().expect("utf-8 path"),
            rejecting_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf-8 stdout"),
        "different\nwitness: [(a, 0)]\nleft_accepts: true\nright_accepts: false\n"
    );
    assert!(output.stderr.is_empty());

    let _ = fs::remove_file(rejecting_path);
}

#[test]
fn compare_ignores_sigma_order_only() {
    let reordered_path = unique_temp_path("reordered-small.json");
    let original =
        fs::read_to_string(non_atomic_example_path()).expect("non-atomic example should load");
    let reordered = original.replacen("\"sigma\": [\"a\", \"b\"]", "\"sigma\": [\"b\", \"a\"]", 1);
    assert_ne!(reordered, original, "sigma order replacement should apply");
    fs::write(&reordered_path, reordered).expect("reordered JSON should be written");

    let output = Command::new(bin_path())
        .args([
            "compare",
            non_atomic_example_path().to_str().expect("utf-8 path"),
            reordered_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf-8 stdout"),
        "equivalent\n"
    );
    assert!(output.stderr.is_empty());

    let _ = fs::remove_file(reordered_path);
}

#[test]
fn compare_reports_declared_alphabet_mismatch_without_witness() {
    let output = Command::new(bin_path())
        .args([
            "compare",
            atomic_example_path().to_str().expect("utf-8 path"),
            non_atomic_example_path().to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    assert_eq!(
        stdout,
        "different\nreason: alphabet mismatch\nleft_sigma: [\"a\"]\nright_sigma: [\"a\", \"b\"]\n"
    );
    assert!(!stdout.contains("witness:"));
    assert!(output.stderr.is_empty());
}

#[test]
fn compare_handles_equivalent_empty_alphabet_inputs() {
    let left_path = write_temp_json("empty-left.json", empty_sigma_rejecting_json());

    let output = Command::new(bin_path())
        .args([
            "compare",
            left_path.to_str().expect("utf-8 path"),
            left_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf-8 stdout"),
        "equivalent\n"
    );
    assert!(output.stderr.is_empty());

    let _ = fs::remove_file(left_path);
}

#[test]
fn compare_reports_empty_word_witness_for_empty_alphabet_mismatch() {
    let left_path = write_temp_json("empty-reject.json", empty_sigma_rejecting_json());
    let right_path = write_temp_json("empty-accept.json", empty_sigma_accepting_json());

    let output = Command::new(bin_path())
        .args([
            "compare",
            left_path.to_str().expect("utf-8 path"),
            right_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf-8 stdout"),
        "different\nwitness: []\nleft_accepts: false\nright_accepts: true\n"
    );
    assert!(output.stderr.is_empty());

    let _ = fs::remove_file(left_path);
    let _ = fs::remove_file(right_path);
}

#[test]
fn compare_reports_missing_input_file() {
    let missing_path = unique_temp_path("missing-compare.json");

    let output = Command::new(bin_path())
        .args([
            "compare",
            missing_path.to_str().expect("utf-8 path"),
            atomic_example_path().to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("CLI should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("utf-8 stderr");
    assert!(stderr.contains("I/O error"), "stderr was: {stderr}");
}

#[test]
fn compare_reports_invalid_json() {
    let invalid_path = unique_temp_path("invalid-compare.json");
    fs::write(&invalid_path, "{").expect("invalid JSON fixture should be written");

    let output = Command::new(bin_path())
        .args([
            "compare",
            invalid_path.to_str().expect("utf-8 path"),
            atomic_example_path().to_str().expect("utf-8 path"),
        ])
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
