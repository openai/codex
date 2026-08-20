use crate::shell_detect::ShellType;

#[cfg(all(test, unix))]
#[path = "shell_snapshot_tests.rs"]
mod tests;

const EXCLUDED_EXPORT_VARS: &[&str] = &["PWD", "OLDPWD"];
const EXPORT_CAPTURE_MARKER: &str = "# Capture exported variables";

/// Returns the shell-native script used to capture restorable shell state.
///
/// Command Prompt does not expose the POSIX or PowerShell state required by
/// this representation and therefore does not support snapshots.
pub fn snapshot_script(shell_type: ShellType) -> Option<String> {
    match shell_type {
        ShellType::Zsh => Some(zsh_snapshot_script()),
        ShellType::Bash => Some(bash_snapshot_script()),
        ShellType::Sh => Some(sh_snapshot_script()),
        ShellType::PowerShell => Some(powershell_snapshot_script().to_string()),
        ShellType::Cmd => None,
    }
}

/// Captures shell state and a separate NUL-delimited exported environment.
///
/// Keeping exports outside the restorable script lets executors apply their
/// environment policy after shell profiles have run.
pub fn snapshot_state_and_environment_script(shell_type: ShellType) -> Option<String> {
    let script = snapshot_script(shell_type)?;
    let (state, _) = script.split_once(EXPORT_CAPTURE_MARKER)?;
    Some(format!("{state}printf '\\0'\n/usr/bin/env -0\n"))
}

fn excluded_exports_regex() -> String {
    EXCLUDED_EXPORT_VARS.join("|")
}

