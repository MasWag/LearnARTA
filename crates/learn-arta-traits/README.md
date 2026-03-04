# learn-arta-traits

Shared oracle and counterexample traits for LearnARTA.

This crate provides the `MembershipOracle` and `EquivalenceOracle` traits used by the learner and
oracle crates, plus the common counterexample type aliases used around the workspace.

## Install

```bash
cargo add learn-arta-core learn-arta-traits
```

Add `learn-arta` when you want to run the active learner against your own oracle implementations.

## Example

```rust
use learn_arta_core::TimedWord;
use learn_arta_traits::MembershipOracle;
use std::convert::Infallible;

struct EvenLengthMq;

impl MembershipOracle for EvenLengthMq {
    type Symbol = char;
    type Error = Infallible;

    fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
        Ok(w.len() % 2 == 0)
    }
}
```

Use `learn-arta-oracles` if you want ready-made exact and caching oracle implementations instead of
defining the traits yourself.
