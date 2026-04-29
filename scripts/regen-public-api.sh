#!/usr/bin/env bash
set -euo pipefail

public_api_crates=(
  core-api
)
default_public_api_toolchain="nightly-2025-09-18"

check=false
if [[ "${1:-}" == "--check" ]]; then
  check=true
  shift
fi

if [[ "$#" -gt 0 ]]; then
  requested_crates=("$@")
else
  requested_crates=("${public_api_crates[@]}")
fi

for requested_crate in "${requested_crates[@]}"; do
  found=false
  for public_api_crate in "${public_api_crates[@]}"; do
    if [[ "${requested_crate}" == "${public_api_crate}" ]]; then
      found=true
      break
    fi
  done

  if [[ "${found}" != true ]]; then
    echo "unsupported public API crate: ${requested_crate}" >&2
    echo "supported crates: ${public_api_crates[*]}" >&2
    exit 1
  fi
done

required_cargo_public_api_version="0.51.0"
installed_cargo_public_api_version=""
if command -v cargo-public-api >/dev/null 2>&1; then
  installed_cargo_public_api_version="$(cargo-public-api --version | awk '{print $2}')"
fi

if [[ "${installed_cargo_public_api_version}" != "${required_cargo_public_api_version}" ]]; then
  cargo install cargo-public-api \
    --version "${required_cargo_public_api_version}" \
    --locked
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
codex_rs_dir="${repo_root}/codex-rs"
toolchain="${CODEX_PUBLIC_API_TOOLCHAIN:-${default_public_api_toolchain}}"
generated_paths=()

for crate in "${requested_crates[@]}"; do
  output_path="${codex_rs_dir}/${crate}/public-api.txt"
  generated_paths+=("codex-rs/${crate}/public-api.txt")

  (
    cd "${codex_rs_dir}"
    cargo "+${toolchain}" public-api \
      --color never \
      --manifest-path "${crate}/Cargo.toml" \
      --omit blanket-impls,auto-trait-impls,auto-derived-impls \
      >"${output_path}"
  )

  echo "wrote ${output_path}"
done

if [[ "${check}" == true ]]; then
  git -C "${repo_root}" diff --exit-code -- "${generated_paths[@]}"
fi
