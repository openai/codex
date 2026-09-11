//! Normalize rendered context after requests have been grouped into windows.
//! Known guidance is rewritten only when requested; volatile values get stable snapshot labels.

use super::ContextSnapshotOptions;
use super::MAX_SNAPSHOT_LINE_CHARS;
use codex_protocol::protocol::APPS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use regex_lite::Regex;
use serde_json::Value;
use std::sync::OnceLock;

const RETAINED_SEGMENT_EDGE_LINES: usize = 16;

// Normalize or replace text in one place, after the structural renderer selects a content item.
// Per-snapshot state gives distinct working directories stable labels without fixture-specific names.
#[derive(Default)]
pub(super) struct Normalizer {
    working_directories: Vec<String>,
    workspace_roots: Vec<String>,
    prompt_cache_keys: Vec<String>,
    uuids: Vec<String>,
}

impl Normalizer {
    pub(super) fn normalize_uuids(&mut self, text: &str) -> String {
        uuid_regex()
            .replace_all(text, |captures: &regex_lite::Captures<'_>| {
                let id = captures[0].to_ascii_lowercase();
                let index = self
                    .uuids
                    .iter()
                    .position(|known| *known == id)
                    .unwrap_or_else(|| {
                        self.uuids.push(id);
                        self.uuids.len() - 1
                    });
                format!("<UUID {}>", index + 1)
            })
            .into_owned()
    }

    pub(super) fn prompt_cache_key(&mut self, key: &str) -> String {
        let index = self
            .prompt_cache_keys
            .iter()
            .position(|known| known == key)
            .unwrap_or_else(|| {
                self.prompt_cache_keys.push(key.to_string());
                self.prompt_cache_keys.len() - 1
            });
        format!("<PROMPT_CACHE_KEY {}>", index + 1)
    }

    pub(super) fn normalize_or_replace(
        &mut self,
        text: &str,
        rewrite_known_segments: bool,
    ) -> String {
        let text = normalize_line_endings(text);
        // Code Mode reports elapsed wall time in an otherwise stable tool output header.
        let text = if text.starts_with("Script completed\nWall time ") {
            static WALL_TIME: OnceLock<Regex> = OnceLock::new();
            WALL_TIME
                .get_or_init(|| {
                    Regex::new(r"(?m)^Wall time [0-9]+(?:\.[0-9]+)? seconds$")
                        .expect("code mode wall time regex")
                })
                .replace(&text, "Wall time <DURATION> seconds")
                .into_owned()
        } else {
            text
        };
        if rewrite_known_segments && text.starts_with("<environment_context>") {
            return routine_context_placeholder(&self.environment(&text))
                .expect("environment context has a placeholder");
        }
        if rewrite_known_segments && let Some(replacement) = routine_context_placeholder(&text) {
            return replacement;
        }
        let text = if text.starts_with("<environment_context>") {
            self.environment(&text)
        } else if !rewrite_known_segments && text.starts_with("# AGENTS.md instructions for ") {
            let (header, body) = text.split_once('\n').unwrap_or((&text, ""));
            if let Some((_, directory)) = header.split_once("for ")
                && (directory.starts_with('/') || directory.as_bytes().get(1) == Some(&b':'))
            {
                format!("# AGENTS.md instructions for <AGENTS_DIRECTORY>\n{body}")
            } else {
                text
            }
        } else {
            text
        };
        normalize_skill_references(&self.normalize_uuids(&text))
    }

    fn environment(&mut self, text: &str) -> String {
        static CWD: OnceLock<Regex> = OnceLock::new();
        static PATH: OnceLock<Regex> = OnceLock::new();
        static HOST: OnceLock<Regex> = OnceLock::new();
        let cwd = CWD.get_or_init(|| Regex::new(r"<cwd>([^<]+)</cwd>").expect("cwd regex"));
        let path = PATH.get_or_init(|| {
            Regex::new(r"(<(cwd|root|path|glob)>)([^<]*)").expect("environment path regex")
        });
        let host = HOST.get_or_init(|| {
            Regex::new(r"(<(shell|shell_version|current_date|timezone)>)[^<]*")
                .expect("host context regex")
        });
        for captures in cwd.captures_iter(text) {
            let directory = captures[1].to_string();
            if !self.working_directories.iter().any(|known| {
                directory.strip_prefix(known).is_some_and(|suffix| {
                    suffix.is_empty() || suffix.starts_with('/') || suffix.starts_with('\\')
                })
            }) {
                self.working_directories.push(directory);
            }
        }
        for captures in path.captures_iter(text) {
            if &captures[2] != "root" {
                continue;
            }
            let root = &captures[3];
            let is_under = |base: &str| {
                root.strip_prefix(base).is_some_and(|suffix| {
                    suffix.is_empty() || suffix.starts_with('/') || suffix.starts_with('\\')
                })
            };
            if (root.starts_with('/') || root.as_bytes().get(1) == Some(&b':'))
                && !self.working_directories.iter().any(|cwd| is_under(cwd))
                && !self.workspace_roots.iter().any(|known| is_under(known))
            {
                self.workspace_roots.push(root.to_string());
            }
        }
        let mut aliases = self
            .working_directories
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let label = if index == 0 {
                    "<CWD>".to_string()
                } else {
                    format!("<CWD {}>", index + 1)
                };
                (path.clone(), label)
            })
            .chain(
                self.workspace_roots
                    .iter()
                    .enumerate()
                    .map(|(index, path)| (path.clone(), format!("<WORKSPACE_ROOT {}>", index + 1))),
            )
            .collect::<Vec<_>>();
        aliases.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
        let text = path.replace_all(text, |captures: &regex_lite::Captures<'_>| {
            let value = &captures[3];
            let replacement = aliases.iter().find_map(|(path, label)| {
                let suffix = value.strip_prefix(path)?;
                if suffix.is_empty() || suffix.starts_with('/') || suffix.starts_with('\\') {
                    Some(format!("{label}{}", suffix.replace('\\', "/")))
                } else {
                    None
                }
            });
            format!(
                "{}{}",
                &captures[1],
                replacement.as_deref().unwrap_or(value)
            )
        });
        host.replace_all(&text, |captures: &regex_lite::Captures<'_>| {
            let label = match &captures[2] {
                "shell" => "HOST_SHELL",
                "shell_version" => "HOST_SHELL_VERSION",
                "current_date" => "CURRENT_DATE",
                "timezone" => "HOST_TIMEZONE",
                _ => unreachable!(),
            };
            format!("{}<{label}>", &captures[1])
        })
        .into_owned()
    }
}

