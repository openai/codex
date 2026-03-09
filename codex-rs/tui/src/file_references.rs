use crate::exec_command::relativize_to_home;
use codex_core::git_info::get_git_repo_root;
use codex_utils_string::normalize_markdown_hash_location_suffix;
use regex_lite::Regex;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

static COLON_LOCATION_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r":(?P<line>\d+)(?::(?P<col>\d+))?(?:[-–]\d+(?::\d+)?)?$")
        .unwrap_or_else(|error| panic!("invalid location suffix regex: {error}"))
});

static HASH_LOCATION_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#L(?P<line>\d+)(?:C(?P<col>\d+))?(?:-L\d+(?:C\d+)?)?$")
        .unwrap_or_else(|error| panic!("invalid hash location regex: {error}"))
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParsedFileReference {
    pub(crate) resolved_path: PathBuf,
    pub(crate) display_path: String,
    pub(crate) location_suffix: String,
    pub(crate) line: Option<usize>,
    pub(crate) col: Option<usize>,
}

impl ParsedFileReference {
    pub(crate) fn display_text(&self) -> String {
        format!("{}{}", self.display_path, self.location_suffix)
    }
}

pub(crate) fn parse_local_file_reference_token(
    token: &str,
    cwd: &Path,
) -> Option<ParsedFileReference> {
    let (path_text, line, col, location_suffix) = strip_location_suffix(token);
    let resolved_path = resolve_existing_local_path(path_text, cwd)?;
    if !looks_like_file_reference(path_text, &resolved_path, cwd) {
        return None;
    }

    Some(ParsedFileReference {
        display_path: display_path_for(&resolved_path, cwd),
        resolved_path,
        location_suffix,
        line,
        col,
    })
}

pub(crate) fn extract_local_path_location_suffix(dest_url: &str) -> Option<String> {
    if !is_local_path_like_link(dest_url) {
        return None;
    }

    normalized_location_suffix(dest_url)
}

pub(crate) fn text_has_location_suffix(text: &str) -> bool {
    normalized_location_suffix(text).is_some()
}

pub(crate) fn is_local_path_like_link(dest_url: &str) -> bool {
    dest_url.starts_with("file://")
        || dest_url.starts_with('/')
        || dest_url.starts_with("~/")
        || dest_url.starts_with("./")
        || dest_url.starts_with("../")
        || dest_url.starts_with("\\\\")
        || matches!(
            dest_url.as_bytes(),
            [drive, b':', separator, ..]
                if drive.is_ascii_alphabetic() && matches!(separator, b'/' | b'\\')
        )
}

pub(crate) fn display_path_for(path: &Path, cwd: &Path) -> String {
    let rendered = if path.is_relative() {
        path.display().to_string()
    } else if let Ok(stripped) = path.strip_prefix(cwd) {
        stripped.display().to_string()
    } else {
        let path_in_same_repo = match (get_git_repo_root(cwd), get_git_repo_root(path)) {
            (Some(cwd_repo), Some(path_repo)) => cwd_repo == path_repo,
            _ => false,
        };
        let chosen = if path_in_same_repo {
            pathdiff::diff_paths(path, cwd).unwrap_or_else(|| path.to_path_buf())
        } else {
            relativize_to_home(path)
                .map(|relative| PathBuf::from_iter([Path::new("~"), relative.as_path()]))
                .unwrap_or_else(|| path.to_path_buf())
        };
        chosen.display().to_string()
    };

    collapse_middle_directories(rendered)
}

fn normalized_location_suffix(text: &str) -> Option<String> {
    if let Some(captures) = HASH_LOCATION_SUFFIX_RE.captures(text)
        && let Some(full) = captures.get(0)
    {
        return normalize_markdown_hash_location_suffix(full.as_str());
    }

    COLON_LOCATION_SUFFIX_RE
        .captures(text)
        .and_then(|captures| captures.get(0).map(|suffix| suffix.as_str().to_string()))
}

