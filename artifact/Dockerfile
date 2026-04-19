FROM rust:1.95.0-slim-trixie
LABEL maintainer="Masaki Waga <masakiwaga@gmail.com>"

# Install required build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        # Used for pulling the LearnARTA repository
        git \
        # Used for building LearnARTA
        build-essential cmake libclang-dev \
        # Used during experiments
        time \
        # Used for log conversion
        pipx jq jo && \
    rm -rf /var/lib/apt/lists/*

# Install jc with pipx for log conversion
ENV PATH="/root/.local/bin:${PATH}"
RUN pipx install jc

# Clone and build LearnARTA
RUN mkdir /LearnARTA && \
    cd /LearnARTA && \
    git init && \
    git remote add origin https://github.com/MasWag/LearnARTA.git && \
    git fetch --depth 1 origin 4f652c02e7bc63d455f0d5ca30fb2c3c1d8615b6 && \
    git checkout FETCH_HEAD && \
    git submodule update --init && \
    cargo build --release

# Remove git (only needed for cloning) to reduce image size
RUN apt-get purge -y --auto-remove git && \
    rm -rf /var/lib/apt/lists/*
