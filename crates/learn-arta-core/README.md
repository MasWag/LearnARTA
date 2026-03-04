# learn-arta-core

Core ARTA data structures for LearnARTA.

This crate provides:

- exact half-unit delay normalization via `DelayRep`
- integer-or-infinity guard intervals
- timed words
- positive Boolean state formulas over locations
- ARTA construction and determinism validation
- canonical JSON I/O and DOT rendering

## Install

```bash
cargo add learn-arta-core
```

## Example

```rust
use learn_arta_core::time::interval::Interval;
use learn_arta_core::{
    ArtaBuilder, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId, StateFormula,
    TimedWord,
};

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

let arta = builder.build()?;
let word = TimedWord::from_vec(vec![('a', DelayRep::from_integer(0))]);
assert!(arta.accepts(&word));
# Ok(())
# }
```

Use `learn-arta-core` directly when you need ARTA modeling, validation, JSON import/export, or DOT
generation without the active learner.
