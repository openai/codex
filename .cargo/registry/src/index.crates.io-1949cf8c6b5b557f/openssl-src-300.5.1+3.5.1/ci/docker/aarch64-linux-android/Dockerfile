FROM ubuntu:24.04

RUN apt-get update -y && apt-get install -y --no-install-recommends \
  ca-certificates \
  curl \
  make \
  perl \
  python \
  unzip \
  gcc \
  libc6-dev

RUN curl -O https://dl.google.com/android/repository/android-ndk-r15b-linux-x86_64.zip
RUN unzip -q android-ndk-r15b-linux-x86_64.zip
RUN android-ndk-r15b/build/tools/make_standalone_toolchain.py \
              --unified-headers \
              --install-dir /android/ndk \
              --arch arm64 \
              --api 24

ENV PATH=$PATH:/android/ndk/bin \
  CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android-gcc \
  CARGO_TARGET_AARCH64_LINUX_ANDROID_RUNNER=echo
