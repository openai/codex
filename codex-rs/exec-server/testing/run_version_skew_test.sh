#!/usr/bin/env bash

set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script_path="${script_directory}/run_version_skew.sh"
fake_bin="$(mktemp -d "${TMPDIR:-/tmp}/codex-version-skew-test.XXXXXX")"
trap 'rm -rf "${fake_bin:?}"' EXIT

for tool in bash dirname mkdir mktemp rm sed; do
  ln -s "$(command -v "${tool}")" "${fake_bin}/${tool}"
done

printf '%s\n' \
  '#!/usr/bin/env bash' \
  'case "${1:-}" in' \
  '  -s) printf "Linux\\n" ;;' \
  '  -m) printf "x86_64\\n" ;;' \
  '  *) exit 1 ;;' \
  'esac' >"${fake_bin}/uname"
chmod +x "${fake_bin}/uname"

printf '%s\n' \
  '#!/usr/bin/env bash' \
  'exit 1' >"${fake_bin}/pkg-config"
chmod +x "${fake_bin}/pkg-config"

printf '%s\n' \
  '#!/usr/bin/env bash' \
  'printf "reached cargo\\n" >&2' \
  'exit 97' >"${fake_bin}/cargo"
chmod +x "${fake_bin}/cargo"

set +e
output="$(PATH="${fake_bin}" "${script_path}" latest 2>&1)"
status=$?
set -e

if [[ "${status}" -ne 97 || "${output}" != *"reached cargo"* ]]; then
  printf '%s\n' "expected non-Debian Linux without voice metadata to reach cargo" >&2
  printf 'status: %s\n%s\n' "${status}" "${output}" >&2
  exit 1
fi
