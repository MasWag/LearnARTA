#!/usr/bin/env bash
###====###
# Name
#  bootstrap-symbolicautomata.sh
#
# Description
#  Initialize the symbolicautomata submodule and build the models module
#  so that tools/drta-size can depend on it.
#
# Synopsis
#  ./scripts/bootstrap-symbolicautomata.sh
###====###

set -eu

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)

# Initialize and update the symbolicautomata submodule
git -C "$REPO_ROOT" submodule init vendor/symbolicautomata
git -C "$REPO_ROOT" submodule update vendor/symbolicautomata

# Build the models module and install it to the local Maven repository
MVN="mvn"
if ! command -v mvn >/dev/null 2>&1; then
    echo "error: Maven not found. Install mvn or ensure it is in PATH." >&2
    exit 1
fi

echo "Building symbolicautomata models module ..."
$MVN -f "${REPO_ROOT}/vendor/symbolicautomata/models/pom.xml" clean install

echo "bootstrap-symbolicautomata.sh: done."
