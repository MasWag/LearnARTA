// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{collections::BTreeMap, sync::Arc};

use learn_arta_core::{
    Arta, ArtaBuilder, ArtaError, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId,
    StateFormula, TimedWord, time::interval::Interval,
};
use proptest::{
    collection::{btree_map, vec},
    prelude::*,
};

const MAX_GUARD: u32 = 6;
const MAX_LOCATIONS: usize = 4;
const MAX_WORD_LEN: usize = 6;

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
            Self::Top => DagStateFormula::top(mgr),
            Self::Bot => DagStateFormula::bot(mgr),
            Self::Var(index) => DagStateFormula::var(mgr, vars[*index % vars.len()].clone()),
            Self::And(terms) => DagStateFormula::and(
                mgr,
                terms
                    .iter()
                    .map(|term| term.build(mgr, vars))
                    .collect::<Vec<_>>(),
            ),
            Self::Or(terms) => DagStateFormula::or(
                mgr,
                terms
                    .iter()
                    .map(|term| term.build(mgr, vars))
                    .collect::<Vec<_>>(),
            ),
        }
    }

    fn eval(&self, valuation: &[bool]) -> bool {
        match self {
            Self::Top => true,
            Self::Bot => false,
            Self::Var(index) => valuation[*index % valuation.len()],
            Self::And(terms) => terms.iter().all(|term| term.eval(valuation)),
            Self::Or(terms) => terms.iter().any(|term| term.eval(valuation)),
        }
    }
}

fn raw_formula_strategy(var_count: usize) -> BoxedStrategy<RawFormula> {
    let leaf = prop_oneof![
        Just(RawFormula::Top),
        Just(RawFormula::Bot),
        (0usize..var_count).prop_map(RawFormula::Var),
    ];

    leaf.prop_recursive(4, 48, 8, |inner| {
        prop_oneof![
            vec(inner.clone(), 0..=4).prop_map(RawFormula::And),
            vec(inner, 0..=4).prop_map(RawFormula::Or),
        ]
    })
    .boxed()
}

fn symbol_strategy() -> impl Strategy<Value = char> {
    prop_oneof![Just('a'), Just('b')]
}

fn finite_delay_strategy(max_floor: u32) -> impl Strategy<Value = DelayRep> {
    (0u32..=max_floor, any::<bool>()).prop_map(|(floor, is_integer)| {
        if is_integer {
            DelayRep::from_integer(floor)
        } else {
            DelayRep::from_floor_plus_half(floor)
        }
    })
}

fn timed_word_strategy() -> impl Strategy<Value = TimedWord<char>> {
    vec(
        (symbol_strategy(), finite_delay_strategy(MAX_GUARD)),
        0..=MAX_WORD_LEN,
    )
    .prop_map(TimedWord::from_vec)
}

fn location(index: usize) -> LocationId {
    LocationId::new(format!("q{index}"))
}

#[derive(Clone, Debug)]
struct IntervalSpec {
    lower: u32,
    lower_inclusive: bool,
    upper: Option<u32>,
    upper_inclusive: bool,
}

impl IntervalSpec {
    fn build(&self) -> Interval {
        Interval::from_bounds(
            self.lower_inclusive,
            self.lower,
            self.upper_inclusive,
            self.upper,
        )
        .expect("interval spec is generated valid by construction")
    }

    fn expected_contains(&self, delay: DelayRep) -> bool {
        if delay.is_infinity() {
            return false;
        }

        let half_units = delay.half_units();
        let lower = self.lower * 2 + u32::from(!self.lower_inclusive);
        if half_units < lower {
            return false;
        }

        match self.upper {
            Some(upper) => {
                let upper = upper * 2 - u32::from(!self.upper_inclusive);
                half_units <= upper
            }
            None => true,
        }
    }

