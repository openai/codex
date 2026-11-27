//! Local file-link destination parsing and display for Markdown transcripts.
//!
//! Local Markdown links preserve their labels. Since ordinary file destinations have no trusted
//! navigation capability, the renderer also displays their target, shortening absolute paths
//! relative to the session working directory when possible. This module only formats destinations;
//! it does not resolve files or infer execution-host paths from the frontend filesystem.

use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use codex_utils_string::normalize_markdown_hash_location_suffix;
use regex_lite::Regex;
use std::path::Path;
use std::sync::LazyLock;
use url::Url;

static COLON_LOCATION_SUFFIX_RE: LazyLock<Regex> =
    LazyLock::new(
        || match Regex::new(r":\d+(?::\d+)?(?:[-–]\d+(?::\d+)?)?$") {
            Ok(regex) => regex,
            Err(error) => panic!("invalid location suffix regex: {error}"),
        },
    );

static HASH_LOCATION_SUFFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| match Regex::new(r"^L\d+(?:C\d+)?(?:-L\d+(?:C\d+)?)?$") {
        Ok(regex) => regex,
        Err(error) => panic!("invalid hash location regex: {error}"),
    });

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SourcePoint {
    pub(crate) line: usize,
    pub(crate) column: Option<usize>,
}

/// One-based source coordinates. An end point denotes an inclusive range of lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SourceLocation {
    pub(crate) start: SourcePoint,
    pub(crate) end: Option<SourcePoint>,
}

impl SourceLocation {
    pub(crate) fn single(line: usize, column: Option<usize>) -> Self {
        Self {
            // Single locations historically treat zero coordinates as one.
            start: SourcePoint {
                line: line.max(1),
                column: column.map(|n| n.max(1)),
            },
            end: None,
        }
    }

