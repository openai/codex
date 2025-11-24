#! /usr/bin/env bash

cargo readme > ./README.md
git add ./README.md
git commit -m "Update readme" || true
