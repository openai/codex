//! All display normalization for context snapshots: known text, volatile values and tool schemas.
//! Request grouping stays independent of these display rules.

use super::ContextSnapshotOptions;
use regex_lite::Regex;
use serde_json::Value;
use std::sync::OnceLock;

const GUARDIAN_INSTRUCTIONS_PREFIX: &str = "You are judging one planned coding-agent action.";

#[derive(Clone, Copy)]
pub(super) enum TextSource<'a> {
    ModelInstructions,
    Message(&'a str),
    FunctionArguments,
    Other,
}

// Normalize or replace text in one place, after the structural renderer selects a content item.
// Per-snapshot state gives distinct working directories stable labels without fixture-specific names.
#[derive(Default)]
pub(super) struct Normalizer {
    working_directories: Vec<String>,
    workspace_roots: Vec<String>,
    temporary_directories: Vec<String>,
    prompt_cache_keys: Vec<String>,
    uuids: Vec<String>,
}

impl Normalizer {
    pub(super) fn observe_request(&mut self, input: &[Value]) {
        // Permissions precede environment context, but can cite the same paths.
        for item in input.iter().filter(|item| item["role"] == "user") {
            for part in item["content"].as_array().into_iter().flatten() {
                if let Some(text) = part["text"].as_str()
                    && guidance_tag(text) == Some("environment_context")
                {
                    self.environment(text);
                }
            }
        }
    }

    pub(super) fn text(
        &mut self,
        text: &str,
        source: TextSource<'_>,
        options: &ContextSnapshotOptions,
    ) -> String {
        let text = if matches!(source, TextSource::FunctionArguments) {
            self.arguments(text)
        } else {
            text.to_string()
        };
        let text = normalize_line_endings(&text);
        let segment = known_segment_name(&text, source);
        let text = self.normalize_values(&text);
        let text = match segment.as_deref() {
            Some("PERMISSIONS_INSTRUCTIONS") => self.permissions(&text),
            Some("ENVIRONMENT_CONTEXT") => self.environment(&text),
            _ => text,
        };
        if options.rewrite_known_segments
            && let Some(segment) = segment
        {
            let marker = format!("<{segment}>");
            if segment == "COMPACTION_SUMMARY"
                && let Some((_, summary)) = text.split_once('\n')
                && !summary.is_empty()
            {
                // Only the standard preamble is guidance; the generated summary is conversation data.
                format!("{marker}\n{summary}")
            } else {
                marker
            }
        } else {
            text
        }
    }

    pub(super) fn json(&mut self, value: &Value) -> Value {
        let mut value = value.clone();
        normalize_json(&mut value, &mut |text| {
            normalize_stable_text(&self.normalize_uuids(text))
        });
        value
    }

    fn arguments(&mut self, text: &str) -> String {
        serde_json::from_str::<Value>(text)
            .map(|value| self.json(&value).to_string())
            .unwrap_or_else(|_| text.to_string())
    }

