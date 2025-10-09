# --------------------------
# Stage 1: build Perl with perlbrew (throwaway)
# --------------------------
FROM ubuntu:latest AS perl-builder

SHELL ["/bin/bash", "-c"]

ARG DEBIAN_FRONTEND=noninteractive
ARG PERL_VERSION=5.42.0
ENV PERLBREW_ROOT=/root/perl5/perlbrew

RUN apt-get update && apt-get install -y \
    curl ca-certificates bash build-essential \
 && rm -rf /var/lib/apt/lists/*

# Install perlbrew and compile Perl (skip manpages to reduce size)
RUN \curl -L https://install.perlbrew.pl | bash  && \
    source ${PERLBREW_ROOT}/etc/bashrc  && \
    (perlbrew install -n perl-${PERL_VERSION}  || cat /root/perl5/perlbrew/build.perl-${PERL_VERSION}.log)



# ==========================
# Stage 2: runtime only
# ==========================
FROM ubuntu:latest

SHELL ["/bin/bash", "-c"]

# Prevent interactive prompts during apt install
ARG DEBIAN_FRONTEND=noninteractive
ARG PERL_VERSION=5.42.0
ENV PERLBREW_ROOT=/root/perl5/perlbrew \
    PERLBREW_HOME=/root/.perlbrew

# Install basic deps
RUN apt-get update && apt-get install -y \
    curl git ca-certificates bash net-tools \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install nvm
ENV NVM_DIR=/root/.nvm
RUN mkdir -p $NVM_DIR
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash

# https://github.com/nvm-sh/nvm?tab=readme-ov-file#installing-in-docker
# Install latest Node.js via nvm and make it default
RUN source $NVM_DIR/nvm.sh && \
    nvm install node && \
    nvm alias default node && \
    nvm install-latest-npm

# Install Codex CLI globally
RUN source $NVM_DIR/nvm.sh && \
    npm install -g @openai/codex

# Install perl
RUN \curl -L https://install.perlbrew.pl | bash

RUN cat <<-EOF >> $HOME/.bashrc
    if [ -f ~/perl5/perlbrew/etc/bashrc ]; then
      source ~/perl5/perlbrew/etc/bashrc
      source ~/perl5/perlbrew/etc/perlbrew-completion.bash
      alias pb="perlbrew"
      complete -F _perlbrew_compgen pb
    fi
EOF

COPY --from=perl-builder \
    ${PERLBREW_ROOT}/perls/perl-${PERL_VERSION}/ ${PERLBREW_ROOT}/perls/perl-${PERL_VERSION}/

RUN source ${PERLBREW_ROOT}/etc/bashrc && \
    perlbrew switch perl-${PERL_VERSION} --switch && \
    perlbrew install-cpanm

RUN cat <<-EOF >> $HOME/.inputrc
    # https://unix.stackexchange.com/a/402398/129967
    "\e[A": history-search-backward
    "\e[B": history-search-forward

    # https://news.ycombinator.com/item?id=11213689
    # "\e[5~": history-substring-search-backward
    # "\e[6~": history-substring-search-forward
EOF

# Info
VOLUME /app
VOLUME /root/.codex

WORKDIR /app

# Default command
CMD ["/bin/bash"]
