FROM ubuntu:24.04

RUN apt-get update -y && apt-get install -y --no-install-recommends \
  ca-certificates \
  make \
  perl \
  gcc \
  libc6-dev \
  gcc-arm-linux-gnueabi \
  libc6-dev-armel-cross

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABI_RUNNER=echo \
  CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABI_LINKER=arm-linux-gnueabi-gcc
