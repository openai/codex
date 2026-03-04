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
use syntect::highlighting::Color as SyntectColor;
use syntect::highlighting::FontStyle;
use syntect::highlighting::Highlighter;
use syntect::highlighting::Style as SyntectStyle;
use syntect::highlighting::Theme;
use syntect::highlighting::ThemeSet;
use syntect::parsing::Scope;
use syntect::parsing::SyntaxReference;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use two_face::theme::EmbeddedThemeName;

// -- Global singletons -------------------------------------------------------

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<RwLock<Theme>> = OnceLock::new();
static THEME_OVERRIDE: OnceLock<Option<String>> = OnceLock::new();
static CODEX_HOME: OnceLock<Option<PathBuf>> = OnceLock::new();

// Syntect/bat encode ANSI palette semantics in alpha:
// `a=0` => indexed ANSI palette via RGB payload, `a=1` => terminal default.
const ANSI_ALPHA_INDEX: u8 = 0x00;
const ANSI_ALPHA_DEFAULT: u8 = 0x01;
const OPAQUE_ALPHA: u8 = 0xFF;

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(two_face::syntax::extra_newlines)
}

// NOTE: We intentionally do NOT emit a runtime diagnostic when an ANSI-family
// theme (ansi, base16, base16-256) lacks the expected alpha-channel marker
// encoding.  If the upstream two_face/syntect theme format changes, the
// `ansi_themes_use_only_ansi_palette_colors` test will catch it at build
// time — long before it reaches users.  A runtime warning would be
// unactionable noise since users can't fix upstream themes.

/// Set the user-configured syntax theme override and codex home path.
///
/// Call this with the **final resolved config** (after onboarding, resume, and
/// fork reloads complete). The first call persists `name` and `codex_home` in
/// `OnceLock`s used by startup/default theme resolution.
///
/// Subsequent calls cannot change the persisted `OnceLock` values, but they
/// still update the runtime theme immediately for live preview flows.
///
/// Returns user-facing warnings for actionable configuration issues, such as
/// unknown/invalid theme names or duplicate override persistence.
pub(crate) fn set_theme_override(
    name: Option<String>,
    codex_home: Option<PathBuf>,
) -> Option<String> {
    let warning = validate_theme_name(name.as_deref(), codex_home.as_deref());
    let override_set_ok = THEME_OVERRIDE.set(name.clone()).is_ok();
    let codex_home_set_ok = CODEX_HOME.set(codex_home.clone()).is_ok();
    if THEME.get().is_some() {
        set_syntax_theme(resolve_theme_with_override(
            name.as_deref(),
            codex_home.as_deref(),
        ));
    }
    if !override_set_ok || !codex_home_set_ok {
        // This should never happen in practice — set_theme_override is only
        // called once at startup.  Keep as a debug breadcrumb in case a second
        // call site is added in the future.
        tracing::debug!("set_theme_override called more than once; OnceLock values unchanged");
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
                "Custom theme \"{name}\" at {custom_theme_path_display} could not \
                 be loaded (invalid .tmTheme format). Falling back to the default theme."
            ));
        }
    }
    Some(format!(
        "Theme \"{name}\" not found. Using the default theme. \
         To use a custom theme, place a .tmTheme file at \
         {custom_theme_path_display}."
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

/// Build the theme from current override/default-theme settings.
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
        tracing::debug!("Theme \"{name}\" not recognized; using default theme");
    }

    ts.get(adaptive_default_embedded_theme_name()).clone()
}

/// Build the theme from current override/default-theme settings.
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

/// Raw RGB background colors extracted from syntax theme diff/markup scopes.
///
/// These are theme-provided colors, not yet adapted for any particular color
/// depth.  [`diff_render`](crate::diff_render) converts them to ratatui
/// `Color` values via `color_from_rgb_for_level` after deciding whether to
/// emit truecolor or quantized ANSI-256.
///
/// Both fields are `None` when the active theme defines no relevant scope
/// backgrounds, in which case the diff renderer falls back to its hardcoded
/// palette.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DiffScopeBackgroundRgbs {
    pub inserted: Option<(u8, u8, u8)>,
    pub deleted: Option<(u8, u8, u8)>,
}

/// Query the active syntax theme for diff-scope background colors.
///
/// Prefers `markup.inserted` / `markup.deleted` (the TextMate convention used
/// by most VS Code themes) and falls back to `diff.inserted` / `diff.deleted`
/// (used by some older `.tmTheme` files).
pub(crate) fn diff_scope_background_rgbs() -> DiffScopeBackgroundRgbs {
    let theme = current_syntax_theme();
    diff_scope_background_rgbs_for_theme(&theme)
}

