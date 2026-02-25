//! Syntax highlighting engine for the TUI.
//!
//! Wraps [syntect] with the [two_face] grammar and theme bundles to provide
//! ~250-language syntax highlighting and 32 bundled color themes.  The module
//! owns four process-global singletons:
//!
//! | Singleton | Type | Purpose |
//! |---|---|---|
//! | `SYNTAX_SET` | `OnceLock<SyntaxSet>` | Grammar database, immutable after init |
//! | `THEME` | `OnceLock<RwLock<Theme>>` | Active color theme, swappable at runtime |
//! | `THEME_OVERRIDE` | `OnceLock<Option<String>>` | Persisted user preference (write-once) |
//! | `CODEX_HOME` | `OnceLock<Option<PathBuf>>` | Root for custom `.tmTheme` discovery |
//!
//! **Lifecycle:** call [`set_theme_override`] once at startup (after the final
//! config is resolved) to persist the user preference and seed the `THEME`
//! lock.  After that, [`set_syntax_theme`] and [`current_syntax_theme`] can
//! swap/snapshot the theme for live preview.  All highlighting functions read
//! the theme via `theme_lock()`.
//!
//! **Guardrails:** inputs exceeding 512 KB or 10 000 lines are rejected early
//! (returns `None`) to prevent pathological CPU/memory usage.  Callers must
//! fall back to plain unstyled text.

use ratatui::style::Color as RtColor;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::RwLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;
use syntect::highlighting::Style as SyntectStyle;
use syntect::highlighting::Theme;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxReference;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use two_face::theme::EmbeddedThemeName;

// -- Global singletons -------------------------------------------------------

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<RwLock<Theme>> = OnceLock::new();
static THEME_OVERRIDE: OnceLock<Option<String>> = OnceLock::new();
static CODEX_HOME: OnceLock<Option<PathBuf>> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(two_face::syntax::extra_newlines)
}

/// Set the user-configured syntax theme override and codex home path.
///
/// Call this with the **final resolved config** (after onboarding, resume, and
/// fork reloads complete). The first call persists `name` and `codex_home` in
/// `OnceLock`s used by startup/default theme resolution.
///
/// Subsequent calls cannot change the persisted `OnceLock` values, but they
/// still update the runtime theme immediately for live preview flows.
///
/// Returns a warning message when the configured theme name cannot be
/// resolved to a bundled theme or a custom `.tmTheme` file on disk.
/// The caller should surface this via `Config::startup_warnings` so it
/// appears as a `⚠` banner in the TUI.
pub(crate) fn set_theme_override(
    name: Option<String>,
    codex_home: Option<PathBuf>,
) -> Option<String> {
    let mut warning = validate_theme_name(name.as_deref(), codex_home.as_deref());
    let override_set_ok = THEME_OVERRIDE.set(name.clone()).is_ok();
    let codex_home_set_ok = CODEX_HOME.set(codex_home.clone()).is_ok();
    if THEME.get().is_some() {
        set_syntax_theme(resolve_theme_with_override(
            name.as_deref(),
            codex_home.as_deref(),
        ));
    }
    if !override_set_ok || !codex_home_set_ok {
        let duplicate_msg = "Ignoring duplicate or late syntax theme override persistence; runtime theme was updated from the latest override, but persisted override config can only be initialized once.";
        tracing::warn!("{duplicate_msg}");
        if warning.is_none() {
            warning = Some(duplicate_msg.to_string());
        }
    }
    warning
}

/// Check whether a theme name resolves to a bundled theme or a custom
/// `.tmTheme` file.  Returns a user-facing warning when it does not.
pub(crate) fn validate_theme_name(name: Option<&str>, codex_home: Option<&Path>) -> Option<String> {
    let name = name?;
    let custom_theme_path_display = codex_home
        .map(|home| custom_theme_path(name, home).display().to_string())
        .unwrap_or_else(|| format!("$CODEX_HOME/themes/{name}.tmTheme"));
    // Bundled themes always resolve.
    if parse_theme_name(name).is_some() {
        return None;
    }
    // Custom themes must parse successfully; an unreadable/invalid file should
    // still surface a startup warning so users can diagnose configuration issues.
    if let Some(home) = codex_home {
        let custom_path = custom_theme_path(name, home);
        if custom_path.is_file() {
            if load_custom_theme(name, home).is_some() {
                return None;
            }
            return Some(format!(
                "Syntax theme \"{name}\" was found at {custom_theme_path_display} \
                 but could not be parsed. Falling back to auto-detection."
            ));
        }
    }
    Some(format!(
        "Unknown syntax theme \"{name}\", falling back to auto-detection. \
         Use a bundled name or place a .tmTheme file at \
         {custom_theme_path_display}"
    ))
}

