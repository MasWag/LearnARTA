# LearnARTA CLI

The [`learn-arta-cli`](../crates/learn-arta-cli) crate is an auxiliary utility crate for working
with LearnARTA's JSON and DOT formats. It is useful for experiments and file-format workflows, but
it is not the primary published API of the repository.

## Running The CLI

Render an ARTA JSON file as DOT:

```bash
cargo run -p learn-arta-cli -- dot examples/small.json
```

Learn a hypothesis with the default HiGHS-backed approximate MILP basis minimizer and emit
canonical JSON to stdout:

```bash
cargo run -p learn-arta-cli -- learn examples/atomic-small.json --quiet
```

Compare two ARTA JSON files:

```bash
cargo run -p learn-arta-cli -- compare examples/atomic-small.json examples/atomic-small.json
```

Select an exact MILP solve explicitly:

```bash
cargo run -p learn-arta-cli -- \
  learn examples/atomic-small.json --basis-minimization exact-milp --quiet
```

Opt out of HiGHS and use the greedy basis minimizer instead:

```bash
cargo run -p learn-arta-cli --no-default-features -- \
  learn examples/atomic-small.json --basis-minimization greedy --quiet
```

## Input Formats

JSON import accepts both canonical LearnARTA ARTA documents and the legacy/original NRTA-style
JSON used in the baseline benchmark corpus. Legacy overlapping guards are canonicalized into a
deterministic ARTA during import, while JSON output always uses the canonical LearnARTA format.

## Related Material

- Workspace and dependency structure: [`architecture/workspace.md`](architecture/workspace.md)
- Build, validation, and reproduction steps: [`reproducibility.md`](reproducibility.md)
- Library-first usage: [`../README.md`](../README.md)
