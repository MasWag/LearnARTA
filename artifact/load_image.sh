#!/bin/sh
################################################################
# Name
#  load_image.sh
#
# Description
#  Load the Docker image archive matching the current machine
#  architecture and tag it as maswag/learnarta:latest.
#
# Prerequisites
#  * Docker
#
# Synopsis
#  ./load_image.sh
#
# Notes
#  * x86_64 and amd64 hosts load learnarta-amd64.tar.
#  * aarch64 and arm64 hosts load learnarta-arm64.tar.
#
# Author
#  Masaki Waga
#
# License
#  Apache 2.0 License
################################################################

set -eu

readonly SCRIPT_DIR="$(CDPATH='' cd "$(dirname "$0")" && pwd)"

if ! command -v docker >/dev/null 2>&1; then
    echo "error: Docker is required. Install Docker." >&2
    exit 1
fi

case "$(uname -m)" in
    x86_64 | amd64)
        readonly IMAGE_ARCH=amd64
        ;;
    aarch64 | arm64)
        readonly IMAGE_ARCH=arm64
        ;;
    *)
        echo "error: Unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

readonly IMAGE_ARCHIVE="$SCRIPT_DIR/learnarta-${IMAGE_ARCH}.tar"

if [ ! -f "$IMAGE_ARCHIVE" ]; then
    echo "error: Docker image archive not found: $IMAGE_ARCHIVE" >&2
    exit 1
fi

docker load --input "$IMAGE_ARCHIVE"
docker image tag "maswag/learnarta:${IMAGE_ARCH}" maswag/learnarta:latest
