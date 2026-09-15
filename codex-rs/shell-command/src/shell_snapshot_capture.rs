//! Capture native shell state and decode complete export records without applying credential policy.
//! Retain native declaration flags and tied-array boundaries; reject incomplete records.

use super::SnapshotStartup;
use super::exports;
use super::literals;
use super::posix_env_path_expansion_function;
use crate::shell_detect::ShellType;
use crate::shell_startup_script;
use std::borrow::Cow;

const SNAPSHOT_COMMAND_HELPER: &str = r#"__codex_snapshot_command() {
  if command -v "$1" >/dev/null 2>&1; then
    "$@"
  else
    command -p "$@"
  fi
}"#;

const SNAPSHOT_ENVIRONMENT: &str = r#"if command -v env >/dev/null 2>&1; then
  "env" -0
else
  # Resolve the fallback separately: Bash's command -p changes the utility's PATH.
  "$(PATH="$(command -p getconf PATH)" command -v env)" -0
fi
"#;

/// Selects startup behavior and the records needed by a snapshot consumer.
/// Shell state and aliases are always captured. Credential preparation needs both
/// declarations and environment values; executor replay needs only the environment.
#[derive(Clone, Copy)]
pub struct SnapshotCaptureOptions {
    pub startup: SnapshotStartup,
    pub declarations: bool,
    pub environment: bool,
}

/// Returns a script that captures native state, aliases, and the requested export records.
///
/// Startup behavior is independent of how the captured records will be rendered.
/// PowerShell and Command Prompt snapshots are not supported by the snapshot callers.
pub fn snapshot_capture_script(
    shell_type: ShellType,
    options: SnapshotCaptureOptions,
) -> Option<String> {
    let script = match shell_type {
        ShellType::Zsh => zsh_snapshot_script(options.startup),
        ShellType::Bash => bash_snapshot_script(options.startup),
        ShellType::Sh => sh_snapshot_script(options.startup),
        ShellType::PowerShell | ShellType::Cmd => return None,
    };
    let declarations = if options.declarations {
        exports::script(shell_type)
    } else {
        String::new()
    };
    Some(
        script
            .replace("SNAPSHOT_EXPORTS", &declarations)
            .replace(
                "SNAPSHOT_DECLARATION_ENVIRONMENT",
                if options.declarations { SNAPSHOT_ENVIRONMENT } else { "" },
            )
            .replace("SNAPSHOT_COMMAND_HELPER", SNAPSHOT_COMMAND_HELPER)
            .replace(
                "SNAPSHOT_STARTUP_ENVIRONMENT",
                if options.declarations && options.environment {
                    "printf '\\0CODEX_SNAPSHOT_POSIX_STARTUP\\0%s\\0' \"$__codex_env_file\"\n    SNAPSHOT_ENVIRONMENT\n    printf '\\0'"
                } else {
                    ""
                },
            )
            .replace(
                "SNAPSHOT_ENVIRONMENT",
                if options.environment {
                    SNAPSHOT_ENVIRONMENT
                } else {
                    ""
                },
            ),
    )
}

fn zsh_snapshot_script(shell_startup: SnapshotStartup) -> String {
    let startup = match shell_startup {
        SnapshotStartup::Interactive => shell_startup_script(ShellType::Zsh),
        SnapshotStartup::NonInteractive => "",
    };
    let script = r##"print '# Snapshot file'
print '# Unset all aliases to avoid conflicts with functions'
print 'unalias -a 2>/dev/null || true'
print '# Functions'
functions
print ''
SNAPSHOT_COMMAND_HELPER
setopt_count=$(setopt | __codex_snapshot_command wc -l | __codex_snapshot_command tr -d ' ')
print "# setopts $setopt_count"
setopt | __codex_snapshot_command sed 's/^/setopt /'
print ''
printf '\0'
alias_count=$(alias -L | __codex_snapshot_command wc -l | __codex_snapshot_command tr -d ' ')
print "# aliases $alias_count"
alias -L
print ''
printf '\0'
SNAPSHOT_EXPORTS
printf '\0'
SNAPSHOT_ENVIRONMENT
"##;
    format!("{startup}{script}")
}

fn bash_snapshot_script(shell_startup: SnapshotStartup) -> String {
    let startup = match shell_startup {
        SnapshotStartup::Interactive => shell_startup_script(ShellType::Bash),
        SnapshotStartup::NonInteractive => "",
    };
    let script = r##"echo '# Snapshot file'
echo '# Unset all aliases to avoid conflicts with functions'
echo 'unalias -a 2>/dev/null || true'
shopt -p || true
echo '# Functions'
declare -f
echo ''
SNAPSHOT_COMMAND_HELPER
bash_opts=$(set -o | __codex_snapshot_command awk '$2=="on"{print $1}')
bash_opt_count=$(printf '%s\n' "$bash_opts" | __codex_snapshot_command sed '/^$/d' | __codex_snapshot_command wc -l | __codex_snapshot_command tr -d ' ')
echo "# setopts $bash_opt_count"
if [ -n "$bash_opts" ]; then
  printf 'set -o %s\n' $bash_opts
fi
echo ''
printf '\0'
alias_count=$(alias -p | __codex_snapshot_command wc -l | __codex_snapshot_command tr -d ' ')
echo "# aliases $alias_count"
alias -p
echo ''
printf '\0'
SNAPSHOT_EXPORTS
printf '\0'
SNAPSHOT_ENVIRONMENT
"##;
    format!("{startup}{script}")
}

