#!/bin/bash
################################################################
# Name
#  log_to_json.sh
#
# Description
#  Convert a benchmark terminal log produced by scripts/run.sh and its
#  sibling GNU time report into a single JSON summary on stdout.
#
# Prerequisites
#  * Bash
#  * jc
#  * jo
#  * jq
#
# Synopsis
#  ./scripts/log_to_json.sh [LOG_FILE]
#
# Notes
#  * LOG_FILE should be the *.log file emitted by scripts/run.sh.
#  * The script reads the matching *.gtime file next to LOG_FILE.
#
# Example
#  ./scripts/log_to_json.sh \
#    ./logs/3_2_2/3_2_2-1/learn-arta-20260319-124250.log
#
# Author
#  Masaki Waga
#
# License
#  Apache 2.0 License
################################################################

set -eu

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 [LOG_FILE]"
    exit 1
fi

# Check if jc, jo, and jq are available
if ! command -v jc >/dev/null 2>&1; then
    echo "error: jc is required. Install jc." >&2
    exit 1
fi
if ! command -v jo >/dev/null 2>&1; then
    echo "error: jo is required. Install jo." >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    echo "error: jq is required. Install jq." >&2
    exit 1
fi

# Parse the log filename to extract the mode, suite name, and benchmark name
readonly LOG_NAME_NO_SUFFIX="${1%.*}"
readonly LOG_NAME="${LOG_NAME_NO_SUFFIX}.log"
readonly GTIME_LOG="${LOG_NAME_NO_SUFFIX}.gtime"
MODE="$(basename "$LOG_NAME" | cut -d'-' -f1,2)"
BENCHMARK_NAME="$(basename "$(dirname "$LOG_NAME")")"
readonly SUITE_NAME="${BENCHMARK_NAME%-*}"
ID="${SUITE_NAME}-${BENCHMARK_NAME}-$(basename "$LOG_NAME_NO_SUFFIX")"

# Parse the plaintext log and convert it to JSON using jo
cat "$LOG_NAME" |
    if [ "$MODE" = learn-arta ]; then
        awk '/hypothesis states/ {num_states=$NF} /Number of Equivalence queries/ {print "eq_queries="$NF} /with caching/ {print "mem_queries="$NF} /rows/ {print "rows="$NF} /columns/ {print "columns="$NF} END{print "num_states="num_states}'
    elif [ "$MODE" = nlstar-rta ]; then
        awk '/prime rows/ {print "num_states="$NF} /equivalence/ {print "eq_queries="$NF} /membership/ {print "mem_queries="$NF} /of S/ {s=$NF} /of R/ {print "rows="(s+$NF)} /of E/ {print "columns="($NF + 1)}'
    else
        echo "Error: Invalid mode. Use 'learn-arta' or 'nlstar-rta'." >&2
        kill 0
    fi |
    xargs jo "id=$ID" "benchmark_name=$BENCHMARK_NAME" "suite_name=$SUITE_NAME" |
    jq -s 'add' - <(jc --time < "$GTIME_LOG")
