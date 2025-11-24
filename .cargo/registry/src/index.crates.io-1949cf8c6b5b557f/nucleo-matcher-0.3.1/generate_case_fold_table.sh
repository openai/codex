#!/usr/bin/env bash
set -e

dir=$(pwd)
mkdir /tmp/ucd-15.0.0
cd /tmp/ucd-15.0.0
curl -LO https://www.unicode.org/Public/zipped/15.0.0/UCD.zip
unzip UCD.zip

cd "${dir}"
cargo install ucd-generate
ucd-generate case-folding-simple /tmp/ucd-15.0.0 --chars > src/chars/case_fold.rs
rm -rf /tmp/ucd-15.0.0
