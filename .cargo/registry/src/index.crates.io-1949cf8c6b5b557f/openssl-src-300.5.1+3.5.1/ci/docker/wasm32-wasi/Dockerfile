FROM ubuntu:22.04

RUN apt-get update -y && apt-get install -y --no-install-recommends \
  ca-certificates \
  curl wget \
  make \
  perl \
  gcc \
  xz-utils \
  libc6-dev

RUN curl https://wasmtime.dev/install.sh -sSf | bash
ENV PATH="$PATH:/root/.wasmtime/bin"

# Install WASI-SDK
ENV WASI_VERSION=20
ENV WASI_VERSION_FULL=${WASI_VERSION}.0
RUN wget https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-${WASI_VERSION}/wasi-sdk-${WASI_VERSION_FULL}-linux.tar.gz
RUN tar xvf wasi-sdk-${WASI_VERSION_FULL}-linux.tar.gz -C /root

# WASI env setup
ENV WASI_SDK_PATH=/root/wasi-sdk-${WASI_VERSION_FULL}
ENV CC_wasm32-wasi="${WASI_SDK_PATH}/bin/clang --sysroot=${WASI_SDK_PATH}/share/wasi-sysroot"
ENV CARGO_TARGET_WASM32_WASI_LINKER="${WASI_SDK_PATH}/bin/clang"
ENV CARGO_TARGET_WASM32_WASI_RUNNER=wasmtime

ENV RUSTFLAGS=-Ctarget-feature=-crt-static

