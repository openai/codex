//! Capture native export declarations and unset POSIX exports.
//! Set POSIX values are captured in bulk for quoting in Rust.

use crate::shell_detect::ShellType;

pub(super) fn script(shell_type: ShellType) -> String {
    let script = match shell_type {
        ShellType::Bash => {
            r#"(
  while IFS= read -r __codex_snapshot_export_name; do
    case "$__codex_snapshot_export_name" in
      ""|[0-9]*|*[!A-Za-z0-9_]*|PWD|OLDPWD) continue ;;
    esac
    RECORD_START
    declare -xp "$__codex_snapshot_export_name" 2>/dev/null || true
    RECORD_END
  done < <(compgen -e)
)
"#
        }
        ShellType::Zsh => {
            r#"(
  unsetopt rcquotes
  # Some Zsh builds do not include the zsh/parameter module.
  for __codex_snapshot_export_name in ${(f)"$(typeset +x)"}; do
    case "$__codex_snapshot_export_name" in
      ""|[0-9]*|*[!A-Za-z0-9_]*|PWD|OLDPWD) continue ;;
    esac
    case "${(tP)__codex_snapshot_export_name}" in
      *readonly*) continue ;;
      *export*) ;;
      *) continue ;;
    esac
    RECORD_START
    typeset -xp "$__codex_snapshot_export_name"
    RECORD_END
  done
)
"#
        }
        ShellType::Sh => {
            r#"if export -p >/dev/null 2>&1; then
export -p | __codex_snapshot_command awk '
/^(export|declare -x|typeset -x) [A-Za-z_][A-Za-z0-9_]*$/ {
  name=$0
  sub(/^(export|declare -x|typeset -x) /, "", name)
  if (name ~ /^(PWD|OLDPWD)$/) {
    next
  }
  if (!seen[name]++) {
    print name
  }
}' | while IFS= read -r __codex_snapshot_export_name; do
  # Only the validated identifier enters eval. Set values come from the bulk environment.
  if eval '[ "${'"$__codex_snapshot_export_name"'+x}" = x ]'; then
    continue
  fi
  # Only preserve an unset export if the native shell confirms it already exists.
  if (
    set +a
    set -- "$__codex_snapshot_export_name" "$(export -p)"
    export "$1"
    [ "$2" = "$(export -p)" ]
  ); then
    RECORD_START
    printf 'export %s\n' "$__codex_snapshot_export_name"
    RECORD_END
  fi
done
fi
"#
        }
        ShellType::PowerShell | ShellType::Cmd => return String::new(),
    };
    script
        .replace(
            "RECORD_START",
            "printf '%s\\0' \"$__codex_snapshot_export_name\"",
        )
        .replace("RECORD_END", "printf '\\0'")
}
