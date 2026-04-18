#!/bin/sh -eu
####################################################
# NAME
#  run_small.sh
#
# Description
#  Invoke scripts/run.sh for benchmark directories 3_2_2, 4_2_2, 5_2_2,
#  6_2_2, and 8_2_2 under baselines/NLStarRTA/test, first with LearnARTA
#  and then with NLStarRTA.
#
# Synopsis
#  ./scripts/run_small.sh
#
# Requirements
#  * Rust must be installed to run learn-arta.
#  * A native C/C++ toolchain is required because default LearnARTA benchmark
#    runs use the HiGHS-backed MILP backend.
#  * Python 3 must be installed to run NLStarRTA.
#  * GNU time must be available because scripts/run.sh requires it.
#
# Portability
#  This script should work with POSIX sh
#
# Author
#  Masaki Waga
#
# License
#  Apache License, Version 2.0
####################################################

SCRIPT_DIR="$(CDPATH='' cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(CDPATH='' cd "$SCRIPT_DIR/.." && pwd)"
readonly RUNNER="$REPO_ROOT/scripts/run.sh"
readonly BENCHMARK_ROOT="$REPO_ROOT/baselines/NLStarRTA/test"

if ! find \
  "$BENCHMARK_ROOT/3_2_2" \
  "$BENCHMARK_ROOT/4_2_2" \
  "$BENCHMARK_ROOT/5_2_2" \
  "$BENCHMARK_ROOT/6_2_2" \
  "$BENCHMARK_ROOT/8_2_2" \
  -mindepth 1 -maxdepth 1 -type f -name '*_*_*-*.json' | IFS= read -r _; then
  printf 'no benchmark files/cases found under selected suites in %s. Probably you did not initialize git submodule.\n' "$BENCHMARK_ROOT" >&2
  exit 1
fi

# Run LearnARTA
find \
  "$BENCHMARK_ROOT/3_2_2" \
  "$BENCHMARK_ROOT/4_2_2" \
  "$BENCHMARK_ROOT/5_2_2" \
  "$BENCHMARK_ROOT/6_2_2" \
  "$BENCHMARK_ROOT/8_2_2" \
  -type f -name '*_*_*-*.json' \
  -exec "$RUNNER" learn-arta {} \;

# Run NLStarRTA
find \
  "$BENCHMARK_ROOT/3_2_2" \
  "$BENCHMARK_ROOT/4_2_2" \
  "$BENCHMARK_ROOT/5_2_2" \
  "$BENCHMARK_ROOT/6_2_2" \
  "$BENCHMARK_ROOT/8_2_2" \
  -type f -name '*_*_*-*.json' \
  -exec "$RUNNER" nlstar-rta {} \;
