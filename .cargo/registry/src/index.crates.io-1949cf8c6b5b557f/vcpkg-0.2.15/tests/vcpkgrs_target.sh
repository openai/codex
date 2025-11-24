#!/bin/bash
set -ex

SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $SCRIPTDIR

export VCPKG_ROOT=$SCRIPTDIR/../vcp
export VCPKGRS_TRIPLET=test-triplet
export VCPKG_DEFAULT_TRIPLET=test-triplet

cp $VCPKG_ROOT/triplets/x64-linux.cmake $VCPKG_ROOT/triplets/test-triplet.cmake
for port in harfbuzz ; do
    # check that the port fails before it is installed
    $VCPKG_ROOT/vcpkg remove --no-binarycaching $port  || true
    cargo clean --manifest-path $port/Cargo.toml
    cargo run --manifest-path $port/Cargo.toml && exit 2
    echo THIS FAILURE IS EXPECTED
    echo This is to ensure that we are not spuriously succeeding because the libraries already exist somewhere on the build machine.
    # disable binary caching because it breaks this build as of vcpkg 53e6588 (since vcpkg 52a9d9a)
    $VCPKG_ROOT/vcpkg install --no-binarycaching $port
    cargo run --manifest-path $port/Cargo.toml
done
