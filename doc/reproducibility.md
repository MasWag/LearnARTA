# Reproducibility And Validation

For the automated validation surface, see [`../.github/workflows/ci.yml`](../.github/workflows/ci.yml).
For the pinned local development toolchain, see [`../rust-toolchain.toml`](../rust-toolchain.toml).
For artifact packaging, smoke tests, and full paper-experiment reproduction, see
[`../artifact/README.md`](../artifact/README.md).
This page only keeps the source-tree details that are not already captured there.

## Smoke Test

End-to-end smoke test on a checked-in example:

```bash
cargo run -p learn-arta-cli -- \
  learn examples/atomic-small.json --output /tmp/atomic-small-hypothesis.json
cargo run -p learn-arta-cli -- \
  compare examples/atomic-small.json /tmp/atomic-small-hypothesis.json
```

## Baseline Submodule

The [`../baselines/NLStarRTA`](../baselines/NLStarRTA) directory is an optional Git submodule and
is not part of the Rust workspace build or test path.

Fetch it only if you want to run the baseline-oriented experiment scripts documented in
[`../artifact/README.md`](../artifact/README.md):

```bash
git submodule update --init --recursive
```
