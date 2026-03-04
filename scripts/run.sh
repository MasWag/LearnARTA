#!/bin/sh
################################################################
# Name
#  run.sh
#
# Description
#  Run one benchmark JSON case with LearnARTA or NLStarRTA and store the
#  terminal log, GNU time report, and LearnARTA result JSON under
#  logs/{suite_name}/{benchmark_name}/.
#
# Prerequisites
#  * Rust (for LearnARTA)
#  * Native C/C++ toolchain when reproducing LearnARTA MILP-backed runs
#  * Python 3 (for NLStarRTA)
#  * GNU time (gtime or GNU /usr/bin/time)
#
# Synopsis
#  ./scripts/run.sh [learn-arta|nlstar-rta] [JSON_FILE]
#
# Notes
#  * JSON_FILE should follow the benchmark naming convention
#    {suite_name}-{case_id}.json.
#  * LearnARTA writes a sibling *.result.json file in the log directory.
#
# Example
#  ./scripts/run.sh \
#    learn-arta \
#    ./baselines/NLStarRTA/test/3_2_2/3_2_2-1.json
#
# Author
#  Masaki Waga
#
# License
#  Apache 2.0 License
################################################################

set -eu

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 [learn-arta|nlstar-rta] [JSON_FILE]"
    exit 1
fi

# Check if GNU time is available
if command -v gtime >/dev/null 2>&1; then
    TIMER_COMMAND=gtime
elif [ -x /usr/bin/time ] && /usr/bin/time --version 2>/dev/null | grep -q 'GNU'; then
    TIMER_COMMAND=/usr/bin/time
else
    echo "error: GNU time is required. Install gtime or provide GNU /usr/bin/time." >&2
    exit 1
fi

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)

# We assume that the JSON filename consists of [suite_name]-[id].json
readonly MODE="$1"
readonly JSON_PATH="$2"
JSON_FILENAME=$(basename "$JSON_PATH")
readonly SUITE_NAME="${JSON_FILENAME%-*}"
readonly BENCHMARK_NAME="${JSON_FILENAME%.*}"

# Ensure that SUITE_NAME and BENCHMARK_NAME are not empty
if [ -z "$SUITE_NAME" ] || [ -z "$BENCHMARK_NAME" ]; then
    echo "Error: Could not extract suite name and benchmark name from JSON filename."
    exit 1
fi

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
LOG_ID="${MODE}-${TIMESTAMP}"

# Create the log directory if it does not exist
mkdir -p "${REPO_ROOT}/logs/${SUITE_NAME}/${BENCHMARK_NAME}"

GTIME_LOG="${REPO_ROOT}/logs/${SUITE_NAME}/${BENCHMARK_NAME}/${LOG_ID}.gtime"
RESULT_LOG="${REPO_ROOT}/logs/${SUITE_NAME}/${BENCHMARK_NAME}/${LOG_ID}.result.json"
TERMINAL_LOG="${REPO_ROOT}/logs/${SUITE_NAME}/${BENCHMARK_NAME}/${LOG_ID}.log"
if [ "$MODE" = learn-arta ]; then
    # Build LearnARTA before running
    cd "${REPO_ROOT}" || exit
    cargo build --release -p learn-arta-cli --bin learn-arta-cli
    cd - >/dev/null || exit
    # Run LearnARTA using the HiGHS-backed approximate MILP basis minimizer,
    # matching the benchmark configuration used for comparison runs.
    ${TIMER_COMMAND} -v -o "${GTIME_LOG}" \
                     "${REPO_ROOT}/target/release/learn-arta-cli" learn "${JSON_PATH}" --basis-minimization approx-milp --output "${RESULT_LOG}" 2>&1 |
        tee "${TERMINAL_LOG}"
elif [ "$MODE" = nlstar-rta ]; then
    # Run NLStarRTA
    ${TIMER_COMMAND} -v -o "${GTIME_LOG}" \
                     python3 "${REPO_ROOT}/baselines/NLStarRTA/learn.py" "${JSON_PATH}" 2>&1 |
        tee "${TERMINAL_LOG}"
else
    echo "Error: Invalid mode. Use 'learn-arta' or 'nlstar-rta'."
    exit 1
fi
