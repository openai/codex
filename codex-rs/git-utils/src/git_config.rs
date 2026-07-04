use std::io;
use std::path::Component;
use std::path::Path;

use crate::git_command::GitRunner;

pub(crate) fn parse_git_boolean(value: &[u8]) -> Option<bool> {
    parse_git_boolean_with_minimum(value, i128::from(i32::MIN))
}

/// Parse the Git boolean grammar while excluding numeric `INT_MIN`.
///
/// Older supported Git releases reject that one signed endpoint after unit
/// expansion, so environment gates that must succeed across versions use this
/// conservative variant. The accepted spellings otherwise share one parser.
pub(crate) fn parse_git_boolean_symmetric_i32(value: &[u8]) -> Option<bool> {
    parse_git_boolean_with_minimum(value, -i128::from(i32::MAX))
}

fn parse_git_boolean_with_minimum(value: &[u8], minimum: i128) -> Option<bool> {
    if value.eq_ignore_ascii_case(b"true")
        || value.eq_ignore_ascii_case(b"yes")
        || value.eq_ignore_ascii_case(b"on")
    {
        return Some(true);
    }
    if value.is_empty()
        || value.eq_ignore_ascii_case(b"false")
        || value.eq_ignore_ascii_case(b"no")
        || value.eq_ignore_ascii_case(b"off")
    {
        return Some(false);
    }

    // Git parses the remaining boolean spellings through `git_parse_int`: C
    // base-0 syntax, an optional binary-unit suffix, and signed `int` bounds.
    let value = std::str::from_utf8(value)
        .ok()?
        .trim_start_matches(|value: char| value.is_ascii_whitespace());
    let (negative, unsigned) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        Some(_) => (false, value),
        None => return None,
    };
    let (base, unsigned) = if unsigned.starts_with("0x") || unsigned.starts_with("0X") {
        (16, &unsigned[2..])
    } else if unsigned.starts_with('0') {
        (8, unsigned)
    } else {
        (10, unsigned)
    };
    let digit_count = unsigned
        .bytes()
        .take_while(|byte| match base {
            8 => matches!(byte, b'0'..=b'7'),
            10 => byte.is_ascii_digit(),
            16 => byte.is_ascii_hexdigit(),
            _ => false,
        })
        .count();
    if digit_count == 0 {
        return None;
    }
    let (digits, suffix) = unsigned.split_at(digit_count);
    let factor = if suffix.is_empty() {
        1_i128
    } else if suffix.eq_ignore_ascii_case("k") {
        1024
    } else if suffix.eq_ignore_ascii_case("m") {
        1024 * 1024
    } else if suffix.eq_ignore_ascii_case("g") {
        1024 * 1024 * 1024
    } else {
        return None;
    };
    let magnitude = i128::from_str_radix(digits, base).ok()?;
    let signed = if negative { -magnitude } else { magnitude };
    let value = signed.checked_mul(factor)?;
    (minimum..=i128::from(i32::MAX))
        .contains(&value)
        .then_some(value != 0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GitConfigScope {
    Unknown,
    System,
    Global,
    Local,
    Worktree,
    Command,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitConfigEntry {
    pub(crate) scope: GitConfigScope,
    pub(crate) origin: GitConfigOrigin,
    pub(crate) key: String,
    pub(crate) value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitConfigOrigin {
    CommandLine,
    File(std::path::PathBuf),
}

pub(crate) fn parse_config_entries(output: &[u8]) -> io::Result<Vec<GitConfigEntry>> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(invalid_config_output("unterminated Git config output"));
    };
    let fields = body.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.len() % 3 != 0 {
        return Err(invalid_config_output("incomplete Git config record"));
    }

    let mut entries = Vec::new();
    for record in fields.chunks_exact(3) {
        let scope = parse_scope(record[0])?;
        let origin = parse_config_origin(record[1])?;
        let entry = parse_utf8_field(record[2], "config key/value")?;
        let Some((key, value)) = entry.split_once('\n') else {
            return Err(invalid_config_output(
                "Git config record has no key/value separator",
            ));
        };
        if key.is_empty() {
            return Err(invalid_config_output("empty Git config key"));
        }
        let entry = GitConfigEntry {
            scope,
            origin,
            key: key.to_string(),
            value: value.to_string(),
        };
        entries.push(entry);
    }
    Ok(entries)
}

/// Parse the `--show-origin` form used by Git versions that predate
/// `config --show-scope`. The record order still reflects effective config
/// precedence, so duplicate include directives remain ordered. Scope is
/// retained as best-effort metadata only; command-line entries remain
/// distinguishable.
pub(crate) fn parse_config_entries_with_origins(output: &[u8]) -> io::Result<Vec<GitConfigEntry>> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(invalid_config_output("unterminated Git config output"));
    };
    let fields = body.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.len() % 2 != 0 {
        return Err(invalid_config_output("incomplete Git config record"));
    }

    let mut entries = Vec::new();
    for record in fields.chunks_exact(2) {
        let origin = parse_config_origin(record[0])?;
        let entry = parse_utf8_field(record[1], "config key/value")?;
        let Some((key, value)) = entry.split_once('\n') else {
            return Err(invalid_config_output(
                "Git config record has no key/value separator",
            ));
        };
        if key.is_empty() {
            return Err(invalid_config_output("empty Git config key"));
        }
        let scope = if origin == GitConfigOrigin::CommandLine {
            GitConfigScope::Command
        } else {
            // Older Git does not expose the scope. Selection depends on the
            // emitted precedence order, not on this informational label.
            GitConfigScope::Local
        };
        entries.push(GitConfigEntry {
            scope,
            origin,
            key: key.to_string(),
            value: value.to_string(),
        });
    }
    Ok(entries)
}

