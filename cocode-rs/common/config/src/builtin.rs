//! Built-in model and provider defaults.
//!
//! This module provides default configurations for well-known models
//! that are compiled into the binary. These serve as the lowest-priority
//! layer in the configuration resolution.

// Built-in prompt templates (embedded at compile time)
const DEFAULT_PROMPT: &str = include_str!("../prompt_with_apply_patch_instructions.md");
const GEMINI_PROMPT: &str = include_str!("../gemini_prompt.md");
const GPT_5_2_PROMPT: &str = include_str!("../gpt_5_2_prompt.md");
const GPT_5_2_CODEX_PROMPT: &str = include_str!("../gpt-5.2-codex_prompt.md");

// Built-in output style templates (embedded at compile time)
const OUTPUT_STYLE_EXPLANATORY: &str = include_str!("../output_style_explanatory.md");
const OUTPUT_STYLE_LEARNING: &str = include_str!("../output_style_learning.md");

use crate::types::ProviderConfig;
use crate::types::ProviderType;
use cocode_protocol::Capability;
use cocode_protocol::ConfigShellToolType;
use cocode_protocol::ModelInfo;
use cocode_protocol::ThinkingLevel;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Get built-in model defaults for a model ID.
///
/// Returns `None` if no built-in defaults exist for this model.
pub fn get_model_defaults(model_id: &str) -> Option<ModelInfo> {
    BUILTIN_MODELS.get().and_then(|m| m.get(model_id).cloned())
}

/// Get built-in provider defaults for a provider name.
///
/// Returns `None` if no built-in defaults exist for this provider.
pub fn get_provider_defaults(provider_name: &str) -> Option<ProviderConfig> {
    BUILTIN_PROVIDERS
        .get()
        .and_then(|p| p.get(provider_name).cloned())
}

/// Get all built-in model IDs.
pub fn list_builtin_models() -> Vec<&'static str> {
    BUILTIN_MODELS
        .get()
        .map(|m| m.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// Get all built-in provider names.
pub fn list_builtin_providers() -> Vec<&'static str> {
    BUILTIN_PROVIDERS
        .get()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// Get a built-in output style by name (case-insensitive).
///
/// Supported styles:
/// - `"explanatory"` - Educational insights while completing tasks
/// - `"learning"` - Hands-on learning with TODO(human) contributions
///
/// Returns `None` if the style name is not recognized.
pub fn get_output_style(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "explanatory" => Some(OUTPUT_STYLE_EXPLANATORY),
        "learning" => Some(OUTPUT_STYLE_LEARNING),
        _ => None,
    }
}

/// List all built-in output style names.
pub fn list_builtin_output_styles() -> Vec<&'static str> {
    vec!["explanatory", "learning"]
}

/// A custom output style loaded from a file.
#[derive(Debug, Clone)]
pub struct CustomOutputStyle {
    /// Style name (derived from filename).
    pub name: String,
    /// Style description (from frontmatter or first line).
    pub description: Option<String>,
    /// Full style content (the markdown body).
    pub content: String,
    /// Source file path.
    pub path: PathBuf,
}

/// Output style metadata parsed from YAML frontmatter.
#[derive(Debug, Clone, Default)]
pub struct OutputStyleFrontmatter {
    /// Style name override (defaults to filename).
    pub name: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Whether to keep the coding-instructions marker.
    pub keep_coding_instructions: Option<bool>,
}

/// Parse YAML frontmatter from markdown content.
///
/// Frontmatter is delimited by `---` at the start and end.
/// Returns (frontmatter, remaining_content).
fn parse_frontmatter(content: &str) -> (OutputStyleFrontmatter, &str) {
    let content = content.trim_start();

    // Check for frontmatter delimiter
    if !content.starts_with("---") {
        return (OutputStyleFrontmatter::default(), content);
    }

    // Find the closing delimiter
    let after_first = &content[3..].trim_start_matches(['\r', '\n']);
    if let Some(end_idx) = after_first.find("\n---") {
        let yaml_content = &after_first[..end_idx];
        let remaining = &after_first[end_idx + 4..].trim_start_matches(['\r', '\n', '-']);

        // Parse YAML content (simple key: value parsing)
        let mut fm = OutputStyleFrontmatter::default();
        for line in yaml_content.lines() {
            let line = line.trim();
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim().trim_matches('"').trim_matches('\'');
                match key {
                    "name" => fm.name = Some(value.to_string()),
                    "description" => fm.description = Some(value.to_string()),
                    "keep-coding-instructions" | "keep_coding_instructions" => {
                        fm.keep_coding_instructions = value.parse().ok();
                    }
                    _ => {} // Ignore unknown keys
                }
            }
        }

        return (fm, remaining);
    }

    (OutputStyleFrontmatter::default(), content)
}

