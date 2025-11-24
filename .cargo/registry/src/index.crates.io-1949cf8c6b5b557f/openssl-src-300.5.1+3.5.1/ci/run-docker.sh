#!/bin/sh

target=$1

if [ ! -d ci/docker/$1 ]; then
  exec ci/run.sh $1
fi

set -ex

docker build \
  --rm \
  --tag openssl-src-ci \
  ci/docker/$1

docker run \
  --rm \
  --volume `rustc --print sysroot`:/rust:ro \
  --volume `pwd`:/usr/code:ro \
  --volume `pwd`/target:/usr/code/target \
  --volume $HOME/.cargo:/cargo \
  --env CARGO_HOME=/cargo \
  --workdir /usr/code \
  openssl-src-ci \
  bash -c "PATH=\"\$PATH:/rust/bin:/cargo/bin\" ci/run.sh $target"

