#!/bin/bash -eu

tree_sitter="$ROOT"/target/"$TARGET"/release/tree-sitter

if [[ $BUILD_CMD == cross ]]; then
  cross.sh "$CROSS_RUNNER" "$tree_sitter" "$@"
else
  exec "$tree_sitter" "$@"
fi