    fn boundary_probes(&self) -> Vec<DelayRep> {
        let mut probes = vec![DelayRep::ZERO];
        let lower_half = self.lower * 2;
        for candidate in [
            lower_half.saturating_sub(1),
            lower_half,
            lower_half.saturating_add(1),
        ] {
            probes.push(DelayRep::from_half_units(candidate));
        }

        match self.upper {
            Some(upper) => {
                let upper_half = upper * 2;
                for candidate in [
                    upper_half.saturating_sub(1),
                    upper_half,
                    upper_half.saturating_add(1),
                ] {
                    probes.push(DelayRep::from_half_units(candidate));
                }
            }
            None => {
                for offset in 0..=2 {
                    probes.push(DelayRep::from_half_units(
                        lower_half.saturating_add(2 + offset),
                    ));
                }
            }
        }

        probes.sort_unstable();
        probes.dedup();
        probes
    }
}

fn interval_spec_strategy() -> impl Strategy<Value = IntervalSpec> {
    (0u32..=MAX_GUARD, any::<bool>(), any::<bool>()).prop_flat_map(
        |(lower, infinite_upper, lower_inclusive)| {
            let upper_strategy: BoxedStrategy<Option<u32>> = if infinite_upper {
                Just(None).boxed()
            } else {
                (lower..=MAX_GUARD).prop_map(Some).boxed()
            };

            (
                Just(lower),
                Just(lower_inclusive),
                upper_strategy,
                any::<bool>(),
            )
                .prop_map(|(lower, lower_inclusive, upper, upper_inclusive)| {
                    match upper {
                        Some(upper) if upper == lower => IntervalSpec {
                            lower,
                            lower_inclusive: true,
                            upper: Some(upper),
                            upper_inclusive: true,
                        },
                        Some(upper) => IntervalSpec {
                            lower,
                            lower_inclusive,
                            upper: Some(upper),
                            upper_inclusive,
                        },
                        None => IntervalSpec {
                            lower,
                            lower_inclusive,
                            upper: None,
                            upper_inclusive: false,
                        },
                    }
                })
        },
    )
}

#[derive(Clone, Debug)]
struct ArtaSpec {
    location_count: usize,
    init: RawFormula,
    accepting: Vec<bool>,
    transitions: BTreeMap<(usize, char, u32), RawFormula>,
}

impl ArtaSpec {
    fn build(
        &self,
    ) -> (
        Arc<DagStateFormulaManager>,
        Vec<LocationId>,
        Arta<char, DagStateFormula>,
    ) {
        let mgr = DagStateFormulaManager::new();
        let vars = (0..self.location_count).map(location).collect::<Vec<_>>();
        let init = self.init.build(&mgr, &vars);
        let mut builder = ArtaBuilder::new(init);

        for loc in &vars {
            builder.add_location(loc.clone());
        }
        for (idx, is_accepting) in self.accepting.iter().copied().enumerate() {
            if is_accepting {
                builder.add_accepting(vars[idx].clone());
            }
        }
        for ((source_idx, symbol, point), target) in &self.transitions {
            builder.add_transition(
                vars[*source_idx].clone(),
                *symbol,
                Interval::closed(*point, *point).expect("singleton interval must be valid"),
                target.build(&mgr, &vars),
            );
        }

        let arta = builder
            .build()
            .expect("generated ARTA must be deterministic");
        (mgr, vars, arta)
    }

    fn max_guard_constant(&self) -> u32 {
        self.transitions
            .keys()
            .map(|(_, _, point)| *point)
            .max()
            .unwrap_or(0)
    }
}

fn arta_spec_strategy() -> impl Strategy<Value = ArtaSpec> {
    (1usize..=MAX_LOCATIONS).prop_flat_map(|location_count| {
        let key_strategy = (0usize..location_count, symbol_strategy(), 0u32..=MAX_GUARD);
        (
            Just(location_count),
            raw_formula_strategy(location_count),
            vec(any::<bool>(), location_count),
            btree_map(key_strategy, raw_formula_strategy(location_count), 0..=12),
        )
            .prop_map(|(location_count, init, accepting, transitions)| ArtaSpec {
                location_count,
                init,
                accepting,
                transitions,
            })
    })
}

