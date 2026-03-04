// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Alternating Real-Time Automaton (ARTA) data structure and builder.

mod builder;
mod determinism;
mod execution;
mod simplify;

use crate::{
    error::IntervalError,
    location::LocationId,
    state_formula::{DagStateFormula, StateFormula},
    time::{DelayRep, interval::Interval},
};
use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::Hash,
};
use thiserror::Error;

pub use builder::ArtaBuilder;

/// One guarded outgoing transition in an ARTA transition bucket.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuardedTransition<C = DagStateFormula> {
    /// Guard interval.
    pub guard: Interval,
    /// Successor state formula.
    pub target: C,
}

/// Errors related to ARTA construction or validation.
#[derive(Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ArtaError<A> {
    /// Invalid interval used in a transition.
    #[error("invalid interval in {context}: {source}")]
    InvalidInterval {
        /// Human-readable description of where the invalid interval appeared.
        context: String,
        /// Underlying interval validation failure.
        #[source]
        source: IntervalError,
    },
    /// Reference to an unknown location.
    #[error("unknown location: {loc}")]
    UnknownLocation {
        /// Location identifier that was referenced but not declared.
        loc: LocationId,
    },
    /// Overlapping guards for the same `(location, symbol)` pair.
    #[error(
        "non-deterministic transitions for location {loc}: overlapping guards {guard1} and {guard2} with witness {witness}"
    )]
    NonDeterministic {
        /// Source location whose outgoing transitions overlap.
        loc: LocationId,
        /// Input symbol for the overlapping transition bucket.
        symbol: A,
        /// First overlapping guard interval.
        guard1: Interval,
        /// Second overlapping guard interval.
        guard2: Interval,
        /// Concrete delay that belongs to both guards.
        witness: DelayRep,
    },
}

/// Manual `Debug` impl that avoids the `A: Debug` bound imposed by `#[derive(Debug)]`.
///
/// `symbol` is shown as its Rust type name (via [`std::any::type_name`]) because the
/// library's alphabet contract is only `A: Eq + Hash + Clone`, not `A: Debug`.
/// Requiring `A: Debug` would gate `ArtaError<A>: std::error::Error` on that extra bound,
/// breaking callers that propagate errors over non-`Debug` alphabets.
impl<A> fmt::Debug for ArtaError<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInterval { context, source } => f
                .debug_struct("InvalidInterval")
                .field("context", context)
                .field("source", source)
                .finish(),
            Self::UnknownLocation { loc } => {
                f.debug_struct("UnknownLocation").field("loc", loc).finish()
            }
            Self::NonDeterministic {
                loc,
                symbol: _,
                guard1,
                guard2,
                witness,
            } => f
                .debug_struct("NonDeterministic")
                .field("loc", loc)
                .field("symbol", &std::any::type_name::<A>())
                .field("guard1", guard1)
                .field("guard2", guard2)
                .field("witness", witness)
                .finish(),
        }
    }
}

/// An Alternating Real-Time Automaton (ARTA).
///
/// The automaton uses partial-transition semantics: if no outgoing guard in a
/// `(location, symbol)` bucket contains the queried delay, the next formula is
/// `⊥`.
#[derive(Debug, Clone)]
pub struct Arta<A, C = DagStateFormula> {
    /// Set of all locations in the automaton.
    locations: HashSet<LocationId>,
    /// Accepting locations.
    accepting: HashSet<LocationId>,
    /// Initial state formula.
    init: C,
    /// Transition relation grouped by `(location, symbol)`.
    transitions: HashMap<(LocationId, A), Vec<GuardedTransition<C>>>,
}