/// Load custom output styles from the specified directory.
///
/// Scans for `*.md` files and parses them as output styles.
/// Files should optionally have YAML frontmatter with:
/// - `name`: Style name (defaults to filename without extension)
/// - `description`: Human-readable description
/// - `keep-coding-instructions`: Whether to preserve coding instruction markers
///
/// # Example File Structure
///
/// ```markdown
/// ---
/// name: concise
/// description: Short, direct responses without explanations
/// ---
/// You should be concise and direct.
/// Avoid unnecessary explanations.
/// ```
pub fn load_custom_output_styles(dir: &Path) -> Vec<CustomOutputStyle> {
    if !dir.is_dir() {
        return Vec::new();
    }

    let mut styles = Vec::new();

    // Read directory entries
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Only process .md files
        if path.extension().is_some_and(|ext| ext == "md") {
            if let Some(style) = load_single_style(&path) {
                styles.push(style);
            }
        }
    }

    // Sort by name for consistent ordering
    styles.sort_by(|a, b| a.name.cmp(&b.name));
    styles
}

/// Load a single output style from a file.
fn load_single_style(path: &Path) -> Option<CustomOutputStyle> {
    let content = fs::read_to_string(path).ok()?;

    // Parse frontmatter
    let (frontmatter, body) = parse_frontmatter(&content);

    // Derive name from frontmatter or filename
    let name = frontmatter.name.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string()
    });

    // Use frontmatter description or extract from first line
    let description = frontmatter.description.or_else(|| {
        body.lines()
            .next()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                // Truncate long descriptions
                if line.len() > 100 {
                    format!("{}...", &line[..97])
                } else {
                    line.to_string()
                }
            })
    });

    Some(CustomOutputStyle {
        name,
        description,
        content: body.trim().to_string(),
        path: path.to_path_buf(),
    })
}

/// Get the default output styles directory.
///
/// Returns `~/.cocode/output-styles/` on Unix-like systems.
pub fn default_output_styles_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cocode").join("output-styles"))
}

/// Load all output styles (built-in + custom).
///
/// Returns a combined list with built-in styles first, then custom styles.
/// Custom styles can shadow built-in styles with the same name.
pub fn load_all_output_styles() -> Vec<OutputStyleInfo> {
    let mut styles = Vec::new();

    // Add built-in styles
    for name in list_builtin_output_styles() {
        if let Some(content) = get_output_style(name) {
            styles.push(OutputStyleInfo {
                name: name.to_string(),
                description: builtin_style_description(name),
                content: content.to_string(),
                source: OutputStyleSource::Builtin,
            });
        }
    }

    // Add custom styles from default directory
    if let Some(dir) = default_output_styles_dir() {
        for custom in load_custom_output_styles(&dir) {
            styles.push(OutputStyleInfo {
                name: custom.name,
                description: custom.description,
                content: custom.content,
                source: OutputStyleSource::Custom(custom.path),
            });
        }
    }

    styles
}

/// Get description for built-in styles.
fn builtin_style_description(name: &str) -> Option<String> {
    match name {
        "explanatory" => Some("Educational insights while completing tasks".to_string()),
        "learning" => Some("Hands-on learning with TODO(human) contributions".to_string()),
        _ => None,
    }
}

/// Information about an output style.
#[derive(Debug, Clone)]
pub struct OutputStyleInfo {
    /// Style name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Full style content.
    pub content: String,
    /// Source of the style.
    pub source: OutputStyleSource,
}

/// Source of an output style.
#[derive(Debug, Clone)]
pub enum OutputStyleSource {
    /// Built-in style compiled into the binary.
    Builtin,
    /// Custom style loaded from a file.
    Custom(PathBuf),
}

impl OutputStyleSource {
    /// Check if this is a built-in style.
    pub fn is_builtin(&self) -> bool {
        matches!(self, Self::Builtin)
    }

    /// Check if this is a custom style.
    pub fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }
}