/// Map a kebab-case theme name to the corresponding `EmbeddedThemeName`.
fn parse_theme_name(name: &str) -> Option<EmbeddedThemeName> {
    match name {
        "ansi" => Some(EmbeddedThemeName::Ansi),
        "base16" => Some(EmbeddedThemeName::Base16),
        "base16-eighties-dark" => Some(EmbeddedThemeName::Base16EightiesDark),
        "base16-mocha-dark" => Some(EmbeddedThemeName::Base16MochaDark),
        "base16-ocean-dark" => Some(EmbeddedThemeName::Base16OceanDark),
        "base16-ocean-light" => Some(EmbeddedThemeName::Base16OceanLight),
        "base16-256" => Some(EmbeddedThemeName::Base16_256),
        "catppuccin-frappe" => Some(EmbeddedThemeName::CatppuccinFrappe),
        "catppuccin-latte" => Some(EmbeddedThemeName::CatppuccinLatte),
        "catppuccin-macchiato" => Some(EmbeddedThemeName::CatppuccinMacchiato),
        "catppuccin-mocha" => Some(EmbeddedThemeName::CatppuccinMocha),
        "coldark-cold" => Some(EmbeddedThemeName::ColdarkCold),
        "coldark-dark" => Some(EmbeddedThemeName::ColdarkDark),
        "dark-neon" => Some(EmbeddedThemeName::DarkNeon),
        "dracula" => Some(EmbeddedThemeName::Dracula),
        "github" => Some(EmbeddedThemeName::Github),
        "gruvbox-dark" => Some(EmbeddedThemeName::GruvboxDark),
        "gruvbox-light" => Some(EmbeddedThemeName::GruvboxLight),
        "inspired-github" => Some(EmbeddedThemeName::InspiredGithub),
        "1337" => Some(EmbeddedThemeName::Leet),
        "monokai-extended" => Some(EmbeddedThemeName::MonokaiExtended),
        "monokai-extended-bright" => Some(EmbeddedThemeName::MonokaiExtendedBright),
        "monokai-extended-light" => Some(EmbeddedThemeName::MonokaiExtendedLight),
        "monokai-extended-origin" => Some(EmbeddedThemeName::MonokaiExtendedOrigin),
        "nord" => Some(EmbeddedThemeName::Nord),
        "one-half-dark" => Some(EmbeddedThemeName::OneHalfDark),
        "one-half-light" => Some(EmbeddedThemeName::OneHalfLight),
        "solarized-dark" => Some(EmbeddedThemeName::SolarizedDark),
        "solarized-light" => Some(EmbeddedThemeName::SolarizedLight),
        "sublime-snazzy" => Some(EmbeddedThemeName::SublimeSnazzy),
        "two-dark" => Some(EmbeddedThemeName::TwoDark),
        "zenburn" => Some(EmbeddedThemeName::Zenburn),
        _ => None,
    }
}

/// Build the expected path for a custom theme file.
fn custom_theme_path(name: &str, codex_home: &Path) -> PathBuf {
    codex_home.join("themes").join(format!("{name}.tmTheme"))
}

/// Try to load a custom `.tmTheme` file from `{codex_home}/themes/{name}.tmTheme`.
fn load_custom_theme(name: &str, codex_home: &Path) -> Option<Theme> {
    ThemeSet::get_theme(custom_theme_path(name, codex_home)).ok()
}

fn adaptive_default_theme_selection() -> (EmbeddedThemeName, &'static str) {
    match crate::terminal_palette::default_bg() {
        Some(bg) if crate::color::is_light(bg) => {
            (EmbeddedThemeName::CatppuccinLatte, "catppuccin-latte")
        }
        _ => (EmbeddedThemeName::CatppuccinMocha, "catppuccin-mocha"),
    }
}

fn adaptive_default_embedded_theme_name() -> EmbeddedThemeName {
    adaptive_default_theme_selection().0
}

/// Return the kebab-case name of the adaptive default syntax theme selected
/// from terminal background lightness.
pub(crate) fn adaptive_default_theme_name() -> &'static str {
    adaptive_default_theme_selection().1
}

/// Build the theme from current override/auto-detection settings.
/// Extracted from the old `theme()` init closure so it can be reused.
fn resolve_theme_with_override(name: Option<&str>, codex_home: Option<&Path>) -> Theme {
    let ts = two_face::theme::extra();

    // Honor user-configured theme if valid.
    if let Some(name) = name {
        // 1. Try bundled theme by kebab-case name.
        if let Some(theme_name) = parse_theme_name(name) {
            return ts.get(theme_name).clone();
        }
        // 2. Try loading {CODEX_HOME}/themes/{name}.tmTheme from disk.
        if let Some(home) = codex_home
            && let Some(theme) = load_custom_theme(name, home)
        {
            return theme;
        }
        tracing::warn!("unknown syntax theme \"{name}\", falling back to auto-detection");
    }

    ts.get(adaptive_default_embedded_theme_name()).clone()
}

