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

# Ubuntu 24.04 ships with user 'ubuntu' already created with UID 1000.
USER ubuntu

# install Rust + musl target as dev user
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && \
    ~/.cargo/bin/rustup target add aarch64-unknown-linux-musl && \
    ~/.cargo/bin/rustup component add clippy rustfmt

ENV PATH="/home/ubuntu/.cargo/bin:${PATH}"

WORKDIR /workspace
