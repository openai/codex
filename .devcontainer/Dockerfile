FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

ARG TZ
ARG DEBIAN_FRONTEND=noninteractive
ARG NODE_MAJOR=22
ARG RUST_TOOLCHAIN=1.92.0
ARG CODEX_NPM_VERSION=latest

ENV TZ="$TZ"

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        curl \
        git \
        ca-certificates \
        pkg-config \
        clang \
        musl-tools \
        libssl-dev \
        libsqlite3-dev \
        just \
        python3 \
        python3-pip \
        jq \
        less \
        man-db \
        unzip \
        ripgrep \
        fzf \
        fd-find \
        zsh \
        dnsutils \
        iproute2 \
        ipset \
        iptables \
        aggregate \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" | bash - \
    && apt-get update \
    && apt-get install -y --no-install-recommends nodejs \
    && npm install -g corepack@latest "@openai/codex@${CODEX_NPM_VERSION}" \
    && corepack enable \
    && corepack prepare pnpm@10.28.2 --activate \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

COPY .devcontainer/init-firewall.sh /usr/local/bin/init-firewall.sh
COPY .devcontainer/post_install.py /opt/post_install.py
COPY .devcontainer/post-start.sh /opt/post_start.sh

RUN chmod 500 /usr/local/bin/init-firewall.sh \
    && chmod 755 /opt/post_start.sh \
    && chmod 644 /opt/post_install.py \
    && chown vscode:vscode /opt/post_install.py

RUN install -d -m 0775 -o vscode -g vscode /commandhistory /workspace \
    && touch /commandhistory/.bash_history /commandhistory/.zsh_history \
    && chown vscode:vscode /commandhistory/.bash_history /commandhistory/.zsh_history

USER vscode
ENV PATH="/home/vscode/.cargo/bin:${PATH}"
WORKDIR /workspace

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}" \
    && rustup component add clippy rustfmt rust-src \
    && rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
