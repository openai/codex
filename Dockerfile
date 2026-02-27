FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive

# Required native deps for codex-rs crates and sandbox builds.
RUN apt-get update && \
    apt-get install -y --no-install-recommends software-properties-common && \
    add-apt-repository --yes universe && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
    bash \
    build-essential \
    ca-certificates \
    clang \
    curl \
    git \
    libcap-dev \
    libssl-dev \
    make \
    musl-tools \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

USER ubuntu

ENV CARGO_HOME=/home/ubuntu/.cargo
ENV RUSTUP_HOME=/home/ubuntu/.rustup
ENV PATH=/home/ubuntu/.cargo/bin:${PATH}

# Match codex-rs rust-toolchain.toml up front so runtime commands don't re-download it.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none && \
    rustup toolchain install 1.93.0 --profile minimal --component clippy --component rustfmt --component rust-src && \
    rustup default 1.93.0 && \
    cargo install just && \
    cargo install --locked cargo-nextest && \
    cargo install cargo-insta

WORKDIR /workspace

# Typical usage:
#   docker build -t codex-dev .
#   docker run --rm -it -v "$PWD":/workspace/codex -w /workspace/codex/codex-rs codex-dev bash
CMD ["/bin/bash"]
