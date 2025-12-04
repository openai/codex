#!/bin/bash -eu

exec docker run --rm -v /home/runner:/home/runner -w "$PWD" "$CROSS_IMAGE" "$@"