pub(crate) fn read_config_entries_without_includes(
    git: &GitRunner,
    cwd: &Path,
    git_config_args: &[String],
    pattern: &str,
    probe: &str,
    config_file: Option<&Path>,
) -> io::Result<Vec<GitConfigEntry>> {
    read_config_entries_with_fallback(git, cwd, git_config_args, pattern, probe, config_file)
}

fn read_config_entries_with_fallback(
    git: &GitRunner,
    cwd: &Path,
    git_config_args: &[String],
    pattern: &str,
    probe: &str,
    config_file: Option<&Path>,
) -> io::Result<Vec<GitConfigEntry>> {
    let scoped = run_effective_config_query(
        git,
        cwd,
        git_config_args,
        pattern,
        /*show_scope*/ true,
        config_file,
    )?;
    if scoped
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return parse_config_entries(&scoped.stdout);
    }

    let legacy = run_effective_config_query(
        git,
        cwd,
        git_config_args,
        pattern,
        /*show_scope*/ false,
        config_file,
    )?;
    if !legacy
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return Err(io::Error::other(format!(
            "git {probe} config probe failed with status {}: {}",
            legacy.status,
            String::from_utf8_lossy(&legacy.stderr).trim()
        )));
    }
    parse_config_entries_with_origins(&legacy.stdout)
}

fn run_effective_config_query(
    git: &GitRunner,
    cwd: &Path,
    git_config_args: &[String],
    pattern: &str,
    show_scope: bool,
    config_file: Option<&Path>,
) -> io::Result<std::process::Output> {
    let mut command = git.command_for_cwd(cwd)?;
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        .arg("config");
    if let Some(config_file) = config_file {
        command.arg("--file").arg(config_file);
    }
    command.arg("--null");
    if show_scope {
        command.arg("--show-scope");
    }
    command.args(["--show-origin", "--no-includes", "--get-regexp", pattern]);
    git.output(command)
}

pub(crate) fn path_is_within(path: &Path, root: &Path) -> bool {
    let mut path_components = path.components();
    for root_component in root.components() {
        let Some(path_component) = path_components.next() else {
            return false;
        };
        if !components_equal(path_component, root_component) {
            return false;
        }
    }
    true
}

#[cfg(windows)]
fn components_equal(left: Component<'_>, right: Component<'_>) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

#[cfg(not(windows))]
fn components_equal(left: Component<'_>, right: Component<'_>) -> bool {
    left == right
}

fn parse_scope(scope: &[u8]) -> io::Result<GitConfigScope> {
    match scope {
        b"unknown" => Ok(GitConfigScope::Unknown),
        b"system" => Ok(GitConfigScope::System),
        b"global" => Ok(GitConfigScope::Global),
        b"local" => Ok(GitConfigScope::Local),
        b"worktree" => Ok(GitConfigScope::Worktree),
        b"command" => Ok(GitConfigScope::Command),
        _ => Err(invalid_config_output("unknown Git config scope")),
    }
}

fn parse_config_origin(origin: &[u8]) -> io::Result<GitConfigOrigin> {
    if origin == b"command line:" {
        return Ok(GitConfigOrigin::CommandLine);
    }
    let path = origin
        .strip_prefix(b"file:")
        .ok_or_else(|| invalid_config_output("unsupported Git config origin"))?;
    if path.is_empty() || path.contains(&0) {
        return Err(invalid_config_output("empty Git config origin"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;

        Ok(GitConfigOrigin::File(std::path::PathBuf::from(
            std::ffi::OsString::from_vec(path.to_vec()),
        )))
    }
    #[cfg(not(unix))]
    {
        Ok(GitConfigOrigin::File(std::path::PathBuf::from(
            parse_utf8_field(path, "Git config origin path")?,
        )))
    }
}

fn parse_utf8_field<'a>(field: &'a [u8], name: &str) -> io::Result<&'a str> {
    std::str::from_utf8(field).map_err(|_| invalid_config_output(&format!("non-UTF-8 {name}")))
}

fn invalid_config_output(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "git_config_tests.rs"]
mod tests;
