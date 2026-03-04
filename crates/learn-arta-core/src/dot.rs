// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deterministic DOT format visualization for ARTA.
//!
//! Successor state formulas are rendered in disjunctive normal form (DNF):
//! - singleton disjuncts are rendered as direct edges to location nodes
//! - non-singleton disjuncts are rendered using black filled intermediate nodes
//!   with fan-out edges to each location in the conjunction

use crate::{
    arta::{Arta, GuardedTransition},
    location::LocationId,
    state_formula::DagStateFormula,
};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    fmt::Debug,
    hash::{Hash, Hasher},
};

/// Options controlling DOT rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DotOptions {
    /// Preserve Unicode symbols (`⊤`, `⊥`, `∞`) in labels.
    pub unicode: bool,
    /// Include a plaintext `L0 = ...` initial-state-formula annotation node.
    pub show_init_node: bool,
}

impl Default for DotOptions {
    fn default() -> Self {
        Self {
            unicode: false,
            show_init_node: true,
        }
    }
}

#[derive(Debug)]
struct TransitionRender<'a> {
    source: &'a LocationId,
    symbol_debug: String,
    symbol_hash: u64,
    guard_sort_key: (u64, u64),
    target_string: String,
    transition: &'a GuardedTransition<DagStateFormula>,
}

impl<A> Arta<A, DagStateFormula>
where
    A: Eq + Hash + Clone + Debug,
{
    /// Render this ARTA as a deterministic DOT graph string.
    pub fn to_dot_string(&self) -> String {
        self.to_dot_string_with(&DotOptions::default())
    }

    /// Render this ARTA as DOT using the provided options.
    pub fn to_dot_string_with(&self, opt: &DotOptions) -> String {
        let mut out = String::from("digraph ARTA {\n");
        out.push_str("  rankdir=LR;\n");

        if opt.show_init_node {
            let init_label = render_label(&format!("L0 = {}", self.init()), opt);
            out.push_str(&format!(
                "  \"__init\" [shape=plaintext, label=\"{init_label}\"];\n"
            ));
        }

        let mut locations: Vec<LocationId> = self.locations().iter().cloned().collect();
        locations.sort_by(|lhs, rhs| lhs.name().cmp(rhs.name()));

        let mut location_node_ids = HashMap::with_capacity(locations.len());
        for (index, location) in locations.iter().enumerate() {
            let node_id = format!("l_{index}");
            location_node_ids.insert(location.clone(), node_id.clone());

            let shape = if self.accepting().contains(location) {
                "doublecircle"
            } else {
                "circle"
            };
            let label = render_label(location.name(), opt);
            out.push_str(&format!(
                "  \"{node_id}\" [shape={shape}, label=\"{label}\"];\n"
            ));
        }

        let mut transitions = Vec::new();
        for ((source, symbol), edges) in self.transitions() {
            let symbol_debug = format!("{symbol:?}");
            let symbol_hash = hash_value(symbol);
            for transition in edges {
                transitions.push(TransitionRender {
                    source,
                    symbol_debug: symbol_debug.clone(),
                    symbol_hash,
                    guard_sort_key: transition.guard.sort_key(),
                    target_string: transition.target.to_string(),
                    transition,
                });
            }
        }

        transitions.sort_by(|lhs, rhs| {
            lhs.source
                .name()
                .cmp(rhs.source.name())
                .then(lhs.symbol_debug.cmp(&rhs.symbol_debug))
                .then(lhs.symbol_hash.cmp(&rhs.symbol_hash))
                .then(lhs.guard_sort_key.cmp(&rhs.guard_sort_key))
                .then(lhs.target_string.cmp(&rhs.target_string))
        });

        let mut and_node_counter = 0usize;
        for transition in transitions {
            if let Some(source_node_id) = location_node_ids.get(transition.source) {
                let edge_label_raw = format!(
                    "{} : {}",
                    transition.symbol_debug, transition.transition.guard
                );
                let edge_label = render_label(&edge_label_raw, opt);

                let mut disjuncts = transition.transition.target.to_dnf();
                for disjunct in &mut disjuncts {
                    disjunct.sort_by(|lhs, rhs| lhs.name().cmp(rhs.name()));
                    disjunct.dedup();
                }
                disjuncts.sort_by_key(|disjunct| (disjunct.len(), disjunct_key(disjunct)));
                disjuncts.dedup();

                for disjunct in disjuncts {
                    if disjunct.len() == 1 {
                        if let Some(target_node_id) = location_node_ids.get(&disjunct[0]) {
                            out.push_str(&format!(
                                "  \"{source_node_id}\" -> \"{target_node_id}\" [label=\"{edge_label}\"];\n"
                            ));
                        }
                        continue;
                    }

                    let and_node_id = format!("and_{and_node_counter}");
                    and_node_counter += 1;
                    out.push_str(&format!(
                        "  \"{and_node_id}\" [shape=box, style=filled, fillcolor=black, label=\"\", width=0.2, height=0.2, fixedsize=true];\n"
                    ));
                    out.push_str(&format!(
                        "  \"{source_node_id}\" -> \"{and_node_id}\" [label=\"{edge_label}\"];\n"
                    ));
                    for loc in disjunct {
                        if let Some(target_node_id) = location_node_ids.get(&loc) {
                            out.push_str(&format!(
                                "  \"{and_node_id}\" -> \"{target_node_id}\";\n"
                            ));
                        }
                    }
                }
            }
        }

        out.push_str("}\n");
        out
    }
}

