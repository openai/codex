use std::{
    collections::{BTreeMap, HashSet},
    fmt::Write,
    fs,
    io::{self, Write as _},
    path::{self, Path, PathBuf},
    str,
    sync::{atomic::AtomicUsize, Arc},
    time::Instant,
};

use ansi_colours::{ansi256_from_rgb, rgb_from_ansi256};
use anstyle::{Ansi256Color, AnsiColor, Color, Effects, RgbColor};
use anyhow::Result;
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Value};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter, HtmlRenderer};
use tree_sitter_loader::Loader;

pub const HTML_HEAD_HEADER: &str = "
<!doctype HTML>
<head>
  <title>Tree-sitter Highlighting</title>
  <style>
    body {
      font-family: monospace
    }
    .line-number {
      user-select: none;
      text-align: right;
      color: rgba(27,31,35,.3);
      padding: 0 10px;
    }
    .line {
      white-space: pre;
    }
  </style>";

pub const HTML_BODY_HEADER: &str = "
</head>
<body>
";

pub const HTML_FOOTER: &str = "
</body>
";

#[derive(Debug, Default)]
pub struct Style {
    pub ansi: anstyle::Style,
    pub css: Option<String>,
}

#[derive(Debug)]
pub struct Theme {
    pub styles: Vec<Style>,
    pub highlight_names: Vec<String>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct ThemeConfig {
    #[serde(default)]
    pub theme: Theme,
}

impl Theme {
    pub fn load(path: &path::Path) -> io::Result<Self> {
        let json = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&json).unwrap_or_default())
    }

    #[must_use]
    pub fn default_style(&self) -> Style {
        Style::default()
    }
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut styles = Vec::new();
        let mut highlight_names = Vec::new();
        if let Ok(colors) = BTreeMap::<String, Value>::deserialize(deserializer) {
            styles.reserve(colors.len());
            highlight_names.reserve(colors.len());
            for (name, style_value) in colors {
                let mut style = Style::default();
                parse_style(&mut style, style_value);
                highlight_names.push(name);
                styles.push(style);
            }
        }
        Ok(Self {
            styles,
            highlight_names,
        })
    }
}

impl Serialize for Theme {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.styles.len()))?;
        for (name, style) in self.highlight_names.iter().zip(&self.styles) {
            let style = &style.ansi;
            let color = style.get_fg_color().map(|color| match color {
                Color::Ansi(color) => match color {
                    AnsiColor::Black => json!("black"),
                    AnsiColor::Blue => json!("blue"),
                    AnsiColor::Cyan => json!("cyan"),
                    AnsiColor::Green => json!("green"),
                    AnsiColor::Magenta => json!("purple"),
                    AnsiColor::Red => json!("red"),
                    AnsiColor::White => json!("white"),
                    AnsiColor::Yellow => json!("yellow"),
                    _ => unreachable!(),
                },
                Color::Ansi256(Ansi256Color(n)) => json!(n),
                Color::Rgb(RgbColor(r, g, b)) => json!(format!("#{r:x?}{g:x?}{b:x?}")),
            });
            let effects = style.get_effects();
            if effects.contains(Effects::BOLD)
                || effects.contains(Effects::ITALIC)
                || effects.contains(Effects::UNDERLINE)
            {
                let mut style_json = BTreeMap::new();
                if let Some(color) = color {
                    style_json.insert("color", color);
                }
                if effects.contains(Effects::BOLD) {
                    style_json.insert("bold", Value::Bool(true));
                }
                if effects.contains(Effects::ITALIC) {
                    style_json.insert("italic", Value::Bool(true));
                }
                if effects.contains(Effects::UNDERLINE) {
                    style_json.insert("underline", Value::Bool(true));
                }
                map.serialize_entry(&name, &style_json)?;
            } else if let Some(color) = color {
                map.serialize_entry(&name, &color)?;
            } else {
                map.serialize_entry(&name, &Value::Null)?;
            }
        }
        map.end()
    }
}

