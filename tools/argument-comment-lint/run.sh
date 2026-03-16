#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
lint_path="$repo_root/tools/argument-comment-lint"
manifest_path="$repo_root/codex-rs/Cargo.toml"
strict_lint="uncommented_anonymous_literal_argument"
noise_lint="unknown_lints"

has_manifest_path=false
has_package_selection=false
has_no_deps=false
has_library_selection=false
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
        --workspace)
            has_package_selection=true
            ;;
        --no-deps)
            has_no_deps=true
            ;;
        --lib|--lib-path)
            expect_value="library_selection"
            ;;
        --lib=*|--lib-path=*)
            has_library_selection=true
            ;;
    esac
done

cmd=(cargo dylint --path "$lint_path")
if [[ "$has_library_selection" == false ]]; then
    cmd+=(--all)
fi
if [[ "$has_manifest_path" == false ]]; then
    cmd+=(--manifest-path "$manifest_path")
fi
if [[ "$has_package_selection" == false ]]; then
    cmd+=(--workspace)
fi
if [[ "$has_no_deps" == false ]]; then
    cmd+=(--no-deps)
fi
cmd+=("$@")

if [[ "${DYLINT_RUSTFLAGS:-}" != *"$strict_lint"* ]]; then
    export DYLINT_RUSTFLAGS="${DYLINT_RUSTFLAGS:+${DYLINT_RUSTFLAGS} }-D $strict_lint"
fi
if [[ "${DYLINT_RUSTFLAGS:-}" != *"$noise_lint"* ]]; then
    export DYLINT_RUSTFLAGS="${DYLINT_RUSTFLAGS:+${DYLINT_RUSTFLAGS} }-A $noise_lint"
fi

if [[ -z "${CARGO_INCREMENTAL:-}" ]]; then
    export CARGO_INCREMENTAL=0
fi

exec "${cmd[@]}"