fn zsh_snapshot_script() -> String {
    let excluded = excluded_exports_regex();
    let script = r##"if [[ -n "$ZDOTDIR" ]]; then
  rc="$ZDOTDIR/.zshrc"
else
  rc="$HOME/.zshrc"
fi
[[ -r "$rc" ]] && . "$rc"
print '# Snapshot file'
print '# Unset all aliases to avoid conflicts with functions'
print 'unalias -a 2>/dev/null || true'
print '# Functions'
functions
print ''
setopt_count=$(setopt | wc -l | tr -d ' ')
print "# setopts $setopt_count"
setopt | sed 's/^/setopt /'
print ''
alias_count=$(alias -L | wc -l | tr -d ' ')
print "# aliases $alias_count"
alias -L
print ''
# Capture exported variables
export_lines=$(export -p | awk '
/^(export|declare -x|typeset -x) / {
  line=$0
  name=line
  sub(/^(export|declare -x|typeset -x) /, "", name)
  if (name ~ /^-[A-Za-z]*r[A-Za-z]* /) {
    next
  }
  if (name ~ /^-[A-Za-z]*T[A-Za-z]* /) {
    sub(/^-[A-Za-z]*T[A-Za-z]* /, "", name)
    sub(/ [A-Za-z_][A-Za-z0-9_]*=.*/, "", name)
  }
  sub(/=.*/, "", name)
  if (name ~ /^(EXCLUDED_EXPORTS)$/) {
    next
  }
  if (name ~ /^[A-Za-z_][A-Za-z0-9_]*$/) {
    print line
  }
}')
export_count=$(printf '%s\n' "$export_lines" | sed '/^$/d' | wc -l | tr -d ' ')
print "# exports $export_count"
if [[ -n "$export_lines" ]]; then
  print -r -- "$export_lines"
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

fn bash_snapshot_script() -> String {
    let excluded = excluded_exports_regex();
    let script = r##"if [ -z "$BASH_ENV" ] && [ -r "$HOME/.bashrc" ]; then
  . "$HOME/.bashrc"
fi
echo '# Snapshot file'
echo '# Unset all aliases to avoid conflicts with functions'
unalias -a 2>/dev/null || true
echo '# Functions'
declare -f
echo ''
bash_opts=$(set -o | awk '$2=="on"{print $1}')
bash_opt_count=$(printf '%s\n' "$bash_opts" | sed '/^$/d' | wc -l | tr -d ' ')
echo "# setopts $bash_opt_count"
if [ -n "$bash_opts" ]; then
  printf 'set -o %s\n' $bash_opts
fi
echo ''
alias_count=$(alias -p | wc -l | tr -d ' ')
echo "# aliases $alias_count"
alias -p
echo ''
# Capture exported variables
export_lines=$(
  while IFS= read -r name; do
    if [[ "$name" =~ ^(EXCLUDED_EXPORTS)$ ]]; then
      continue
    fi
    if [[ ! "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
      continue
    fi
    declare -xp "$name" 2>/dev/null || true
  done < <(compgen -e)
)
export_count=$(printf '%s\n' "$export_lines" | sed '/^$/d' | wc -l | tr -d ' ')
echo "# exports $export_count"
if [ -n "$export_lines" ]; then
  printf '%s\n' "$export_lines"
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

fn sh_snapshot_script() -> String {
    let excluded = excluded_exports_regex();
    let script = r##"if [ -n "$ENV" ] && [ -r "$ENV" ]; then
  . "$ENV"
fi
echo '# Snapshot file'
echo '# Unset all aliases to avoid conflicts with functions'
unalias -a 2>/dev/null || true
echo '# Functions'
if command -v typeset >/dev/null 2>&1; then
  typeset -f
elif command -v declare >/dev/null 2>&1; then
  declare -f
fi
echo ''
if set -o >/dev/null 2>&1; then
  sh_opts=$(set -o | awk '$2=="on"{print $1}')
  sh_opt_count=$(printf '%s\n' "$sh_opts" | sed '/^$/d' | wc -l | tr -d ' ')
  echo "# setopts $sh_opt_count"
  if [ -n "$sh_opts" ]; then
    printf 'set -o %s\n' $sh_opts
  fi
else
  echo '# setopts 0'
fi
echo ''
if alias >/dev/null 2>&1; then
  alias_count=$(alias | wc -l | tr -d ' ')
  echo "# aliases $alias_count"
  alias
  echo ''
else
  echo '# aliases 0'
fi
# Capture exported variables
if export -p >/dev/null 2>&1; then
  export_lines=$(export -p | awk '
/^(export|declare -x|typeset -x) / {
  line=$0
  name=line
  sub(/^(export|declare -x|typeset -x) /, "", name)
  sub(/=.*/, "", name)
  if (name ~ /^(EXCLUDED_EXPORTS)$/) {
    next
  }
  if (name ~ /^[A-Za-z_][A-Za-z0-9_]*$/) {
    print line
  }
}')
  export_count=$(printf '%s\n' "$export_lines" | sed '/^$/d' | wc -l | tr -d ' ')
  echo "# exports $export_count"
  if [ -n "$export_lines" ]; then
    printf '%s\n' "$export_lines"
  fi
else
  export_count=$(env | sort | awk -F= '$1 ~ /^[A-Za-z_][A-Za-z0-9_]*$/ { count++ } END { print count }')
  echo "# exports $export_count"
  env | sort | while IFS='=' read -r key value; do
    case "$key" in
      ""|[0-9]*|*[!A-Za-z0-9_]*|EXCLUDED_EXPORTS) continue ;;
    esac
    escaped=$(printf "%s" "$value" | sed "s/'/'\"'\"'/g")
    printf "export %s='%s'\n" "$key" "$escaped"
  done
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

fn powershell_snapshot_script() -> &'static str {
    r##"$ErrorActionPreference = 'Stop'
Write-Output '# Snapshot file'
Write-Output '# Unset all aliases to avoid conflicts with functions'
Write-Output 'Remove-Item Alias:* -ErrorAction SilentlyContinue'
Write-Output '# Functions'
Get-ChildItem Function: | ForEach-Object {
    "function {0} {{`n{1}`n}}" -f $_.Name, $_.Definition
}
Write-Output ''
$aliases = Get-Alias
Write-Output ("# aliases " + $aliases.Count)
$aliases | ForEach-Object {
    "Set-Alias -Name {0} -Value {1}" -f $_.Name, $_.Definition
}
Write-Output ''
$envVars = Get-ChildItem Env:
Write-Output ("# exports " + $envVars.Count)
$envVars | ForEach-Object {
    $escaped = $_.Value -replace "'", "''"
    "`$env:{0}='{1}'" -f $_.Name, $escaped
}
"##
}