/// Build the theme from current override/auto-detection settings.
/// Extracted from the old `theme()` init closure so it can be reused.
fn build_default_theme() -> Theme {
    let name = THEME_OVERRIDE.get().and_then(|name| name.as_deref());
    let codex_home = CODEX_HOME
        .get()
        .and_then(|codex_home| codex_home.as_deref());
    resolve_theme_with_override(name, codex_home)
}

fn theme_lock() -> &'static RwLock<Theme> {
    THEME.get_or_init(|| RwLock::new(build_default_theme()))
}

/// Swap the active syntax theme at runtime (for live preview).
pub(crate) fn set_syntax_theme(theme: Theme) {
    let mut guard = match theme_lock().write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = theme;
}

/// Clone the current syntax theme (e.g. to save for cancel-restore).
pub(crate) fn current_syntax_theme() -> Theme {
    match theme_lock().read() {
        Ok(theme) => theme.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

/// Return the configured kebab-case theme name when it resolves; otherwise
/// return the adaptive auto-detected default theme name.
///
/// This intentionally reflects persisted configuration/default selection, not
/// transient runtime swaps applied via `set_syntax_theme`.
pub(crate) fn configured_theme_name() -> String {
    // Explicit user override?
    if let Some(Some(name)) = THEME_OVERRIDE.get() {
        if parse_theme_name(name).is_some() {
            return name.clone();
        }
        if let Some(Some(home)) = CODEX_HOME.get()
            && load_custom_theme(name, home).is_some()
        {
            return name.clone();
        }
    }
    adaptive_default_theme_name().to_string()
}

/// Resolve a theme name to a `Theme` (bundled or custom). Returns `None`
/// when the name is unknown and no matching `.tmTheme` file exists.
pub(crate) fn resolve_theme_by_name(name: &str, codex_home: Option<&Path>) -> Option<Theme> {
    let ts = two_face::theme::extra();
    // Bundled theme?
    if let Some(embedded) = parse_theme_name(name) {
        return Some(ts.get(embedded).clone());
    }
    // Custom .tmTheme file?
    if let Some(home) = codex_home
        && let Some(theme) = load_custom_theme(name, home)
    {
        return Some(theme);
    }
    None
}

/// A theme available in the picker, either bundled or loaded from a custom
/// `.tmTheme` file under `{CODEX_HOME}/themes/`.
pub(crate) struct ThemeEntry {
    /// Kebab-case identifier used for config persistence and theme resolution.
    pub name: String,
    /// `true` when this entry was discovered from a `.tmTheme` file on disk
    /// rather than the embedded two-face bundle.
    pub is_custom: bool,
}

/// List all available theme names: bundled themes + custom `.tmTheme` files
/// found in `{codex_home}/themes/`.
pub(crate) fn list_available_themes(codex_home: Option<&Path>) -> Vec<ThemeEntry> {
    let mut entries: Vec<ThemeEntry> = BUILTIN_THEME_NAMES
        .iter()
        .map(|name| ThemeEntry {
            name: name.to_string(),
            is_custom: false,
        })
        .collect();

    // Discover custom themes on disk, deduplicating against builtins.
    if let Some(home) = codex_home {
        let themes_dir = home.join("themes");
        if let Ok(read_dir) = std::fs::read_dir(&themes_dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("tmTheme")
                    && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                {
                    let name = stem.to_string();
                    let is_valid_theme = ThemeSet::get_theme(&path).is_ok();
                    if is_valid_theme && !entries.iter().any(|e| e.name == name) {
                        entries.push(ThemeEntry {
                            name,
                            is_custom: true,
                        });
                    }
                }
            }
        }
    }

    // Keep picker ordering stable across platforms/filesystems while sorting
    // custom and bundled themes together, case-insensitively.
    entries.sort_by_cached_key(|entry| (entry.name.to_ascii_lowercase(), entry.name.clone()));

    entries
}

/// All 32 bundled theme names in kebab-case, ordered alphabetically.
const BUILTIN_THEME_NAMES: &[&str] = &[
    "1337",
    "ansi",
    "base16",
    "base16-256",
    "base16-eighties-dark",
    "base16-mocha-dark",
    "base16-ocean-dark",
    "base16-ocean-light",
    "catppuccin-frappe",
    "catppuccin-latte",
    "catppuccin-macchiato",
    "catppuccin-mocha",
    "coldark-cold",
    "coldark-dark",
    "dark-neon",
    "dracula",
    "github",
    "gruvbox-dark",
    "gruvbox-light",
    "inspired-github",
    "monokai-extended",
    "monokai-extended-bright",
    "monokai-extended-light",
    "monokai-extended-origin",
    "nord",
    "one-half-dark",
    "one-half-light",
    "solarized-dark",
    "solarized-light",
    "sublime-snazzy",
    "two-dark",
    "zenburn",
];

// -- Style conversion (syntect -> ratatui) ------------------------------------

/// Convert a syntect `Style` to a ratatui `Style`.
///
/// Syntax highlighting themes inherently produce RGB colors, so we allow
/// `Color::Rgb` here despite the project-wide preference for ANSI colors.
#[allow(clippy::disallowed_methods)]
fn convert_style(syn_style: SyntectStyle) -> Style {
    let mut rt_style = Style::default();

    // Map foreground color when visible.
    let fg = syn_style.foreground;
    if fg.a > 0 {
        rt_style = rt_style.fg(RtColor::Rgb(fg.r, fg.g, fg.b));
    }
    // Intentionally skip background to avoid overwriting terminal bg.

    if syn_style.font_style.contains(FontStyle::BOLD) {
        rt_style.add_modifier |= Modifier::BOLD;
    }
    // Intentionally skip italic — many terminals render it poorly or not at all.
    // Intentionally skip underline — themes like Dracula use underline on type
    // scopes (entity.name.type, support.class) which produces distracting
    // underlines on type/module names in terminal output.

    rt_style
}

// -- Syntax lookup ------------------------------------------------------------

/// Try to find a syntect `SyntaxReference` for the given language identifier.
///
/// two-face's extended syntax set (~250 languages) resolves most names and
/// extensions directly.  We only patch the few aliases it cannot handle.
fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    let ss = syntax_set();

    // Aliases that two-face does not resolve on its own.
    let patched = match lang {
        "csharp" | "c-sharp" => "c#",
        "golang" => "go",
        "python3" => "python",
        "shell" => "bash",
        _ => lang,
    };

    // Try by token (matches file_extensions case-insensitively).
    if let Some(s) = ss.find_syntax_by_token(patched) {
        return Some(s);
    }
    // Try by exact syntax name (e.g. "Rust", "Python").
    if let Some(s) = ss.find_syntax_by_name(patched) {
        return Some(s);
    }
    // Try case-insensitive name match (e.g. "rust" -> "Rust").
    let lower = patched.to_ascii_lowercase();
    if let Some(s) = ss
        .syntaxes()
        .iter()
        .find(|s| s.name.to_ascii_lowercase() == lower)
    {
        return Some(s);
    }
    // Try raw input as file extension.
    if let Some(s) = ss.find_syntax_by_extension(lang) {
        return Some(s);
    }
    None
}

