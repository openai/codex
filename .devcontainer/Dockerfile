FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive
# enable 'universe' because musl-tools & clang live there
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    software-properties-common && \
    add-apt-repository --yes universe

# now install build deps
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential curl git ca-certificates \
    pkg-config clang musl-tools libssl-dev just && \
    rm -rf /var/lib/apt/lists/*

# Install Bazel via Bazelisk (mirrors bazelbuild/setup-bazelisk@v3).
ARG BAZELISK_VERSION=latest
RUN arch="$(uname -m)" && \
    case "$arch" in \
      x86_64) arch="amd64" ;; \
      aarch64) arch="arm64" ;; \
      *) echo "Unsupported architecture: $arch" >&2; exit 1 ;; \
    esac && \
    if [ "$BAZELISK_VERSION" = "latest" ]; then \
      url="https://github.com/bazelbuild/bazelisk/releases/latest/download/bazelisk-linux-${arch}"; \
    else \
      url="https://github.com/bazelbuild/bazelisk/releases/download/v${BAZELISK_VERSION}/bazelisk-linux-${arch}"; \
    fi && \
    curl -fsSL "$url" -o /usr/local/bin/bazelisk && \
    chmod +x /usr/local/bin/bazelisk && \
    ln -s /usr/local/bin/bazelisk /usr/local/bin/bazel

# Install dotslash.
RUN curl -LSfs "https://github.com/facebook/dotslash/releases/download/v0.5.8/dotslash-ubuntu-22.04.$(uname -m).tar.gz" | tar fxz - -C /usr/local/bin

# Ubuntu 24.04 ships with user 'ubuntu' already created with UID 1000.
USER ubuntu

# install Rust + musl target as dev user
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && \
    ~/.cargo/bin/rustup target add aarch64-unknown-linux-musl && \
    ~/.cargo/bin/rustup component add clippy rustfmt

ENV PATH="/home/ubuntu/.cargo/bin:${PATH}"

WORKDIR /workspace