    fn normalize_uuids(&mut self, text: &str) -> String {
        uuid_regex()
            .replace_all(text, |captures: &regex_lite::Captures<'_>| {
                let id = captures[0].to_ascii_lowercase();
                format!("<UUID {}>", stable_index(&mut self.uuids, &id))
            })
            .into_owned()
    }

    pub(super) fn prompt_cache_key(&mut self, key: &str) -> String {
        format!(
            "<PROMPT_CACHE_KEY {}>",
            stable_index(&mut self.prompt_cache_keys, key)
        )
    }

    fn normalize_values(&mut self, text: &str) -> String {
        // Code Mode reports elapsed wall time in an otherwise stable tool output header.
        let text = if text.starts_with("Script completed\nWall time ") {
            static WALL_TIME: OnceLock<Regex> = OnceLock::new();
            WALL_TIME
                .get_or_init(|| {
                    Regex::new(r"(?m)^Wall time [0-9]+(?:\.[0-9]+)? seconds$")
                        .expect("code mode wall time regex")
                })
                .replace(text, "Wall time <DURATION> seconds")
                .into_owned()
        } else {
            text.to_string()
        };
        let text = if text.starts_with("# AGENTS.md instructions for ") {
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

    pub(super) fn environment(&mut self, text: &str) -> String {
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
        let aliases = self.path_aliases();
        let text = path.replace_all(text, |captures: &regex_lite::Captures<'_>| {
            let value = &captures[3];
            format!(
                "{}{}",
                &captures[1],
                normalize_path(value, &aliases).as_deref().unwrap_or(value)
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

    fn path_aliases(&self) -> Vec<(String, String)> {
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
        aliases
    }

    fn permissions(&mut self, text: &str) -> String {
        static PATH: OnceLock<Regex> = OnceLock::new();
        let path = PATH.get_or_init(|| Regex::new(r"`([^`]+)`").expect("permission path regex"));
        let aliases = self.path_aliases();
        text.split('\n')
            .map(|line| {
                let writable = line.trim_start().starts_with("The writable root");
                if !writable && !line.starts_with("- path `") && !line.starts_with("- glob `") {
                    return line.to_string();
                }
                path.replace_all(line, |captures: &regex_lite::Captures<'_>| {
                    let value = &captures[1];
                    let normalized =
                        normalize_path(value, &aliases).or_else(|| self.temporary_path(value));
                    format!("`{}`", normalized.as_deref().unwrap_or(value))
                })
                .into_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn temporary_path(&mut self, value: &str) -> Option<String> {
        // tempfile uses randomized `.tmp…` directories under the host's configured temp root.
        // Keep other names, including stable policy paths, visible to the fingerprint.
        let temp_dir = std::env::temp_dir().to_string_lossy().replace('\\', "/");
        let value = value.replace('\\', "/");
        let suffix = value.strip_prefix(temp_dir.trim_end_matches('/'))?;
        if suffix.is_empty() {
            return Some("<TEMP_DIR>".to_string());
        }
        let suffix = suffix.strip_prefix('/')?;
        let first = suffix.split('/').next()?;
        let tail = &suffix[first.len()..];
        if !first.strip_prefix(".tmp").is_some_and(|random| {
            !random.is_empty()
                && random
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        }) {
            return Some(format!("<TEMP_DIR>/{suffix}"));
        }
        let index = stable_index(&mut self.temporary_directories, first);
        Some(format!("<TEMP_DIR>/<TEMP {index}>{tail}"))
    }
}

fn stable_index(values: &mut Vec<String>, value: &str) -> usize {
    if let Some(index) = values.iter().position(|known| known == value) {
        index + 1
    } else {
        values.push(value.to_string());
        values.len()
    }
}

fn normalize_path(value: &str, aliases: &[(String, String)]) -> Option<String> {
    aliases.iter().find_map(|(path, label)| {
        let suffix = value.strip_prefix(path)?;
        (suffix.is_empty() || suffix.starts_with('/') || suffix.starts_with('\\'))
            .then(|| format!("{label}{}", suffix.replace('\\', "/")))
    })
}

fn known_segment_name(text: &str, source: TextSource<'_>) -> Option<String> {
    let prefixes: &[(&str, &str)] = match source {
        TextSource::ModelInstructions => &[
            (GUARDIAN_INSTRUCTIONS_PREFIX, "GUARDIAN_INSTRUCTIONS"),
            ("", "MODEL_INSTRUCTIONS"),
        ],
        TextSource::Message("developer") => {
            &[(GUARDIAN_INSTRUCTIONS_PREFIX, "GUARDIAN_INSTRUCTIONS")]
        }
        TextSource::Message("user") => &[
            ("# AGENTS.md instructions", "AGENTS_MD"),
            (
                "You are performing a CONTEXT CHECKPOINT COMPACTION.",
                "SUMMARIZATION_PROMPT",
            ),
            (
                "Another language model started to solve this problem",
                "COMPACTION_SUMMARY",
            ),
        ],
        _ => &[],
    };
    if let Some((_, name)) = prefixes
        .iter()
        .find(|(prefix, _)| text.starts_with(*prefix))
    {
        return Some((*name).to_string());
    }
    let tag = guidance_tag(text)?;
    matches!(
        (source, tag),
        (
            TextSource::Message("developer"),
            "permissions instructions"
                | "collaboration_mode"
                | "multi_agent_role"
                | "multi_agent_mode"
                | "apps_instructions"
                | "skills_instructions"
                | "plugins_instructions"
                | "model_switch"
                | "personality_spec"
        ) | (TextSource::Message("user"), "environment_context")
    )
    .then(|| tag.replace(' ', "_").to_ascii_uppercase())
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

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn normalize_stable_text(text: &str) -> String {
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

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn normalize_json(value: &mut Value, normalize: &mut impl FnMut(&str) -> String) {
    match value {
        Value::String(text) => *text = normalize(text),
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| normalize_json(value, normalize)),
        Value::Object(map) => {
            map.sort_keys();
            for value in map.values_mut() {
                normalize_json(value, normalize);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

// Shell guidance and its yield-time wording intentionally differ on Windows. Compare all other
// tool schema fields, while keeping the context snapshots identical across operating systems.
pub(super) fn portable_tool_schema(tool: &Value) -> Value {
    const BASE: &str =
        "Runs a command in a PTY, returning output or a session ID for ongoing interaction.";
    // Exact current Windows-only suffix. A change to its wording must be reviewed explicitly.
    const WINDOWS_SAFETY_SUFFIX_HASH: u64 = 0x6de3_23e4_7060_6128;
    const UNIX_WAIT: &str =
        "Wait before yielding output. Defaults to 10000 ms; effective range is 250-30000 ms.";
    const WINDOWS_WAIT: &str = "Maximum time to wait before returning a session ID for a still-running command. Commands that finish sooner return immediately. For ordinary commands, omit this parameter to use the 10000 ms default. Effective range on Windows is 10000-30000 ms.";
    let mut stable = tool.clone();
    if let Some(Value::Array(members)) = stable.get_mut("tools") {
        for member in members {
            *member = portable_tool_schema(member);
        }
    }
    let definition = if stable.get("function").is_some() {
        stable
            .get_mut("function")
            .expect("function tool definition")
    } else {
        &mut stable
    };
    if definition.get("name").and_then(Value::as_str) == Some("exec") {
        if let Some(Value::String(description)) = definition.get_mut("description") {
            // Code mode embeds the exec_command description and its TypeScript declaration
            // inside the exec tool. Apply the same narrow platform normalization there.
            if let Some((start, section)) = description.split_once("### `exec_command`\n")
                && let Some(rest) = section.strip_prefix(BASE)
                && let Some((suffix, _)) = rest.split_once("\n\nexec tool declaration:")
                && suffix.starts_with("\n\nWindows safety rules:")
                && fnv1a(normalize_line_endings(suffix).as_bytes()) == WINDOWS_SAFETY_SUFFIX_HASH
            {
                *description =
                    format!("{start}### `exec_command`\n{BASE}{}", &rest[suffix.len()..]);
            }
            for platform_wait in [UNIX_WAIT, WINDOWS_WAIT] {
                *description = description.replace(
                    &format!("  // {platform_wait}"),
                    "  // <PLATFORM_WAIT_GUIDANCE>",
                );
            }
        }
        return stable;
    }
    if definition.get("name").and_then(Value::as_str) != Some("exec_command") {
        return stable;
    }
    if let Some(Value::String(description)) = definition.get_mut("description")
        && description.strip_prefix(BASE).is_some_and(|rest| {
            rest.starts_with("\n\nWindows safety rules:")
                && fnv1a(normalize_line_endings(rest).as_bytes()) == WINDOWS_SAFETY_SUFFIX_HASH
        })
    {
        *description = BASE.to_string();
    }
    if let Some(Value::String(wait)) =
        definition.pointer_mut("/parameters/properties/yield_time_ms/description")
        && (wait == UNIX_WAIT || wait == WINDOWS_WAIT)
    {
        *wait = "<PLATFORM_WAIT_GUIDANCE>".to_string();
    }
    stable
}

pub(super) fn is_bundled_model_instructions(text: &str) -> bool {
    static PROMPTS: OnceLock<Vec<String>> = OnceLock::new();
    text == codex_models_manager::model_info::BASE_INSTRUCTIONS
        || PROMPTS
            .get_or_init(|| {
                codex_models_manager::bundled_models_response()
                    .expect("bundled model catalog")
                    .models
                    .into_iter()
                    .filter_map(|model| model.model_messages?.instructions_template)
                    .collect()
            })
            .iter()
            .any(|prompt| prompt == text)
}

// Only complete, balanced wrappers identify guidance. Adjacent or malformed wrappers remain text.
fn guidance_tag(text: &str) -> Option<&str> {
    let text = text.trim();
    let (tag, rest) = text.strip_prefix('<')?.split_once('>')?;
    let open = &text[..tag.len() + 2];
    let close = format!("</{tag}>");
    let mut depth = 1;
    for (offset, _) in rest.match_indices('<') {
        let after = &rest[offset..];
        if after.starts_with(open) {
            depth += 1;
        } else if after.starts_with(&close) {
            depth -= 1;
            if depth == 0 {
                return (after == close.as_str()).then_some(tag);
            }
        }
    }
    None
}
