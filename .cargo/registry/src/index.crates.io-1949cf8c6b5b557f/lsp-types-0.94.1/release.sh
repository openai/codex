#!/bin/sh
set -ex

LEVEL=$1
if [ -z "$LEVEL" ]; then
    echo "Expected patch, minor or major"
    exit 1
fi

clog --$LEVEL

git add CHANGELOG.md
git commit -m "Update changelog"

cargo release $LEVEL --execute
