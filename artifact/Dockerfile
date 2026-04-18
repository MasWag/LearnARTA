FROM rust:slim
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
RUN git clone --depth 1 https://github.com/MasWag/LearnARTA.git /LearnARTA --recursive && \
    cd /LearnARTA && \
    cargo build --release

# Remove git (only needed for cloning) to reduce image size
RUN apt-get purge -y --auto-remove git && \
    rm -rf /var/lib/apt/lists/*
