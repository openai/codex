FROM node:24-slim

ARG TZ
ENV TZ="$TZ"
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0

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
COPY container-install/package.json \
  container-install/pnpm-lock.yaml \
  container-install/pnpm-workspace.yaml \
  /opt/codex-install/
COPY dist/codex.tgz /opt/dist/codex.tgz
RUN cd /opt/codex-install \
  && corepack pnpm install --prod --frozen-lockfile \
  && ln -s /opt/codex-install/node_modules/@openai/codex/bin/codex.js /usr/local/bin/codex \
  && rm -rf /opt/dist \
  && rm -rf /opt/codex-install/node_modules/.cache \
  && rm -rf /opt/codex-install/node_modules/@openai/codex/tests \
  && rm -rf /opt/codex-install/node_modules/@openai/codex/docs

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