impl Default for Theme {
    fn default() -> Self {
        serde_json::from_value(json!({
            "attribute": {"color": 124, "italic": true},
            "comment": {"color": 245, "italic": true},
            "constant": 94,
            "constant.builtin": {"color": 94, "bold": true},
            "constructor": 136,
            "embedded": null,
            "function": 26,
            "function.builtin": {"color": 26, "bold": true},
            "keyword": 56,
            "module": 136,
            "number": {"color": 94, "bold": true},
            "operator": {"color": 239, "bold": true},
            "property": 124,
            "property.builtin": {"color": 124, "bold": true},
            "punctuation": 239,
            "punctuation.bracket": 239,
            "punctuation.delimiter": 239,
            "punctuation.special": 239,
            "string": 28,
            "string.special": 30,
            "tag": 18,
            "type": 23,
            "type.builtin": {"color": 23, "bold": true},
            "variable": 252,
            "variable.builtin": {"color": 252, "bold": true},
            "variable.parameter": {"color": 252, "underline": true}
        }))
        .unwrap()
    }
}

fn parse_style(style: &mut Style, json: Value) {
    if let Value::Object(entries) = json {
        for (property_name, value) in entries {
            match property_name.as_str() {
                "bold" => {
                    if value == Value::Bool(true) {
                        style.ansi = style.ansi.bold();
                    }
                }
                "italic" => {
                    if value == Value::Bool(true) {
                        style.ansi = style.ansi.italic();
                    }
                }
                "underline" => {
                    if value == Value::Bool(true) {
                        style.ansi = style.ansi.underline();
                    }
                }
                "color" => {
                    if let Some(color) = parse_color(value) {
                        style.ansi = style.ansi.fg_color(Some(color));
                    }
                }
                _ => {}
            }
        }
        style.css = Some(style_to_css(style.ansi));
    } else if let Some(color) = parse_color(json) {
        style.ansi = style.ansi.fg_color(Some(color));
        style.css = Some(style_to_css(style.ansi));
    } else {
        style.css = None;
    }

    if let Some(Color::Rgb(RgbColor(red, green, blue))) = style.ansi.get_fg_color() {
        if !terminal_supports_truecolor() {
            let ansi256 = Color::Ansi256(Ansi256Color(ansi256_from_rgb((red, green, blue))));
            style.ansi = style.ansi.fg_color(Some(ansi256));
        }
    }
}

fn parse_color(json: Value) -> Option<Color> {
    match json {
        Value::Number(n) => n.as_u64().map(|n| Color::Ansi256(Ansi256Color(n as u8))),
        Value::String(s) => match s.to_lowercase().as_str() {
            "black" => Some(Color::Ansi(AnsiColor::Black)),
            "blue" => Some(Color::Ansi(AnsiColor::Blue)),
            "cyan" => Some(Color::Ansi(AnsiColor::Cyan)),
            "green" => Some(Color::Ansi(AnsiColor::Green)),
            "purple" => Some(Color::Ansi(AnsiColor::Magenta)),
            "red" => Some(Color::Ansi(AnsiColor::Red)),
            "white" => Some(Color::Ansi(AnsiColor::White)),
            "yellow" => Some(Color::Ansi(AnsiColor::Yellow)),
            s => {
                if let Some((red, green, blue)) = hex_string_to_rgb(s) {
                    Some(Color::Rgb(RgbColor(red, green, blue)))
                } else {
                    None
                }
            }
        },
        _ => None,
    }
}

fn hex_string_to_rgb(s: &str) -> Option<(u8, u8, u8)> {
    if s.starts_with('#') && s.len() >= 7 {
        if let (Ok(red), Ok(green), Ok(blue)) = (
            u8::from_str_radix(&s[1..3], 16),
            u8::from_str_radix(&s[3..5], 16),
            u8::from_str_radix(&s[5..7], 16),
        ) {
            Some((red, green, blue))
        } else {
            None
        }
    } else {
        None
    }
}

