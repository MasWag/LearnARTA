// SPDX-License-Identifier: Apache-2.0 OR MIT

use learn_arta_core::{
    Arta, ArtaBuilder, ArtaJsonError, DagStateFormula, DagStateFormulaManager, DelayRep,
    LocationId, ParsedArtaJson, StateFormula, TimedWord, parse_arta_json, parse_arta_json_document,
    read_arta_json_file, to_arta_json_document_string, to_arta_json_string,
};
use std::path::Path;

fn example_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("small.json")
}

fn timed_word(letters: &[(&str, u32)]) -> TimedWord<String> {
    TimedWord::from_vec(
        letters
            .iter()
            .map(|(symbol, half_units)| {
                (symbol.to_string(), DelayRep::from_half_units(*half_units))
            })
            .collect(),
    )
}

fn canonical_example_json() -> String {
    std::fs::read_to_string(example_path()).expect("example JSON should be readable")
}

fn ordering_example(reverse: bool) -> Arta<String, DagStateFormula> {
    let manager = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let q1 = LocationId::new("q1");
    let q2 = LocationId::new("q2");
    let init = DagStateFormula::or(
        &manager,
        vec![
            DagStateFormula::var(&manager, q0.clone()),
            DagStateFormula::var(&manager, q2.clone()),
        ],
    );

    let mut builder = ArtaBuilder::new(init);
    builder.add_location(q2.clone());
    builder.add_location(q0.clone());
    builder.add_location(q1.clone());
    builder.add_accepting(q1.clone());

    let transitions = vec![
        (
            q2.clone(),
            "a".to_string(),
            "[0,+)".parse().unwrap(),
            DagStateFormula::bot(&manager),
        ),
        (
            q1.clone(),
            "b".to_string(),
            "(0,2]".parse().unwrap(),
            DagStateFormula::or(
                &manager,
                vec![
                    DagStateFormula::var(&manager, q1.clone()),
                    DagStateFormula::var(&manager, q2.clone()),
                ],
            ),
        ),
        (
            q0.clone(),
            "a".to_string(),
            "[1,+)".parse().unwrap(),
            DagStateFormula::and(
                &manager,
                vec![
                    DagStateFormula::var(&manager, q2.clone()),
                    DagStateFormula::var(&manager, q1.clone()),
                ],
            ),
        ),
        (
            q0,
            "a".to_string(),
            "[0,1)".parse().unwrap(),
            DagStateFormula::var(&manager, q1),
        ),
    ];

    let mut transitions = transitions;
    if reverse {
        transitions.reverse();
    }

    for (source, symbol, guard, target) in transitions {
        builder.add_transition(source, symbol, guard, target);
    }

    builder.build().expect("valid ARTA")
}

#[test]
fn example_json_loads_successfully() {
    let arta = read_arta_json_file(example_path()).expect("example should parse");

    assert_eq!(arta.locations().len(), 3);
    assert_eq!(arta.accepting().len(), 1);
    assert!(!arta.accepts(&TimedWord::empty()));
    assert!(arta.accepts(&timed_word(&[("a", 0)])));
}

#[test]
fn json_roundtrip_is_stable_and_semantic() {
    let document = parse_arta_json_document(&canonical_example_json()).expect("input should parse");
    let json1 = to_arta_json_document_string(&document).expect("writer should succeed");
    let reloaded = parse_arta_json_document(&json1).expect("written JSON should parse");
    let json2 = to_arta_json_document_string(&reloaded).expect("writer should stay stable");

    assert_eq!(json1, json2);
    assert!(!json1.contains("[\n"));
    assert_eq!(document.name, reloaded.name);
    assert_eq!(document.sigma, reloaded.sigma);
    assert!(json1.contains("\"l\": [\"q0\", \"q1\", \"q2\"]"));
    assert!(json1.contains("\"sigma\": [\"a\", \"b\"]"));
    assert!(json1.contains("\"init\": [\"q0\", \"q2\"]"));
    assert!(json1.contains("\"0\": [\"q0\", \"a\", \"[0,1)\", \"q1\"]"));
    for word in [
        timed_word(&[]),
        timed_word(&[("a", 0)]),
        timed_word(&[("a", 2)]),
        timed_word(&[("a", 2), ("b", 2)]),
    ] {
        assert_eq!(document.arta.accepts(&word), reloaded.arta.accepts(&word));
    }
}

#[test]
fn writer_regenerates_stable_transition_ids_and_ordering() {
    let forward = ordering_example(false);
    let reverse = ordering_example(true);

    let forward_json = to_arta_json_string(&forward, "ordered").expect("writer should succeed");
    let reverse_json = to_arta_json_string(&reverse, "ordered").expect("writer should succeed");

    assert_eq!(forward_json, reverse_json);
}

#[test]
fn parse_reports_unknown_symbol() {
    let err = parse_arta_json(
        r#"{
          "name":"bad-symbol",
          "l":["q0","q1"],
          "sigma":[],
          "tran":{"0":["q0","a","[0,1)","q1"]},
          "init":["q0"],
          "accept":["q1"]
        }"#,
    )
    .unwrap_err();

    assert!(matches!(err, ArtaJsonError::UnknownSymbol { .. }));
}

