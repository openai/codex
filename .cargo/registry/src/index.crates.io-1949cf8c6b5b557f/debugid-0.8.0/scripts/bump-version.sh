#!/bin/bash
set -euo pipefail

if [ "$(uname -s)" != "Linux" ]; then
    echo "Please use the GitHub Action."
    exit 1
fi

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $SCRIPT_DIR/..

OLD_VERSION="${1}"
NEW_VERSION="${2}"

echo "Bumping version: ${NEW_VERSION}"

find . -name Cargo.toml -type f -exec sed -i '' -e "s/^version.*/version = \"$NEW_VERSION\"/" {} \;