// -- Guardrail constants ------------------------------------------------------

/// Skip highlighting for inputs larger than 512 KB to avoid excessive memory
/// and CPU usage.  Callers fall back to plain unstyled text.
const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;

/// Skip highlighting for inputs with more than 10,000 lines.
const MAX_HIGHLIGHT_LINES: usize = 10_000;

/// Check whether an input exceeds the safe highlighting limits.
///
/// Callers that highlight content in a loop (e.g. per diff-line) should
/// pre-check the aggregate size with this function and skip highlighting
/// entirely when it returns `true`.
pub(crate) fn exceeds_highlight_limits(total_bytes: usize, total_lines: usize) -> bool {
    total_bytes > MAX_HIGHLIGHT_BYTES || total_lines > MAX_HIGHLIGHT_LINES
}

// -- Core highlighting --------------------------------------------------------

/// Parse `code` using syntect for `lang` and return per-line styled spans.
/// Each inner Vec represents one source line.  Returns None when the language
/// is not recognized or the input exceeds safety limits.
fn highlight_to_line_spans(code: &str, lang: &str) -> Option<Vec<Vec<Span<'static>>>> {
    // Empty input has nothing to highlight; fall back to the plain text path
    // which correctly produces a single empty Line.
    if code.is_empty() {
        return None;
    }

    // Bail out early for oversized inputs to avoid excessive resource usage.
    // Count actual lines (not newline bytes) to avoid an off-by-one when
    // the input does not end with a newline.
    if code.len() > MAX_HIGHLIGHT_BYTES || code.lines().count() > MAX_HIGHLIGHT_LINES {
        return None;
    }

    let syntax = find_syntax(lang)?;
    let theme_guard = match theme_lock().read() {
        Ok(theme_guard) => theme_guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut h = HighlightLines::new(syntax, &theme_guard);
    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();

    for line in LinesWithEndings::from(code) {
        let ranges = h.highlight_line(line, syntax_set()).ok()?;
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (style, text) in ranges {
            // Strip trailing line endings (LF and CR) since we handle line
            // breaks ourselves.  CRLF inputs would otherwise leave a stray \r.
            let text = text.trim_end_matches(['\n', '\r']);
            if text.is_empty() {
                continue;
            }
            spans.push(Span::styled(text.to_string(), convert_style(style)));
        }
        if spans.is_empty() {
            spans.push(Span::raw(String::new()));
        }
        lines.push(spans);
    }

    Some(lines)
}