    // The suffix matchers validate the shape and normalize both spellings to colon form.
    fn parse(suffix: &str) -> Option<Self> {
        let mut points = suffix.strip_prefix(':')?.split(['-', '–']);
        let parse_point = |point: &str| {
            let mut fields = point.split(':');
            Some(SourcePoint {
                line: fields.next()?.parse().ok()?,
                column: fields.next().map(str::parse).transpose().ok()?,
            })
        };
        let start = parse_point(points.next()?)?;
        let Some(end) = points.next() else {
            return Some(Self::single(start.line, start.column));
        };
        let end = parse_point(end)?;
        // Missing columns span from the start of the first line to the end of the last.
        let first = (start.line, start.column.unwrap_or(1));
        let last = (end.line, end.column.unwrap_or(usize::MAX));
        if start.line == 0 || start.column == Some(0) || end.column == Some(0) || first > last {
            return None;
        }
        Some(Self {
            start,
            end: Some(end),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FileLink {
    pub(crate) path: String,
    pub(crate) location: Option<SourceLocation>,
}

impl FileLink {
    pub(crate) fn is_image(&self) -> bool {
        self.location.is_none() && crate::sidebar_image::supports_path(&self.path)
    }
}

pub(crate) fn is_markdown_file(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| {
        ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown")
    })
}

pub(super) fn local_image_path(
    destination: &str,
    cwd: Option<&Path>,
) -> Option<std::path::PathBuf> {
    let link = sidebar_file_link(destination)?;
    let path = Path::new(&link.path);
    if link.location.is_some() || is_absolute_local_link_path(&link.path) != path.is_absolute() {
        return None;
    }
    let resolved = cwd.unwrap_or_else(|| Path::new("")).join(path);
    #[cfg(windows)]
    if !matches!(resolved.components().next(), Some(std::path::Component::Prefix(p))
        if matches!(p.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
    {
        return None;
    }
    resolved.is_absolute().then_some(resolved)
}

pub(crate) fn sidebar_file_link(destination: &str) -> Option<FileLink> {
    if destination.len() > 8192 || destination.contains('?') {
        return None;
    }
    if destination.starts_with("file://") && Url::parse(destination).ok()?.has_host() {
        return None;
    }
    let (path, location) = parse_local_link_target(destination, /*for_navigation*/ true)?;
    if destination.contains('#') && location.is_none() {
        return None;
    }
    let location = match location {
        Some(suffix) => Some(SourceLocation::parse(&suffix)?),
        None => None,
    };
    (!path.is_empty()
        && !path.ends_with('/')
        && path.len() <= 4096
        && !path.starts_with('~')
        && !path.starts_with("//")
        && !path.chars().any(char::is_control)
        && (is_absolute_local_link_path(&path) || Url::parse(&path).is_err()))
    .then_some(FileLink { path, location })
}

/// Recognize complete, path-shaped range tokens in prose. Explicit Markdown links also
/// support spaces and extensionless filenames. Never infer a file from a bare number or URL.
pub(super) fn bare_file_ranges<'a>(
    text: &'a str,
    prefix: &str,
) -> impl Iterator<Item = (std::range::Range<usize>, FileLink)> + 'a {
    // A formatting boundary may split a path or URL (e.g. src/__tests__/file.ts).
    // Only start a link on a complete token, including one enclosed in emphasis.
    let prefix = prefix.strip_suffix('\\').unwrap_or(prefix);
    let at_boundary = prefix
        .trim_end_matches(['*', '_', '~'])
        .chars()
        .next_back()
        .is_none_or(|ch| {
            ch.is_whitespace() || matches!(ch, '(' | '[' | '<' | '|' | '>' | '"' | '\'')
        });
    text.split_whitespace().filter_map(move |token| {
        if !at_boundary && text.substr_range(token)?.start == 0 {
            return None;
        }
        let token =
            token.trim_end_matches([')', ']', '>', '"', '\'', '.', ',', ';', ':', '!', '?']);
        if !token.ends_with(|ch: char| ch.is_ascii_digit())
            || !token.contains(['-', '–'])
            || !token.contains([':', '#'])
        {
            return None;
        }
        // Strip unmatched prose wrappers, preserving balanced [id] and (group) directories.
        let mut paren = token
            .matches('(')
            .count()
            .saturating_sub(token.matches(')').count());
        let mut bracket = token
            .matches('[')
            .count()
            .saturating_sub(token.matches(']').count());
        let candidate = token.trim_start_matches(|ch| {
            let remaining = match ch {
                '(' => &mut paren,
                '[' => &mut bracket,
                _ => return matches!(ch, '<' | '"' | '\''),
            };
            let trim = *remaining > 0;
            *remaining = remaining.saturating_sub(1);
            trim
        });
        let link = sidebar_file_link(candidate)?;
        if link.location?.end.is_none()
            || link.path.contains("://")
            || !link.path.contains(['.', '/', '\\'])
        {
            return None;
        }
        Some((text.substr_range(candidate)?, link))
    })
}

pub(super) fn is_local_path_like_link(dest_url: &str) -> bool {
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

/// Parse a local link target into normalized path text plus an optional location suffix.
///
/// This accepts the path shapes Codex emits today: `file://` URLs, absolute and relative paths,
/// `~/...`, Windows paths, and `#L..C..` or `:line:col` suffixes.
pub(super) fn render_local_link_target(dest_url: &str, cwd: Option<&Path>) -> Option<String> {
    let (path_text, location_suffix) =
        parse_local_link_target(dest_url, /*for_navigation*/ false)?;
    let mut rendered = display_local_link_path(&path_text, cwd);
    if let Some(location_suffix) = location_suffix {
        rendered.push_str(&location_suffix);
    }
    Some(rendered)
}

/// Split a local-link destination into `(normalized_path_text, location_suffix)`.
///
/// The returned path text never includes a trailing `#L..` or `:line[:col]` suffix. Path
/// normalization preserves home-relative paths and uses forward slashes as separators.
/// A backslash in a POSIX file URI is part of the filename and is preserved.
///
/// Navigation rejects partial ranges before decoding the filename.
fn parse_local_link_target(
    dest_url: &str,
    for_navigation: bool,
) -> Option<(String, Option<String>)> {
    if dest_url.starts_with("file://") {
        let url = Url::parse(dest_url).ok()?;
        // Infer the path's OS from the URI, not the host running the TUI.
        let path = PathUri::parse(&url[..url::Position::AfterPath]).ok()?;
        let native_path = path.inferred_native_path_string();
        let path_text = if path.infer_path_convention() == Some(PathConvention::Posix) {
            native_path
        } else {
            normalize_local_link_path_text(&native_path)
        };
        let location_suffix = url
            .fragment()
            .and_then(normalize_hash_location_suffix_fragment);
        return Some((path_text, location_suffix));
    }

    let mut path_text = dest_url;
    let mut location_suffix = None;
    // Prefer `#L..` style fragments when both forms are present so URLs like `path#L10` do not
    // get misparsed as a plain path ending in `:10`.
    if let Some((candidate_path, fragment)) = dest_url.rsplit_once('#')
        && let Some(normalized) = normalize_hash_location_suffix_fragment(fragment)
    {
        path_text = candidate_path;
        location_suffix = Some(normalized);
    }
    // A valid hash location disambiguates colons and dashes in POSIX filenames.
    if location_suffix.is_none() {
        if let Some(suffix) = extract_colon_location_suffix(path_text) {
            let path_len = path_text.len().saturating_sub(suffix.len());
            path_text = &path_text[..path_len];
            location_suffix = Some(suffix);
        }
        if for_navigation && has_unparsed_range(path_text) {
            return None;
        }
    }
    let decoded_path_text =
        urlencoding::decode(path_text).unwrap_or(std::borrow::Cow::Borrowed(path_text));
    Some((
        normalize_local_link_path_text(&decoded_path_text),
        location_suffix,
    ))
}

// After removing a valid location, a remaining point followed by a dash
// signals a malformed or extra range. Ignore directories and filename stems.
fn has_unparsed_range(path: &str) -> bool {
    let tail = path.rsplit(['.', '/', '\\']).next().unwrap_or(path);
    let tail = &tail[tail.rfind('#').unwrap_or(0)..];
    tail.rsplit(['-', '–']).skip(1).any(|point| {
        COLON_LOCATION_SUFFIX_RE.is_match(point)
            || point
                .strip_prefix('#')
                .is_some_and(|fragment| HASH_LOCATION_SUFFIX_RE.is_match(fragment))
    })
}

/// Normalize a hash fragment like `L12` or `L12C3-L14C9` into the display suffix we render.
///
/// Returns `None` for fragments that are not location references. This deliberately ignores other
/// `#...` fragments so non-location hashes stay part of the path text.
fn normalize_hash_location_suffix_fragment(fragment: &str) -> Option<String> {
    HASH_LOCATION_SUFFIX_RE
        .is_match(fragment)
        .then(|| format!("#{fragment}"))
        .and_then(|suffix| normalize_markdown_hash_location_suffix(&suffix))
}

/// Extract a trailing `:line`, `:line:col`, or range suffix from a plain path-like string.
///
/// The suffix must occur at the end of the input; embedded colons elsewhere in the path are left
/// alone. This is what keeps Windows drive letters like `C:/...` from being misread as locations.
pub(super) fn extract_colon_location_suffix(path_text: &str) -> Option<String> {
    COLON_LOCATION_SUFFIX_RE
        .find(path_text)
        .filter(|matched| matched.end() == path_text.len())
        .map(|matched| matched.as_str().to_string())
}

/// Normalize local-path text into the transcript display form.
///
/// Display normalization is intentionally lexical: it does not touch the filesystem, resolve
/// symlinks, or collapse `.` / `..`. It only converts separators to forward slashes and rewrites
/// UNC-style `\\\\server\\share` inputs into `//server/share` so later prefix checks operate on a
/// stable representation.
fn normalize_local_link_path_text(path_text: &str) -> String {
    // Render all local link paths with forward slashes so display and prefix stripping are stable
    // across mixed Windows and Unix-style inputs.
    if let Some(rest) = path_text.strip_prefix("\\\\") {
        format!("//{}", rest.replace('\\', "/").trim_start_matches('/'))
    } else {
        path_text.replace('\\', "/")
    }
}

fn is_absolute_local_link_path(path_text: &str) -> bool {
    path_text.starts_with('/')
        || path_text.starts_with("//")
        || matches!(
            path_text.as_bytes(),
            [drive, b':', b'/', ..] if drive.is_ascii_alphabetic()
        )
}

/// Remove trailing separators from a local path without destroying root semantics.
///
/// Roots like `/`, `//`, and `C:/` stay intact so callers can still distinguish "the root itself"
/// from "a path under the root".
fn trim_trailing_local_path_separator(path_text: &str) -> &str {
    if path_text == "/" || path_text == "//" {
        return path_text;
    }
    if matches!(path_text.as_bytes(), [drive, b':', b'/'] if drive.is_ascii_alphabetic()) {
        return path_text;
    }
    path_text.trim_end_matches('/')
}

/// Strip `cwd_text` from the start of `path_text` when `path_text` is strictly underneath it.
///
/// Returns the relative remainder without a leading slash. If the path equals the cwd exactly, this
/// returns `None` so callers can keep rendering the full path instead of collapsing it to an empty
/// string.
fn strip_local_path_prefix<'a>(path_text: &'a str, cwd_text: &str) -> Option<&'a str> {
    let path_text = trim_trailing_local_path_separator(path_text);
    let cwd_text = trim_trailing_local_path_separator(cwd_text);
    if path_text == cwd_text {
        return None;
    }

    // Treat filesystem roots specially so `/tmp/x` under `/` becomes `tmp/x` instead of being
    // left unchanged by the generic prefix-stripping branch.
    if cwd_text == "/" || cwd_text == "//" {
        return path_text.strip_prefix('/');
    }

    path_text
        .strip_prefix(cwd_text)
        .and_then(|rest| rest.strip_prefix('/'))
}

/// Choose the visible path text for a local link after normalization.
///
/// Relative paths (including `~/...`) stay relative. Absolute paths prefer cwd-relative display
/// and otherwise stay absolute: the frontend home may differ from the execution host's home.
/// This is display logic only, not filesystem canonicalization.
fn display_local_link_path(path_text: &str, cwd: Option<&Path>) -> String {
    let path_text = normalize_local_link_path_text(path_text);
    if !is_absolute_local_link_path(&path_text) {
        return path_text;
    }

    if let Some(cwd) = cwd {
        // Only the session cwd is known to refer to the execution host.
        let cwd_text = normalize_local_link_path_text(&cwd.to_string_lossy());
        if let Some(stripped) = strip_local_path_prefix(&path_text, &cwd_text) {
            return stripped.to_string();
        }
    }

    path_text
}

#[cfg(test)]
#[path = "local_links_tests.rs"]
mod tests;
