use std::collections::BTreeMap;
use std::io;
use std::path::Component;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GitConfigScope {
    System,
    Global,
    Local,
    Worktree,
    Command,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitConfigEntry {
    pub(crate) scope: GitConfigScope,
    pub(crate) origin: String,
    pub(crate) key: String,
    pub(crate) value: String,
}

pub(crate) fn parse_effective_config(
    output: &[u8],
) -> io::Result<BTreeMap<String, GitConfigEntry>> {
    if output.is_empty() {
        return Ok(BTreeMap::new());
    }
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(invalid_config_output("unterminated Git config output"));
    };
    let fields = body.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.len() % 3 != 0 {
        return Err(invalid_config_output("incomplete Git config record"));
    }

    let mut effective = BTreeMap::new();
    for record in fields.chunks_exact(3) {
        let scope = parse_scope(record[0])?;
        let origin = parse_utf8_field(record[1], "config origin")?;
        if origin.is_empty() {
            return Err(invalid_config_output("empty Git config origin"));
        }
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
            origin: origin.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        };
        effective.insert(key.to_string(), entry);
    }
    Ok(effective)
}

/// Parse the `--show-origin` form used by Git versions that predate
/// `config --show-scope`. The record order still reflects effective config
/// precedence, which is what helper selection relies on. Scope is retained as
/// best-effort metadata only; command-line entries remain distinguishable.
pub(crate) fn parse_effective_config_with_origins(
    output: &[u8],
) -> io::Result<BTreeMap<String, GitConfigEntry>> {
    if output.is_empty() {
        return Ok(BTreeMap::new());
    }
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(invalid_config_output("unterminated Git config output"));
    };
    let fields = body.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.len() % 2 != 0 {
        return Err(invalid_config_output("incomplete Git config record"));
    }

    let mut effective = BTreeMap::new();
    for record in fields.chunks_exact(2) {
        let origin = parse_utf8_field(record[0], "config origin")?;
        if origin.is_empty() {
            return Err(invalid_config_output("empty Git config origin"));
        }
        let entry = parse_utf8_field(record[1], "config key/value")?;
        let Some((key, value)) = entry.split_once('\n') else {
            return Err(invalid_config_output(
                "Git config record has no key/value separator",
            ));
        };
        if key.is_empty() {
            return Err(invalid_config_output("empty Git config key"));
        }
        let scope = if origin == "command line:" {
            GitConfigScope::Command
        } else {
            // Older Git does not expose the scope. Selection depends on the
            // emitted precedence order, not on this informational label.
            GitConfigScope::Local
        };
        effective.insert(
            key.to_string(),
            GitConfigEntry {
                scope,
                origin: origin.to_string(),
                key: key.to_string(),
                value: value.to_string(),
            },
        );
    }
    Ok(effective)
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
        b"system" => Ok(GitConfigScope::System),
        b"global" => Ok(GitConfigScope::Global),
        b"local" => Ok(GitConfigScope::Local),
        b"worktree" => Ok(GitConfigScope::Worktree),
        b"command" => Ok(GitConfigScope::Command),
        _ => Err(invalid_config_output("unknown Git config scope")),
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
