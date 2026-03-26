#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest_path="$repo_root/codex-rs/Cargo.toml"
dotslash_manifest="$repo_root/tools/argument-comment-lint/argument-comment-lint"

has_manifest_path=false
has_package_selection=false
has_library_selection=false
has_no_deps=false
expect_value=""

for arg in "$@"; do
    if [[ -n "$expect_value" ]]; then
        case "$expect_value" in
            manifest_path)
                has_manifest_path=true
                ;;
            package_selection)
                has_package_selection=true
                ;;
            library_selection)
                has_library_selection=true
                ;;
        esac
        expect_value=""
        continue
    fi

    case "$arg" in
        --)
            break
            ;;
        --manifest-path)
            expect_value="manifest_path"
            ;;
        --manifest-path=*)
            has_manifest_path=true
            ;;
        -p|--package)
            expect_value="package_selection"
            ;;
        --package=*)
            has_package_selection=true
            ;;
        --lib|--lib-path)
            expect_value="library_selection"
            ;;
        --lib=*|--lib-path=*)
            has_library_selection=true
            ;;
        --workspace)
            has_package_selection=true
            ;;
        --no-deps)
            has_no_deps=true
            ;;
    esac
done

lint_args=()
if [[ "$has_manifest_path" == false ]]; then
    lint_args+=(--manifest-path "$manifest_path")
fi
if [[ "$has_package_selection" == false ]]; then
    lint_args+=(--workspace)
fi
if [[ "$has_no_deps" == false ]]; then
    lint_args+=(--no-deps)
fi
lint_args+=("$@")

if ! command -v dotslash >/dev/null 2>&1; then
    cat >&2 <<EOF
argument-comment-lint prebuilt wrapper requires dotslash.
Install dotslash, or use:
  ./tools/argument-comment-lint/run.sh ...
EOF
    exit 1
fi

if command -v rustup >/dev/null 2>&1; then
    rustup_bin_dir="$(dirname "$(command -v rustup)")"
    path_entries=()
    while IFS= read -r entry; do
        [[ -n "$entry" && "$entry" != "$rustup_bin_dir" ]] && path_entries+=("$entry")
    done < <(printf '%s\n' "${PATH//:/$'\n'}")
    PATH="$rustup_bin_dir"
    if ((${#path_entries[@]} > 0)); then
        PATH+=":$(IFS=:; echo "${path_entries[*]}")"
    fi
    export PATH

    if [[ -z "${RUSTUP_HOME:-}" ]]; then
        rustup_home="$(rustup show home 2>/dev/null || true)"
        if [[ -n "$rustup_home" ]]; then
            export RUSTUP_HOME="$rustup_home"
        fi
    fi
fi

package_entrypoint="$(dotslash -- fetch "$dotslash_manifest")"
bin_dir="$(cd "$(dirname "$package_entrypoint")" && pwd)"
package_root="$(cd "$bin_dir/.." && pwd)"
library_dir="$package_root/lib"

cargo_dylint="$bin_dir/cargo-dylint"
if [[ ! -x "$cargo_dylint" ]]; then
    cargo_dylint="$bin_dir/cargo-dylint.exe"
fi
if [[ ! -x "$cargo_dylint" ]]; then
    echo "bundled cargo-dylint executable not found under $bin_dir" >&2
    exit 1
fi

shopt -s nullglob
libraries=("$library_dir"/*@*)
shopt -u nullglob
if [[ ${#libraries[@]} -eq 0 ]]; then
    echo "no packaged Dylint library found in $library_dir" >&2
    exit 1
fi
if [[ ${#libraries[@]} -ne 1 ]]; then
    echo "expected exactly one packaged Dylint library in $library_dir" >&2
    exit 1
fi

library_path="${libraries[0]}"
library_filename="$(basename "$library_path")"
normalized_library_path="$library_path"
library_ext=".${library_filename##*.}"
library_stem="${library_filename%.*}"
if [[ "$library_stem" =~ ^(.+@nightly-[0-9]{4}-[0-9]{2}-[0-9]{2})-.+$ ]]; then
    normalized_library_filename="${BASH_REMATCH[1]}$library_ext"
    temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/argument-comment-lint.XXXXXX")"
    normalized_library_path="$temp_dir/$normalized_library_filename"
    cp "$library_path" "$normalized_library_path"
fi

if [[ -n "${DYLINT_RUSTFLAGS:-}" ]]; then
    if [[ "$DYLINT_RUSTFLAGS" != *"-D uncommented-anonymous-literal-argument"* ]]; then
        DYLINT_RUSTFLAGS+=" -D uncommented-anonymous-literal-argument"
    fi
    if [[ "$DYLINT_RUSTFLAGS" != *"-A unknown_lints"* ]]; then
        DYLINT_RUSTFLAGS+=" -A unknown_lints"
    fi
else
    DYLINT_RUSTFLAGS="-D uncommented-anonymous-literal-argument -A unknown_lints"
fi
export DYLINT_RUSTFLAGS

if [[ -z "${CARGO_INCREMENTAL:-}" ]]; then
    export CARGO_INCREMENTAL=0
fi

command=("$cargo_dylint" dylint --lib-path "$normalized_library_path")
if [[ "$has_library_selection" == false ]]; then
    command+=(--all)
fi
command+=("${lint_args[@]}")

exec "${command[@]}"
