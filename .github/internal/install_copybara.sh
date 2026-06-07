#!/usr/bin/env bash

set -euo pipefail

readonly COPYBARA_VERSION="v20260601"
readonly COPYBARA_SHA256="207dc1699246d3117b84a0515089846c8515d4f5701bac2741963c302ba13d7d"
readonly COPYBARA_URL="https://github.com/google/copybara/releases/download/${COPYBARA_VERSION}/copybara_deploy.jar"

install_dir="${1:-${RUNNER_TEMP:-/tmp}/copybara}"
mkdir -p "${install_dir}"

copybara_jar="${install_dir}/copybara_deploy.jar"
curl -fsSL "${COPYBARA_URL}" -o "${copybara_jar}"

if command -v sha256sum >/dev/null 2>&1; then
  actual_sha256="$(sha256sum "${copybara_jar}" | awk '{ print $1 }')"
else
  actual_sha256="$(shasum -a 256 "${copybara_jar}" | awk '{ print $1 }')"
fi

if [[ "${actual_sha256}" != "${COPYBARA_SHA256}" ]]; then
  echo "Copybara checksum mismatch for ${COPYBARA_URL}" >&2
  echo "expected: ${COPYBARA_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
fi

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "copybara_jar=${copybara_jar}" >> "${GITHUB_OUTPUT}"
else
  printf '%s\n' "${copybara_jar}"
fi