/// Find an output style by name.
///
/// Searches both built-in and custom styles. Custom styles take precedence
/// when there's a name conflict.
pub fn find_output_style(name: &str) -> Option<OutputStyleInfo> {
    let name_lower = name.to_lowercase();

    // Check custom styles first (they take precedence)
    if let Some(dir) = default_output_styles_dir() {
        for custom in load_custom_output_styles(&dir) {
            if custom.name.to_lowercase() == name_lower {
                return Some(OutputStyleInfo {
                    name: custom.name,
                    description: custom.description,
                    content: custom.content,
                    source: OutputStyleSource::Custom(custom.path),
                });
            }
        }
    }

    // Fall back to built-in styles
    if let Some(content) = get_output_style(name) {
        return Some(OutputStyleInfo {
            name: name.to_string(),
            description: builtin_style_description(&name_lower),
            content: content.to_string(),
            source: OutputStyleSource::Builtin,
        });
    }

    None
}

// Lazily initialized built-in models
static BUILTIN_MODELS: OnceLock<HashMap<String, ModelInfo>> = OnceLock::new();
static BUILTIN_PROVIDERS: OnceLock<HashMap<String, ProviderConfig>> = OnceLock::new();

/// Initialize built-in defaults (called automatically on first access).
fn init_builtin_models() -> HashMap<String, ModelInfo> {
    let mut models = HashMap::new();

    // OpenAI GPT-5
    models.insert(
        "gpt-5".to_string(),
        ModelInfo {
            display_name: Some("GPT-5".to_string()),
            base_instructions: Some(DEFAULT_PROMPT.to_string()),
            context_window: Some(272000),
            max_output_tokens: Some(32000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::StructuredOutput,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(250000),
            effective_context_window_percent: Some(95),
            default_thinking_level: Some(ThinkingLevel::medium()),
            supported_thinking_levels: Some(vec![
                ThinkingLevel::low(),
                ThinkingLevel::medium(),
                ThinkingLevel::high(),
            ]),
            ..Default::default()
        },
    );

    // OpenAI GPT-5.2
    models.insert(
        "gpt-5.2".to_string(),
        ModelInfo {
            display_name: Some("GPT-5.2".to_string()),
            base_instructions: Some(GPT_5_2_PROMPT.to_string()),
            context_window: Some(272000),
            max_output_tokens: Some(64000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::ExtendedThinking,
                Capability::ReasoningSummaries,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(250000),
            effective_context_window_percent: Some(95),
            default_thinking_level: Some(ThinkingLevel::medium()),
            supported_thinking_levels: Some(vec![
                ThinkingLevel::low(),
                ThinkingLevel::medium(),
                ThinkingLevel::high(),
                ThinkingLevel::xhigh(),
            ]),
            shell_type: Some(ConfigShellToolType::ShellCommand),
            ..Default::default()
        },
    );

    // OpenAI GPT-5.2 Codex (optimized for coding)
    models.insert(
        "gpt-5.2-codex".to_string(),
        ModelInfo {
            display_name: Some("GPT-5.2 Codex".to_string()),
            description: Some("GPT-5.2 optimized for coding tasks".to_string()),
            base_instructions: Some(GPT_5_2_CODEX_PROMPT.to_string()),
            context_window: Some(272000),
            max_output_tokens: Some(64000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::ExtendedThinking,
                Capability::ReasoningSummaries,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(250000),
            effective_context_window_percent: Some(95),
            default_thinking_level: Some(ThinkingLevel::medium()),
            supported_thinking_levels: Some(vec![
                ThinkingLevel::low(),
                ThinkingLevel::medium(),
                ThinkingLevel::high(),
                ThinkingLevel::xhigh(),
            ]),
            shell_type: Some(ConfigShellToolType::ShellCommand),
            ..Default::default()
        },
    );

    // Google Gemini 3 Pro
    models.insert(
        "gemini-3-pro".to_string(),
        ModelInfo {
            display_name: Some("Gemini 3 Pro".to_string()),
            base_instructions: Some(GEMINI_PROMPT.to_string()),
            context_window: Some(300000),
            max_output_tokens: Some(32000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(280000),
            effective_context_window_percent: Some(95),
            ..Default::default()
        },
    );

    // Google Gemini 3 Flash
    models.insert(
        "gemini-3-flash".to_string(),
        ModelInfo {
            display_name: Some("Gemini 3 Flash".to_string()),
            base_instructions: Some(GEMINI_PROMPT.to_string()),
            context_window: Some(300000),
            max_output_tokens: Some(16000),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
                Capability::ParallelToolCalls,
            ]),
            auto_compact_token_limit: Some(280000),
            effective_context_window_percent: Some(95),
            ..Default::default()
        },
    );

    models
}

fn init_builtin_providers() -> HashMap<String, ProviderConfig> {
    use crate::types::WireApi;

    let mut providers = HashMap::new();

    providers.insert(
        "openai".to_string(),
        ProviderConfig {
            name: "openai".to_string(),
            provider_type: ProviderType::Openai,
            base_url: "https://api.openai.com/v1".to_string(),
            timeout_secs: 600,
            env_key: Some("OPENAI_API_KEY".to_string()),
            api_key: None,
            streaming: true,
            wire_api: WireApi::Responses,
            overrides: HashMap::new(),
            models: Vec::new(),
            extra: None,
            interceptors: Vec::new(),
        },
    );

    providers.insert(
        "gemini".to_string(),
        ProviderConfig {
            name: "gemini".to_string(),
            provider_type: ProviderType::Gemini,
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            timeout_secs: 600,
            env_key: Some("GOOGLE_API_KEY".to_string()),
            api_key: None,
            streaming: true,
            wire_api: WireApi::Chat,
            overrides: HashMap::new(),
            models: Vec::new(),
            extra: None,
            interceptors: Vec::new(),
        },
    );

    providers
}

// Force initialization by accessing the locks
pub(crate) fn ensure_initialized() {
    let _ = BUILTIN_MODELS.get_or_init(init_builtin_models);
    let _ = BUILTIN_PROVIDERS.get_or_init(init_builtin_providers);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::ReasoningEffort;

    #[test]
    fn test_get_model_defaults() {
        ensure_initialized();

        let gpt5 = get_model_defaults("gpt-5").unwrap();
        assert_eq!(gpt5.display_name, Some("GPT-5".to_string()));
        assert_eq!(gpt5.context_window, Some(272000));

        let gemini = get_model_defaults("gemini-3-pro").unwrap();
        assert_eq!(gemini.display_name, Some("Gemini 3 Pro".to_string()));

        let unknown = get_model_defaults("unknown-model");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_get_provider_defaults() {
        ensure_initialized();

        let openai = get_provider_defaults("openai").unwrap();
        assert_eq!(openai.name, "openai");
        assert_eq!(openai.env_key, Some("OPENAI_API_KEY".to_string()));

        let gemini = get_provider_defaults("gemini").unwrap();
        assert_eq!(gemini.name, "gemini");
        assert_eq!(gemini.provider_type, ProviderType::Gemini);

        let unknown = get_provider_defaults("unknown-provider");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_list_builtin_models() {
        ensure_initialized();

        let models = list_builtin_models();
        assert!(models.contains(&"gpt-5"));
        assert!(models.contains(&"gpt-5.2"));
        assert!(models.contains(&"gpt-5.2-codex"));
        assert!(models.contains(&"gemini-3-pro"));
        assert!(models.contains(&"gemini-3-flash"));
    }

    #[test]
    fn test_list_builtin_providers() {
        ensure_initialized();

        let providers = list_builtin_providers();
        assert!(providers.contains(&"openai"));
        assert!(providers.contains(&"gemini"));
    }

    #[test]
    fn test_model_capabilities() {
        ensure_initialized();

        let gpt5 = get_model_defaults("gpt-5").unwrap();
        let caps = gpt5.capabilities.unwrap();
        assert!(caps.contains(&Capability::TextGeneration));
        assert!(caps.contains(&Capability::Vision));
        assert!(caps.contains(&Capability::ToolCalling));
        assert!(caps.contains(&Capability::ParallelToolCalls));

        let gpt52 = get_model_defaults("gpt-5.2").unwrap();
        let caps = gpt52.capabilities.unwrap();
        assert!(caps.contains(&Capability::ExtendedThinking));
        assert!(caps.contains(&Capability::ReasoningSummaries));
    }

    #[test]
    fn test_thinking_models() {
        ensure_initialized();

        let gpt5 = get_model_defaults("gpt-5").unwrap();
        assert!(gpt5.default_thinking_level.is_some());
        assert!(gpt5.supported_thinking_levels.is_some());

        let default_level = gpt5.default_thinking_level.unwrap();
        assert_eq!(default_level.effort, ReasoningEffort::Medium);

        let levels = gpt5.supported_thinking_levels.unwrap();
        assert!(levels.iter().any(|l| l.effort == ReasoningEffort::Low));
        assert!(levels.iter().any(|l| l.effort == ReasoningEffort::Medium));
        assert!(levels.iter().any(|l| l.effort == ReasoningEffort::High));

        let gpt52 = get_model_defaults("gpt-5.2").unwrap();
        let levels = gpt52.supported_thinking_levels.unwrap();
        assert!(levels.iter().any(|l| l.effort == ReasoningEffort::XHigh));
    }

    #[test]
    fn test_shell_type() {
        ensure_initialized();

        let gpt52 = get_model_defaults("gpt-5.2").unwrap();
        assert_eq!(gpt52.shell_type, Some(ConfigShellToolType::ShellCommand));

        let gpt5 = get_model_defaults("gpt-5").unwrap();
        assert_eq!(gpt5.shell_type, None); // Default
    }

    #[test]
    fn test_gpt52_codex() {
        ensure_initialized();

        let codex = get_model_defaults("gpt-5.2-codex").unwrap();
        assert_eq!(codex.display_name, Some("GPT-5.2 Codex".to_string()));
        assert_eq!(codex.context_window, Some(272000));
        assert_eq!(codex.max_output_tokens, Some(64000));
        assert_eq!(codex.shell_type, Some(ConfigShellToolType::ShellCommand));

        let caps = codex.capabilities.unwrap();
        assert!(caps.contains(&Capability::ExtendedThinking));
        assert!(caps.contains(&Capability::ReasoningSummaries));
        assert!(caps.contains(&Capability::ParallelToolCalls));

        let levels = codex.supported_thinking_levels.unwrap();
        assert!(levels.iter().any(|l| l.effort == ReasoningEffort::Low));
        assert!(levels.iter().any(|l| l.effort == ReasoningEffort::Medium));
        assert!(levels.iter().any(|l| l.effort == ReasoningEffort::High));
        assert!(levels.iter().any(|l| l.effort == ReasoningEffort::XHigh));
    }

    #[test]
    fn test_builtin_models_have_instructions() {
        ensure_initialized();

        // All built-in models should have base_instructions
        for slug in list_builtin_models() {
            let model = get_model_defaults(slug).unwrap();
            assert!(
                model.base_instructions.is_some(),
                "Model {slug} should have base_instructions"
            );
            // Verify instructions are non-empty
            let instructions = model.base_instructions.as_ref().unwrap();
            assert!(
                !instructions.is_empty(),
                "Model {slug} should have non-empty base_instructions"
            );
        }
    }

    #[test]
    fn test_get_output_style_explanatory() {
        let style = get_output_style("explanatory").unwrap();
        assert!(style.contains("Explanatory Style Active"));
        assert!(style.contains("Insight Format"));

        // Test case-insensitive variants
        assert_eq!(style, get_output_style("Explanatory").unwrap());
        assert_eq!(style, get_output_style("EXPLANATORY").unwrap());
        assert_eq!(style, get_output_style("ExPlAnAtOrY").unwrap());
    }

    #[test]
    fn test_get_output_style_learning() {
        let style = get_output_style("learning").unwrap();
        assert!(style.contains("Learning Style Active"));
        assert!(style.contains("TODO(human)"));

        // Test case-insensitive variants
        assert_eq!(style, get_output_style("Learning").unwrap());
        assert_eq!(style, get_output_style("LEARNING").unwrap());
        assert_eq!(style, get_output_style("LeArNiNg").unwrap());
    }

    #[test]
    fn test_get_output_style_unknown() {
        let style = get_output_style("unknown");
        assert!(style.is_none());
    }

    #[test]
    fn test_list_builtin_output_styles() {
        let styles = list_builtin_output_styles();
        assert!(styles.contains(&"explanatory"));
        assert!(styles.contains(&"learning"));
        assert_eq!(styles.len(), 2);
    }

    #[test]
    fn test_parse_frontmatter_empty() {
        let (fm, body) = parse_frontmatter("Hello world");
        assert!(fm.name.is_none());
        assert!(fm.description.is_none());
        assert_eq!(body, "Hello world");
    }

    #[test]
    fn test_parse_frontmatter_simple() {
        let content = r#"---
name: concise
description: Short responses
---
Body content here."#;

        let (fm, body) = parse_frontmatter(content);
        assert_eq!(fm.name, Some("concise".to_string()));
        assert_eq!(fm.description, Some("Short responses".to_string()));
        assert!(body.contains("Body content here"));
    }

    #[test]
    fn test_parse_frontmatter_quoted_values() {
        let content = r#"---
name: "my-style"
description: 'A quoted description'
---
Content"#;

        let (fm, _body) = parse_frontmatter(content);
        assert_eq!(fm.name, Some("my-style".to_string()));
        assert_eq!(fm.description, Some("A quoted description".to_string()));
    }

    #[test]
    fn test_parse_frontmatter_keep_coding_instructions() {
        let content = r#"---
name: test
keep-coding-instructions: true
---
Content"#;

        let (fm, _body) = parse_frontmatter(content);
        assert_eq!(fm.keep_coding_instructions, Some(true));
    }

    #[test]
    fn test_load_custom_output_styles_empty_dir() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let styles = load_custom_output_styles(tmp.path());
        assert!(styles.is_empty());
    }

    #[test]
    fn test_load_custom_output_styles_with_files() {
        let tmp = tempfile::tempdir().expect("create temp dir");

        // Create a simple style file
        std::fs::write(tmp.path().join("concise.md"), "Be concise and direct.")
            .expect("write file");

        // Create a style with frontmatter
        std::fs::write(
            tmp.path().join("verbose.md"),
            r#"---
name: verbose
description: Detailed explanations
---
Provide detailed explanations for every action."#,
        )
        .expect("write file");

        let styles = load_custom_output_styles(tmp.path());
        assert_eq!(styles.len(), 2);

        // Check concise style (no frontmatter)
        let concise = styles.iter().find(|s| s.name == "concise").unwrap();
        assert_eq!(concise.content, "Be concise and direct.");

        // Check verbose style (with frontmatter)
        let verbose = styles.iter().find(|s| s.name == "verbose").unwrap();
        assert_eq!(
            verbose.description,
            Some("Detailed explanations".to_string())
        );
        assert!(verbose.content.contains("detailed explanations"));
    }

    #[test]
    fn test_load_custom_output_styles_nonexistent_dir() {
        let styles = load_custom_output_styles(Path::new("/nonexistent/xyz"));
        assert!(styles.is_empty());
    }

    #[test]
    fn test_load_custom_output_styles_ignores_non_md() {
        let tmp = tempfile::tempdir().expect("create temp dir");

        // Create various files
        std::fs::write(tmp.path().join("style.md"), "Valid style").expect("write");
        std::fs::write(tmp.path().join("notes.txt"), "Not a style").expect("write");
        std::fs::write(tmp.path().join("config.json"), "{}").expect("write");

        let styles = load_custom_output_styles(tmp.path());
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].name, "style");
    }

    #[test]
    fn test_find_output_style_builtin() {
        let style = find_output_style("explanatory").unwrap();
        assert_eq!(style.name, "explanatory");
        assert!(style.source.is_builtin());
        assert!(style.content.contains("Explanatory Style Active"));
    }

    #[test]
    fn test_find_output_style_case_insensitive() {
        let style1 = find_output_style("EXPLANATORY").unwrap();
        let style2 = find_output_style("Explanatory").unwrap();
        let style3 = find_output_style("explanatory").unwrap();

        assert_eq!(style1.content, style2.content);
        assert_eq!(style2.content, style3.content);
    }

    #[test]
    fn test_find_output_style_not_found() {
        let style = find_output_style("nonexistent-style");
        assert!(style.is_none());
    }

    #[test]
    fn test_output_style_source() {
        let builtin = OutputStyleSource::Builtin;
        assert!(builtin.is_builtin());
        assert!(!builtin.is_custom());

        let custom = OutputStyleSource::Custom(PathBuf::from("/test/style.md"));
        assert!(!custom.is_builtin());
        assert!(custom.is_custom());
    }

    #[test]
    fn test_load_all_output_styles() {
        let styles = load_all_output_styles();

        // Should have at least the built-in styles
        assert!(styles.len() >= 2);
        assert!(styles.iter().any(|s| s.name == "explanatory"));
        assert!(styles.iter().any(|s| s.name == "learning"));
    }
}
