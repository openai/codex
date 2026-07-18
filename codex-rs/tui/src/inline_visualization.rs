//! Terminal fallback for assistant-authored inline visualization directives.

mod viewer;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::DateTime;
use codex_protocol::ThreadId;
use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use rand::RngCore as _;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use url::Url;
use uuid::Uuid;

use self::viewer::materialize_document;

pub(crate) const DIRECTIVE_PREFIX: &str = "::codex-inline-vis{";
const MAX_FRAGMENT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct InlineVisualizationContext {
    visualizations_dir: PathBuf,
    thread_dir: PathBuf,
}

impl InlineVisualizationContext {
    pub(crate) fn new(codex_home: &Path, thread_id: ThreadId) -> Option<Self> {
        Self::new_with_writable_roots(codex_home, thread_id, std::iter::empty())
    }

    pub(crate) fn from_config(
        config: &crate::legacy_core::config::Config,
        thread_id: ThreadId,
    ) -> Option<Self> {
        let writable_roots = config
            .permissions
            .file_system_sandbox_policy()
            .get_writable_roots_with_cwd(config.cwd.as_path());
        Self::new_with_writable_roots(
            config.codex_home.as_path(),
            thread_id,
            writable_roots.iter().map(|root| root.root.as_path()),
        )
    }

    fn new_with_writable_roots<'a>(
        codex_home: &Path,
        thread_id: ThreadId,
        writable_roots: impl IntoIterator<Item = &'a Path>,
    ) -> Option<Self> {
        let thread_id = thread_id.to_string();
        let uuid = Uuid::parse_str(&thread_id).ok()?;
        let timestamp = uuid.get_timestamp()?;
        let (seconds, nanos) = timestamp.to_unix();
        let created_at = DateTime::from_timestamp(i64::try_from(seconds).ok()?, nanos)?;
        let visualizations_dir = codex_home.join("visualizations");
        let granted_thread_dirs = writable_roots
            .into_iter()
            .filter(|root| is_visualization_thread_dir(&visualizations_dir, root))
            .collect::<Vec<_>>();
        let thread_dir = match granted_thread_dirs.as_slice() {
            [thread_dir] => (*thread_dir).to_path_buf(),
            _ => visualizations_dir
                .join(created_at.format("%Y/%m/%d").to_string())
                .join(thread_id),
        };
        Some(Self {
            visualizations_dir,
            thread_dir,
        })
    }

    fn link_for(&self, file: &str) -> Option<Url> {
        let relative = Path::new(file);
        if relative
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("html")
            || !matches!(
                relative.components().collect::<Vec<_>>().as_slice(),
                [Component::Normal(_)]
            )
        {
            return None;
        }

        let visualizations_dir = fs::canonicalize(&self.visualizations_dir).ok()?;
        let thread_dir = fs::canonicalize(&self.thread_dir).ok()?;
        if !thread_dir.starts_with(&visualizations_dir) {
            return None;
        }
        let fragment_path = fs::canonicalize(thread_dir.join(relative)).ok()?;
        if !fragment_path.starts_with(&thread_dir) {
            return None;
        }
        let viewer_path = materialize_document(&fragment_path, &thread_dir).ok()?;
        Url::from_file_path(viewer_path).ok()
    }
}

fn is_visualization_thread_dir(visualizations_dir: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(visualizations_dir) else {
        return false;
    };
    let components = relative.components().collect::<Vec<_>>();
    matches!(
        components.as_slice(),
        [
            Component::Normal(_),
            Component::Normal(_),
            Component::Normal(_),
            Component::Normal(thread_id)
        ] if Uuid::parse_str(&thread_id.to_string_lossy()).is_ok()
    )
}

pub(crate) struct InlineVisualizationRewrite<'a> {
    pub(crate) markdown: Cow<'a, str>,
    // Markdown rendering only recognizes web links. Random placeholders let the renderer build the
    // link ranges normally, then allow the caller to retarget only links created from directives.
    pub(crate) trusted_file_links: HashMap<String, TrustedFileLink>,
}

