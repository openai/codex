#!/usr/bin/env bash
set -euo pipefail

resolve_runfile() {
  local logical_path="$1"
  local launcher_runfiles="$0.runfiles"
  local workspace_name="__WORKSPACE_NAME__"
  local runfiles_root

  for runfiles_root in "${RUNFILES_DIR:-}" "${TEST_SRCDIR:-}" "${launcher_runfiles}"; do
    if [[ -z "${runfiles_root}" ]]; then
      continue
    fi
    for candidate in \
      "${runfiles_root}/${logical_path}" \
      "${runfiles_root}/${TEST_WORKSPACE:-}/${logical_path}" \
      "${runfiles_root}/${workspace_name}/${logical_path}" \
      "${runfiles_root}/_main/${logical_path}"; do
      if [[ -e "${candidate}" ]]; then
        printf '%s\n' "${candidate}"
        return 0
      fi
    done
  done

  local manifest="${RUNFILES_MANIFEST_FILE:-}"
  if [[ -z "${manifest}" ]]; then
    if [[ -f "$0.runfiles_manifest" ]]; then
      manifest="$0.runfiles_manifest"
    elif [[ -f "$0.exe.runfiles_manifest" ]]; then
      manifest="$0.exe.runfiles_manifest"
    fi
  fi

  if [[ -n "${manifest}" && -f "${manifest}" ]]; then
    local resolved
    resolved="$(awk -v path="${logical_path}" '$1 == path || substr($1, length($1) - length(path), length(path) + 1) == "/" path { $1 = ""; sub(/^ /, ""); print; exit }' "${manifest}")"
    if [[ -n "${resolved}" ]]; then
      printf '%s\n' "${resolved}"
      return 0
    fi
  fi

  echo "failed to resolve runfile: ${logical_path}" >&2
  return 1
}

binary="$(resolve_runfile "__BINARY__")"
RUNFILE_ENV_ARGS=()

__RUNFILE_ENV_EXPORTS__

if (( ${#RUNFILE_ENV_ARGS[@]} > 0 )); then
  exec env "${RUNFILE_ENV_ARGS[@]}" "${binary}" "$@"
else
  exec "${binary}" "$@"
fi
