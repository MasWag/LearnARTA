# learn-arta

Active learning for Alternating Real-Time Automata (ARTA).

This crate provides the observation-table machinery, cohesion repairs, basis minimization, and
hypothesis construction used by LearnARTA's learner.

## Install

Typical setup:

```bash
cargo add learn-arta-core learn-arta-traits learn-arta
```

Pure-Rust setup without the default HiGHS-backed MILP basis minimizer:

```bash
cargo add learn-arta --no-default-features
```

Add `learn-arta-oracles` if you also want the reference exact/caching/white-box oracle
implementations from this workspace.

## Example

```rust
use learn_arta::ActiveArtaLearner;
use learn_arta_core::time::interval::Interval;
use learn_arta_core::{
    ArtaBuilder, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId, StateFormula,
    TimedWord,
};
use learn_arta_traits::MembershipOracle;
use std::convert::Infallible;

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

#[derive(Clone)]
struct ExactMq(learn_arta_core::Arta<char>);

impl MembershipOracle for ExactMq {
    type Symbol = char;
    type Error = Infallible;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        Ok(self.0.accepts(w))
    }
}

let mut learner = ActiveArtaLearner::<char>::new();
let mut mq = ExactMq(target.clone());
let hypothesis = learner.build_hypothesis(&mut mq)?;

let word = TimedWord::from_vec(vec![('a', DelayRep::from_integer(0))]);
assert!(target.accepts(&word));
assert!(hypothesis.accepts(&word));
# Ok(())
# }
```
