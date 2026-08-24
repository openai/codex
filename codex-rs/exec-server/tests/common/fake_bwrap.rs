use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn write_fake_bwrap(bin_dir: &Path) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(bin_dir)?;
    let fake_bwrap = bin_dir.join("bwrap");
    std::fs::write(
        &fake_bwrap,
        r#"#!/bin/bash
set -euo pipefail

for arg in "$@"; do
  if [[ "${arg}" == "--help" ]]; then
    echo "Usage: bwrap --argv0 --perms --as-pid-1"
    exit 0
  fi
done

args=("$@")
argv0=""
command_start=-1
for i in "${!args[@]}"; do
  if [[ "${args[$i]}" == "--argv0" && $((i + 1)) -lt ${#args[@]} ]]; then
    argv0="${args[$((i + 1))]}"
  fi
  if [[ "${args[$i]}" == "--" ]]; then
    command_start=$((i + 1))
    break
  fi
done

if [[ "${command_start}" -lt 0 || "${command_start}" -ge "${#args[@]}" ]]; then
  echo "fake bwrap did not find an inner command" >&2
  exit 125
fi

cmd=("${args[@]:$command_start}")
case "${cmd[0]}" in
  /usr/bin/true|/bin/true|true)
    exec "${cmd[@]}"
    ;;
esac

printf '%s\n' "$*" >> "${0}.log"
if [[ -f "${0}.fail-once" ]]; then
  rm -f "${0}.fail-once"
  echo "forced fake bwrap failure" >&2
  exit 125
fi

if [[ -n "${argv0}" ]]; then
  exec -a "${argv0}" "${cmd[@]}"
fi
exec "${cmd[@]}"
"#,
    )?;
    let mut permissions = std::fs::metadata(&fake_bwrap)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_bwrap, permissions)?;
    Ok(fake_bwrap)
}