impl<A, C> Arta<A, C>
where
    A: Eq + Hash + Clone,
    C: StateFormula<Var = LocationId>,
{
    /// Construct a new ARTA and validate its core invariants.
    ///
    /// This constructor performs the same validation as [`ArtaBuilder::build`]:
    /// every referenced location must be declared, every guard interval must be
    /// valid, and every `(location, symbol)` transition bucket must be
    /// deterministic.
    pub fn new(
        locations: impl IntoIterator<Item = LocationId>,
        init: C,
        accepting: impl IntoIterator<Item = LocationId>,
        transitions: HashMap<(LocationId, A), Vec<GuardedTransition<C>>>,
    ) -> Result<Self, ArtaError<A>> {
        let mut builder = ArtaBuilder::new(init);
        builder.add_locations(locations);
        for loc in accepting {
            builder.add_accepting(loc);
        }
        for ((loc, symbol), edges) in transitions {
            for edge in edges {
                builder.add_transition(loc.clone(), symbol.clone(), edge.guard, edge.target);
            }
        }
        builder.build()
    }

    /// Borrow the set of all locations.
    pub fn locations(&self) -> &HashSet<LocationId> {
        &self.locations
    }

    /// Borrow the set of accepting locations.
    pub fn accepting(&self) -> &HashSet<LocationId> {
        &self.accepting
    }

    /// Borrow the initial state formula.
    pub fn init(&self) -> &C {
        &self.init
    }

    /// Borrow the full transition relation.
    ///
    /// The outer map uses hash-based key ordering. Within each bucket, outgoing
    /// transitions are sorted by guard bounds for deterministic execution and
    /// rendering.
    pub fn transitions(&self) -> &HashMap<(LocationId, A), Vec<GuardedTransition<C>>> {
        &self.transitions
    }

    /// Returns the largest finite integer endpoint used in any guard.
    ///
    /// `+∞` upper endpoints are ignored.
    pub fn max_guard_constant(&self) -> u32 {
        let mut max_constant = 0u32;
        for edges in self.transitions.values() {
            for edge in edges {
                max_constant = max_constant.max(edge.guard.lower_bound());
                if let Some(upper) = edge.guard.upper_bound() {
                    max_constant = max_constant.max(upper);
                }
            }
        }
        max_constant
    }

    /// Return outgoing guarded transitions for `(loc, symbol)` if present.
    ///
    /// The returned slice is sorted by normalized guard bounds.
    pub fn outgoing(&self, loc: &LocationId, symbol: &A) -> Option<&[GuardedTransition<C>]> {
        self.transitions
            .get(&(loc.clone(), symbol.clone()))
            .map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::TimeError,
        state_formula::{DagStateFormula, DagStateFormulaManager, StateFormula},
        time::interval::Interval,
        timed_word::TimedWord,
    };
    use proptest::prelude::*;
    use std::collections::HashMap;

    fn loc(name: &str) -> LocationId {
        LocationId::new(name)
    }

    fn build_base_builder() -> (
        std::sync::Arc<DagStateFormulaManager>,
        ArtaBuilder<char, DagStateFormula>,
    ) {
        let mgr = DagStateFormulaManager::new();
        let q0 = loc("q0");
        let q1 = loc("q1");
        let init = DagStateFormula::var(&mgr, q0.clone());

        let mut builder = ArtaBuilder::new(init);
        builder.add_location(q0);
        builder.add_location(q1);
        (mgr, builder)
    }

    #[test]
    fn determinism_accepts_adjacent_non_overlapping_guards() {
        let (mgr, mut builder) = build_base_builder();
        let q0 = loc("q0");
        let q1 = loc("q1");
        let target = DagStateFormula::var(&mgr, q1);

        builder.add_transition(
            q0.clone(),
            'a',
            Interval::left_open_right_closed(0, 1).expect("valid interval"),
            target.clone(),
        );
        builder.add_transition(
            q0.clone(),
            'a',
            Interval::open(1, 2).expect("valid interval"),
            target,
        );

        let arta = builder.build().expect("builder should accept");
        let outgoing = arta.outgoing(&q0, &'a').expect("transitions should exist");
        assert_eq!(outgoing.len(), 2);
    }

    #[test]
    fn determinism_rejects_overlap_and_reports_valid_witness() {
        let (mgr, mut builder) = build_base_builder();
        let q0 = loc("q0");
        let q1 = loc("q1");
        let target = DagStateFormula::var(&mgr, q1);

        builder.add_transition(
            q0.clone(),
            'a',
            Interval::closed(0, 1).expect("valid interval"),
            target.clone(),
        );
        builder.add_transition(
            q0,
            'a',
            Interval::open(0, 2).expect("valid interval"),
            target,
        );

        let err = builder
            .build()
            .expect_err("overlap should fail determinism");
        match err {
            ArtaError::NonDeterministic {
                guard1,
                guard2,
                witness,
                ..
            } => {
                assert!(guard1.contains(witness));
                assert!(guard2.contains(witness));
            }
            other => panic!("expected NonDeterministic error, got {other:?}"),
        }
    }

    #[test]
    fn overlap_is_allowed_for_different_symbols_or_locations() {
        let (mgr, mut builder) = build_base_builder();
        let q0 = loc("q0");
        let q1 = loc("q1");
        let target = DagStateFormula::var(&mgr, q1.clone());
        let overlap = Interval::closed(0, 2).expect("valid interval");

        builder.add_transition(q0.clone(), 'a', overlap.clone(), target.clone());
        builder.add_transition(q0, 'b', overlap.clone(), target.clone());
        builder.add_transition(q1, 'a', overlap, target);

        assert!(builder.build().is_ok());
    }

    #[test]
    fn duplicate_transition_is_deduplicated() {
        let (mgr, mut builder) = build_base_builder();
        let q0 = loc("q0");
        let q1 = loc("q1");
        let target = DagStateFormula::var(&mgr, q1);
        let guard = Interval::closed(1, 1).expect("valid interval");

        builder.add_transition(q0.clone(), 'a', guard.clone(), target.clone());
        builder.add_transition(q0.clone(), 'a', guard, target);

        let arta = builder.build().expect("duplicates should be deduplicated");
        let outgoing = arta.outgoing(&q0, &'a').expect("transitions should exist");
        assert_eq!(outgoing.len(), 1);
    }

    fn simplify_fixture() -> Arta<char, DagStateFormula> {
        let mgr = DagStateFormulaManager::new();
        let q0 = loc("q0");
        let q1 = loc("q1");
        let q2 = loc("q2");

        let init = DagStateFormula::or(
            &mgr,
            vec![
                DagStateFormula::var(&mgr, q0.clone()),
                DagStateFormula::and(
                    &mgr,
                    vec![
                        DagStateFormula::var(&mgr, q0.clone()),
                        DagStateFormula::var(&mgr, q1.clone()),
                    ],
                ),
                DagStateFormula::and(
                    &mgr,
                    vec![
                        DagStateFormula::var(&mgr, q0.clone()),
                        DagStateFormula::var(&mgr, q1.clone()),
                        DagStateFormula::var(&mgr, q2.clone()),
                    ],
                ),
            ],
        );

        let merged_target = DagStateFormula::or(
            &mgr,
            vec![
                DagStateFormula::var(&mgr, q1.clone()),
                DagStateFormula::and(
                    &mgr,
                    vec![
                        DagStateFormula::var(&mgr, q1.clone()),
                        DagStateFormula::var(&mgr, q2.clone()),
                    ],
                ),
            ],
        );

        let mut builder = ArtaBuilder::new(init);
        builder
            .add_location(q0.clone())
            .add_location(q1.clone())
            .add_location(q2.clone())
            .add_accepting(q1.clone())
            .add_transition(
                q0.clone(),
                'a',
                Interval::closed(0, 0).expect("valid singleton"),
                merged_target,
            )
            .add_transition(
                q0.clone(),
                'a',
                Interval::left_open_right_closed(0, 1).expect("valid interval"),
                DagStateFormula::var(&mgr, q1),
            )
            .add_transition(
                q0,
                'a',
                Interval::open(1, 2).expect("valid interval"),
                DagStateFormula::bot(&mgr),
            );

        builder.build().expect("simplify fixture should build")
    }

    fn transition_snapshot(
        arta: &Arta<char, DagStateFormula>,
    ) -> Vec<(String, char, String, String)> {
        let mut snapshot = arta
            .transitions
            .iter()
            .flat_map(|((source, symbol), edges)| {
                edges.iter().map(move |edge| {
                    (
                        source.name().to_string(),
                        *symbol,
                        edge.guard.to_string(),
                        edge.target.to_string(),
                    )
                })
            })
            .collect::<Vec<_>>();
        snapshot.sort();
        snapshot
    }

    fn overlap_pair_strategy() -> impl Strategy<Value = (Interval, Interval)> {
        (0u32..40, 0u32..40, 0u32..40, 0u32..40).prop_filter_map(
            "overlapping non-identical interval pair",
            |(a, b, c, d)| {
                let (l1, u1) = if a <= b { (a, b) } else { (b, a) };
                let (l2, u2) = if c <= d { (c, d) } else { (d, c) };
                if u1 < l2 || u2 < l1 || (l1 == l2 && u1 == u2) {
                    return None;
                }
                Some((
                    Interval::closed(l1, u1).ok()?,
                    Interval::closed(l2, u2).ok()?,
                ))
            },
        )
    }

    fn execution_example1_arta() -> Arta<char, DagStateFormula> {
        let mgr = DagStateFormulaManager::new();
        let l0 = loc("l0");
        let init = DagStateFormula::var(&mgr, l0.clone());
        let loop_guard = Interval::from_bounds(true, 0, false, None).expect("valid [0,∞)");

        let mut builder = ArtaBuilder::new(init);
        builder
            .add_location(l0.clone())
            .add_accepting(l0.clone())
            .add_transition(l0.clone(), 'a', loop_guard, DagStateFormula::var(&mgr, l0));
        builder.build().expect("example 1 should build")
    }

    fn formula_strategy() -> impl Strategy<Value = DagStateFormula> {
        #[derive(Clone, Debug)]
        enum RawFormula {
            Top,
            Bot,
            Var(usize),
            And(Vec<RawFormula>),
            Or(Vec<RawFormula>),
        }

        impl RawFormula {
            fn build(
                &self,
                mgr: &std::sync::Arc<DagStateFormulaManager>,
                vars: &[LocationId],
            ) -> DagStateFormula {
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
        }

        let raw = prop_oneof![
            Just(RawFormula::Top),
            Just(RawFormula::Bot),
            (0usize..3usize).prop_map(RawFormula::Var),
        ]
        .prop_recursive(4, 64, 8, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..=3).prop_map(RawFormula::And),
                prop::collection::vec(inner, 0..=3).prop_map(RawFormula::Or),
            ]
        });

        raw.prop_map(|rf| {
            let mgr = DagStateFormulaManager::new();
            let vars = vec![loc("p0"), loc("p1"), loc("p2")];
            rf.build(&mgr, &vars)
        })
    }

    fn timed_word_strategy() -> impl Strategy<Value = TimedWord<char>> {
        prop::collection::vec(
            (
                prop_oneof![Just('a'), Just('b')],
                (0u32..=20u32).prop_map(DelayRep::from_half_units),
            ),
            0..=6,
        )
        .prop_map(TimedWord::from_vec)
    }

    #[test]
    fn execution_example1_accepts_any_a_word_with_finite_delays() {
        let arta = execution_example1_arta();
        assert!(arta.accepts(&TimedWord::empty()));

        let w = TimedWord::from_vec(vec![
            ('a', DelayRep::from_integer(0)),
            ('a', DelayRep::from_half_units(3)),
            ('a', DelayRep::from_integer(9)),
        ]);
        assert!(arta.accepts(&w));
    }

    #[test]
    fn execution_example2_missing_transition_rejects() {
        let mgr = DagStateFormulaManager::new();
        let l0 = loc("l0");
        let init = DagStateFormula::var(&mgr, l0.clone());
        let mut builder = ArtaBuilder::new(init);
        builder.add_location(l0.clone());
        let arta = builder.build().expect("example 2 should build");

        assert!(!arta.accepts(&TimedWord::empty()));
        assert!(!arta.accepts(&TimedWord::from_vec(vec![('a', DelayRep::from_integer(1))])));
    }

    #[test]
    fn simplify_rewrites_init_and_targets_removes_bot_and_merges_guards() {
        let mut arta = simplify_fixture();
        let q0 = loc("q0");

        arta.simplify();

        assert_eq!(arta.init.to_string(), "loc(q0)");

        let outgoing = arta
            .outgoing(&q0, &'a')
            .expect("simplified transitions should exist");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].guard.to_string(), "[0,1]");
        assert_eq!(outgoing[0].target.to_string(), "loc(q1)");
    }

    #[test]
    fn simplify_is_idempotent() {
        let mut once = simplify_fixture();
        once.simplify();

        let mut twice = once.clone();
        twice.simplify();

        assert_eq!(once.init, twice.init);
        assert_eq!(transition_snapshot(&once), transition_snapshot(&twice));
    }

    #[test]
    fn simplify_preserves_acceptance_on_representative_words() {
        let arta = simplify_fixture();
        let mut simplified = arta.clone();
        simplified.simplify();

        let words = [
            TimedWord::empty(),
            TimedWord::from_vec(vec![('a', DelayRep::from_integer(0))]),
            TimedWord::from_vec(vec![('a', DelayRep::from_half_units(1))]),
            TimedWord::from_vec(vec![('a', DelayRep::from_integer(1))]),
            TimedWord::from_vec(vec![('a', DelayRep::from_integer(2))]),
        ];

        for word in words {
            assert_eq!(arta.accepts(&word), simplified.accepts(&word));
        }
    }

    #[test]
    fn execution_example3_eval_distinguishes_and_vs_or() {
        let mgr = DagStateFormulaManager::new();
        let l1 = loc("l1");
        let l2 = loc("l2");

        let and_formula = DagStateFormula::and(
            &mgr,
            vec![
                DagStateFormula::var(&mgr, l1.clone()),
                DagStateFormula::var(&mgr, l2.clone()),
            ],
        );
        let or_formula = DagStateFormula::or(
            &mgr,
            vec![
                DagStateFormula::var(&mgr, l1.clone()),
                DagStateFormula::var(&mgr, l2.clone()),
            ],
        );

        let arta = Arta::new(
            vec![l1.clone(), l2.clone()],
            DagStateFormula::top(&mgr),
            vec![l1],
            HashMap::<(LocationId, char), Vec<GuardedTransition<DagStateFormula>>>::new(),
        )
        .expect("valid arta for eval");

        assert!(!arta.eval(&and_formula));
        assert!(arta.eval(&or_formula));
    }

    #[test]
    fn step_location_missing_transition_returns_bot() {
        let mgr = DagStateFormulaManager::new();
        let l0 = loc("l0");
        let init = DagStateFormula::var(&mgr, l0.clone());
        let mut builder = ArtaBuilder::new(init);
        builder.add_location(l0.clone());
        let arta = builder.build().expect("arta should build");

        let bot = arta.step_location(l0, &'a', DelayRep::from_integer(0));
        assert_eq!(bot, DagStateFormula::bot(&mgr));
    }

    #[test]
    fn run_from_empty_word_is_identity() {
        let arta = execution_example1_arta();
        let result = arta.run_from(&arta.init, &TimedWord::empty());
        assert_eq!(result, arta.init);
    }

    #[test]
    fn accepts_f64_converts_and_delegates() {
        let arta = execution_example1_arta();
        let as_f64 = vec![('a', 0.0), ('a', 1.3), ('a', 2.0)];
        assert!(arta.accepts_f64(&as_f64).expect("valid delays"));
    }

    #[test]
    fn accepts_f64_propagates_time_errors() {
        let arta = execution_example1_arta();
        let bad = vec![('a', -0.1)];
        assert!(matches!(
            arta.accepts_f64(&bad),
            Err(TimeError::Negative(_))
        ));
    }

    proptest! {
        #[test]
        fn prop_disjoint_interval_set_builds(
            points in prop::collection::btree_set(0u32..80u32, 0..8),
        ) {
            let mgr = DagStateFormulaManager::new();
            let q0 = loc("q0");
            let q1 = loc("q1");
            let init = DagStateFormula::var(&mgr, q0.clone());
            let target = DagStateFormula::var(&mgr, q1.clone());

            let mut builder = ArtaBuilder::new(init);
            builder.add_location(q0.clone()).add_location(q1);
            for point in points {
                let guard = Interval::closed(point, point).expect("singleton interval is valid");
                builder.add_transition(q0.clone(), 'a', guard, target.clone());
            }

            prop_assert!(builder.build().is_ok());
        }

        #[test]
        fn prop_overlapping_pair_reports_nondeterminism(
            (g1, g2) in overlap_pair_strategy(),
        ) {
            let mgr = DagStateFormulaManager::new();
            let q0 = loc("q0");
            let q1 = loc("q1");
            let init = DagStateFormula::var(&mgr, q0.clone());
            let target = DagStateFormula::var(&mgr, q1.clone());

            let mut builder = ArtaBuilder::new(init);
            builder.add_location(q0.clone()).add_location(q1);
            builder.add_transition(q0.clone(), 'a', g1, target.clone());
            builder.add_transition(q0, 'a', g2, target);

            let err = builder.build().expect_err("overlap should fail determinism");
            match err {
                ArtaError::NonDeterministic { guard1, guard2, witness, .. } => {
                    prop_assert!(guard1.contains(witness));
                    prop_assert!(guard2.contains(witness));
                }
                other => prop_assert!(false, "expected NonDeterministic, got {:?}", other),
            }
        }

        #[test]
        fn prop_run_from_homomorphism_and(
            phi in formula_strategy(),
            psi in formula_strategy(),
            w in timed_word_strategy(),
        ) {
            let mgr = DagStateFormulaManager::new();
            let vars = vec![loc("p0"), loc("p1"), loc("p2")];
            let init = DagStateFormula::var(&mgr, vars[0].clone());
            let mut builder = ArtaBuilder::new(init);
            for v in &vars {
                builder.add_location(v.clone());
            }

            let a_guard = Interval::from_bounds(true, 0, false, None).expect("valid [0,∞)");
            let b_guard = Interval::from_bounds(true, 0, false, None).expect("valid [0,∞)");
            for v in &vars {
                builder.add_transition(v.clone(), 'a', a_guard.clone(), DagStateFormula::var(&mgr, v.clone()));
                builder.add_transition(v.clone(), 'b', b_guard.clone(), DagStateFormula::var(&mgr, v.clone()));
            }
            let arta = builder.build().expect("self-loop arta should build");

            let lhs_input = DagStateFormula::and(&mgr, vec![phi.clone(), psi.clone()]);
            let lhs = arta.run_from(&lhs_input, &w);
            let rhs = DagStateFormula::and(
                &mgr,
                vec![arta.run_from(&phi, &w), arta.run_from(&psi, &w)],
            );
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn prop_run_from_homomorphism_or(
            phi in formula_strategy(),
            psi in formula_strategy(),
            w in timed_word_strategy(),
        ) {
            let mgr = DagStateFormulaManager::new();
            let vars = vec![loc("p0"), loc("p1"), loc("p2")];
            let init = DagStateFormula::var(&mgr, vars[0].clone());
            let mut builder = ArtaBuilder::new(init);
            for v in &vars {
                builder.add_location(v.clone());
            }

            let a_guard = Interval::from_bounds(true, 0, false, None).expect("valid [0,∞)");
            let b_guard = Interval::from_bounds(true, 0, false, None).expect("valid [0,∞)");
            for v in &vars {
                builder.add_transition(v.clone(), 'a', a_guard.clone(), DagStateFormula::var(&mgr, v.clone()));
                builder.add_transition(v.clone(), 'b', b_guard.clone(), DagStateFormula::var(&mgr, v.clone()));
            }
            let arta = builder.build().expect("self-loop arta should build");

            let lhs_input = DagStateFormula::or(&mgr, vec![phi.clone(), psi.clone()]);
            let lhs = arta.run_from(&lhs_input, &w);
            let rhs = DagStateFormula::or(
                &mgr,
                vec![arta.run_from(&phi, &w), arta.run_from(&psi, &w)],
            );
            prop_assert_eq!(lhs, rhs);
        }
    }
}