fn style_to_css(style: anstyle::Style) -> String {
    let mut result = String::new();
    let effects = style.get_effects();
    if effects.contains(Effects::UNDERLINE) {
        write!(&mut result, "text-decoration: underline;").unwrap();
    }
    if effects.contains(Effects::BOLD) {
        write!(&mut result, "font-weight: bold;").unwrap();
    }
    if effects.contains(Effects::ITALIC) {
        write!(&mut result, "font-style: italic;").unwrap();
    }
    if let Some(color) = style.get_fg_color() {
        write_color(&mut result, color);
    }
    result
}

fn write_color(buffer: &mut String, color: Color) {
    match color {
        Color::Ansi(color) => match color {
            AnsiColor::Black => write!(buffer, "color: black").unwrap(),
            AnsiColor::Red => write!(buffer, "color: red").unwrap(),
            AnsiColor::Green => write!(buffer, "color: green").unwrap(),
            AnsiColor::Yellow => write!(buffer, "color: yellow").unwrap(),
            AnsiColor::Blue => write!(buffer, "color: blue").unwrap(),
            AnsiColor::Magenta => write!(buffer, "color: purple").unwrap(),
            AnsiColor::Cyan => write!(buffer, "color: cyan").unwrap(),
            AnsiColor::White => write!(buffer, "color: white").unwrap(),
            _ => unreachable!(),
        },
        Color::Ansi256(Ansi256Color(n)) => {
            let (r, g, b) = rgb_from_ansi256(n);
            write!(buffer, "color: #{r:02x}{g:02x}{b:02x}").unwrap();
        }
        Color::Rgb(RgbColor(r, g, b)) => write!(buffer, "color: #{r:02x}{g:02x}{b:02x}").unwrap(),
    }
}

fn terminal_supports_truecolor() -> bool {
    std::env::var("COLORTERM")
        .is_ok_and(|truecolor| truecolor == "truecolor" || truecolor == "24bit")
}

pub struct HighlightOptions {
    pub theme: Theme,
    pub check: bool,
    pub captures_path: Option<PathBuf>,
    pub inline_styles: bool,
    pub html: bool,
    pub quiet: bool,
    pub print_time: bool,
    pub cancellation_flag: Arc<AtomicUsize>,
}

