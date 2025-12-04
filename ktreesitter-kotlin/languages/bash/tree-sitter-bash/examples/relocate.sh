#!/bin/bash

# Change the cli shebang to point at the specified node
# Useful for when the program is moved around after install.
# Also used by the default 'make install' in node to point
# npm at the newly installed node, rather than the first one
# in the PATH, which would be the default otherwise.

# bash /path/to/npm/scripts/relocate.sh $nodepath
# If $nodepath is blank, then it'll use /usr/bin/env

dir="$(dirname "$(dirname "$0")")"
cli="$dir"/bin/npm-cli.js
tmp="$cli".tmp

node="$1"
if [ "x$node" = "x" ]; then
  node="/usr/bin/env node"
fi
node="#!$node"

sed -e 1d "$cli" > "$tmp"
echo "$node" > "$cli"
cat "$tmp" >> "$cli"
rm "$tmp"
chmod ogu+x $cli
