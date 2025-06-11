FROM ubuntu:22.04

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
    pkg-config clang musl-tools libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# non-root dev user
ARG USER=dev
ARG UID=1000
RUN useradd -m -u $UID $USER
USER $USER

# install Rust + musl target as dev user
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && \
    ~/.cargo/bin/rustup target add aarch64-unknown-linux-musl

ENV PATH="/home/${USER}/.cargo/bin:${PATH}"

WORKDIR /workspace
