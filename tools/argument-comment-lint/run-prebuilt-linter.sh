#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest_path="$repo_root/codex-rs/Cargo.toml"
dotslash_manifest="$repo_root/tools/argument-comment-lint/argument-comment-lint"

has_manifest_path=false
has_package_selection=false
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
    # cargo-dylint still expects the rustup shims to win over direct toolchain
    # cargo binaries on PATH.
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
fi

exec "$dotslash_manifest" "${lint_args[@]}"