pub(super) fn render_text(
    text: &str,
    options: &ContextSnapshotOptions,
    normalizer: &mut Normalizer,
) -> String {
    let normalized = normalizer.normalize_or_replace(text, options.rewrite_known_segments);
    let mut lines = normalized.split('\n').collect::<Vec<_>>();
    // Trim only when the omitted middle is larger than one retained edge.
    // The hidden middle has a fingerprint; visible edits change only their own lines.
    let marker = (lines.len() > RETAINED_SEGMENT_EDGE_LINES * 3).then(|| {
        let omitted =
            &lines[RETAINED_SEGMENT_EDGE_LINES..lines.len() - RETAINED_SEGMENT_EDGE_LINES];
        let omitted_text = omitted.join("\n");
        format!(
            "<OMITTED {} LINES; ~{} TOKENS; hash={}>",
            omitted.len(),
            omitted_text.chars().count() / 4,
            fingerprint(&Value::String(omitted_text))
        )
    });
    if let Some(marker) = &marker {
        lines.splice(
            RETAINED_SEGMENT_EDGE_LINES..lines.len() - RETAINED_SEGMENT_EDGE_LINES,
            [marker.as_str()],
        );
    }
    lines
        .into_iter()
        .map(|line| {
            shorten(
                line.trim_end(),
                MAX_SNAPSHOT_LINE_CHARS,
                /*keep_tail*/ true,
                /*hash*/ true,
            )
            .replace('\\', "\\\\")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn shorten(text: &str, limit: usize, keep_tail: bool, hash: bool) -> String {
    let len = text.chars().count();
    if len <= limit {
        return text.to_string();
    }
    let budget = limit.saturating_sub(3);
    let head = if keep_tail { budget * 2 / 3 } else { budget };
    let tail = budget - head;
    let mut result = text.chars().take(head).collect::<String>();
    result.push_str("...");
    if tail > 0 {
        result.extend(text.chars().skip(len - tail));
    }
    if hash {
        result.push_str(&format!(
            " [hash={}]",
            fingerprint(&Value::String(text.to_string()))
        ));
    }
    result
}

fn routine_context_placeholder(text: &str) -> Option<String> {
    let replacement = if text.starts_with("<permissions instructions>") {
        "<PERMISSIONS_INSTRUCTIONS>"
    } else if text.starts_with(APPS_INSTRUCTIONS_OPEN_TAG) {
        "<APPS_INSTRUCTIONS>"
    } else if text.starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG) {
        "<SKILLS_INSTRUCTIONS>"
    } else if text.starts_with(PLUGINS_INSTRUCTIONS_OPEN_TAG) {
        "<PLUGINS_INSTRUCTIONS>"
    } else if text.starts_with("# AGENTS.md instructions") {
        "<AGENTS_MD>"
    } else if let Some(body) = text.strip_prefix("<model_switch>") {
        return Some(guidance_with_intro("<MODEL_SWITCH>", body));
    } else if let Some(body) = text.strip_prefix("<personality_spec>") {
        return Some(guidance_with_intro("<PERSONALITY_SPEC>", body));
    } else if text.starts_with("You are judging one planned coding-agent action.") {
        return Some(format!(
            "<GUARDIAN_INSTRUCTIONS:hash={}>",
            fingerprint(&Value::String(text.to_string()))
        ));
    } else if text.starts_with("You are performing a CONTEXT CHECKPOINT COMPACTION.") {
        "<SUMMARIZATION_PROMPT>"
    } else if text.starts_with("<environment_context>") {
        let count = text
            .split_once("<subagents>")
            .and_then(|(_, rest)| rest.split_once("</subagents>"))
            .map(|(agents, _)| {
                agents
                    .lines()
                    .filter(|line| line.trim_start().starts_with("- "))
                    .count()
            })
            .unwrap_or(0);
        let cwd = text
            .split_once("<cwd>")
            .and_then(|(_, rest)| rest.split_once("</cwd>"))
            .map(|(directory, _)| format!(":cwd={directory}"))
            .unwrap_or_default();
        return Some(if count == 0 {
            format!("<ENVIRONMENT_CONTEXT{cwd}>")
        } else {
            format!("<ENVIRONMENT_CONTEXT{cwd}:subagents={count}>")
        });
    } else if text.starts_with("Another language model started to solve this problem") {
        return Some(text.split_once('\n').map_or_else(
            || "<COMPACTION_SUMMARY>".to_string(),
            |(_, body)| format!("<COMPACTION_SUMMARY>\n{body}"),
        ));
    } else {
        return None;
    };
    Some(replacement.to_string())
}

fn guidance_with_intro(marker: &str, body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map_or_else(|| marker.to_string(), |intro| format!("{marker}\n{intro}"))
}

fn normalize_skill_references(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            let line = if let Some((before, after)) = line.split_once("<path>")
                && let Some((path, rest)) = after.split_once("</path>")
            {
                format!("{before}<path>{}</path>{rest}", normalize_skill_path(path))
            } else if let Some((before, path)) = line.split_once(" = `")
                && before.starts_with("- `r")
                && let Some(path) = path.strip_suffix('`')
            {
                format!("{before} = `{}`", normalize_skill_path(path))
            } else if let Some((before, path)) = line.split_once("(file: ")
                && let Some(path) = path.strip_suffix(')')
            {
                format!("{before}(file: {})", normalize_skill_path(path))
            } else {
                line.to_string()
            };
            normalize_stable_text(&line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_skill_path(path: &str) -> String {
    if !path.starts_with('/') && !path.starts_with('\\') && path.as_bytes().get(1) != Some(&b':') {
        return path.to_string();
    }
    let path = path.replace('\\', "/");
    if let Some((_, rest)) = path.rsplit_once("/plugins/cache/") {
        format!("<PLUGINS_CACHE>/{rest}")
    } else if let Some((_, rest)) = path.rsplit_once("/.agents/skills/") {
        format!("<PROJECT_SKILLS>/{rest}")
    } else if let Some((_, rest)) = path.rsplit_once("/skills/") {
        format!("<SKILLS_ROOT>/{rest}")
    } else if path.ends_with("/skills") {
        "<SKILLS_ROOT>".to_string()
    } else {
        path
    }
}

pub(super) fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(super) fn normalize_stable_text(text: &str) -> String {
    static SYSTEM_SKILL_PATH: OnceLock<Regex> = OnceLock::new();
    static TURN_TIME: OnceLock<Regex> = OnceLock::new();
    static SANDBOX: OnceLock<Regex> = OnceLock::new();
    let skill = SYSTEM_SKILL_PATH.get_or_init(|| {
        Regex::new(r"/[^)\n]*/skills/\.system/([^/\n]+)/SKILL\.md")
            .expect("system skill path regex")
    });
    let time = TURN_TIME
        .get_or_init(|| Regex::new(r#""turn_started_at_unix_ms":\d+"#).expect("turn time regex"));
    let sandbox =
        SANDBOX.get_or_init(|| Regex::new(r#""sandbox":"[^"]+""#).expect("sandbox regex"));
    let text = normalize_line_endings(text);
    let text = skill.replace_all(&text, "<SYSTEM_SKILLS_ROOT>/$1/SKILL.md");
    let text = uuid_regex().replace_all(&text, "<UUID>");
    let text = time.replace_all(&text, r#""turn_started_at_unix_ms":<UNIX_MS>"#);
    sandbox
        .replace_all(&text, r#""sandbox":"<SANDBOX>""#)
        .into_owned()
}

fn uuid_regex() -> &'static Regex {
    static UUID: OnceLock<Regex> = OnceLock::new();
    UUID.get_or_init(|| {
        Regex::new(
            r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
        )
        .expect("uuid regex")
    })
}

// Hash complete normalized values whenever the readable rendering clips content.
pub(super) fn fingerprint(value: &Value) -> String {
    let mut normalized = value.clone();
    normalize_json(&mut normalized, &mut normalize_stable_text);
    let bytes = serde_json::to_vec(&normalized).expect("snapshot value should serialize");
    format!("{:016x}", fnv1a(&bytes))
}

pub(super) fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub(super) fn normalize_json(value: &mut Value, normalize: &mut impl FnMut(&str) -> String) {
    match value {
        Value::String(text) => *text = normalize(text),
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| normalize_json(value, normalize)),
        Value::Object(map) => {
            let mut entries = std::mem::take(map).into_iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (key, mut value) in entries {
                normalize_json(&mut value, normalize);
                map.insert(key, value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}