fn arta_run_case_strategy()
-> impl Strategy<Value = (ArtaSpec, RawFormula, RawFormula, TimedWord<char>)> {
    arta_spec_strategy().prop_flat_map(|spec| {
        let location_count = spec.location_count;
        (
            Just(spec),
            raw_formula_strategy(location_count),
            raw_formula_strategy(location_count),
            timed_word_strategy(),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 96,
        .. ProptestConfig::default()
    })]

    #[test]
    fn prop_delay_rep_integer_half_integer_invariants(
        floor in 0u32..=MAX_GUARD,
        is_integer in any::<bool>(),
    ) {
        let delay = if is_integer {
            DelayRep::from_integer(floor)
        } else {
            DelayRep::from_floor_plus_half(floor)
        };

        prop_assert_eq!(delay.is_integer(), is_integer);
        prop_assert_eq!(delay.floor_int(), Some(floor));
        prop_assert_eq!(delay.ceil_int(), Some(if is_integer { floor } else { floor + 1 }));
        prop_assert_eq!(DelayRep::try_from_f64(delay.to_f64()), Ok(delay));
    }

    #[test]
    fn prop_interval_contains_matches_boundary_expectations(spec in interval_spec_strategy()) {
        let interval = spec.build();
        for probe in spec.boundary_probes() {
            prop_assert_eq!(interval.contains(probe), spec.expected_contains(probe));
        }
    }

    #[test]
    fn prop_interval_intersection_is_symmetric(
        left in interval_spec_strategy(),
        right in interval_spec_strategy(),
    ) {
        let left = left.build();
        let right = right.build();
        prop_assert_eq!(left.intersection(&right), right.intersection(&left));
    }

    #[test]
    fn prop_interval_pick_witness_is_contained(spec in interval_spec_strategy()) {
        let interval = spec.build();
        let witness = interval
            .pick_witness()
            .expect("generated interval should contain a representable witness");
        prop_assert!(interval.contains(witness));
    }

    #[test]
    fn prop_config_commutativity_under_reordering(
        location_count in 1usize..=MAX_LOCATIONS,
        terms in vec(raw_formula_strategy(MAX_LOCATIONS), 0..=6),
    ) {
        let mgr = DagStateFormulaManager::new();
        let vars = (0..location_count).map(location).collect::<Vec<_>>();
        let built = terms
            .iter()
            .map(|term| term.build(&mgr, &vars))
            .collect::<Vec<_>>();
        let mut reversed = built.clone();
        reversed.reverse();

        prop_assert_eq!(
            DagStateFormula::and(&mgr, built.clone()),
            DagStateFormula::and(&mgr, reversed.clone())
        );
        prop_assert_eq!(
            DagStateFormula::or(&mgr, built),
            DagStateFormula::or(&mgr, reversed)
        );
    }

    #[test]
    fn prop_config_idempotence_and_constant_folding(
        location_count in 1usize..=MAX_LOCATIONS,
        raw in raw_formula_strategy(MAX_LOCATIONS),
    ) {
        let mgr = DagStateFormulaManager::new();
        let vars = (0..location_count).map(location).collect::<Vec<_>>();
        let formula = raw.build(&mgr, &vars);
        let top = DagStateFormula::top(&mgr);
        let bot = DagStateFormula::bot(&mgr);

        prop_assert_eq!(
            DagStateFormula::and(&mgr, vec![formula.clone(), formula.clone()]),
            formula.clone()
        );
        prop_assert_eq!(
            DagStateFormula::or(&mgr, vec![formula.clone(), formula.clone()]),
            formula.clone()
        );
        prop_assert_eq!(
            DagStateFormula::and(&mgr, vec![formula.clone(), top]),
            formula.clone()
        );
        prop_assert_eq!(
            DagStateFormula::or(&mgr, vec![formula.clone(), bot.clone()]),
            formula.clone()
        );
        prop_assert_eq!(DagStateFormula::and(&mgr, vec![formula.clone(), bot.clone()]), bot);
        prop_assert_eq!(
            DagStateFormula::or(&mgr, vec![formula.clone(), DagStateFormula::top(&mgr)]),
            DagStateFormula::top(&mgr)
        );
    }

    #[test]
    fn prop_substitute_var_matches_substitution_image(
        location_count in 1usize..=MAX_LOCATIONS,
        var_idx in 0usize..MAX_LOCATIONS,
        sub_raw in raw_formula_strategy(MAX_LOCATIONS),
    ) {
        let mgr = DagStateFormulaManager::new();
        let vars = (0..location_count).map(location).collect::<Vec<_>>();
        let var = DagStateFormula::var(&mgr, vars[var_idx % location_count].clone());
        let sub_formula = sub_raw.build(&mgr, &vars);

        let substituted = DagStateFormula::substitute(&mgr, &var, |_| sub_formula.clone());
        prop_assert_eq!(substituted, sub_formula);
    }

    #[test]
    fn prop_eval_bool_matches_direct_recursive_eval(
        (location_count, raw, valuation) in (1usize..=MAX_LOCATIONS).prop_flat_map(|location_count| (
            Just(location_count),
            raw_formula_strategy(location_count),
            vec(any::<bool>(), location_count),
        )),
    ) {
        let mgr = DagStateFormulaManager::new();
        let vars = (0..location_count).map(location).collect::<Vec<_>>();
        let config = raw.build(&mgr, &vars);
        let expected = raw.eval(&valuation);
        let actual = DagStateFormula::eval_bool(&config, |loc| {
            let idx = vars
                .iter()
                .position(|candidate| candidate == &loc)
                .expect("valuation only queries known locations");
            valuation[idx]
        });

        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn prop_arta_builder_accepts_disjoint_singleton_guards(spec in arta_spec_strategy()) {
        let (_, _, arta) = spec.build();
        prop_assert_eq!(arta.max_guard_constant(), spec.max_guard_constant());
    }

    #[test]
    fn prop_arta_builder_rejects_overlap_with_valid_witness(
        location_count in 1usize..=MAX_LOCATIONS,
        overlap_point in 0u32..=MAX_GUARD,
        upper in 0u32..=MAX_GUARD,
    ) {
        let mgr = DagStateFormulaManager::new();
        let vars = (0..location_count).map(location).collect::<Vec<_>>();
        let init = DagStateFormula::var(&mgr, vars[0].clone());
        let mut builder = ArtaBuilder::new(init);
        for loc in &vars {
            builder.add_location(loc.clone());
        }

        let right_upper = overlap_point.max(upper);
        let left_guard = Interval::closed(overlap_point, overlap_point).expect("singleton guard");
        let right_guard = Interval::closed(overlap_point, right_upper).expect("overlapping guard");
        let left_target = DagStateFormula::var(&mgr, vars[0].clone());
        let right_target = DagStateFormula::bot(&mgr);

        builder.add_transition(vars[0].clone(), 'a', left_guard, left_target);
        builder.add_transition(vars[0].clone(), 'a', right_guard, right_target);

        let err = builder.build().expect_err("overlapping guards must be rejected");
        match err {
            ArtaError::NonDeterministic {
                guard1,
                guard2,
                witness,
                ..
            } => {
                prop_assert!(guard1.contains(witness));
                prop_assert!(guard2.contains(witness));
            }
            other => prop_assert!(false, "expected NonDeterministic, got {other:?}"),
        }
    }

    #[test]
    fn prop_arta_run_preserves_top_bot_and_boolean_homomorphism(
        (spec, left_raw, right_raw, word) in arta_run_case_strategy(),
    ) {
        let (mgr, vars, arta) = spec.build();
        let left = left_raw.build(&mgr, &vars);
        let right = right_raw.build(&mgr, &vars);
        let top = DagStateFormula::top(&mgr);
        let bot = DagStateFormula::bot(&mgr);

        prop_assert_eq!(arta.run_from(&top, &word), top);
        prop_assert_eq!(arta.run_from(&bot, &word), bot);
        prop_assert_eq!(
            arta.run_from(&DagStateFormula::and(&mgr, vec![left.clone(), right.clone()]), &word),
            DagStateFormula::and(&mgr, vec![arta.run_from(&left, &word), arta.run_from(&right, &word)])
        );
        prop_assert_eq!(
            arta.run_from(&DagStateFormula::or(&mgr, vec![left.clone(), right.clone()]), &word),
            DagStateFormula::or(&mgr, vec![arta.run_from(&left, &word), arta.run_from(&right, &word)])
        );
        prop_assert_eq!(arta.accepts(&word), arta.eval(&arta.run_from(arta.init(), &word)));
    }
}
