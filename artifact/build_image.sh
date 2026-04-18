#!/bin/sh
################################################################
# Name
#  build_image.sh
#
# Description
#  Build Docker image archives for linux/amd64 and linux/arm64 from
#  artifact/Dockerfile and store them under artifact/.
#
# Prerequisites
#  * Docker
#  * Docker Buildx
#
# Synopsis
#  ./build_image.sh
#
# Notes
#  * The generated archives are learnarta-amd64.tar and
#    learnarta-arm64.tar.
#  * The exported images are tagged as maswag/learnarta:amd64 and
#    maswag/learnarta:arm64.
#  * The script also loads these images into the local Docker image store
#    so they are ready for docker push.
#
# Author
#  Masaki Waga
#
# License
#  Apache 2.0 License
################################################################

set -eu

readonly SCRIPT_DIR="$(CDPATH='' cd "$(dirname "$0")" && pwd)"
readonly REPO_ROOT="$(CDPATH='' cd "$SCRIPT_DIR/.." && pwd)"
readonly DOCKERFILE_PATH="$SCRIPT_DIR/Dockerfile"

if ! command -v docker >/dev/null 2>&1; then
    echo "error: Docker is required. Install Docker." >&2
    exit 1
fi

if ! docker buildx version >/dev/null 2>&1; then
    echo "error: Docker Buildx is required. Enable Docker Buildx." >&2
    exit 1
fi

# Build the linux/amd64 Docker archive.
docker buildx build \
    --platform linux/amd64 \
    --tag maswag/learnarta:amd64 \
    --file "$DOCKERFILE_PATH" \
    --output "type=docker,dest=$SCRIPT_DIR/learnarta-amd64.tar" \
    "$REPO_ROOT"

# Load the linux/amd64 image so it is ready for docker push.
docker load --input "$SCRIPT_DIR/learnarta-amd64.tar"

# Build the linux/arm64 Docker archive.
docker buildx build \
    --platform linux/arm64 \
    --tag maswag/learnarta:arm64 \
    --file "$DOCKERFILE_PATH" \
    --output "type=docker,dest=$SCRIPT_DIR/learnarta-arm64.tar" \
    "$REPO_ROOT"

# Load the linux/arm64 image so it is ready for docker push.
docker load --input "$SCRIPT_DIR/learnarta-arm64.tar"
