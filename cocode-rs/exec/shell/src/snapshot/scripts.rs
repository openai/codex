//! Shell-specific snapshot scripts.
//!
//! Each script captures the user's shell environment including:
//! - Functions
//! - Shell options
//! - Aliases
//! - Environment variables (exports)
//!
//! The scripts output a marker (`# Snapshot file`) followed by shell code
//! that can be sourced to restore the captured state.

/// Environment variables to exclude from snapshots.
///
/// These variables are typically set by the shell itself and should not be
/// captured as they may cause issues when restored.
pub const EXCLUDED_EXPORT_VARS: &[&str] = &["PWD", "OLDPWD"];

/// Returns the regex pattern for excluded exports.
fn excluded_exports_regex() -> String {
    EXCLUDED_EXPORT_VARS.join("|")
}

/// Returns the zsh snapshot script.
///
/// This script:
/// 1. Sources the user's .zshrc
/// 2. Captures all functions
/// 3. Captures shell options (setopt)
/// 4. Captures aliases
/// 5. Captures exports (filtered for valid names and exclusions)
pub fn zsh_snapshot_script() -> String {
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
print "# exports $export_count"
if [[ -n "$export_lines" ]]; then
  print -r -- "$export_lines"
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

/// Returns the bash snapshot script.
///
/// This script:
/// 1. Sources the user's .bashrc
/// 2. Captures all functions (declare -f)
/// 3. Captures shell options (set -o)
/// 4. Captures aliases (alias -p)
/// 5. Captures exports (filtered for valid names and exclusions)
pub fn bash_snapshot_script() -> String {
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
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

/// Returns the POSIX sh snapshot script.
///
/// This script uses POSIX-compatible commands and has fallbacks for systems
/// that may not support all features.
pub fn sh_snapshot_script() -> String {
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

/// Returns the PowerShell snapshot script.
///
/// Note: PowerShell support is limited and may not work on all systems.
pub fn powershell_snapshot_script() -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_excluded_exports_regex() {
        let regex = excluded_exports_regex();
        assert!(regex.contains("PWD"));
        assert!(regex.contains("OLDPWD"));
        assert!(regex.contains("|"));
    }

    #[test]
    fn test_zsh_script_contains_marker() {
        let script = zsh_snapshot_script();
        assert!(script.contains("# Snapshot file"));
        assert!(script.contains("unalias -a"));
        assert!(script.contains("functions"));
        assert!(script.contains("setopt"));
        assert!(script.contains("alias -L"));
        assert!(script.contains("export -p"));
    }

    #[test]
    fn test_bash_script_contains_marker() {
        let script = bash_snapshot_script();
        assert!(script.contains("# Snapshot file"));
        assert!(script.contains("unalias -a"));
        assert!(script.contains("declare -f"));
        assert!(script.contains("set -o"));
        assert!(script.contains("alias -p"));
        assert!(script.contains("export -p"));
    }

    #[test]
    fn test_sh_script_contains_marker() {
        let script = sh_snapshot_script();
        assert!(script.contains("# Snapshot file"));
        assert!(script.contains("unalias -a"));
        assert!(script.contains("export -p"));
        // Should have fallbacks for function capture
        assert!(script.contains("typeset -f"));
        assert!(script.contains("declare -f"));
    }

    #[test]
    fn test_powershell_script_contains_marker() {
        let script = powershell_snapshot_script();
        assert!(script.contains("# Snapshot file"));
        assert!(script.contains("Remove-Item Alias:*"));
        assert!(script.contains("Get-ChildItem Function:"));
        assert!(script.contains("Get-Alias"));
        assert!(script.contains("Get-ChildItem Env:"));
    }

    #[test]
    fn test_scripts_filter_excluded_vars() {
        let zsh = zsh_snapshot_script();
        let bash = bash_snapshot_script();
        let sh = sh_snapshot_script();

        // All scripts should filter PWD and OLDPWD
        for script in [&zsh, &bash, &sh] {
            assert!(script.contains("PWD|OLDPWD") || script.contains("EXCLUDED_EXPORTS"));
            // The actual replacement should have happened
            assert!(!script.contains("EXCLUDED_EXPORTS"));
        }
    }

    /// Tests that the bash snapshot script correctly filters out excluded
    /// environment variables (PWD, OLDPWD) and invalid variable names.
    #[cfg(unix)]
    #[test]
    fn test_bash_snapshot_filters_invalid_exports() {
        use std::process::Command;

        let output = Command::new("/bin/bash")
            .arg("-c")
            .arg(bash_snapshot_script())
            .env("BASH_ENV", "/dev/null")
            .env("VALID_NAME", "ok")
            .env("PWD", "/tmp/stale")
            .env("BAD-NAME", "broken")
            .output()
            .expect("run bash");

        assert!(output.status.success(), "bash script should succeed");
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            stdout.contains("VALID_NAME"),
            "should include valid exports"
        );
        assert!(
            !stdout.contains("PWD=/tmp/stale"),
            "should exclude PWD from exports"
        );
        assert!(
            !stdout.contains("BAD-NAME"),
            "should exclude invalid variable names (containing -)"
        );
    }
}
