// SPDX-License-Identifier: Apache-2.0 OR MIT

//! JSON import/export for [`Arta<String, DagStateFormula>`] and ARTA documents.
//!
//! Import accepts both canonical LearnARTA JSON and legacy/original NRTA JSON.
//! Legacy overlapping transition guards are canonicalized into an equivalent
//! deterministic ARTA during parsing. Export always emits canonical
//! deterministic LearnARTA JSON.

use crate::{
    arta::{Arta, ArtaBuilder, ArtaError},
    error::IntervalError,
    location::LocationId,
    state_formula::{DagStateFormula, DagStateFormulaManager, StateFormula},
    time::interval::Interval,
};
use serde::{Deserialize, Serialize};
use serde_json::ser::Formatter;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs, io,
    path::Path,
    sync::Arc,
};
use thiserror::Error;

/// Parsed ARTA JSON document with preserved metadata.
#[derive(Debug, Clone)]
pub struct ParsedArtaJson {
    /// Human-readable name of the automaton.
    pub name: String,
    /// Declared input alphabet from the JSON document.
    pub sigma: Vec<String>,
    /// Parsed ARTA itself.
    pub arta: Arta<String, DagStateFormula>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    Array,
    Object,
}

#[derive(Debug, Clone, Copy)]
struct ContainerState {
    kind: ContainerKind,
    compact: bool,
    has_value: bool,
}

#[derive(Debug, Clone)]
struct InlineArrayFormatter<'a> {
    current_indent: usize,
    indent: &'a [u8],
    stack: Vec<ContainerState>,
}

impl<'a> InlineArrayFormatter<'a> {
    fn new(indent: &'a [u8]) -> Self {
        Self {
            current_indent: 0,
            indent,
            stack: Vec::new(),
        }
    }
}

impl<'a> Formatter for InlineArrayFormatter<'a> {
    fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.stack.push(ContainerState {
            kind: ContainerKind::Array,
            compact: true,
            has_value: false,
        });
        writer.write_all(b"[")
    }

    fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let _ = self.stack.pop();
        writer.write_all(b"]")
    }

    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn end_array_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if let Some(frame) = self.stack.last_mut() {
            frame.has_value = true;
        }
        Ok(())
    }

    fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let compact = self.stack.last().is_some_and(|frame| frame.compact);
        if !compact {
            self.current_indent += 1;
        }
        self.stack.push(ContainerState {
            kind: ContainerKind::Object,
            compact,
            has_value: false,
        });
        writer.write_all(b"{")
    }

    fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let frame = self.stack.pop().unwrap_or(ContainerState {
            kind: ContainerKind::Object,
            compact: false,
            has_value: false,
        });

        if frame.compact {
            return writer.write_all(b"}");
        }

        self.current_indent = self.current_indent.saturating_sub(1);
        if frame.has_value {
            writer.write_all(b"\n")?;
            write_indent(writer, self.current_indent, self.indent)?;
        }

        writer.write_all(b"}")
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let frame = self.stack.last().copied().unwrap_or(ContainerState {
            kind: ContainerKind::Object,
            compact: false,
            has_value: false,
        });
        debug_assert_eq!(frame.kind, ContainerKind::Object);

        if frame.compact {
            if first {
                Ok(())
            } else {
                writer.write_all(b", ")
            }
        } else {
            writer.write_all(if first { b"\n" } else { b",\n" })?;
            write_indent(writer, self.current_indent, self.indent)
        }
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(b": ")
    }

    fn end_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if let Some(frame) = self.stack.last_mut() {
            frame.has_value = true;
        }
        Ok(())
    }
}

fn write_indent<W>(writer: &mut W, level: usize, indent: &[u8]) -> io::Result<()>
where
    W: ?Sized + io::Write,
{
    for _ in 0..level {
        writer.write_all(indent)?;
    }
    Ok(())
}

