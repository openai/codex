FROM node:24-slim

ARG TZ
ENV TZ="$TZ"

# Install basic development tools, ca-certificates, and iptables/ipset, then clean up apt cache to reduce image size
RUN apt-get update && apt-get install -y --no-install-recommends \
  aggregate \
  ca-certificates \
  curl \
  dnsutils \
  fzf \
  gh \
  git \
  gnupg2 \
  iproute2 \
  ipset \
  iptables \
  jq \
  less \
  man-db \
  procps \
  unzip \
  ripgrep \
  zsh \
  && rm -rf /var/lib/apt/lists/*

ARG USERNAME=node

# Install codex
COPY dist/codex.tgz codex.tgz
RUN mkdir -p /usr/local/lib/codex-cli \
  && tar -xzf codex.tgz -C /usr/local/lib/codex-cli --strip-components=1 \
  && ln -s /usr/local/lib/codex-cli/bin/codex.js /usr/local/bin/codex \
  && rm codex.tgz \
  && rm -rf /usr/local/lib/codex-cli/node_modules/.cache \
  && rm -rf /usr/local/lib/codex-cli/tests \
  && rm -rf /usr/local/lib/codex-cli/docs

# Set up non-root user
USER node

# Inside the container we consider the environment already sufficiently locked
# down, therefore instruct Codex CLI to allow running without sandboxing.
ENV CODEX_UNSAFE_ALLOW_NO_SANDBOX=1

# Copy and set up firewall script as root.
USER root
COPY scripts/init_firewall.sh /usr/local/bin/
RUN chmod 500 /usr/local/bin/init_firewall.sh

# Drop back to non-root.
USER node
