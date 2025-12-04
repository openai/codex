#!/bin/bash -eu

tree_sitter="$ROOT"/target/"$TARGET"/release/tree-sitter

if [[ $BUILD_CMD == cross ]]; then
  cross.sh make CC="$CC" AR="$AR" "$@"
else
  exec make "$@"
fi