/// Errors returned while reading or writing ARTA JSON.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ArtaJsonError {
    /// Reading or writing a JSON file failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON syntax or shape did not match the expected document structure.
    #[error("invalid JSON document: {0}")]
    Json(#[from] serde_json::Error),
    /// The JSON document referenced a symbol not declared in `sigma`.
    #[error("{context} uses undeclared symbol {symbol}")]
    UnknownSymbol {
        /// Human-readable location within the JSON document.
        context: String,
        /// Undeclared symbol string from the JSON payload.
        symbol: String,
    },
    /// The JSON document used a malformed transition identifier key.
    #[error("invalid transition id {id}: expected a decimal string")]
    InvalidTransitionId {
        /// Transition object key that failed to parse as a decimal identifier.
        id: String,
    },
    /// A guard string could not be parsed into an [`Interval`].
    #[error("invalid guard {guard} in {context}: {source}")]
    InvalidGuard {
        /// Human-readable location within the JSON document.
        context: String,
        /// Original textual guard representation.
        guard: String,
        /// Underlying interval parsing failure.
        #[source]
        source: IntervalError,
    },
    /// A JSON formula object used an invalid arity for `and` or `or`.
    #[error("invalid {operator} arity in {context}: expected at least 2 operands, got {len}")]
    InvalidFormulaArity {
        /// Human-readable location within the JSON document.
        context: String,
        /// Formula operator name (`and` or `or`).
        operator: &'static str,
        /// Observed operand count.
        len: usize,
    },
    /// `init` arrays must contain at least one location name.
    #[error("init array must contain at least one location")]
    EmptyInit,
    /// The initial state formula cannot be represented as a list of locations.
    #[error("initial formula {formula} cannot be encoded as an init list of locations")]
    UnsupportedInitFormula {
        /// Pretty-printed initial formula that cannot be emitted as an init list.
        formula: String,
    },
    /// The parsed automaton failed core ARTA validation.
    #[error("invalid ARTA JSON: {0}")]
    Arta(#[from] ArtaError<String>),
}

/// Parse an ARTA from a JSON string.
pub fn parse_arta_json(src: &str) -> Result<Arta<String, DagStateFormula>, ArtaJsonError> {
    Ok(parse_arta_json_document(src)?.arta)
}

/// Parse an ARTA JSON document while preserving its metadata.
pub fn parse_arta_json_document(src: &str) -> Result<ParsedArtaJson, ArtaJsonError> {
    let document: JsonArtaReadDocument = serde_json::from_str(src)?;
    document.into_parsed_arta_json()
}

/// Read an ARTA from a JSON file.
pub fn read_arta_json_file(
    path: impl AsRef<Path>,
) -> Result<Arta<String, DagStateFormula>, ArtaJsonError> {
    Ok(read_arta_json_file_document(path)?.arta)
}

/// Read an ARTA JSON document while preserving its metadata.
pub fn read_arta_json_file_document(
    path: impl AsRef<Path>,
) -> Result<ParsedArtaJson, ArtaJsonError> {
    let src = fs::read_to_string(path)?;
    parse_arta_json_document(&src)
}

/// Serialize an ARTA to a canonical pretty-printed JSON string.
pub fn to_arta_json_string(
    arta: &Arta<String, DagStateFormula>,
    name: &str,
) -> Result<String, ArtaJsonError> {
    let document = JsonArtaWriteDocument::from_parts(arta, name, infer_sigma(arta));
    serialize_json_document(&document)
}

/// Serialize an ARTA JSON document to a canonical pretty-printed JSON string.
pub fn to_arta_json_document_string(document: &ParsedArtaJson) -> Result<String, ArtaJsonError> {
    let document =
        JsonArtaWriteDocument::from_parts(&document.arta, &document.name, document.sigma.clone());
    serialize_json_document(&document)
}

/// Write an ARTA to a canonical pretty-printed JSON file.
pub fn write_arta_json_file(
    arta: &Arta<String, DagStateFormula>,
    name: &str,
    path: impl AsRef<Path>,
) -> Result<(), ArtaJsonError> {
    let json = to_arta_json_string(arta, name)?;
    fs::write(path, json)?;
    Ok(())
}

/// Write an ARTA JSON document to a canonical pretty-printed JSON file.
pub fn write_arta_json_document_file(
    document: &ParsedArtaJson,
    path: impl AsRef<Path>,
) -> Result<(), ArtaJsonError> {
    let json = to_arta_json_document_string(document)?;
    fs::write(path, json)?;
    Ok(())
}

fn serialize_json_document<T>(document: &T) -> Result<String, ArtaJsonError>
where
    T: Serialize,
{
    let mut output = Vec::new();
    let formatter = InlineArrayFormatter::new(b"  ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut output, formatter);
    document.serialize(&mut serializer)?;
    Ok(String::from_utf8(output).expect("serde_json emits UTF-8"))
}

fn infer_sigma(arta: &Arta<String, DagStateFormula>) -> Vec<String> {
    arta.transitions()
        .keys()
        .map(|(_, symbol)| symbol.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonArtaReadDocument {
    name: String,
    l: Vec<String>,
    sigma: Vec<String>,
    tran: BTreeMap<String, JsonTransition>,
    init: JsonInit,
    accept: Vec<String>,
}

impl JsonArtaReadDocument {
    fn into_parsed_arta_json(self) -> Result<ParsedArtaJson, ArtaJsonError> {
        let JsonArtaReadDocument {
            name,
            l,
            sigma,
            tran,
            init,
            accept,
        } = self;

        let manager = DagStateFormulaManager::new();
        let init = init.into_state_formula(&manager)?;

        let mut builder = ArtaBuilder::new(init);
        for loc in l {
            builder.add_location(LocationId::new(loc));
        }
        for loc in accept {
            builder.add_accepting(LocationId::new(loc));
        }

        let declared_sigma: BTreeSet<String> = sigma.iter().cloned().collect();
        let mut transition_buckets =
            HashMap::<(LocationId, String), Vec<ParsedJsonTransition>>::new();
        for (transition_id, JsonTransition(source, symbol, guard, target)) in tran {
            if !transition_id.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(ArtaJsonError::InvalidTransitionId { id: transition_id });
            }
            if !declared_sigma.contains(&symbol) {
                return Err(ArtaJsonError::UnknownSymbol {
                    context: format!("transition {transition_id}"),
                    symbol,
                });
            }

            let guard_context = format!("transition {transition_id}");
            let guard =
                guard
                    .parse::<Interval>()
                    .map_err(|source| ArtaJsonError::InvalidGuard {
                        context: guard_context,
                        guard,
                        source,
                    })?;
            guard.validate().map_err(|interval_error| {
                ArtaJsonError::Arta(ArtaError::InvalidInterval {
                    context: format!("transition {transition_id} from {source}"),
                    source: interval_error,
                })
            })?;
            let target = target
                .into_state_formula(&manager, format!("transition {transition_id} target"))?;

            let source = LocationId::new(source);
            transition_buckets
                .entry((source.clone(), symbol.clone()))
                .or_default()
                .push(ParsedJsonTransition {
                    source,
                    symbol,
                    guard,
                    target,
                });
        }

        for transitions in transition_buckets.into_values() {
            for transition in canonicalize_transition_bucket(&manager, transitions) {
                builder.add_transition(
                    transition.source,
                    transition.symbol,
                    transition.guard,
                    transition.target,
                );
            }
        }

        let arta = builder.build().map_err(ArtaJsonError::from)?;

        Ok(ParsedArtaJson { name, sigma, arta })
    }
}

#[derive(Debug)]
struct ParsedJsonTransition {
    source: LocationId,
    symbol: String,
    guard: Interval,
    target: DagStateFormula,
}

#[derive(Debug, Default)]
struct BucketEvents {
    adds: Vec<DagStateFormula>,
    removes: Vec<DagStateFormula>,
}

fn canonicalize_transition_bucket(
    manager: &Arc<DagStateFormulaManager>,
    transitions: Vec<ParsedJsonTransition>,
) -> Vec<ParsedJsonTransition> {
    let mut events = BTreeMap::<u32, BucketEvents>::new();
    let mut source = None;
    let mut symbol = None;

    for transition in transitions {
        let (start, end) = transition
            .guard
            .representable_half_range()
            .expect("validated guard should have a representable half-unit range");

        source.get_or_insert_with(|| transition.source.clone());
        symbol.get_or_insert_with(|| transition.symbol.clone());

        events
            .entry(start)
            .or_default()
            .adds
            .push(transition.target.clone());
        if let Some(end) = end {
            let end_exclusive = end
                .checked_add(1)
                .expect("validated finite guards must end before the infinity sentinel");
            events
                .entry(end_exclusive)
                .or_default()
                .removes
                .push(transition.target);
        }
    }

    let Some(source) = source else {
        return Vec::new();
    };
    let Some(symbol) = symbol else {
        return Vec::new();
    };

    let mut canonical = Vec::new();
    let mut active_targets = Vec::<(DagStateFormula, usize)>::new();
    let max_half_units = u32::MAX - 1;
    let mut events = events.into_iter().peekable();

    while let Some((point, boundary_events)) = events.next() {
        for target in boundary_events.removes {
            decrement_active_target(&mut active_targets, target);
        }
        for target in boundary_events.adds {
            increment_active_target(&mut active_targets, target);
        }

        if active_targets.is_empty() || point > max_half_units {
            continue;
        }

        let end = events.peek().map(|(next_point, _)| next_point - 1);
        let target = DagStateFormula::or(
            manager,
            active_targets.iter().map(|(target, _)| target.clone()),
        );
        let guard = Interval::from_representable_half_range(point, end)
            .expect("event boundaries should reconstruct a valid interval");
        push_or_merge_transition(
            &mut canonical,
            ParsedJsonTransition {
                source: source.clone(),
                symbol: symbol.clone(),
                guard,
                target,
            },
        );
    }

    canonical
}

fn increment_active_target(
    active_targets: &mut Vec<(DagStateFormula, usize)>,
    target: DagStateFormula,
) {
    if let Some((_, count)) = active_targets
        .iter_mut()
        .find(|(active_target, _)| *active_target == target)
    {
        *count += 1;
    } else {
        active_targets.push((target, 1));
    }
}

fn decrement_active_target(
    active_targets: &mut Vec<(DagStateFormula, usize)>,
    target: DagStateFormula,
) {
    if let Some(index) = active_targets
        .iter()
        .position(|(active_target, _)| *active_target == target)
    {
        if active_targets[index].1 == 1 {
            active_targets.remove(index);
        } else {
            active_targets[index].1 -= 1;
        }
    }
}

fn push_or_merge_transition(
    transitions: &mut Vec<ParsedJsonTransition>,
    transition: ParsedJsonTransition,
) {
    if let Some(previous) = transitions.last_mut()
        && previous.target == transition.target
        && let Some(merged) = previous.guard.try_merge_adjacent(&transition.guard)
    {
        previous.guard = merged;
        return;
    }

    transitions.push(transition);
}

#[derive(Debug, Serialize)]
struct JsonArtaWriteDocument {
    name: String,
    l: Vec<String>,
    sigma: Vec<String>,
    tran: BTreeMap<String, JsonTransition>,
    init: JsonInit,
    accept: Vec<String>,
}

impl JsonArtaWriteDocument {
    fn from_parts(arta: &Arta<String, DagStateFormula>, name: &str, sigma: Vec<String>) -> Self {
        let mut locations = arta
            .locations()
            .iter()
            .map(|loc| loc.name().to_string())
            .collect::<Vec<_>>();
        locations.sort();

        let mut accepting = arta
            .accepting()
            .iter()
            .map(|loc| loc.name().to_string())
            .collect::<Vec<_>>();
        accepting.sort();

        let mut transitions = arta
            .transitions()
            .iter()
            .flat_map(|((source, symbol), edges)| {
                edges.iter().map(move |edge| {
                    let target = JsonFormula::from_state_formula(&edge.target);
                    JsonTransitionRecord {
                        source: source.name().to_string(),
                        symbol: symbol.clone(),
                        guard_sort_key: edge.guard.sort_key(),
                        guard: interval_to_json_guard(&edge.guard),
                        target_sort_key: target.sort_key(),
                        target,
                    }
                })
            })
            .collect::<Vec<_>>();

        transitions.sort_by(|lhs, rhs| {
            lhs.source
                .cmp(&rhs.source)
                .then(lhs.symbol.cmp(&rhs.symbol))
                .then(lhs.guard_sort_key.cmp(&rhs.guard_sort_key))
                .then(lhs.target_sort_key.cmp(&rhs.target_sort_key))
        });

        let tran = transitions
            .into_iter()
            .enumerate()
            .map(|(index, transition)| {
                (
                    index.to_string(),
                    JsonTransition(
                        transition.source,
                        transition.symbol,
                        transition.guard,
                        transition.target,
                    ),
                )
            })
            .collect();

        Self {
            name: name.to_string(),
            l: locations,
            sigma,
            tran,
            init: JsonInit::from_state_formula(arta.init()),
            accept: accepting,
        }
    }
}

#[derive(Debug)]
struct JsonTransitionRecord {
    source: String,
    symbol: String,
    guard_sort_key: (u64, u64),
    guard: String,
    target_sort_key: String,
    target: JsonFormula,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct JsonTransition(String, String, String, JsonFormula);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum JsonInit {
    Locations(Vec<String>),
    Formula(JsonFormula),
}

impl JsonInit {
    fn into_state_formula(
        self,
        manager: &std::sync::Arc<DagStateFormulaManager>,
    ) -> Result<DagStateFormula, ArtaJsonError> {
        match self {
            Self::Locations(locations) => parse_init_locations(locations, manager),
            Self::Formula(formula) => formula.into_state_formula(manager, "init".to_string()),
        }
    }

    fn from_state_formula(formula: &DagStateFormula) -> Self {
        if let Some(locations) = legacy_init_locations_from_state_formula(formula) {
            Self::Locations(locations)
        } else {
            Self::Formula(JsonFormula::from_state_formula(formula))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum JsonFormula {
    Atom(String),
    And(JsonAndFormula),
    Or(JsonOrFormula),
    Const(JsonConstFormula),
}

impl JsonFormula {
    fn and(terms: Vec<Self>) -> Self {
        Self::And(JsonAndFormula { and: terms })
    }

    fn or(terms: Vec<Self>) -> Self {
        Self::Or(JsonOrFormula { or: terms })
    }

    fn const_value(value: bool) -> Self {
        Self::Const(JsonConstFormula { value })
    }

    fn into_state_formula(
        self,
        manager: &std::sync::Arc<DagStateFormulaManager>,
        context: String,
    ) -> Result<DagStateFormula, ArtaJsonError> {
        match self {
            Self::Atom(location) => Ok(DagStateFormula::var(manager, LocationId::new(location))),
            Self::Const(constant) => {
                if constant.value {
                    Ok(DagStateFormula::top(manager))
                } else {
                    Ok(DagStateFormula::bot(manager))
                }
            }
            Self::And(and_formula) => {
                if and_formula.and.len() < 2 {
                    return Err(ArtaJsonError::InvalidFormulaArity {
                        context,
                        operator: "and",
                        len: and_formula.and.len(),
                    });
                }
                let terms = and_formula
                    .and
                    .into_iter()
                    .enumerate()
                    .map(|(index, term)| {
                        term.into_state_formula(manager, format!("{context}.and[{index}]"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(DagStateFormula::and(manager, terms))
            }
            Self::Or(or_formula) => {
                if or_formula.or.len() < 2 {
                    return Err(ArtaJsonError::InvalidFormulaArity {
                        context,
                        operator: "or",
                        len: or_formula.or.len(),
                    });
                }
                let terms = or_formula
                    .or
                    .into_iter()
                    .enumerate()
                    .map(|(index, term)| {
                        term.into_state_formula(manager, format!("{context}.or[{index}]"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(DagStateFormula::or(manager, terms))
            }
        }
    }

    fn from_state_formula(formula: &DagStateFormula) -> Self {
        let mut dnf = formula.to_dnf();
        for conjunction in &mut dnf {
            conjunction.sort_by(|lhs, rhs| lhs.name().cmp(rhs.name()));
            conjunction.dedup();
        }
        dnf.sort_by_key(|conjunction| conjunction_sort_key(conjunction));
        dnf.dedup();

        if dnf.is_empty() {
            return Self::const_value(false);
        }
        if dnf.iter().any(Vec::is_empty) {
            return Self::const_value(true);
        }

        let disjuncts = dnf
            .into_iter()
            .map(|conjunction| {
                if conjunction.len() == 1 {
                    Self::Atom(conjunction[0].name().to_string())
                } else {
                    Self::and(
                        conjunction
                            .into_iter()
                            .map(|location| Self::Atom(location.name().to_string()))
                            .collect(),
                    )
                }
            })
            .collect::<Vec<_>>();

        if disjuncts.len() == 1 {
            disjuncts
                .into_iter()
                .next()
                .unwrap_or_else(|| Self::const_value(false))
        } else {
            Self::or(disjuncts)
        }
    }

    fn sort_key(&self) -> String {
        match self {
            Self::Atom(atom) => format!("atom:{atom}"),
            Self::Const(constant) => format!("const:{}", constant.value),
            Self::And(and_formula) => format!(
                "and:{}",
                and_formula
                    .and
                    .iter()
                    .map(Self::sort_key)
                    .collect::<Vec<_>>()
                    .join("\u{001f}")
            ),
            Self::Or(or_formula) => format!(
                "or:{}",
                or_formula
                    .or
                    .iter()
                    .map(Self::sort_key)
                    .collect::<Vec<_>>()
                    .join("\u{001f}")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonAndFormula {
    and: Vec<JsonFormula>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonOrFormula {
    or: Vec<JsonFormula>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonConstFormula {
    #[serde(rename = "const")]
    value: bool,
}

fn interval_to_json_guard(interval: &Interval) -> String {
    interval.to_string().replace('∞', "+")
}

fn parse_init_locations(
    locations: Vec<String>,
    manager: &std::sync::Arc<DagStateFormulaManager>,
) -> Result<DagStateFormula, ArtaJsonError> {
    if locations.is_empty() {
        return Err(ArtaJsonError::EmptyInit);
    }

    Ok(DagStateFormula::or(
        manager,
        locations
            .into_iter()
            .map(|loc| DagStateFormula::var(manager, LocationId::new(loc))),
    ))
}

fn legacy_init_locations_from_state_formula(formula: &DagStateFormula) -> Option<Vec<String>> {
    let mut dnf = formula.to_dnf();
    for conjunction in &mut dnf {
        conjunction.sort_by(|lhs, rhs| lhs.name().cmp(rhs.name()));
        conjunction.dedup();
    }
    dnf.sort_by_key(|conjunction| conjunction_sort_key(conjunction));
    dnf.dedup();

    if dnf.is_empty() {
        return None;
    }

    let mut locations = Vec::with_capacity(dnf.len());
    for conjunction in dnf {
        if conjunction.len() != 1 {
            return None;
        }
        locations.push(conjunction[0].name().to_string());
    }

    Some(locations)
}

fn conjunction_sort_key(conjunction: &[LocationId]) -> String {
    conjunction
        .iter()
        .map(LocationId::name)
        .collect::<Vec<_>>()
        .join("\u{001f}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::interval::Interval;

    fn loc(name: &str) -> LocationId {
        LocationId::new(name)
    }

    #[test]
    fn legacy_init_list_is_parsed_as_disjunction() {
        let arta = parse_arta_json(
            r#"{
              "name":"legacy",
              "l":["q0","q1"],
              "sigma":[],
              "tran":{},
              "init":["q0","q1"],
              "accept":["q1"]
            }"#,
        )
        .unwrap();

        assert_eq!(arta.init().to_string(), "(loc(q0) | loc(q1))");
    }

    #[test]
    fn init_locations_require_disjunction_of_atoms() {
        let manager = DagStateFormulaManager::new();
        let unsupported = DagStateFormula::and(
            &manager,
            vec![
                DagStateFormula::var(&manager, loc("q0")),
                DagStateFormula::var(&manager, loc("q1")),
            ],
        );

        assert_eq!(legacy_init_locations_from_state_formula(&unsupported), None);
    }

    #[test]
    fn json_formula_from_state_formula_uses_constants_and_dnf() {
        let manager = DagStateFormulaManager::new();
        let formula = DagStateFormula::or(
            &manager,
            vec![
                DagStateFormula::and(
                    &manager,
                    vec![
                        DagStateFormula::var(&manager, loc("q1")),
                        DagStateFormula::var(&manager, loc("q0")),
                    ],
                ),
                DagStateFormula::var(&manager, loc("q2")),
            ],
        );

        let json = JsonFormula::from_state_formula(&formula);
        assert_eq!(
            json,
            JsonFormula::or(vec![
                JsonFormula::and(vec![
                    JsonFormula::Atom("q0".to_string()),
                    JsonFormula::Atom("q1".to_string()),
                ]),
                JsonFormula::Atom("q2".to_string()),
            ])
        );
        assert_eq!(
            JsonFormula::from_state_formula(&DagStateFormula::top(&manager)),
            JsonFormula::const_value(true)
        );
        assert_eq!(
            JsonFormula::from_state_formula(&DagStateFormula::bot(&manager)),
            JsonFormula::const_value(false)
        );
    }

    #[test]
    fn interval_writer_uses_ascii_plus() {
        let interval = Interval::from_bounds(true, 4, false, None).unwrap();
        assert_eq!(interval_to_json_guard(&interval), "[4,+)");
    }

    #[test]
    fn rejects_formula_arity_below_two() {
        let err = parse_arta_json(
            r#"{
              "name":"bad",
              "l":["q0"],
              "sigma":["a"],
              "tran":{"0":["q0","a","[0,1)",{"and":["q0"]}]},
              "init":["q0"],
              "accept":[]
            }"#,
        )
        .unwrap_err();

        assert!(matches!(
            err,
            ArtaJsonError::InvalidFormulaArity {
                operator: "and",
                len: 1,
                ..
            }
        ));
    }
}
