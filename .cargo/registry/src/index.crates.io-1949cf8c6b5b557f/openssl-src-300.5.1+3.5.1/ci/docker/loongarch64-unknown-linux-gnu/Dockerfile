FROM ubuntu:24.04

RUN apt-get update -y && apt-get install -y --no-install-recommends \
  ca-certificates \
  make \
  perl \
  gcc \
  libc6-dev \
  gcc-14-loongarch64-linux-gnu \
  libc6-dev-loong64-cross
ENV CC_loongarch64_unknown_linux_gnu=loongarch64-linux-gnu-gcc-14 \
  CARGO_TARGET_LOONGARCH64_UNKNOWN_LINUX_GNU_RUNNER=echo \
  CARGO_TARGET_LOONGARCH64_UNKNOWN_LINUX_GNU_LINKER=loongarch64-linux-gnu-gcc-14
