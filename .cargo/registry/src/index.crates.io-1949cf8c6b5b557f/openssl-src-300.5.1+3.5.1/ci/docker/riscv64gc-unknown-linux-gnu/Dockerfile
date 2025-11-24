FROM ubuntu:24.04

RUN apt-get update -y && apt-get install -y --no-install-recommends \
  ca-certificates \
  make \
  perl \
  gcc \
  libc6-dev \
  gcc-riscv64-linux-gnu \
  libc6-dev-riscv64-cross
ENV CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_RUNNER=echo \
  CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_LINKER=riscv64-linux-gnu-gcc