/// Pure extraction helper, separated from the global theme singleton so tests
/// can pass arbitrary themes.
fn diff_scope_background_rgbs_for_theme(theme: &Theme) -> DiffScopeBackgroundRgbs {
    let highlighter = Highlighter::new(theme);
    let inserted = scope_background_rgb(&highlighter, "markup.inserted")
        .or_else(|| scope_background_rgb(&highlighter, "diff.inserted"));
    let deleted = scope_background_rgb(&highlighter, "markup.deleted")
        .or_else(|| scope_background_rgb(&highlighter, "diff.deleted"));
    DiffScopeBackgroundRgbs { inserted, deleted }
}

/// Extract the background color for a single TextMate scope, if defined.
fn scope_background_rgb(highlighter: &Highlighter<'_>, scope_name: &str) -> Option<(u8, u8, u8)> {
    let scope = Scope::new(scope_name).ok()?;
    let bg = highlighter.style_mod_for_stack(&[scope]).background?;
    Some((bg.r, bg.g, bg.b))
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

/// Map a low ANSI palette index (0–7) to ratatui's named color variants,
/// falling back to `Indexed(n)` for indices 8–255.
///
/// Named variants are preferred over `Indexed(0)`…`Indexed(7)` because many
/// terminals apply bold/bright treatment differently for named vs indexed
/// colors, and ANSI themes expect the named behavior.
///
/// `clippy::disallowed_methods` is explicitly allowed here because this helper
/// intentionally constructs `ratatui::style::Color::Indexed`.
#[allow(clippy::disallowed_methods)]
fn ansi_palette_color(index: u8) -> RtColor {
    match index {
        0x00 => RtColor::Black,
        0x01 => RtColor::Red,
        0x02 => RtColor::Green,
        0x03 => RtColor::Yellow,
        0x04 => RtColor::Blue,
        0x05 => RtColor::Magenta,
        0x06 => RtColor::Cyan,
        // ANSI code 37 is "white", represented as `Gray` in ratatui.
        0x07 => RtColor::Gray,
        n => RtColor::Indexed(n),
    }
}

/// Decode a syntect foreground `Color` into a ratatui color, respecting the
/// alpha-channel encoding that bat's `ansi`, `base16`, and `base16-256` themes
/// use to signal ANSI palette semantics instead of true RGB.
///
/// Returns `None` when the color signals "use the terminal's default
/// foreground", allowing the caller to omit the foreground attribute entirely.
///
/// Passing a color from a standard RGB theme (alpha 0xFF) returns
/// `Some(Rgb(..))`, so this function is backward-compatible with non-ANSI
/// themes. Unexpected intermediate alpha values are treated as RGB.
///
/// `clippy::disallowed_methods` is explicitly allowed here because this helper
/// intentionally constructs `ratatui::style::Color::Rgb`.
#[allow(clippy::disallowed_methods)]
fn convert_syntect_color(color: SyntectColor) -> Option<RtColor> {
    match color.a {
        // Bat-compatible encoding used by `ansi`, `base16`, and `base16-256`:
        // alpha 0x00 means `r` stores an ANSI palette index, not RGB red.
        ANSI_ALPHA_INDEX => Some(ansi_palette_color(color.r)),
        // alpha 0x01 means "use terminal default foreground/background".
        ANSI_ALPHA_DEFAULT => None,
        OPAQUE_ALPHA => Some(RtColor::Rgb(color.r, color.g, color.b)),
        // Non-ANSI alpha values appear in some bundled themes; treat as plain RGB.
        _ => Some(RtColor::Rgb(color.r, color.g, color.b)),
    }
}

/// Convert a syntect `Style` to a ratatui `Style`.
///
/// Most themes produce RGB colors. The built-in `ansi`/`base16`/`base16-256`
/// themes encode ANSI palette semantics in the alpha channel, matching bat.
fn convert_style(syn_style: SyntectStyle) -> Style {
    let mut rt_style = Style::default();

    if let Some(fg) = convert_syntect_color(syn_style.foreground) {
        rt_style = rt_style.fg(fg);
    }
    // Intentionally skip background to avoid overwriting terminal bg.
    // If background support is added later, decode with `convert_syntect_color`
    // to reuse the same alpha-marker semantics as foreground.

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

/// Core highlighter that accepts an explicit theme reference.
///
/// This keeps production behavior and test behavior on the same code path:
/// production callers pass the global theme lock, while tests can pass a
/// concrete theme without mutating process-global state.
fn highlight_to_line_spans_with_theme(
    code: &str,
    lang: &str,
    theme: &Theme,
) -> Option<Vec<Vec<Span<'static>>>> {
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
    let mut h = HighlightLines::new(syntax, theme);
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

/// Parse `code` using syntect for `lang` and return per-line styled spans.
/// Each inner Vec represents one source line.  Returns None when the language
/// is not recognized or the input exceeds safety limits.
fn highlight_to_line_spans(code: &str, lang: &str) -> Option<Vec<Vec<Span<'static>>>> {
    let theme_guard = match theme_lock().read() {
        Ok(theme_guard) => theme_guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    highlight_to_line_spans_with_theme(code, lang, &theme_guard)
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
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;
    use syntect::highlighting::Color as SyntectColor;
    use syntect::highlighting::ScopeSelectors;
    use syntect::highlighting::StyleModifier;
    use syntect::highlighting::ThemeItem;
    use syntect::highlighting::ThemeSettings;

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

    fn write_tmtheme_with_diff_backgrounds(
        path: &Path,
        inserted_scope: &str,
        inserted_background: &str,
        deleted_scope: &str,
        deleted_background: &str,
    ) {
        let contents = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>name</key><string>Custom Diff Theme</string>
<key>settings</key><array>
<dict>
<key>settings</key><dict>
<key>foreground</key><string>#FFFFFF</string>
<key>background</key><string>#000000</string>
</dict>
</dict>
<dict>
<key>scope</key><string>{inserted_scope}</string>
<key>settings</key><dict>
<key>background</key><string>{inserted_background}</string>
</dict>
</dict>
<dict>
<key>scope</key><string>{deleted_scope}</string>
<key>settings</key><dict>
<key>background</key><string>{deleted_background}</string>
</dict>
</dict>
</array>
</dict></plist>"#
        );
        std::fs::write(path, contents).unwrap();
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

    fn unique_foreground_colors_for_theme(theme_name: &str) -> Vec<String> {
        let theme = resolve_theme_by_name(theme_name, None)
            .unwrap_or_else(|| panic!("expected built-in theme {theme_name} to resolve"));
        let lines = highlight_to_line_spans_with_theme(
            "fn main() { let answer = 42; println!(\"hello\"); }\n",
            "rust",
            &theme,
        )
        .expect("expected highlighted spans");
        let mut colors: Vec<String> = lines
            .iter()
            .flat_map(|line| line.iter().filter_map(|span| span.style.fg))
            .map(|fg| format!("{fg:?}"))
            .collect();
        colors.sort();
        colors.dedup();
        colors
    }

    fn theme_item(scope: &str, background: Option<(u8, u8, u8)>) -> ThemeItem {
        ThemeItem {
            scope: ScopeSelectors::from_str(scope).expect("scope selector should parse"),
            style: StyleModifier {
                background: background.map(|(r, g, b)| SyntectColor { r, g, b, a: 255 }),
                ..StyleModifier::default()
            },
        }
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
                a: 0xFF,
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
    fn style_conversion_uses_ansi_named_color_when_alpha_is_zero_low_index() {
        let syn = SyntectStyle {
            foreground: syntect::highlighting::Color {
                r: 0x02,
                g: 0,
                b: 0,
                a: 0,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0xFF,
            },
            font_style: FontStyle::empty(),
        };
        let rt = convert_style(syn);
        assert_eq!(rt.fg, Some(RtColor::Green));
    }

    #[test]
    fn style_conversion_uses_indexed_color_when_alpha_is_zero_high_index() {
        let syn = SyntectStyle {
            foreground: syntect::highlighting::Color {
                r: 0x9a,
                g: 0,
                b: 0,
                a: 0,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0xFF,
            },
            font_style: FontStyle::empty(),
        };
        let rt = convert_style(syn);
        assert!(matches!(rt.fg, Some(RtColor::Indexed(0x9a))));
    }

    #[test]
    fn style_conversion_uses_terminal_default_when_alpha_is_one() {
        let syn = SyntectStyle {
            foreground: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 1,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0xFF,
            },
            font_style: FontStyle::empty(),
        };
        let rt = convert_style(syn);
        assert_eq!(rt.fg, None);
    }

    #[test]
    fn style_conversion_unexpected_alpha_falls_back_to_rgb() {
        let syn = SyntectStyle {
            foreground: syntect::highlighting::Color {
                r: 10,
                g: 20,
                b: 30,
                a: 0x80,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0xFF,
            },
            font_style: FontStyle::empty(),
        };
        let rt = convert_style(syn);
        assert!(matches!(rt.fg, Some(RtColor::Rgb(10, 20, 30))));
    }

    #[test]
    fn ansi_palette_color_maps_ansi_white_to_gray() {
        assert_eq!(ansi_palette_color(0x07), RtColor::Gray);
    }

    #[test]
    fn ansi_family_themes_use_terminal_palette_colors_not_rgb() {
        for theme_name in ["ansi", "base16", "base16-256"] {
            let theme = resolve_theme_by_name(theme_name, None)
                .unwrap_or_else(|| panic!("expected built-in theme {theme_name} to resolve"));
            let lines = highlight_to_line_spans_with_theme(
                "fn main() { let answer = 42; println!(\"hello\"); }\n",
                "rust",
                &theme,
            )
            .expect("expected highlighted spans");
            let mut has_non_default_fg = false;
            for line in &lines {
                for span in line {
                    match span.style.fg {
                        Some(RtColor::Rgb(..)) => {
                            panic!("theme {theme_name} produced RGB foreground: {span:?}")
                        }
                        Some(_) => has_non_default_fg = true,
                        None => {}
                    }
                }
            }
            assert!(
                has_non_default_fg,
                "theme {theme_name} should produce at least one non-default foreground color"
            );
        }
    }

    #[test]
    fn ansi_family_foreground_palette_snapshot() {
        let mut out = String::new();
        for theme_name in ["ansi", "base16", "base16-256"] {
            let colors = unique_foreground_colors_for_theme(theme_name);
            out.push_str(&format!("{theme_name}:\n"));
            for color in colors {
                out.push_str(&format!("  {color}\n"));
            }
        }
        assert_snapshot!("ansi_family_foreground_palette", out);
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
    fn diff_scope_backgrounds_prefer_markup_scope_then_diff_fallback() {
        let theme = Theme {
            settings: ThemeSettings::default(),
            scopes: vec![
                theme_item("markup.inserted", Some((10, 20, 30))),
                theme_item("diff.deleted", Some((40, 50, 60))),
            ],
            ..Theme::default()
        };
        let rgbs = diff_scope_background_rgbs_for_theme(&theme);
        assert_eq!(
            rgbs,
            DiffScopeBackgroundRgbs {
                inserted: Some((10, 20, 30)),
                deleted: Some((40, 50, 60)),
            }
        );
    }

    #[test]
    fn diff_scope_backgrounds_return_none_when_no_background_scope_matches() {
        let theme = Theme {
            settings: ThemeSettings::default(),
            scopes: vec![theme_item("constant.numeric", Some((1, 2, 3)))],
            ..Theme::default()
        };
        let rgbs = diff_scope_background_rgbs_for_theme(&theme);
        assert_eq!(
            rgbs,
            DiffScopeBackgroundRgbs {
                inserted: None,
                deleted: None,
            }
        );
    }

    #[test]
    fn bundled_theme_can_provide_diff_scope_backgrounds() {
        let theme =
            resolve_theme_by_name("github", None).expect("expected built-in GitHub theme to load");
        let rgbs = diff_scope_background_rgbs_for_theme(&theme);
        assert!(
            rgbs.inserted.is_some() && rgbs.deleted.is_some(),
            "expected built-in theme to provide insert/delete backgrounds, got {rgbs:?}"
        );
    }

    #[test]
    fn custom_tmtheme_diff_scope_backgrounds_are_resolved() {
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir(&themes_dir).unwrap();
        write_tmtheme_with_diff_backgrounds(
            &themes_dir.join("custom-diff.tmTheme"),
            "diff.inserted",
            "#102030",
            "markup.deleted",
            "#405060",
        );

        let theme = resolve_theme_by_name("custom-diff", Some(dir.path()))
            .expect("expected custom theme to resolve");
        let rgbs = diff_scope_background_rgbs_for_theme(&theme);
        assert_eq!(
            rgbs,
            DiffScopeBackgroundRgbs {
                inserted: Some((16, 32, 48)),
                deleted: Some((64, 80, 96)),
            }
        );
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
                .is_some_and(|msg| msg.contains("could not be loaded")),
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