fn hash_value<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn disjunct_key(disjunct: &[LocationId]) -> String {
    let mut key = String::new();
    for (idx, loc) in disjunct.iter().enumerate() {
        if idx > 0 {
            key.push('\x1f');
        }
        key.push_str(loc.name());
    }
    key
}

fn render_label(raw: &str, opt: &DotOptions) -> String {
    let normalized = normalize_label(raw, opt.unicode);
    escape_dot_label(&normalized)
}

fn normalize_label(raw: &str, unicode: bool) -> String {
    if unicode {
        return raw.to_string();
    }

    raw.replace('⊤', "TOP")
        .replace('⊥', "BOT")
        .replace('∞', "inf")
}

fn escape_dot_label(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use crate::{
        arta::{Arta, GuardedTransition},
        dot::DotOptions,
        location::LocationId,
        state_formula::{DagStateFormula, DagStateFormulaManager, StateFormula},
        time::interval::Interval,
    };
    use std::collections::HashMap;

    #[test]
    fn dot_tiny_arta_matches_expected_snapshot() {
        let mgr = DagStateFormulaManager::new();
        let q0 = LocationId::new("q0");
        let q1 = LocationId::new("q1");

        let mut transitions = HashMap::new();
        transitions.insert(
            (q0.clone(), 'b'),
            vec![GuardedTransition {
                guard: Interval::closed(1, 2).expect("valid interval"),
                target: DagStateFormula::var(&mgr, q0.clone()),
            }],
        );
        transitions.insert(
            (q0.clone(), 'a'),
            vec![GuardedTransition {
                guard: Interval::left_closed_right_open(0, 1).expect("valid interval"),
                target: DagStateFormula::var(&mgr, q1.clone()),
            }],
        );

        let arta = Arta::new(
            vec![q1.clone(), q0.clone()],
            DagStateFormula::var(&mgr, q0),
            vec![q1],
            transitions,
        )
        .expect("valid arta");

        let expected = concat!(
            "digraph ARTA {\n",
            "  rankdir=LR;\n",
            "  \"__init\" [shape=plaintext, label=\"L0 = loc(q0)\"];\n",
            "  \"l_0\" [shape=circle, label=\"q0\"];\n",
            "  \"l_1\" [shape=doublecircle, label=\"q1\"];\n",
            "  \"l_0\" -> \"l_1\" [label=\"'a' : [0,1)\"];\n",
            "  \"l_0\" -> \"l_0\" [label=\"'b' : [1,2]\"];\n",
            "}\n"
        );
        assert_eq!(arta.to_dot_string(), expected);
    }

    #[test]
    fn dot_output_is_identical_for_shuffled_transition_insertion() {
        let arta_a = build_shuffled_arta(true);
        let arta_b = build_shuffled_arta(false);
        assert_eq!(arta_a.to_dot_string(), arta_b.to_dot_string());
    }

    #[test]
    fn dot_non_singleton_disjunct_uses_black_and_node() {
        let mgr = DagStateFormulaManager::new();
        let q0 = LocationId::new("q0");
        let q1 = LocationId::new("q1");
        let q2 = LocationId::new("q2");

        let mut transitions = HashMap::new();
        transitions.insert(
            (q0.clone(), 'a'),
            vec![GuardedTransition {
                guard: Interval::closed(0, 0).expect("valid interval"),
                target: DagStateFormula::and(
                    &mgr,
                    vec![
                        DagStateFormula::var(&mgr, q1.clone()),
                        DagStateFormula::var(&mgr, q2.clone()),
                    ],
                ),
            }],
        );

        let arta = Arta::new(
            vec![q0, q1, q2],
            DagStateFormula::top(&mgr),
            Vec::<LocationId>::new(),
            transitions,
        )
        .expect("valid arta");

        let dot = arta.to_dot_string();
        assert!(dot.contains("\"and_0\" [shape=box, style=filled, fillcolor=black"));
        assert!(dot.contains("\"l_0\" -> \"and_0\" [label=\"'a' : [0,0]\"]"));
        assert!(dot.contains("\"and_0\" -> \"l_1\";"));
        assert!(dot.contains("\"and_0\" -> \"l_2\";"));
    }

    #[test]
    fn dot_escapes_labels_and_normalizes_ascii_by_default() {
        let mgr = DagStateFormulaManager::new();
        let q0 = LocationId::new("q\"0\nline");
        let q1 = LocationId::new("q\\1");

        let mut transitions = HashMap::new();
        transitions.insert(
            (q0.clone(), String::from("sym\"\\\n\t\r")),
            vec![GuardedTransition {
                guard: Interval::from_bounds(true, 0, false, None).expect("valid interval"),
                target: DagStateFormula::var(&mgr, q1.clone()),
            }],
        );

        let arta = Arta::new(
            vec![q0, q1],
            DagStateFormula::bot(&mgr),
            Vec::<LocationId>::new(),
            transitions,
        )
        .expect("valid arta");

        let dot_default = arta.to_dot_string();
        assert!(dot_default.contains("L0 = BOT"));
        assert!(dot_default.contains("[0,inf)"));
        assert!(dot_default.contains("label=\"q\\\"0\\nline\""));
        assert!(dot_default.contains("label=\"q\\\\1\""));
        assert!(!dot_default.contains("q\"0\nline"));
        assert!(dot_default.contains("sym"));

        let dot_unicode = arta.to_dot_string_with(&DotOptions {
            unicode: true,
            show_init_node: true,
        });
        assert!(dot_unicode.contains("⊥"));
        assert!(dot_unicode.contains("∞"));
        assert!(!dot_unicode.contains("L0 = BOT"));
        assert!(!dot_unicode.contains("[0,inf)"));
    }

    fn build_shuffled_arta(reverse: bool) -> Arta<char, DagStateFormula> {
        let mgr = DagStateFormulaManager::new();
        let q0 = LocationId::new("q0");
        let q1 = LocationId::new("q1");
        let q2 = LocationId::new("q2");

        let mut transitions = HashMap::new();
        let entries = if reverse {
            vec![
                (
                    (q1.clone(), 'a'),
                    GuardedTransition {
                        guard: Interval::left_closed_right_open(2, 3).expect("valid interval"),
                        target: DagStateFormula::var(&mgr, q2.clone()),
                    },
                ),
                (
                    (q0.clone(), 'b'),
                    GuardedTransition {
                        guard: Interval::left_closed_right_open(0, 1).expect("valid interval"),
                        target: DagStateFormula::or(
                            &mgr,
                            vec![
                                DagStateFormula::var(&mgr, q1.clone()),
                                DagStateFormula::var(&mgr, q2.clone()),
                            ],
                        ),
                    },
                ),
                (
                    (q0.clone(), 'a'),
                    GuardedTransition {
                        guard: Interval::closed(0, 0).expect("valid interval"),
                        target: DagStateFormula::var(&mgr, q0.clone()),
                    },
                ),
            ]
        } else {
            vec![
                (
                    (q0.clone(), 'a'),
                    GuardedTransition {
                        guard: Interval::closed(0, 0).expect("valid interval"),
                        target: DagStateFormula::var(&mgr, q0.clone()),
                    },
                ),
                (
                    (q1.clone(), 'a'),
                    GuardedTransition {
                        guard: Interval::left_closed_right_open(2, 3).expect("valid interval"),
                        target: DagStateFormula::var(&mgr, q2.clone()),
                    },
                ),
                (
                    (q0.clone(), 'b'),
                    GuardedTransition {
                        guard: Interval::left_closed_right_open(0, 1).expect("valid interval"),
                        target: DagStateFormula::or(
                            &mgr,
                            vec![
                                DagStateFormula::var(&mgr, q1.clone()),
                                DagStateFormula::var(&mgr, q2.clone()),
                            ],
                        ),
                    },
                ),
            ]
        };

        for (key, edge) in entries {
            transitions.insert(key, vec![edge]);
        }

        let locations = if reverse {
            vec![q2.clone(), q0.clone(), q1.clone()]
        } else {
            vec![q0.clone(), q1.clone(), q2.clone()]
        };
        let accepting = if reverse {
            vec![q2.clone(), q1.clone()]
        } else {
            vec![q1.clone(), q2.clone()]
        };

        Arta::new(
            locations,
            DagStateFormula::var(&mgr, q0),
            accepting,
            transitions,
        )
        .expect("valid arta")
    }
}