pub fn highlight(
    loader: &Loader,
    path: &Path,
    name: &str,
    config: &HighlightConfiguration,
    print_name: bool,
    opts: &HighlightOptions,
) -> Result<()> {
    if opts.check {
        let names = if let Some(path) = opts.captures_path.as_deref() {
            let file = fs::read_to_string(path)?;
            let capture_names = file
                .lines()
                .filter_map(|line| {
                    if line.trim().is_empty() || line.trim().starts_with(';') {
                        return None;
                    }
                    line.split(';').next().map(|s| s.trim().trim_matches('"'))
                })
                .collect::<HashSet<_>>();
            config.nonconformant_capture_names(&capture_names)
        } else {
            config.nonconformant_capture_names(&HashSet::new())
        };
        if names.is_empty() {
            eprintln!("All highlight captures conform to standards.");
        } else {
            eprintln!(
                "Non-standard highlight {} detected:",
                if names.len() > 1 {
                    "captures"
                } else {
                    "capture"
                }
            );
            for name in names {
                eprintln!("* {name}");
            }
        }
    }

    let source = fs::read(path)?;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let time = Instant::now();
    let mut highlighter = Highlighter::new();
    let events =
        highlighter.highlight(config, &source, Some(&opts.cancellation_flag), |string| {
            loader.highlight_config_for_injection_string(string)
        })?;
    let theme = &opts.theme;

    if !opts.quiet && print_name {
        writeln!(&mut stdout, "{name}")?;
    }

    if opts.html {
        if !opts.quiet {
            writeln!(&mut stdout, "{HTML_HEAD_HEADER}")?;
            writeln!(&mut stdout, "  <style>")?;
            let names = theme.highlight_names.iter();
            let styles = theme.styles.iter();
            for (name, style) in names.zip(styles) {
                if let Some(css) = &style.css {
                    writeln!(&mut stdout, "    .{name} {{ {css}; }}")?;
                }
            }
            writeln!(&mut stdout, "  </style>")?;
            writeln!(&mut stdout, "{HTML_BODY_HEADER}")?;
        }

        let mut renderer = HtmlRenderer::new();
        renderer.render(events, &source, &move |highlight, output| {
            if opts.inline_styles {
                output.extend(b"style='");
                output.extend(
                    theme.styles[highlight.0]
                        .css
                        .as_ref()
                        .map_or_else(|| "".as_bytes(), |css_style| css_style.as_bytes()),
                );
                output.extend(b"'");
            } else {
                output.extend(b"class='");
                let mut parts = theme.highlight_names[highlight.0].split('.').peekable();
                while let Some(part) = parts.next() {
                    output.extend(part.as_bytes());
                    if parts.peek().is_some() {
                        output.extend(b" ");
                    }
                }
                output.extend(b"'");
            }
        })?;

        if !opts.quiet {
            writeln!(&mut stdout, "<table>")?;
            for (i, line) in renderer.lines().enumerate() {
                writeln!(
                    &mut stdout,
                    "<tr><td class=line-number>{}</td><td class=line>{line}</td></tr>",
                    i + 1,
                )?;
            }
            writeln!(&mut stdout, "</table>")?;
            writeln!(&mut stdout, "{HTML_FOOTER}")?;
        }
    } else {
        let mut style_stack = vec![theme.default_style().ansi];
        for event in events {
            match event? {
                HighlightEvent::HighlightStart(highlight) => {
                    style_stack.push(theme.styles[highlight.0].ansi);
                }
                HighlightEvent::HighlightEnd => {
                    style_stack.pop();
                }
                HighlightEvent::Source { start, end } => {
                    let style = style_stack.last().unwrap();
                    write!(&mut stdout, "{style}").unwrap();
                    stdout.write_all(&source[start..end])?;
                    write!(&mut stdout, "{style:#}").unwrap();
                }
            }
        }
    }

    if opts.print_time {
        eprintln!("Time: {}ms", time.elapsed().as_millis());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    const JUNGLE_GREEN: &str = "#26A69A";
    const DARK_CYAN: &str = "#00AF87";

    #[test]
    fn test_parse_style() {
        let original_environment_variable = env::var("COLORTERM");

        let mut style = Style::default();
        assert_eq!(style.ansi.get_fg_color(), None);
        assert_eq!(style.css, None);

        // darkcyan is an ANSI color and is preserved
        env::set_var("COLORTERM", "");
        parse_style(&mut style, Value::String(DARK_CYAN.to_string()));
        assert_eq!(
            style.ansi.get_fg_color(),
            Some(Color::Ansi256(Ansi256Color(36)))
        );
        assert_eq!(style.css, Some("color: #00af87".to_string()));

        // junglegreen is not an ANSI color and is preserved when the terminal supports it
        env::set_var("COLORTERM", "truecolor");
        parse_style(&mut style, Value::String(JUNGLE_GREEN.to_string()));
        assert_eq!(
            style.ansi.get_fg_color(),
            Some(Color::Rgb(RgbColor(38, 166, 154)))
        );
        assert_eq!(style.css, Some("color: #26a69a".to_string()));

        // junglegreen gets approximated as cadetblue when the terminal does not support it
        env::set_var("COLORTERM", "");
        parse_style(&mut style, Value::String(JUNGLE_GREEN.to_string()));
        assert_eq!(
            style.ansi.get_fg_color(),
            Some(Color::Ansi256(Ansi256Color(72)))
        );
        assert_eq!(style.css, Some("color: #26a69a".to_string()));

        if let Ok(environment_variable) = original_environment_variable {
            env::set_var("COLORTERM", environment_variable);
        } else {
            env::remove_var("COLORTERM");
        }
    }
}
