#!/bin/sh
#
abidw --out-file ./.github/workflows/libcrypto-abi.xml libcrypto.so
abidw --out-file ./.github/workflows/libssl-abi.xml libssl.so

