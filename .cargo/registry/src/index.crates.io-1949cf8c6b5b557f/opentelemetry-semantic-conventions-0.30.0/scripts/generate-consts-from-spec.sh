#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="${SCRIPT_DIR}/../"

# freeze the spec version and generator version to make generation reproducible
SPEC_VERSION=1.33.0
WEAVER_VERSION=v0.15.0

cd "$CRATE_DIR"

rm -rf semantic-conventions || true
mkdir semantic-conventions
cd semantic-conventions

git init
git remote add origin https://github.com/open-telemetry/semantic-conventions.git
git fetch origin "v$SPEC_VERSION"
git reset --hard FETCH_HEAD
cd "$CRATE_DIR"

SED=(sed -i)
if [[ "$(uname)" = "Darwin" ]]; then
  SED=(sed -i "")
fi

# Keep `SCHEMA_URL` key in sync with spec version
"${SED[@]}" "s/\(opentelemetry.io\/schemas\/\)[^\"]*\"/\1$SPEC_VERSION\"/" scripts/templates/registry/rust/weaver.yaml

docker run --rm \
  --mount type=bind,source=$CRATE_DIR/semantic-conventions/model,target=/home/weaver/source,readonly \
  --mount type=bind,source=$CRATE_DIR/scripts/templates,target=/home/weaver/templates,readonly \
  --mount type=bind,source=$CRATE_DIR/src,target=/home/weaver/target \
  otel/weaver:$WEAVER_VERSION \
  registry generate \
  --registry=/home/weaver/source \
  --templates=/home/weaver/templates \
  rust \
  /home/weaver/target/

# handle doc generation failures
"${SED[@]}" 's/\[2\]\.$//' src/attribute.rs # remove trailing [2] from few of the doc comments

# handle escaping ranges like [0,n] / [0.0, ...] in descriptions/notes which will cause broken intra-doc links
# unescape any mistakenly escaped ranges which actually contained a link (not that we currently have any)
expression='
  s/\[([a-zA-Z0-9\.\s]+,[a-zA-Z0-9\.\s]+)\]/\\[\1\\]/g
  s/\\\[([^\]]+)\]\(([^)]+)\)/[\1](\2)/g
'

"${SED[@]}" -E "${expression}" src/metric.rs
"${SED[@]}" -E "${expression}" src/attribute.rs

# Fix unclosed HTML tag warnings for <key> in doc comments.
# Rustdoc treats <key> as an unclosed HTML tag and fails the build with -D warnings.
# We replace <key> with Markdown code formatting `key` to prevent the error.
# TODO: This workaround should be removed once the upstream generator handles this correctly.
"${SED[@]}" 's/<key>/`key`/g' src/attribute.rs

cargo fmt
