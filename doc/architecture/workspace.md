# LearnARTA Workspace Architecture

This note records the current crate roles and dependency direction inside the
Rust workspace. It is intentionally short: the goal is to make it obvious where
new code belongs and which dependency edges are acceptable.

## Crate Roles

| Crate | Responsibility |
| --- | --- |
| `learn-arta-core` | Core ARTA data model: delays, intervals, timed words, formulas, JSON, and DOT |
| `learn-arta-traits` | Shared traits and query result types used by learners and oracles |
| `learn-arta` | Observation tables, cohesion repair, evidence AFA construction, and hypothesis generation |
| `learn-arta-oracles` | Concrete membership/equivalence oracle implementations |
| `learn-arta-cli` | Auxiliary command-line tooling for experiments and reproducibility |

## Dependency Rules

The intended dependency direction is:

`learn-arta-core <- learn-arta-traits <- {learn-arta, learn-arta-oracles} <- learn-arta-cli`

Read that as:

- `learn-arta-core` is the foundation and should not depend on other workspace crates.
- `learn-arta-traits` may depend on `learn-arta-core`, but not on learner or oracle implementations.
- `learn-arta` and `learn-arta-oracles` may depend on `learn-arta-core` and `learn-arta-traits`.
- `learn-arta-cli` may depend on the library crates, but library crates must not depend on the CLI.

## Non-Workspace Code

- `baselines/` holds third-party or reference implementations used for comparisons.
- Baseline code must stay outside `[workspace].members`.
- Benchmark scripts and log-processing helpers belong under `scripts/`, not inside library crates.

## Documentation Pointers

- Semantics and algorithm notes live under `doc/spec/`.
- User-facing project overview and build instructions live in the repository
  `README.md`.
- If a change introduces a new architectural rule or a new crate-level role,
  update this file together with the code.