pub(super) const BASH_SH_SNAPSHOT_HEADER: &str = "# Snapshot file\n# Bash-backed sh\n";

fn sh_snapshot_script(shell_startup: SnapshotStartup) -> String {
    let startup = match shell_startup {
        SnapshotStartup::Interactive => [
            posix_env_path_expansion_function(),
            r##"if [ -n "${ENV-}" ]; then
  __codex_env_file=$(__codex_snapshot_expand_env "$ENV")
  if [ -r "$__codex_env_file" ] && [ ! -d "$__codex_env_file" ]; then
    SNAPSHOT_STARTUP_ENVIRONMENT
    case "$__codex_env_file" in
      /*) . "$__codex_env_file" ;;
      *) . "./$__codex_env_file" ;;
    esac
  fi
  command unset __codex_env_file
fi
command unset -f __codex_snapshot_expand_env
"##,
        ]
        .join("\n"),
        SnapshotStartup::NonInteractive => String::new(),
    };
    let script = r##"echo '# Snapshot file'
if [ -n "${BASH_VERSINFO-}" ]; then
  echo '# Bash-backed sh'
  shopt -p || true
fi
echo '# Unset all aliases to avoid conflicts with functions'
unalias -a 2>/dev/null || true
echo '# Functions'
if command -v typeset >/dev/null 2>&1; then
  typeset -f
elif command -v declare >/dev/null 2>&1; then
  declare -f
fi
echo ''
SNAPSHOT_COMMAND_HELPER
if set -o >/dev/null 2>&1; then
  sh_opts=$(set -o | __codex_snapshot_command awk '$2=="on"{print $1}')
  sh_opt_count=$(printf '%s\n' "$sh_opts" | __codex_snapshot_command sed '/^$/d' | __codex_snapshot_command wc -l | __codex_snapshot_command tr -d ' ')
  echo "# setopts $sh_opt_count"
  if [ -n "$sh_opts" ]; then
    printf 'set -o %s\n' $sh_opts
  fi
else
  echo '# setopts 0'
fi
echo ''
printf '\0'
if alias >/dev/null 2>&1; then
  alias_count=$(alias | __codex_snapshot_command wc -l | __codex_snapshot_command tr -d ' ')
  echo "# aliases $alias_count"
  alias
  echo ''
else
  echo '# aliases 0'
fi
printf '\0'
SNAPSHOT_EXPORTS
printf '\0'
SNAPSHOT_DECLARATION_ENVIRONMENT
printf '\0'
SNAPSHOT_ENVIRONMENT
"##;
    format!("{startup}{script}")
}

/// Native shell sections captured before applying credential or environment policy.
///
/// Function and alias declarations remain shell source. The shell supplies each export's
/// name and boundary; only tied arrays need syntax decoding. No captured code is evaluated.
pub struct CapturedSnapshot<'a> {
    /// Resolved POSIX startup file and its environment immediately before sourcing it.
    pub startup_environment: Option<CapturedStartupEnvironment<'a>>,
    pub(super) shell_type: ShellType,
    pub(super) state: &'a str,
    pub(super) aliases: &'a str,
    pub(super) exports: Vec<CapturedExport<'a>>,
    /// NUL-delimited environment entries from the same shell invocation.
    pub environment: &'a [u8],
}

/// Optional pre-startup state captured alongside the final snapshot records.
pub struct CapturedStartupEnvironment<'a> {
    pub path: Cow<'a, str>,
    pub environment: &'a [u8],
}

pub(super) struct CapturedExport<'a> {
    pub source: Cow<'a, str>,
    pub key: &'a str,
    pub value: ExportValue,
}

pub(super) enum ExportValue {
    Plain,
    TiedArray(CapturedArray),
    UnparsedArray,
}

pub(super) struct CapturedArray {
    pub span: std::ops::Range<usize>,
    pub values: Vec<Vec<u8>>,
    pub separator: Vec<u8>,
}

impl<'a> CapturedSnapshot<'a> {
    /// Decode the local capture format emitted by `snapshot_capture_script`.
    /// Keep raw environment bytes so callers can discard non-UTF-8 entries independently.
    pub fn parse(shell_type: ShellType, captured: &'a [u8]) -> Option<Self> {
        if !matches!(shell_type, ShellType::Bash | ShellType::Zsh | ShellType::Sh) {
            return None;
        }
        let mut remaining = captured;
        let startup_marker = b"\0CODEX_SNAPSHOT_POSIX_STARTUP\0";
        let startup_environment = if shell_type == ShellType::Sh
            && let Some(start) = remaining
                .windows(startup_marker.len())
                .position(|part| part == startup_marker)
        {
            remaining = &remaining[start + startup_marker.len()..];
            let path = String::from_utf8_lossy(take_record(&mut remaining)?);
            let environment = remaining;
            while !take_record(&mut remaining)?.is_empty() {}
            let environment = &environment[..environment.len() - remaining.len() - 1];
            Some(CapturedStartupEnvironment { path, environment })
        } else {
            None
        };
        // state NUL aliases NUL (name NUL declaration NUL)* NUL environment.
        // POSIX sh additionally captures declaration values as a NUL-delimited environment.
        let state = take_record(&mut remaining)?;
        let marker = b"# Snapshot file";
        let start = state
            .windows(marker.len())
            .position(|part| part == marker)?;
        let state = std::str::from_utf8(&state[start..]).ok()?;
        let aliases = std::str::from_utf8(take_record(&mut remaining)?).ok()?;
        let mut records = Vec::new();
        loop {
            let key = std::str::from_utf8(take_record(&mut remaining)?).ok()?;
            if key.is_empty() {
                if shell_type == ShellType::Sh {
                    loop {
                        let entry = take_record(&mut remaining)?;
                        if entry.is_empty() {
                            break;
                        }
                        let equals = entry.iter().position(|byte| *byte == b'=')?;
                        let Ok(key) = std::str::from_utf8(&entry[..equals]) else {
                            continue;
                        };
                        if key.is_empty()
                            || matches!(key, "PWD" | "OLDPWD")
                            || !key.bytes().enumerate().all(|(index, byte)| {
                                byte == b'_'
                                    || byte.is_ascii_alphabetic()
                                    || index > 0 && byte.is_ascii_digit()
                            })
                        {
                            continue;
                        }
                        let value = String::from_utf8_lossy(&entry[equals + 1..]);
                        records.push(CapturedExport {
                            source: Cow::Owned(format!(
                                "export {key}={}\n",
                                shlex::try_quote(&value).ok()?
                            )),
                            key,
                            value: ExportValue::Plain,
                        });
                    }
                }
                return Some(Self {
                    startup_environment,
                    shell_type,
                    state,
                    aliases,
                    exports: records,
                    environment: remaining,
                });
            }
            let source = String::from_utf8_lossy(take_record(&mut remaining)?);
            // Only tied arrays need syntax decoding; scalar values remain native declarations.
            let tied = shell_type == ShellType::Zsh
                && source
                    .split_whitespace()
                    .skip(/*n*/ 1)
                    .take_while(|word| word.starts_with('-'))
                    .any(|flags| flags.contains('T'));
            let tree = tied
                .then(|| crate::bash::try_parse_shell(&source))
                .flatten();
            let array = tree.as_ref().and_then(|tree| {
                if tree.root_node().has_error() {
                    return None;
                }
                let mut cursor = tree.root_node().walk();
                tree.root_node()
                    .named_child(/*i*/ 0)?
                    .named_children(&mut cursor)
                    .find_map(|child| {
                        child
                            .child_by_field_name("value")
                            .filter(|value| value.kind() == "array")
                    })
            });
            let value = if let Some(array) = array {
                let mut cursor = array.walk();
                let values = array
                    .named_children(&mut cursor)
                    .map(|node| {
                        literals::literal_word_bytes(
                            node,
                            &source,
                            shell_type,
                            literals::Quoting::Posix,
                        )
                    })
                    .collect::<Option<Vec<_>>>();
                let separator = array.parent().and_then(|node| node.next_named_sibling());
                let separator = if let Some(separator) = separator
                    && separator.kind() == "variable_name"
                {
                    separator
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|value| value.as_bytes().to_vec())
                } else if let Some(separator) = separator {
                    literals::literal_word_bytes(
                        separator,
                        &source,
                        shell_type,
                        literals::Quoting::Posix,
                    )
                } else {
                    Some(vec![b':'])
                };
                match (values, separator) {
                    (Some(values), Some(separator)) => ExportValue::TiedArray(CapturedArray {
                        span: array.byte_range(),
                        values,
                        separator,
                    }),
                    _ => ExportValue::UnparsedArray,
                }
            } else if tied {
                ExportValue::UnparsedArray
            } else {
                ExportValue::Plain
            };
            records.push(CapturedExport { source, key, value });
        }
    }
}

fn take_record<'a>(remaining: &mut &'a [u8]) -> Option<&'a [u8]> {
    let boundary = remaining.iter().position(|byte| *byte == 0)?;
    let record = &remaining[..boundary];
    *remaining = &remaining[boundary + 1..];
    Some(record)
}
