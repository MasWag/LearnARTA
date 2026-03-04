// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Positive Boolean state formulas over locations.
//!
//! This module defines a pluggable [`StateFormula`] trait, a canonical
//! semantic-key type [`MinimalModelKey`], and a hash-consed DAG
//! implementation [`DagStateFormula`] managed by [`DagStateFormulaManager`].

mod dag;
mod minimal_model;
mod traits;

pub use dag::{DagStateFormula, DagStateFormulaManager};
pub use minimal_model::MinimalModelKey;
pub use traits::StateFormula;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::LocationId;
    use proptest::prelude::*;
    use std::hash::{Hash, Hasher};
    use std::sync::Arc;

    fn loc(name: &str) -> LocationId {
        LocationId::new(name)
    }

    #[test]
    fn canonicalization_commutative_and_or() {
        let mgr = DagStateFormulaManager::new();
        let x = DagStateFormula::var(&mgr, loc("x"));
        let y = DagStateFormula::var(&mgr, loc("y"));

        assert_eq!(
            DagStateFormula::and(&mgr, vec![x.clone(), y.clone()]),
            DagStateFormula::and(&mgr, vec![y.clone(), x.clone()])
        );
        assert_eq!(
            DagStateFormula::or(&mgr, vec![x.clone(), y.clone()]),
            DagStateFormula::or(&mgr, vec![y, x])
        );
    }

    #[test]
    fn idempotence_for_and_or() {
        let mgr = DagStateFormulaManager::new();
        let x = DagStateFormula::var(&mgr, loc("x"));

        assert_eq!(DagStateFormula::and(&mgr, vec![x.clone(), x.clone()]), x);
        assert_eq!(DagStateFormula::or(&mgr, vec![x.clone(), x.clone()]), x);
    }

    #[test]
    fn constant_folding_rules() {
        let mgr = DagStateFormulaManager::new();
        let x = DagStateFormula::var(&mgr, loc("x"));
        let top = DagStateFormula::top(&mgr);
        let bot = DagStateFormula::bot(&mgr);

        assert_eq!(DagStateFormula::and(&mgr, vec![x.clone(), top]), x);
        assert_eq!(DagStateFormula::or(&mgr, vec![x.clone(), bot.clone()]), x);
        assert_eq!(
            DagStateFormula::and(&mgr, vec![x.clone(), bot.clone()]),
            bot
        );
        assert_eq!(
            DagStateFormula::or(&mgr, vec![x.clone(), DagStateFormula::top(&mgr)]),
            DagStateFormula::top(&mgr)
        );
    }

    #[test]
    fn flattening_rules() {
        let mgr = DagStateFormulaManager::new();
        let x = DagStateFormula::var(&mgr, loc("x"));
        let y = DagStateFormula::var(&mgr, loc("y"));
        let z = DagStateFormula::var(&mgr, loc("z"));

        let nested_and = DagStateFormula::and(&mgr, vec![y.clone(), z.clone()]);
        let flat_and = DagStateFormula::and(&mgr, vec![x.clone(), y.clone(), z.clone()]);
        assert_eq!(
            DagStateFormula::and(&mgr, vec![x.clone(), nested_and]),
            flat_and
        );

        let nested_or = DagStateFormula::or(&mgr, vec![y.clone(), z.clone()]);
        let flat_or = DagStateFormula::or(&mgr, vec![x.clone(), y, z]);
        assert_eq!(DagStateFormula::or(&mgr, vec![x, nested_or]), flat_or);
    }

    #[test]
    fn cross_manager_structural_formulas_are_not_equal() {
        let mgr1 = DagStateFormulaManager::new();
        let mgr2 = DagStateFormulaManager::new();

        let f1 = DagStateFormula::and(
            &mgr1,
            vec![
                DagStateFormula::var(&mgr1, loc("x")),
                DagStateFormula::var(&mgr1, loc("y")),
            ],
        );
        let f2 = DagStateFormula::and(
            &mgr2,
            vec![
                DagStateFormula::var(&mgr2, loc("x")),
                DagStateFormula::var(&mgr2, loc("y")),
            ],
        );

        assert_ne!(f1, f2);
        assert_ne!(hash_value(&f1), hash_value(&f2));
    }

    #[test]
    fn semantic_key_collapses_absorption_but_preserves_structural_inequality() {
        let mgr = DagStateFormulaManager::new();
        let x = loc("x");
        let y = loc("y");

        let base = DagStateFormula::var(&mgr, x.clone());
        let absorbed = DagStateFormula::or(
            &mgr,
            vec![
                base.clone(),
                DagStateFormula::and(
                    &mgr,
                    vec![
                        DagStateFormula::var(&mgr, x.clone()),
                        DagStateFormula::var(&mgr, y),
                    ],
                ),
            ],
        );

        assert_ne!(base, absorbed);
        assert_eq!(base.semantic_key(), absorbed.semantic_key());
        assert_eq!(absorbed.semantic_key().clauses(), &[vec![x]]);
    }

    #[test]
    fn semantic_key_uses_expected_top_and_bot_shapes() {
        let mgr = DagStateFormulaManager::new();

        let top_key: MinimalModelKey<LocationId> = MinimalModelKey::top();
        let bot_key: MinimalModelKey<LocationId> = MinimalModelKey::bot();

        assert_eq!(DagStateFormula::top(&mgr).semantic_key(), top_key);
        assert_eq!(DagStateFormula::bot(&mgr).semantic_key(), bot_key);
        assert_eq!(
            DagStateFormula::top(&mgr).semantic_key().clauses(),
            &[Vec::new()]
        );
        assert!(
            DagStateFormula::bot(&mgr)
                .semantic_key()
                .clauses()
                .is_empty()
        );
    }

    #[test]
    fn cross_manager_terms_are_imported_and_normalized() {
        let mgr1 = DagStateFormulaManager::new();
        let mgr2 = DagStateFormulaManager::new();

        let x1 = DagStateFormula::var(&mgr1, loc("x"));
        let y2 = DagStateFormula::var(&mgr2, loc("y"));

        let mixed = DagStateFormula::and(&mgr1, vec![x1.clone(), y2]);
        let expected = DagStateFormula::and(&mgr1, vec![x1, DagStateFormula::var(&mgr1, loc("y"))]);

        assert_eq!(mixed, expected);
    }

    #[test]
    fn display_is_deterministic() {
        let mgr = DagStateFormulaManager::new();
        let x = DagStateFormula::var(&mgr, loc("x"));
        let y = DagStateFormula::var(&mgr, loc("y"));
        let formula = DagStateFormula::and(&mgr, vec![y, x]);

        assert_eq!(formula.to_string(), "(loc(x) & loc(y))");
    }

    #[derive(Clone, Debug)]
    enum RawFormula {
        Top,
        Bot,
        Var(usize),
        And(Vec<RawFormula>),
        Or(Vec<RawFormula>),
    }

    impl RawFormula {
        fn build(&self, mgr: &Arc<DagStateFormulaManager>, vars: &[LocationId]) -> DagStateFormula {
            match self {
                RawFormula::Top => DagStateFormula::top(mgr),
                RawFormula::Bot => DagStateFormula::bot(mgr),
                RawFormula::Var(i) => DagStateFormula::var(mgr, vars[*i % vars.len()].clone()),
                RawFormula::And(terms) => DagStateFormula::and(
                    mgr,
                    terms.iter().map(|t| t.build(mgr, vars)).collect::<Vec<_>>(),
                ),
                RawFormula::Or(terms) => DagStateFormula::or(
                    mgr,
                    terms.iter().map(|t| t.build(mgr, vars)).collect::<Vec<_>>(),
                ),
            }
        }

        fn tree_size(&self) -> usize {
            match self {
                RawFormula::Top | RawFormula::Bot | RawFormula::Var(_) => 1,
                RawFormula::And(terms) | RawFormula::Or(terms) => {
                    1 + terms.iter().map(Self::tree_size).sum::<usize>()
                }
            }
        }
    }

    fn raw_formula_strategy() -> impl Strategy<Value = RawFormula> {
        let leaf = prop_oneof![
            Just(RawFormula::Top),
            Just(RawFormula::Bot),
            (0usize..4usize).prop_map(RawFormula::Var),
        ];

        leaf.prop_recursive(4, 64, 8, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..=4).prop_map(RawFormula::And),
                prop::collection::vec(inner, 0..=4).prop_map(RawFormula::Or),
            ]
        })
    }

    fn vars_pool() -> Vec<LocationId> {
        vec![loc("v0"), loc("v1"), loc("v2"), loc("v3")]
    }

    proptest! {
        #[test]
        fn prop_commutativity_normalization_and(terms in prop::collection::vec(raw_formula_strategy(), 0..=6)) {
            let mgr = DagStateFormulaManager::new();
            let vars = vars_pool();
            let built: Vec<_> = terms.iter().map(|t| t.build(&mgr, &vars)).collect();
            let mut reversed = built.clone();
            reversed.reverse();

            let lhs = DagStateFormula::and(&mgr, built);
            let rhs = DagStateFormula::and(&mgr, reversed);
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn prop_commutativity_normalization_or(terms in prop::collection::vec(raw_formula_strategy(), 0..=6)) {
            let mgr = DagStateFormulaManager::new();
            let vars = vars_pool();
            let built: Vec<_> = terms.iter().map(|t| t.build(&mgr, &vars)).collect();
            let mut reversed = built.clone();
            reversed.reverse();

            let lhs = DagStateFormula::or(&mgr, built);
            let rhs = DagStateFormula::or(&mgr, reversed);
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn prop_canonical_rebuild(raw in raw_formula_strategy()) {
            let mgr = DagStateFormulaManager::new();
            let vars = vars_pool();

            let f1 = raw.build(&mgr, &vars);
            let f2 = raw.build(&mgr, &vars);
            prop_assert_eq!(f1, f2);
        }

        #[test]
        fn prop_size_and_vars_invariants(raw in raw_formula_strategy()) {
            let mgr = DagStateFormulaManager::new();
            let vars = vars_pool();

            let cfg = raw.build(&mgr, &vars);
            let formula_vars = cfg.vars();

            prop_assert!(cfg.size() >= formula_vars.len());
            prop_assert!(cfg.size() <= raw.tree_size().saturating_mul(2).saturating_add(2));
        }

        #[test]
        fn prop_semantic_key_matches_default_truth_table(raw in raw_formula_strategy()) {
            let mgr = DagStateFormulaManager::new();
            let vars = vars_pool();

            let cfg = raw.build(&mgr, &vars);
            prop_assert_eq!(
                cfg.semantic_key(),
                crate::state_formula::minimal_model::default_semantic_key(&cfg)
            );
        }
    }

    #[test]
    fn to_dnf_matches_expected_shapes() {
        let mgr = DagStateFormulaManager::new();
        let q0 = loc("q0");
        let q1 = loc("q1");
        let q2 = loc("q2");

        let disj = DagStateFormula::or(
            &mgr,
            vec![
                DagStateFormula::var(&mgr, q0.clone()),
                DagStateFormula::var(&mgr, q1.clone()),
            ],
        );
        let conj = DagStateFormula::and(&mgr, vec![disj, DagStateFormula::var(&mgr, q2.clone())]);

        let dnf = conj.to_dnf();
        assert_eq!(dnf.len(), 2);

        let mut opt_a = vec![q0, q2.clone()];
        opt_a.sort();
        let mut opt_b = vec![q1, q2];
        opt_b.sort();

        let mut actual0 = dnf[0].clone();
        actual0.sort();
        let mut actual1 = dnf[1].clone();
        actual1.sort();

        assert!((actual0 == opt_a && actual1 == opt_b) || (actual0 == opt_b && actual1 == opt_a));
    }

    #[test]
    fn substitute_rewrites_vars_and_normalizes() {
        let mgr = DagStateFormulaManager::new();
        let q0 = loc("q0");
        let q1 = loc("q1");
        let q2 = loc("q2");
        let q3 = loc("q3");

        let formula = DagStateFormula::and(
            &mgr,
            vec![
                DagStateFormula::var(&mgr, q0.clone()),
                DagStateFormula::or(
                    &mgr,
                    vec![
                        DagStateFormula::var(&mgr, q1.clone()),
                        DagStateFormula::var(&mgr, q2.clone()),
                    ],
                ),
            ],
        );

        let substituted = DagStateFormula::substitute(&mgr, &formula, |v| {
            if v == q0 {
                DagStateFormula::or(
                    &mgr,
                    vec![
                        DagStateFormula::var(&mgr, q1.clone()),
                        DagStateFormula::bot(&mgr),
                    ],
                )
            } else if v == q1 {
                DagStateFormula::top(&mgr)
            } else if v == q2 {
                DagStateFormula::var(&mgr, q3.clone())
            } else {
                DagStateFormula::var(&mgr, v)
            }
        });

        let expected = DagStateFormula::and(
            &mgr,
            vec![
                DagStateFormula::var(&mgr, q1),
                DagStateFormula::or(
                    &mgr,
                    vec![DagStateFormula::top(&mgr), DagStateFormula::var(&mgr, q3)],
                ),
            ],
        );

        assert_eq!(substituted, expected);
        assert_eq!(
            substituted,
            DagStateFormula::and(&mgr, vec![DagStateFormula::var(&mgr, loc("q1"))])
        );
    }

    #[test]
    fn eval_bool_matches_expected_truth_table() {
        let mgr = DagStateFormulaManager::new();
        let q0 = loc("q0");
        let q1 = loc("q1");
        let q2 = loc("q2");

        let formula = DagStateFormula::or(
            &mgr,
            vec![
                DagStateFormula::and(
                    &mgr,
                    vec![
                        DagStateFormula::var(&mgr, q0.clone()),
                        DagStateFormula::var(&mgr, q1.clone()),
                    ],
                ),
                DagStateFormula::var(&mgr, q2.clone()),
            ],
        );

        assert!(!DagStateFormula::eval_bool(&formula, |v| v == q0));
        assert!(DagStateFormula::eval_bool(&formula, |v| v == q0 || v == q1));
        assert!(DagStateFormula::eval_bool(&formula, |v| v == q2));
        assert!(!DagStateFormula::eval_bool(&formula, |_| false));
    }

    fn hash_value(value: &DagStateFormula) -> u64 {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }
}
