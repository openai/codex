#!/usr/bin/env bash

cargo +nightly fuzz "${1}" fuzz_target_1 "${@:2:99}"