#[test]
fn parse_reports_bad_guard() {
    let err = parse_arta_json(
        r#"{
          "name":"bad-guard",
          "l":["q0","q1"],
          "sigma":["a"],
          "tran":{"0":["q0","a","[2,1)","q1"]},
          "init":["q0"],
          "accept":["q1"]
        }"#,
    )
    .unwrap_err();

    assert!(matches!(err, ArtaJsonError::InvalidGuard { .. }));
}

#[test]
fn parse_canonicalizes_legacy_overlaps_into_disjoint_arta_edges() {
    let arta = parse_arta_json(
        r#"{
          "name":"overlap",
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
    .expect("legacy overlap should be canonicalized");

    let symbol = "a".to_string();
    let outgoing = arta
        .outgoing(&LocationId::new("q0"), &symbol)
        .expect("canonicalized transitions should exist");

    assert_eq!(outgoing.len(), 3);
    assert_eq!(outgoing[0].guard.to_string(), "[0,0]");
    assert_eq!(outgoing[0].target.to_string(), "loc(q1)");
    assert_eq!(outgoing[1].guard.to_string(), "(0,1]");
    assert_eq!(outgoing[1].target.to_string(), "(loc(q1) | loc(q2))");
    assert_eq!(outgoing[2].guard.to_string(), "(1,2)");
    assert_eq!(outgoing[2].target.to_string(), "loc(q2)");

    assert!(!arta.accepts(&timed_word(&[("a", 0)])));
    assert!(arta.accepts(&timed_word(&[("a", 1)])));
    assert!(arta.accepts(&timed_word(&[("a", 3)])));
}

#[test]
fn parse_merges_duplicate_legacy_guards_with_disjunctive_target() {
    let arta = parse_arta_json(
        r#"{
          "name":"duplicate-guard",
          "l":["q0","q1","q2"],
          "sigma":["a"],
          "tran":{
            "0":["q0","a","[0,1)","q1"],
            "1":["q0","a","[0,1)","q2"]
          },
          "init":["q0"],
          "accept":["q2"]
        }"#,
    )
    .expect("duplicate legacy guards should canonicalize");

    let symbol = "a".to_string();
    let outgoing = arta
        .outgoing(&LocationId::new("q0"), &symbol)
        .expect("canonicalized transitions should exist");

    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].guard.to_string(), "[0,1)");
    assert_eq!(outgoing[0].target.to_string(), "(loc(q1) | loc(q2))");
    assert!(arta.accepts(&timed_word(&[("a", 0)])));
    assert!(arta.accepts(&timed_word(&[("a", 1)])));
}

#[test]
fn parse_supports_formula_init_syntax() {
    let parsed = parse_arta_json_document(
        r#"{
          "name":"formula-init",
          "l":["q0","q1"],
          "sigma":["a"],
          "tran":{},
          "init":{"and":["q0","q1"]},
          "accept":["q1"]
        }"#,
    )
    .expect("formula init should parse");

    assert_eq!(parsed.name, "formula-init");
    assert_eq!(parsed.sigma, vec!["a".to_string()]);
    assert_eq!(parsed.arta.init().to_string(), "(loc(q0) & loc(q1))");
}

#[test]
fn writer_roundtrips_non_list_initial_formula() {
    let manager = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let q1 = LocationId::new("q1");

    let init = DagStateFormula::and(
        &manager,
        vec![
            DagStateFormula::var(&manager, q0.clone()),
            DagStateFormula::var(&manager, q1.clone()),
        ],
    );
    let mut builder = ArtaBuilder::new(init);
    builder.add_location(q0);
    builder.add_location(q1.clone());
    builder.add_accepting(q1);

    let document = ParsedArtaJson {
        name: "bad-init".to_string(),
        sigma: vec!["a".to_string()],
        arta: builder.build().expect("ARTA should build"),
    };
    let json = to_arta_json_document_string(&document).expect("writer should support formula init");
    let reloaded = parse_arta_json_document(&json).expect("written JSON should parse");

    assert!(json.contains("\"init\": {"));
    assert!(json.contains("\"and\": [\"q0\", \"q1\"]"));
    assert_eq!(reloaded.name, "bad-init");
    assert_eq!(reloaded.sigma, vec!["a".to_string()]);
    assert_eq!(reloaded.arta.init().to_string(), "(loc(q0) & loc(q1))");
}

#[test]
fn document_writer_preserves_explicit_sigma_even_when_unused() {
    let manager = DagStateFormulaManager::new();
    let q0 = LocationId::new("q0");
    let q1 = LocationId::new("q1");
    let init = DagStateFormula::var(&manager, q0.clone());
    let mut builder = ArtaBuilder::new(init);
    builder
        .add_location(q0.clone())
        .add_location(q1.clone())
        .add_accepting(q1.clone())
        .add_transition(
            q0,
            "a".to_string(),
            "[0,+)".parse().unwrap(),
            DagStateFormula::var(&manager, q1),
        );

    let document = ParsedArtaJson {
        name: "sigma-preserved".to_string(),
        sigma: vec!["a".to_string(), "z".to_string()],
        arta: builder.build().expect("ARTA should build"),
    };
    let json = to_arta_json_document_string(&document).expect("writer should succeed");

    assert!(json.contains("\"sigma\": [\"a\", \"z\"]"));
}