fn strip_location_suffix(token: &str) -> (&str, Option<usize>, Option<usize>, String) {
    if let Some(captures) = HASH_LOCATION_SUFFIX_RE.captures(token)
        && let Some(full) = captures.get(0)
    {
        let line = captures
            .name("line")
            .and_then(|value| value.as_str().parse::<usize>().ok());
        let col = captures
            .name("col")
            .and_then(|value| value.as_str().parse::<usize>().ok());
        let suffix = normalize_markdown_hash_location_suffix(full.as_str()).unwrap_or_default();
        return (&token[..full.start()], line, col, suffix);
    }

    if let Some(captures) = COLON_LOCATION_SUFFIX_RE.captures(token)
        && let Some(full) = captures.get(0)
    {
        let line = captures
            .name("line")
            .and_then(|value| value.as_str().parse::<usize>().ok());
        let col = captures
            .name("col")
            .and_then(|value| value.as_str().parse::<usize>().ok());
        return (&token[..full.start()], line, col, full.as_str().to_string());
    }

    (token, None, None, String::new())
}

fn looks_like_file_reference(token: &str, resolved_path: &Path, cwd: &Path) -> bool {
    token.starts_with("~/")
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with('/')
        || token.starts_with("\\\\")
        || token.contains('/')
        || token.contains('\\')
        || matches!(
            token.as_bytes(),
            [drive, b':', separator, ..]
                if drive.is_ascii_alphabetic() && matches!(separator, b'/' | b'\\')
        )
        || is_cwd_bare_filename_reference(token, resolved_path, cwd)
}

fn is_cwd_bare_filename_reference(token: &str, resolved_path: &Path, cwd: &Path) -> bool {
    if token.is_empty()
        || token.contains(['/', '\\'])
        || token.starts_with("~/")
        || token.starts_with("./")
        || token.starts_with("../")
        || token.contains(':')
        || token.chars().any(char::is_whitespace)
        || !token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return false;
    }

    resolved_path.parent() == Some(cwd) && resolved_path.file_name() == Some(OsStr::new(token))
}

fn resolve_local_path(token: &str, cwd: &Path) -> PathBuf {
    if let Some(path) = token.strip_prefix("~/") {
        return dirs::home_dir()
            .map(|home| home.join(path))
            .unwrap_or_else(|| PathBuf::from(token));
    }

    let path = PathBuf::from(token);
    if path.is_absolute() {
        return path;
    }

    cwd.join(path)
}

fn resolve_existing_local_path(token: &str, cwd: &Path) -> Option<PathBuf> {
    let path = resolve_local_path(token, cwd);
    path.exists().then_some(path)
}

fn collapse_middle_directories(path: String) -> String {
    let separator = if path.contains('\\') { '\\' } else { '/' };
    let starts_with_separator = path.starts_with(separator);
    let mut parts: Vec<&str> = path
        .split(separator)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() <= 3 || path.len() <= 36 {
        return path;
    }

    let prefix = if starts_with_separator {
        separator.to_string()
    } else {
        String::new()
    };

    let keep_last = if path.len() > 52 { 1 } else { 2 };
    if parts.len() <= keep_last + 1 {
        return path;
    }

    let tail_start = parts.len() - keep_last;
    let tail = parts.split_off(tail_start);
    let mut collapsed = format!("{prefix}{}", parts[0]);
    collapsed.push(separator);
    collapsed.push_str("...");
    for part in tail {
        collapsed.push(separator);
        collapsed.push_str(part);
    }

    if collapsed.len() < path.len() {
        collapsed
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_local_file_reference_supports_bare_filenames() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, "[package]\nname = \"demo\"\n").expect("write file");

        let reference =
            parse_local_file_reference_token("Cargo.toml:7", dir.path()).expect("file ref");

        assert_eq!(reference.resolved_path, path);
        assert_eq!(reference.display_text(), "Cargo.toml:7");
        assert_eq!(reference.line, Some(7));
        assert_eq!(reference.col, None);
    }

    #[test]
    fn extract_local_path_location_suffix_normalizes_hash_locations() {
        let suffix = extract_local_path_location_suffix("file:///tmp/src/lib.rs#L74C3-L76C9");
        assert_eq!(suffix.as_deref(), Some(":74:3-76:9"));
    }

    #[test]
    fn display_path_collapses_long_workspace_paths() {
        let cwd = Path::new("/workspace");
        let path = cwd.join("codex-rs/tui/src/bottom_pane/chat_composer.rs");

        let rendered = display_path_for(&path, cwd);

        assert_eq!(rendered, "codex-rs/.../bottom_pane/chat_composer.rs");
    }

    #[test]
    fn display_path_keeps_short_workspace_paths() {
        let cwd = Path::new("/workspace");
        let path = cwd.join("tui/example.png");

        let rendered = display_path_for(&path, cwd);

        assert_eq!(rendered, "tui/example.png");
    }
}