// -- Public API ---------------------------------------------------------------

/// Highlight code in any supported language, returning styled ratatui `Line`s.
///
/// Falls back to plain unstyled text when the language is not recognized or the
/// input exceeds safety guardrails.  Callers can always render the result
/// directly -- the fallback path produces equivalent plain-text lines.
///
/// Used by `markdown_render` for fenced code blocks and by `exec_cell` for bash
/// command highlighting.
pub(crate) fn highlight_code_to_lines(code: &str, lang: &str) -> Vec<Line<'static>> {
    if let Some(line_spans) = highlight_to_line_spans(code, lang) {
        line_spans.into_iter().map(Line::from).collect()
    } else {
        // Fallback: plain text, one Line per source line.
        // Use `lines()` instead of `split('\n')` to avoid a phantom trailing
        // empty element when the input ends with '\n' (as pulldown-cmark emits).
        let mut result: Vec<Line<'static>> =
            code.lines().map(|l| Line::from(l.to_string())).collect();
        if result.is_empty() {
            result.push(Line::from(String::new()));
        }
        result
    }
}

/// Backward-compatible wrapper for bash highlighting used by exec cells.
pub(crate) fn highlight_bash_to_lines(script: &str) -> Vec<Line<'static>> {
    highlight_code_to_lines(script, "bash")
}