pub(crate) struct TrustedFileLink {
    pub(crate) destination: Url,
    pub(crate) markdown_label: String,
    pub(crate) display_label: String,
    pub(crate) markdown_destination_label: String,
}

pub(crate) fn rewrite_inline_visualizations<'a>(
    markdown: &'a str,
    context: Option<&InlineVisualizationContext>,
) -> InlineVisualizationRewrite<'a> {
    if !markdown.contains(DIRECTIVE_PREFIX) {
        return InlineVisualizationRewrite {
            markdown: Cow::Borrowed(markdown),
            trusted_file_links: HashMap::new(),
        };
    }

    let mut code_block_ranges = Vec::<Range<usize>>::new();
    let mut code_block_start = None;
    for (event, range) in Parser::new_ext(markdown, Options::empty()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code_block_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = code_block_start.take() {
                    code_block_ranges.push(start..range.end);
                }
            }
            _ => {}
        }
    }
    if let Some(start) = code_block_start {
        code_block_ranges.push(start..markdown.len());
    }

    let mut rewritten = String::with_capacity(markdown.len());
    let mut trusted_file_links = HashMap::new();
    let mut source_offset = 0;
    for source_line in markdown.split_inclusive('\n') {
        let line_start = source_offset;
        source_offset += source_line.len();
        let (line, newline) = source_line
            .strip_suffix('\n')
            .map_or((source_line, ""), |line| (line, "\n"));
        let trimmed = line.trim();
        let is_code = code_block_ranges
            .iter()
            .any(|range| range.start < source_offset && line_start < range.end);
        if is_code || !trimmed.starts_with(DIRECTIVE_PREFIX) {
            rewritten.push_str(line);
        } else if let Some(file) = parse_directive_file(trimmed) {
            if let Some(destination) = context.and_then(|context| context.link_for(file)) {
                let placeholder = link_placeholder();
                let (markdown_label, display_label) = visualization_link_labels(file);
                let markdown_destination_label = escape_markdown_label(destination.as_str());
                rewritten.push_str(&format!(
                    "{markdown_label}  \n[{markdown_destination_label}]({placeholder})"
                ));
                trusted_file_links.insert(
                    placeholder,
                    TrustedFileLink {
                        destination,
                        markdown_label,
                        display_label,
                        markdown_destination_label,
                    },
                );
            } else {
                rewritten.push_str("_Visualization unavailable on this device._");
            }
        } else if trimmed.ends_with('}') {
            rewritten.push_str("_Visualization unavailable on this device._");
        }
        rewritten.push_str(newline);
    }
    InlineVisualizationRewrite {
        markdown: Cow::Owned(rewritten),
        trusted_file_links,
    }
}

fn visualization_link_labels(file: &str) -> (String, String) {
    let name = Path::new(file)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("generated");
    let escaped_name = escape_markdown_label(name);
    (
        format!("Open {escaped_name} visualization in the browser"),
        format!("Open {name} visualization in the browser"),
    )
}

fn escape_markdown_label(label: &str) -> String {
    let mut escaped = String::with_capacity(label.len());
    for character in label.chars() {
        if character.is_ascii_punctuation() {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn link_placeholder() -> String {
    let mut bytes = [0_u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    let token = URL_SAFE_NO_PAD.encode(bytes);
    format!("https://codex.invalid/inline-visualization/{token}")
}

fn parse_directive_file(directive: &str) -> Option<&str> {
    let attributes = directive
        .strip_prefix(DIRECTIVE_PREFIX)?
        .strip_suffix('}')?
        .trim();
    let value = attributes.strip_prefix("file=\"")?.strip_suffix('"')?;
    (!value.is_empty() && !value.contains('"')).then_some(value)
}

#[cfg(test)]
#[path = "inline_visualization_tests.rs"]
mod tests;
