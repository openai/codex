#!/bin/sh

set -eu

real_ninja="${NINJA_REAL:?NINJA_REAL must point to the downloaded ninja binary}"

try_exec() {
    candidate_dir="$1"
    [ -n "${candidate_dir}" ] || return 1
    [ -d "${candidate_dir}" ] || return 1

    staged_dir="$(mktemp -d "${candidate_dir%/}/codex-ninja.XXXXXX" 2>/dev/null)" || return 1
    staged_ninja="${staged_dir}/ninja"

    if cp "${real_ninja}" "${staged_ninja}" 2>/dev/null \
        && chmod +x "${staged_ninja}" 2>/dev/null \
        && "${staged_ninja}" --version >/dev/null 2>&1; then
        exec "${staged_ninja}" "$@"
    fi

    rm -rf "${staged_dir}"
    return 1
}

for candidate_dir in "${CODEX_NINJA_TMPDIR:-}" /private/var/tmp /var/tmp "${TMPDIR:-}" /tmp "${HOME:-}"; do
    try_exec "${candidate_dir}"
done

echo "codex ninja wrapper: unable to stage an executable Ninja binary" >&2
exit 1
