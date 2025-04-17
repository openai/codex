FROM node:20

ARG TZ
ENV TZ="$TZ"

# Install basic development tools and iptables/ipset
RUN apt update && apt install -y \
  aggregate \
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
  sudo \
  unzip \
  ripgrep \
  zsh

# Ensure default node user has access to /usr/local/share
RUN mkdir -p /usr/local/share/npm-global && \
  chown -R node:node /usr/local/share

ARG USERNAME=node

# Set up non-root user
USER node

# Install pnpm
RUN npm install -g pnpm

# Install global packages
ENV PNPM_HOME=/usr/local/share/pnpm-global
ENV PATH=$PATH:$PNPM_HOME

# Install codex
COPY dist/codex.tgz codex.tgz
RUN pnpm add -g codex.tgz

# Copy and set up firewall script
COPY scripts/init_firewall.sh /usr/local/bin/
USER root
RUN chmod +x /usr/local/bin/init_firewall.sh && \
  echo "node ALL=(root) NOPASSWD: /usr/local/bin/init_firewall.sh" > /etc/sudoers.d/node-firewall && \
  chmod 0440 /etc/sudoers.d/node-firewall
USER node
