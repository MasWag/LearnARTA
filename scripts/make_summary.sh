#!/bin/bash
################################################################
# Name
#  make_summary.sh
#
# Description
#  Convert log files in logs/ to a single JSON file
#  (logs/summary.json).
#
# Prerequisites
#  * Bash (for scripts/log_to_json.sh)
#  * jc (for scripts/log_to_json.sh)
#  * jo (for scripts/log_to_json.sh)
#  * jq (for scripts/log_to_json.sh)
#
# Synopsis
#  ./scripts/make_summary.sh
#
# Author
#  Masaki Waga
#
# License
#  Apache 2.0 License
################################################################

set -euo pipefail

cd "$(dirname "$0")/.."

# Convert logs to JSON and store in logs/summary.json
find logs -type f -name "*.log" -exec ./scripts/log_to_json.sh {} \; |
    jq -s . > logs/summary.json
