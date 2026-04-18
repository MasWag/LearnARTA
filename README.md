# LearnARTA

[![CI](https://github.com/MasWag/LearnARTA/actions/workflows/ci.yml/badge.svg)](https://github.com/MasWag/LearnARTA/actions/workflows/ci.yml)
[![License: Apache-2.0 OR MIT](https://img.shields.io/badge/License-Apache--2.0%20OR%20MIT-blue.svg)](#license)
[![Rustdoc](https://img.shields.io/badge/Rustdoc-latest-orange)](https://maswag.github.io/LearnARTA)

LearnARTA is a Rust prototype library for learning Alternating Real-Time Automata (ARTA).
The repository is library-first: the main published surface is the Rust workspace crates, while
the CLI and benchmark scripts are auxiliary tools for experiments and reproducibility.

## Workspace Crates

| Crate | Role |
|-------|------|
| [`learn-arta-core`](crates/learn-arta-core) | Core ARTA types: delays, intervals, timed words, state formulas, JSON, and DOT rendering |
| [`learn-arta-traits`](crates/learn-arta-traits) | Shared learning/oracle traits and counterexample types used across the workspace |
| [`learn-arta`](crates/learn-arta) | Active learning loop, observation table maintenance, cohesion repair, and hypothesis construction |
| [`learn-arta-oracles`](crates/learn-arta-oracles) | Concrete oracle implementations: exact membership, caching, and exact white-box equivalence |
| [`learn-arta-cli`](crates/learn-arta-cli) | Auxiliary CLI for JSON, DOT, comparison, and learning experiments; not intended as the primary product surface |

## Getting Started

For a typical library integration, start with the core model crate, the shared oracle traits, and
the learner itself:

```bash
cargo add learn-arta-core learn-arta-traits learn-arta
```

Add the reference oracle implementations if you want exact membership, memoization, or the
white-box equivalence oracle provided by this workspace:

```bash
cargo add learn-arta-oracles
```

If you want a pure-Rust setup without the default HiGHS-backed MILP basis minimizer, disable the
default feature on `learn-arta`:

```bash
cargo add learn-arta --no-default-features
```

This library works with Rust >= 1.88.

Minimal end-to-end learning loop:

```rust
use learn_arta::ActiveArtaLearner;
use learn_arta_core::time::interval::Interval;
use learn_arta_core::{
    ArtaBuilder, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId, StateFormula,
    TimedWord,
};
use learn_arta_traits::MembershipOracle;
use std::convert::Infallible;

#[derive(Clone)]
struct ExactMq(learn_arta_core::Arta<char>);

impl MembershipOracle for ExactMq {
    type Symbol = char;
    type Error = Infallible;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        Ok(self.0.accepts(w))
    }
}

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mgr = DagStateFormulaManager::new();
let q0 = LocationId::new("q0");
let init = DagStateFormula::var(&mgr, q0.clone());

let mut builder = ArtaBuilder::new(init);
builder.add_location(q0.clone()).add_accepting(q0.clone());
builder.add_transition(
    q0.clone(),
    'a',
    Interval::closed(0, 0)?,
    DagStateFormula::var(&mgr, q0.clone()),
);
let target = builder.build()?;

let mut learner = ActiveArtaLearner::<char>::new();
let mut mq = ExactMq(target.clone());
let hypothesis = learner.build_hypothesis(&mut mq)?;

let word = TimedWord::from_vec(vec![('a', DelayRep::from_integer(0))]);
assert!(target.accepts(&word));
assert!(hypothesis.accepts(&word));
# Ok(())
# }
```

Use [`learn-arta-core`](crates/learn-arta-core) on its own if you only need ARTA modeling, exact
time normalization, JSON import/export, or DOT rendering. Use
[`learn-arta-oracles`](crates/learn-arta-oracles) if you want concrete oracle implementations
instead of writing the traits yourself.

## Further Reading

- Workspace structure and crate layering: [`doc/architecture/workspace.md`](doc/architecture/workspace.md)
- CLI usage and JSON-format workflows: [`doc/cli.md`](doc/cli.md)
- Build requirements, validation commands, smoke tests, and benchmark reproduction: [`doc/reproducibility.md`](doc/reproducibility.md)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.