/// Highlight code and return per-line styled spans for diff integration.
///
/// Returns `None` when the language is unrecognized or the input exceeds
/// guardrails.  The caller (`diff_render`) uses this signal to fall back to
/// plain diff coloring.
///
/// Each inner `Vec<Span>` corresponds to one source line.  Styles are derived
/// from the active theme but backgrounds are intentionally omitted so the
/// terminal's own background shows through.
pub(crate) fn highlight_code_to_styled_spans(
    code: &str,
    lang: &str,
) -> Option<Vec<Vec<Span<'static>>>> {
    highlight_to_line_spans(code, lang)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn write_minimal_tmtheme(path: &Path) {
        // Minimal valid .tmTheme plist (enough for syntect to parse).
        std::fs::write(
            path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>name</key><string>Test</string>
<key>settings</key><array><dict>
<key>settings</key><dict>
<key>foreground</key><string>#FFFFFF</string>
<key>background</key><string>#000000</string>
</dict></dict></array>
</dict></plist>"#,
        )
        .unwrap();
    }

    /// Reconstruct plain text from highlighted Lines.
    fn reconstructed(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|sp| sp.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn highlight_rust_has_keyword_style() {
        let code = "fn main() {}";
        let lines = highlight_code_to_lines(code, "rust");
        assert_eq!(reconstructed(&lines), code);

        // The `fn` keyword should have a non-default style (some color).
        let fn_span = lines[0].spans.iter().find(|sp| sp.content.as_ref() == "fn");
        assert!(fn_span.is_some(), "expected a span containing 'fn'");
        let style = fn_span.map(|s| s.style).unwrap_or_default();
        assert!(
            style.fg.is_some() || style.add_modifier != Modifier::empty(),
            "expected fn keyword to have non-default style, got {style:?}"
        );
    }

    #[test]
    fn highlight_unknown_lang_falls_back() {
        let code = "some random text";
        let lines = highlight_code_to_lines(code, "xyzlang");
        assert_eq!(reconstructed(&lines), code);
        // Should be plain text with no styling.
        for line in &lines {
            for span in &line.spans {
                assert_eq!(
                    span.style,
                    Style::default(),
                    "expected default style for unknown language"
                );
            }
        }
    }

    #[test]
    fn fallback_trailing_newline_no_phantom_line() {
        // pulldown-cmark sends code block text ending with '\n'.
        // The fallback path (unknown language) must not produce a phantom
        // empty trailing line from that newline.
        let code = "hello world\n";
        let lines = highlight_code_to_lines(code, "xyzlang");
        assert_eq!(
            lines.len(),
            1,
            "trailing newline should not produce phantom blank line, got {lines:?}"
        );
        assert_eq!(reconstructed(&lines), "hello world");
    }

    #[test]
    fn highlight_empty_string() {
        let lines = highlight_code_to_lines("", "rust");
        assert_eq!(lines.len(), 1);
        assert_eq!(reconstructed(&lines), "");
    }

    #[test]
    fn highlight_bash_preserves_content() {
        let script = "echo \"hello world\" && ls -la | grep foo";
        let lines = highlight_bash_to_lines(script);
        assert_eq!(reconstructed(&lines), script);
    }

    #[test]
    fn highlight_crlf_strips_carriage_return() {
        // Windows-style \r\n line endings must not leave a trailing \r in
        // span text — that would propagate into rendered code blocks.
        let code = "fn main() {\r\n    println!(\"hi\");\r\n}\r\n";
        let lines = highlight_code_to_lines(code, "rust");
        for (i, line) in lines.iter().enumerate() {
            for span in &line.spans {
                assert!(
                    !span.content.contains('\r'),
                    "line {i} span {:?} contains \\r",
                    span.content,
                );
            }
        }
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn style_conversion_correctness() {
        let syn = SyntectStyle {
            foreground: syntect::highlighting::Color {
                r: 255,
                g: 128,
                b: 0,
                a: 255,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            font_style: FontStyle::BOLD | FontStyle::ITALIC,
        };
        let rt = convert_style(syn);
        assert_eq!(rt.fg, Some(RtColor::Rgb(255, 128, 0)));
        // Background is intentionally skipped.
        assert_eq!(rt.bg, None);
        assert!(rt.add_modifier.contains(Modifier::BOLD));
        // Italic is intentionally suppressed.
        assert!(!rt.add_modifier.contains(Modifier::ITALIC));
        assert!(!rt.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn convert_style_suppresses_underline() {
        // Dracula (and other themes) set FontStyle::UNDERLINE on type scopes,
        // producing distracting underlines on type names in terminal output.
        // convert_style must suppress underline, just like it suppresses italic.
        let syn = SyntectStyle {
            foreground: syntect::highlighting::Color {
                r: 100,
                g: 200,
                b: 150,
                a: 255,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            font_style: FontStyle::UNDERLINE,
        };
        let rt = convert_style(syn);
        assert!(
            !rt.add_modifier.contains(Modifier::UNDERLINED),
            "convert_style should suppress UNDERLINE from themes — \
             themes like Dracula use underline on type scopes which \
             looks wrong in terminal output"
        );
    }

    #[test]
    fn highlight_multiline_python() {
        let code = "def hello():\n    print(\"hi\")\n    return 42";
        let lines = highlight_code_to_lines(code, "python");
        assert_eq!(reconstructed(&lines), code);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn highlight_code_to_styled_spans_returns_none_for_unknown() {
        assert!(highlight_code_to_styled_spans("x", "xyzlang").is_none());
    }

    #[test]
    fn highlight_code_to_styled_spans_returns_some_for_known() {
        let result = highlight_code_to_styled_spans("let x = 1;", "rust");
        assert!(result.is_some());
        let spans = result.unwrap_or_default();
        assert!(!spans.is_empty());
    }

    #[test]
    fn highlight_markdown_preserves_content() {
        let code = "```sh\nprintf 'fenced within fenced\\n'\n```";
        let lines = highlight_code_to_lines(code, "markdown");
        let result = reconstructed(&lines);
        assert_eq!(
            result, code,
            "markdown highlighting must preserve content exactly"
        );
    }

    #[test]
    fn highlight_large_input_falls_back() {
        // Input exceeding MAX_HIGHLIGHT_BYTES should return None (plain text
        // fallback) rather than attempting to parse.
        let big = "x".repeat(MAX_HIGHLIGHT_BYTES + 1);
        let result = highlight_code_to_styled_spans(&big, "rust");
        assert!(result.is_none(), "oversized input should fall back to None");
    }

    #[test]
    fn highlight_many_lines_falls_back() {
        // Input exceeding MAX_HIGHLIGHT_LINES should return None.
        let many_lines = "let x = 1;\n".repeat(MAX_HIGHLIGHT_LINES + 1);
        let result = highlight_code_to_styled_spans(&many_lines, "rust");
        assert!(result.is_none(), "too many lines should fall back to None");
    }

    #[test]
    fn highlight_many_lines_no_trailing_newline_falls_back() {
        // A snippet with exactly MAX_HIGHLIGHT_LINES+1 lines but no trailing
        // newline has only MAX_HIGHLIGHT_LINES newline bytes.  The guard must
        // count actual lines, not newline bytes, to catch this.
        let mut code = "let x = 1;\n".repeat(MAX_HIGHLIGHT_LINES);
        code.push_str("let x = 1;"); // line MAX_HIGHLIGHT_LINES+1, no trailing \n
        assert_eq!(code.lines().count(), MAX_HIGHLIGHT_LINES + 1);
        let result = highlight_code_to_styled_spans(&code, "rust");
        assert!(
            result.is_none(),
            "MAX_HIGHLIGHT_LINES+1 lines without trailing newline should fall back"
        );
    }

    #[test]
    fn find_syntax_resolves_languages_and_aliases() {
        // Languages resolved directly by two-face's extended syntax set.
        let languages = [
            "javascript",
            "typescript",
            "tsx",
            "python",
            "ruby",
            "rust",
            "go",
            "c",
            "cpp",
            "yaml",
            "bash",
            "kotlin",
            "markdown",
            "sql",
            "lua",
            "zig",
            "swift",
            "java",
            "c#",
            "elixir",
            "haskell",
            "scala",
            "dart",
            "r",
            "perl",
            "php",
            "html",
            "css",
            "json",
            "toml",
            "xml",
            "dockerfile",
        ];
        for lang in languages {
            assert!(
                find_syntax(lang).is_some(),
                "find_syntax({lang:?}) returned None"
            );
        }
        // Common file extensions.
        let extensions = [
            "rs", "py", "js", "ts", "rb", "go", "sh", "md", "yml", "kt", "ex", "hs", "pl", "php",
            "css", "html", "cs",
        ];
        for ext in extensions {
            assert!(
                find_syntax(ext).is_some(),
                "find_syntax({ext:?}) returned None"
            );
        }
        // Patched aliases that two-face cannot resolve on its own.
        for alias in ["csharp", "c-sharp", "golang", "python3", "shell"] {
            assert!(
                find_syntax(alias).is_some(),
                "find_syntax({alias:?}) returned None — patched alias broken"
            );
        }
    }

    #[test]
    fn parse_theme_name_covers_all_variants() {
        let known = [
            ("ansi", EmbeddedThemeName::Ansi),
            ("base16", EmbeddedThemeName::Base16),
            (
                "base16-eighties-dark",
                EmbeddedThemeName::Base16EightiesDark,
            ),
            ("base16-mocha-dark", EmbeddedThemeName::Base16MochaDark),
            ("base16-ocean-dark", EmbeddedThemeName::Base16OceanDark),
            ("base16-ocean-light", EmbeddedThemeName::Base16OceanLight),
            ("base16-256", EmbeddedThemeName::Base16_256),
            ("catppuccin-frappe", EmbeddedThemeName::CatppuccinFrappe),
            ("catppuccin-latte", EmbeddedThemeName::CatppuccinLatte),
            (
                "catppuccin-macchiato",
                EmbeddedThemeName::CatppuccinMacchiato,
            ),
            ("catppuccin-mocha", EmbeddedThemeName::CatppuccinMocha),
            ("coldark-cold", EmbeddedThemeName::ColdarkCold),
            ("coldark-dark", EmbeddedThemeName::ColdarkDark),
            ("dark-neon", EmbeddedThemeName::DarkNeon),
            ("dracula", EmbeddedThemeName::Dracula),
            ("github", EmbeddedThemeName::Github),
            ("gruvbox-dark", EmbeddedThemeName::GruvboxDark),
            ("gruvbox-light", EmbeddedThemeName::GruvboxLight),
            ("inspired-github", EmbeddedThemeName::InspiredGithub),
            ("1337", EmbeddedThemeName::Leet),
            ("monokai-extended", EmbeddedThemeName::MonokaiExtended),
            (
                "monokai-extended-bright",
                EmbeddedThemeName::MonokaiExtendedBright,
            ),
            (
                "monokai-extended-light",
                EmbeddedThemeName::MonokaiExtendedLight,
            ),
            (
                "monokai-extended-origin",
                EmbeddedThemeName::MonokaiExtendedOrigin,
            ),
            ("nord", EmbeddedThemeName::Nord),
            ("one-half-dark", EmbeddedThemeName::OneHalfDark),
            ("one-half-light", EmbeddedThemeName::OneHalfLight),
            ("solarized-dark", EmbeddedThemeName::SolarizedDark),
            ("solarized-light", EmbeddedThemeName::SolarizedLight),
            ("sublime-snazzy", EmbeddedThemeName::SublimeSnazzy),
            ("two-dark", EmbeddedThemeName::TwoDark),
            ("zenburn", EmbeddedThemeName::Zenburn),
        ];
        for (kebab, expected) in &known {
            assert_eq!(
                parse_theme_name(kebab),
                Some(*expected),
                "parse_theme_name({kebab:?}) did not return expected variant"
            );
        }
    }

    #[test]
    fn parse_theme_name_returns_none_for_unknown() {
        assert_eq!(parse_theme_name("nonexistent-theme"), None);
        assert_eq!(parse_theme_name(""), None);
    }

    #[test]
    fn load_custom_theme_from_tmtheme_file() {
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir(&themes_dir).unwrap();
        write_minimal_tmtheme(&themes_dir.join("test-custom.tmTheme"));
        let theme = load_custom_theme("test-custom", dir.path());
        assert!(theme.is_some(), "should load .tmTheme from themes dir");
    }

    #[test]
    fn load_custom_theme_returns_none_for_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_custom_theme("nonexistent", dir.path()).is_none());
    }

    #[test]
    fn validate_theme_name_none_for_bundled() {
        // Bundled themes should never produce a warning.
        assert!(validate_theme_name(Some("dracula"), None).is_none());
        assert!(validate_theme_name(Some("nord"), Some(Path::new("/nonexistent"))).is_none());
    }

    #[test]
    fn validate_theme_name_none_when_no_override() {
        assert!(validate_theme_name(None, None).is_none());
    }

    #[test]
    fn validate_theme_name_warns_for_missing_custom() {
        let dir = tempfile::tempdir().unwrap();
        let warning = validate_theme_name(Some("my-fancy"), Some(dir.path()));
        assert!(warning.is_some(), "should warn when theme file is absent");
        let msg = warning.unwrap();
        assert!(
            msg.contains("my-fancy"),
            "warning should mention the theme name"
        );
    }

    #[test]
    fn validate_theme_name_none_when_custom_file_is_valid() {
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir(&themes_dir).unwrap();
        write_minimal_tmtheme(&themes_dir.join("my-fancy.tmTheme"));
        assert!(
            validate_theme_name(Some("my-fancy"), Some(dir.path())).is_none(),
            "should not warn when custom .tmTheme file parses successfully"
        );
    }

    #[test]
    fn validate_theme_name_warns_when_custom_file_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("my-fancy.tmTheme"), "placeholder").unwrap();
        let warning = validate_theme_name(Some("my-fancy"), Some(dir.path()));
        assert!(
            warning.is_some(),
            "should warn when custom .tmTheme exists but cannot be parsed"
        );
        assert!(
            warning
                .as_deref()
                .is_some_and(|msg| msg.contains("could not be parsed")),
            "warning should explain that the theme file is invalid"
        );
    }

    #[test]
    fn list_available_themes_excludes_invalid_custom_files() {
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir(&themes_dir).unwrap();
        write_minimal_tmtheme(&themes_dir.join("valid-custom.tmTheme"));
        std::fs::write(themes_dir.join("broken-custom.tmTheme"), "not a plist").unwrap();

        let entries = list_available_themes(Some(dir.path()));

        assert!(
            entries
                .iter()
                .any(|entry| entry.name == "valid-custom" && entry.is_custom),
            "expected valid custom theme to be listed"
        );
        assert!(
            !entries
                .iter()
                .any(|entry| entry.name == "broken-custom" && entry.is_custom),
            "expected invalid custom theme to be excluded from list"
        );
    }

    #[test]
    fn list_available_themes_returns_stable_sorted_order() {
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir(&themes_dir).unwrap();
        write_minimal_tmtheme(&themes_dir.join("zzz-custom.tmTheme"));
        write_minimal_tmtheme(&themes_dir.join("Aaa-custom.tmTheme"));
        write_minimal_tmtheme(&themes_dir.join("mmm-custom.tmTheme"));

        let entries = list_available_themes(Some(dir.path()));
        let actual: Vec<(bool, String)> = entries
            .iter()
            .map(|entry| (entry.is_custom, entry.name.clone()))
            .collect();

        let mut expected = actual.clone();
        expected.sort_by_cached_key(|entry| (entry.1.to_ascii_lowercase(), entry.1.clone()));

        assert_eq!(
            actual, expected,
            "theme entries should be stable and sorted case-insensitively across built-in and custom themes"
        );
    }

    #[test]
    fn parse_theme_name_is_exhaustive() {
        use two_face::theme::EmbeddedLazyThemeSet;

        // Every variant in the embedded set must be reachable via parse_theme_name.
        let all_variants = EmbeddedLazyThemeSet::theme_names();

        // Guard: if two-face adds themes, this test forces us to update the mapping.
        assert_eq!(
            all_variants.len(),
            32,
            "two-face theme count changed — update parse_theme_name"
        );

        // Build the set of variants reachable through our kebab-case mapping.
        let kebab_names = [
            "ansi",
            "base16",
            "base16-eighties-dark",
            "base16-mocha-dark",
            "base16-ocean-dark",
            "base16-ocean-light",
            "base16-256",
            "catppuccin-frappe",
            "catppuccin-latte",
            "catppuccin-macchiato",
            "catppuccin-mocha",
            "coldark-cold",
            "coldark-dark",
            "dark-neon",
            "dracula",
            "github",
            "gruvbox-dark",
            "gruvbox-light",
            "inspired-github",
            "1337",
            "monokai-extended",
            "monokai-extended-bright",
            "monokai-extended-light",
            "monokai-extended-origin",
            "nord",
            "one-half-dark",
            "one-half-light",
            "solarized-dark",
            "solarized-light",
            "sublime-snazzy",
            "two-dark",
            "zenburn",
        ];
        let mapped: Vec<EmbeddedThemeName> = kebab_names
            .iter()
            .map(|k| parse_theme_name(k).unwrap_or_else(|| panic!("unmapped kebab name: {k}")))
            .collect();

        // Every variant from two-face must appear in our mapped set.
        for variant in all_variants {
            assert!(
                mapped.contains(variant),
                "EmbeddedThemeName::{variant:?} has no kebab-case mapping in parse_theme_name"
            );
        }
    }
}
