#!/bin/bash
#
# This script can be sourced to ensure VCPKG_ROOT points at a bootstrapped vcpkg repository.
# It will also modify the environment (if sourced) to reflect any overrides in
# vcpkg triplet used neccesary to match the semantics of vcpkg-rs.

if [ "$VCPKG_ROOT" == "" ]; then
  echo "VCPKG_ROOT must be set."
  exit 1
fi

# Bootstrap ./vcp if it doesn't already exist.
if [ ! -d "$VCPKG_ROOT" ]; then
  echo "Bootstrapping ./vcp for systest"
  pushd ..
  git clone https://github.com/microsoft/vcpkg.git vcp
  cd vcp
  if [ "$OS" == "Windows_NT" ]; then
    ./bootstrap-vcpkg.bat
  else
    ./bootstrap-vcpkg.sh
  fi

  popd
fi

# Override triplet used if we are on Windows, as the default there is 32bit
# dynamic, whereas on 64 bit vcpkg-rs will prefer static with dynamic CRT
# linking.
if [ "$OS" == "Windows_NT" -a "$PROCESSOR_ARCHITECTURE" == "AMD64" ] ; then
  export VCPKG_DEFAULT_TRIPLET=x64-windows-static-md
fi
