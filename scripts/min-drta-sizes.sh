#!/usr/bin/env bash
##########
# Name
#  min-drta-sizes.sh
#
# Description
#  Build the drta-size utility and run it against example NRTA JSON files.
#  Outputs CSV rows to stdout.
#
# Synopsis
#  ./scripts/min-drta-sizes.sh [JSON_PATH...]
#
# Examples
#  # Process all examples (default)
#  ./scripts/min-drta-sizes.sh
#
#  # Process specific files
#  ./scripts/min-drta-sizes.sh examples/small.json examples/running.json
##########

set -eu

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)

# Ensure symbolicautomata is bootstrapped
if [ ! -d "${REPO_ROOT}/vendor/symbolicautomata/models/target" ]; then
    echo "symbolicautomata not yet built; running bootstrap-symbolicautomata.sh ..." >&2
    "${REPO_ROOT}/scripts/bootstrap-symbolicautomata.sh" >&2 || {
        echo "bootstrap failed.  Is Java 8+ and Maven 3.2+ installed?" >&2
        exit 1
    }
fi

# Build drta-size
echo "Building drta-size ..." >&2
MVN="mvn"
$MVN -f "${REPO_ROOT}/tools/drta-size/pom.xml" clean package -q >&2

# Determine the target jar
DRIVE_JAR=$(find "${REPO_ROOT}/tools/drta-size/target" -name 'drta-size-*-shade.jar' -o -name 'drta-size-*-jar-with-dependencies.jar' 2>/dev/null | head -1)

if [ -z "$DRIVE_JAR" ]; then
    # Maybe the name differs; try any shaded/with-dependencies jar
    DRIVE_JAR=$(find "${REPO_ROOT}/tools/drta-size/target" -name '*-shaded.jar' -o -name '*-jar-with-dependencies.jar' 2>/dev/null | head -1)
fi

if [ -z "$DRIVE_JAR" ]; then
    DRIVE_JAR=$(find "${REPO_ROOT}/tools/drta-size/target" -name 'drta-size-*' -type f 2>/dev/null | head -1)
fi

if [ -z "$DRIVE_JAR" ]; then
    echo "error: could not find built drta-size jar.  Did mvn succeed?" >&2
    exit 1
fi

# Pass CLI args through (default: examples)
if [ $# -ge 1 ]; then
    exec java -jar "$DRIVE_JAR" "$@"
else
    exec java -jar "$DRIVE_JAR" "examples"
fi
