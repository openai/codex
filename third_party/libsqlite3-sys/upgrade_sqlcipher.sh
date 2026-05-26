#!/bin/sh -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
echo "$SCRIPT_DIR"
cd "$SCRIPT_DIR" || { echo "fatal error" >&2; exit 1; }
cargo clean -p libsqlite3-sys
mkdir -p "$SCRIPT_DIR/../target" "$SCRIPT_DIR/sqlcipher"
export SQLCIPHER_LIB_DIR="$SCRIPT_DIR/sqlcipher"
export SQLCIPHER_INCLUDE_DIR="$SQLCIPHER_LIB_DIR"

SQLCIPHER_VERSION="4.5.7"
# Download and generate sqlcipher amalgamation
mkdir -p $SCRIPT_DIR/sqlcipher.src
[ -e "v${SQLCIPHER_VERSION}.tar.gz" ] || curl -sfL -O "https://github.com/sqlcipher/sqlcipher/archive/v${SQLCIPHER_VERSION}.tar.gz"
tar xzf "v${SQLCIPHER_VERSION}.tar.gz" --strip-components=1 -C "$SCRIPT_DIR/sqlcipher.src"
cd "$SCRIPT_DIR/sqlcipher.src"
./configure --with-crypto-lib=none
make sqlite3.c
cp sqlite3.c sqlite3.h sqlite3ext.h "$SCRIPT_DIR/sqlcipher/"
cd "$SCRIPT_DIR"
rm -rf "v${SQLCIPHER_VERSION}.tar.gz" sqlcipher.src

# Regenerate bindgen file for sqlcipher
rm -f "$SQLCIPHER_LIB_DIR/bindgen_bundled_version.rs"

# cargo update
find "$SCRIPT_DIR/../target" -type f -name bindgen.rs -exec rm {} \;
env LIBSQLITE3_SYS_BUNDLING=1 cargo build --features "sqlcipher buildtime_bindgen session"
find "$SCRIPT_DIR/../target" -type f -name bindgen.rs -exec mv {} "$SQLCIPHER_LIB_DIR/bindgen_bundled_version.rs" \;

# Sanity checks
cd "$SCRIPT_DIR/.." || { echo "fatal error" >&2; exit 1; }
cargo update --quiet
cargo test --features "backup blob chrono functions limits load_extension serde_json trace vtab bundled-sqlcipher-vendored-openssl"
printf '    \e[35;1mFinished\e[0m bundled-sqlcipher-vendored-openssl/sqlcipher tests\n'
