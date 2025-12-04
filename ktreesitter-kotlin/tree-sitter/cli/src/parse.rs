use std::{
    fmt, fs,
    io::{self, StdoutLock, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use anstyle::{AnsiColor, Color, RgbColor};
use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use tree_sitter::{
    ffi, InputEdit, Language, LogType, ParseOptions, ParseState, Parser, Point, Range, Tree,
    TreeCursor,
};

use super::util;
use crate::{fuzz::edits::Edit, test::paint};

#[derive(Debug, Default, Serialize)]
pub struct Stats {
    pub successful_parses: usize,
    pub total_parses: usize,
    pub total_bytes: usize,
    pub total_duration: Duration,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let duration_us = self.total_duration.as_micros();
        let success_rate = if self.total_parses > 0 {
            format!(
                "{:.2}%",
                ((self.successful_parses as f64) / (self.total_parses as f64)) * 100.0,
            )
        } else {
            "N/A".to_string()
        };
        let duration_str = match (self.total_parses, duration_us) {
            (0, _) => "N/A".to_string(),
            (_, 0) => "0 bytes/ms".to_string(),
            (_, _) => format!(
                "{} bytes/ms",
                ((self.total_bytes as u128) * 1_000) / duration_us
            ),
        };
        writeln!(
            f,
            "Total parses: {}; successful parses: {}; failed parses: {}; success percentage: {success_rate}; average speed: {duration_str}",
            self.total_parses,
            self.successful_parses,
            self.total_parses - self.successful_parses,
        )
    }
}

/// Sets the color used in the output of `tree-sitter parse --cst`
#[derive(Debug, Copy, Clone)]
pub struct ParseTheme {
    /// The color of node kinds
    pub node_kind: Option<Color>,
    /// The color of text associated with a node
    pub node_text: Option<Color>,
    /// The color of node fields
    pub field: Option<Color>,
    /// The color of the range information for unnamed nodes
    pub row_color: Option<Color>,
    /// The color of the range information for named nodes
    pub row_color_named: Option<Color>,
    /// The color of extra nodes
    pub extra: Option<Color>,
    /// The color of ERROR nodes
    pub error: Option<Color>,
    /// The color of MISSING nodes and their associated text
    pub missing: Option<Color>,
    /// The color of newline characters
    pub line_feed: Option<Color>,
    /// The color of backticks
    pub backtick: Option<Color>,
    /// The color of literals
    pub literal: Option<Color>,
}

impl ParseTheme {
    const GRAY: Color = Color::Rgb(RgbColor(118, 118, 118));
    const LIGHT_GRAY: Color = Color::Rgb(RgbColor(166, 172, 181));
    const ORANGE: Color = Color::Rgb(RgbColor(255, 153, 51));
    const YELLOW: Color = Color::Rgb(RgbColor(219, 219, 173));
    const GREEN: Color = Color::Rgb(RgbColor(101, 192, 67));

    #[must_use]
    pub const fn empty() -> Self {
        Self {
            node_kind: None,
            node_text: None,
            field: None,
            row_color: None,
            row_color_named: None,
            extra: None,
            error: None,
            missing: None,
            line_feed: None,
            backtick: None,
            literal: None,
        }
    }
}

impl Default for ParseTheme {
    fn default() -> Self {
        Self {
            node_kind: Some(AnsiColor::BrightCyan.into()),
            node_text: Some(Self::GRAY),
            field: Some(AnsiColor::Blue.into()),
            row_color: Some(AnsiColor::White.into()),
            row_color_named: Some(AnsiColor::BrightCyan.into()),
            extra: Some(AnsiColor::BrightMagenta.into()),
            error: Some(AnsiColor::Red.into()),
            missing: Some(Self::ORANGE),
            line_feed: Some(Self::LIGHT_GRAY),
            backtick: Some(Self::GREEN),
            literal: Some(Self::YELLOW),
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl From<Rgb> for RgbColor {
    fn from(val: Rgb) -> Self {
        Self(val.0, val.1, val.2)
    }
}

#[derive(Debug, Copy, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub parse_theme: Option<ParseThemeRaw>,
}

#[derive(Debug, Copy, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ParseThemeRaw {
    pub node_kind: Option<Rgb>,
    pub node_text: Option<Rgb>,
    pub field: Option<Rgb>,
    pub row_color: Option<Rgb>,
    pub row_color_named: Option<Rgb>,
    pub extra: Option<Rgb>,
    pub error: Option<Rgb>,
    pub missing: Option<Rgb>,
    pub line_feed: Option<Rgb>,
    pub backtick: Option<Rgb>,
    pub literal: Option<Rgb>,
}

impl From<ParseThemeRaw> for ParseTheme {
    fn from(value: ParseThemeRaw) -> Self {
        let val_or_default = |val: Option<Rgb>, default: Option<Color>| -> Option<Color> {
            val.map_or(default, |v| Some(Color::Rgb(v.into())))
        };
        let default = Self::default();

        Self {
            node_kind: val_or_default(value.node_kind, default.node_kind),
            node_text: val_or_default(value.node_text, default.node_text),
            field: val_or_default(value.field, default.field),
            row_color: val_or_default(value.row_color, default.row_color),
            row_color_named: val_or_default(value.row_color_named, default.row_color_named),
            extra: val_or_default(value.extra, default.extra),
            error: val_or_default(value.error, default.error),
            missing: val_or_default(value.missing, default.missing),
            line_feed: val_or_default(value.line_feed, default.line_feed),
            backtick: val_or_default(value.backtick, default.backtick),
            literal: val_or_default(value.literal, default.literal),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ParseOutput {
    Normal,
    Quiet,
    Xml,
    Cst,
    Dot,
}

/// A position in a multi-line text document, in terms of rows and columns.
///
/// Rows and columns are zero-based.
///
/// This serves as a serializable wrapper for `Point`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct ParsePoint {
    pub row: usize,
    pub column: usize,
}

impl From<Point> for ParsePoint {
    fn from(value: Point) -> Self {
        Self {
            row: value.row,
            column: value.column,
        }
    }
}

#[derive(Serialize, Default, Debug, Clone)]
pub struct ParseSummary {
    pub file: PathBuf,
    pub successful: bool,
    pub start: Option<ParsePoint>,
    pub end: Option<ParsePoint>,
    pub duration: Option<Duration>,
    pub bytes: Option<usize>,
}

impl ParseSummary {
    #[must_use]
    pub fn new(path: &Path) -> Self {
        Self {
            file: path.to_path_buf(),
            successful: false,
            ..Default::default()
        }
    }
}

#[derive(Serialize, Debug, Default)]
pub struct ParseStats {
    pub parse_summaries: Vec<ParseSummary>,
    pub cumulative_stats: Stats,
}

#[derive(Serialize, ValueEnum, Debug, Copy, Clone, Default, Eq, PartialEq)]
pub enum ParseDebugType {
    #[default]
    Quiet,
    Normal,
    Pretty,
}

pub struct ParseFileOptions<'a> {
    pub edits: &'a [&'a str],
    pub output: ParseOutput,
    pub stats: &'a mut ParseStats,
    pub print_time: bool,
    pub timeout: u64,
    pub debug: ParseDebugType,
    pub debug_graph: bool,
    pub cancellation_flag: Option<&'a AtomicUsize>,
    pub encoding: Option<u32>,
    pub open_log: bool,
    pub no_ranges: bool,
    pub parse_theme: &'a ParseTheme,
}

#[derive(Copy, Clone)]
pub struct ParseResult {
    pub successful: bool,
    pub bytes: usize,
    pub duration: Option<Duration>,
}

pub fn parse_file_at_path(
    parser: &mut Parser,
    language: &Language,
    path: &Path,
    name: &str,
    max_path_length: usize,
    opts: &mut ParseFileOptions,
) -> Result<()> {
    let mut _log_session = None;
    parser.set_language(language)?;
    let mut source_code = fs::read(path).with_context(|| format!("Error reading {name:?}"))?;

    // Render an HTML graph if `--debug-graph` was passed
    if opts.debug_graph {
        _log_session = Some(util::log_graphs(parser, "log.html", opts.open_log)?);
    }
    // Log to stderr if `--debug` was passed
    else if opts.debug != ParseDebugType::Quiet {
        let mut curr_version: usize = 0;
        let use_color = std::env::var("NO_COLOR").map_or(true, |v| v != "1");
        let debug = opts.debug;
        parser.set_logger(Some(Box::new(move |log_type, message| {
            if debug == ParseDebugType::Normal {
                if log_type == LogType::Lex {
                    write!(&mut io::stderr(), "  ").unwrap();
                }
                writeln!(&mut io::stderr(), "{message}").unwrap();
            } else {
                let colors = &[
                    AnsiColor::White,
                    AnsiColor::Red,
                    AnsiColor::Blue,
                    AnsiColor::Green,
                    AnsiColor::Cyan,
                    AnsiColor::Yellow,
                ];
                if message.starts_with("process version:") {
                    let comma_idx = message.find(',').unwrap();
                    curr_version = message["process version:".len()..comma_idx]
                        .parse()
                        .unwrap();
                }
                let color = if use_color {
                    Some(colors[curr_version])
                } else {
                    None
                };
                let mut out = if log_type == LogType::Lex {
                    "  ".to_string()
                } else {
                    String::new()
                };
                out += &paint(color, message);
                writeln!(&mut io::stderr(), "{out}").unwrap();
            }
        })));
    }

    let parse_time = Instant::now();

    #[inline(always)]
    fn is_utf16_le_bom(bom_bytes: &[u8]) -> bool {
        bom_bytes == [0xFF, 0xFE]
    }

    #[inline(always)]
    fn is_utf16_be_bom(bom_bytes: &[u8]) -> bool {
        bom_bytes == [0xFE, 0xFF]
    }

    let encoding = match opts.encoding {
        None if source_code.len() >= 2 => {
            if is_utf16_le_bom(&source_code[0..2]) {
                Some(ffi::TSInputEncodingUTF16LE)
            } else if is_utf16_be_bom(&source_code[0..2]) {
                Some(ffi::TSInputEncodingUTF16BE)
            } else {
                None
            }
        }
        _ => opts.encoding,
    };

    // If the `--cancel` flag was passed, then cancel the parse
    // when the user types a newline.
    //
    // Additionally, if the `--time` flag was passed, end the parse
    // after the specified number of microseconds.
    let start_time = Instant::now();
    let progress_callback = &mut |_: &ParseState| {
        if let Some(cancellation_flag) = opts.cancellation_flag {
            if cancellation_flag.load(Ordering::SeqCst) != 0 {
                return true;
            }
        }

        if opts.timeout > 0 && start_time.elapsed().as_micros() > opts.timeout as u128 {
            return true;
        }

        false
    };

    let parse_opts = ParseOptions::new().progress_callback(progress_callback);

    let tree = match encoding {
        Some(encoding) if encoding == ffi::TSInputEncodingUTF16LE => {
            let source_code_utf16 = source_code
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            parser.parse_utf16_le_with_options(
                &mut |i, _| {
                    if i < source_code_utf16.len() {
                        &source_code_utf16[i..]
                    } else {
                        &[]
                    }
                },
                None,
                Some(parse_opts),
            )
        }
        Some(encoding) if encoding == ffi::TSInputEncodingUTF16BE => {
            let source_code_utf16 = source_code
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            parser.parse_utf16_be_with_options(
                &mut |i, _| {
                    if i < source_code_utf16.len() {
                        &source_code_utf16[i..]
                    } else {
                        &[]
                    }
                },
                None,
                Some(parse_opts),
            )
        }
        _ => parser.parse_with_options(
            &mut |i, _| {
                if i < source_code.len() {
                    &source_code[i..]
                } else {
                    &[]
                }
            },
            None,
            Some(parse_opts),
        ),
    };
    let parse_duration = parse_time.elapsed();

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    if let Some(mut tree) = tree {
        if opts.debug_graph && !opts.edits.is_empty() {
            println!("BEFORE:\n{}", String::from_utf8_lossy(&source_code));
        }

        let edit_time = Instant::now();
        for (i, edit) in opts.edits.iter().enumerate() {
            let edit = parse_edit_flag(&source_code, edit)?;
            perform_edit(&mut tree, &mut source_code, &edit)?;
            tree = parser.parse(&source_code, Some(&tree)).unwrap();

            if opts.debug_graph {
                println!("AFTER {i}:\n{}", String::from_utf8_lossy(&source_code));
            }
        }
        let edit_duration = edit_time.elapsed();

        parser.stop_printing_dot_graphs();

        let parse_duration_ms = parse_duration.as_micros() as f64 / 1e3;
        let edit_duration_ms = edit_duration.as_micros() as f64 / 1e3;
        let mut cursor = tree.walk();

        if opts.output == ParseOutput::Normal {
            let mut needs_newline = false;
            let mut indent_level = 0;
            let mut did_visit_children = false;
            loop {
                let node = cursor.node();
                let is_named = node.is_named();
                if did_visit_children {
                    if is_named {
                        stdout.write_all(b")")?;
                        needs_newline = true;
                    }
                    if cursor.goto_next_sibling() {
                        did_visit_children = false;
                    } else if cursor.goto_parent() {
                        did_visit_children = true;
                        indent_level -= 1;
                    } else {
                        break;
                    }
                } else {
                    if is_named {
                        if needs_newline {
                            stdout.write_all(b"\n")?;
                        }
                        for _ in 0..indent_level {
                            stdout.write_all(b"  ")?;
                        }
                        let start = node.start_position();
                        let end = node.end_position();
                        if let Some(field_name) = cursor.field_name() {
                            write!(&mut stdout, "{field_name}: ")?;
                        }
                        write!(&mut stdout, "({}", node.kind())?;
                        if !opts.no_ranges {
                            write!(
                                &mut stdout,
                                " [{}, {}] - [{}, {}]",
                                start.row, start.column, end.row, end.column
                            )?;
                        }
                        needs_newline = true;
                    }
                    if cursor.goto_first_child() {
                        did_visit_children = false;
                        indent_level += 1;
                    } else {
                        did_visit_children = true;
                    }
                }
            }
            cursor.reset(tree.root_node());
            println!();
        }

        if opts.output == ParseOutput::Cst {
            let lossy_source_code = String::from_utf8_lossy(&source_code);
            let total_width = lossy_source_code
                .lines()
                .enumerate()
                .map(|(row, col)| {
                    (row as f64).log10() as usize + (col.len() as f64).log10() as usize + 1
                })
                .max()
                .unwrap_or(1);
            let mut indent_level = 1;
            let mut did_visit_children = false;
            let mut in_error = false;
            loop {
                if did_visit_children {
                    if cursor.goto_next_sibling() {
                        did_visit_children = false;
                    } else if cursor.goto_parent() {
                        did_visit_children = true;
                        indent_level -= 1;
                        if !cursor.node().has_error() {
                            in_error = false;
                        }
                    } else {
                        break;
                    }
                } else {
                    cst_render_node(
                        opts,
                        &mut cursor,
                        &source_code,
                        &mut stdout,
                        total_width,
                        indent_level,
                        in_error,
                    )?;
                    if cursor.goto_first_child() {
                        did_visit_children = false;
                        indent_level += 1;
                        if cursor.node().has_error() {
                            in_error = true;
                        }
                    } else {
                        did_visit_children = true;
                    }
                }
            }
            cursor.reset(tree.root_node());
            println!();
        }

        if opts.output == ParseOutput::Xml {
            let mut needs_newline = false;
            let mut indent_level = 0;
            let mut did_visit_children = false;
            let mut had_named_children = false;
            let mut tags = Vec::<&str>::new();
            writeln!(&mut stdout, "<?xml version=\"1.0\"?>")?;
            loop {
                let node = cursor.node();
                let is_named = node.is_named();
                if did_visit_children {
                    if is_named {
                        let tag = tags.pop();
                        if had_named_children {
                            for _ in 0..indent_level {
                                stdout.write_all(b"  ")?;
                            }
                        }
                        write!(&mut stdout, "</{}>", tag.expect("there is a tag"))?;
                        // we only write a line in the case where it's the last sibling
                        if let Some(parent) = node.parent() {
                            if parent.child(parent.child_count() - 1).unwrap() == node {
                                stdout.write_all(b"\n")?;
                            }
                        }
                        needs_newline = true;
                    }
                    if cursor.goto_next_sibling() {
                        did_visit_children = false;
                        had_named_children = false;
                    } else if cursor.goto_parent() {
                        did_visit_children = true;
                        had_named_children = is_named;
                        indent_level -= 1;
                        if !is_named && needs_newline {
                            stdout.write_all(b"\n")?;
                            for _ in 0..indent_level {
                                stdout.write_all(b"  ")?;
                            }
                        }
                    } else {
                        break;
                    }
                } else {
                    if is_named {
                        if needs_newline {
                            stdout.write_all(b"\n")?;
                        }
                        for _ in 0..indent_level {
                            stdout.write_all(b"  ")?;
                        }
                        write!(&mut stdout, "<{}", node.kind())?;
                        if let Some(field_name) = cursor.field_name() {
                            write!(&mut stdout, " field=\"{field_name}\"")?;
                        }
                        let start = node.start_position();
                        let end = node.end_position();
                        write!(&mut stdout, " srow=\"{}\"", start.row)?;
                        write!(&mut stdout, " scol=\"{}\"", start.column)?;
                        write!(&mut stdout, " erow=\"{}\"", end.row)?;
                        write!(&mut stdout, " ecol=\"{}\"", end.column)?;
                        write!(&mut stdout, ">")?;
                        tags.push(node.kind());
                        needs_newline = true;
                    }
                    if cursor.goto_first_child() {
                        did_visit_children = false;
                        had_named_children = false;
                        indent_level += 1;
                    } else {
                        did_visit_children = true;
                        let start = node.start_byte();
                        let end = node.end_byte();
                        let value =
                            std::str::from_utf8(&source_code[start..end]).expect("has a string");
                        if !is_named && needs_newline {
                            stdout.write_all(b"\n")?;
                            for _ in 0..indent_level {
                                stdout.write_all(b"  ")?;
                            }
                        }
                        write!(&mut stdout, "{}", html_escape::encode_text(value))?;
                    }
                }
            }
            cursor.reset(tree.root_node());
            println!();
        }

        if opts.output == ParseOutput::Dot {
            util::print_tree_graph(&tree, "log.html", opts.open_log).unwrap();
        }

        let mut first_error = None;
        let mut earliest_node_with_error = None;
        'outer: loop {
            let node = cursor.node();
            if node.has_error() {
                if earliest_node_with_error.is_none() {
                    earliest_node_with_error = Some(node);
                }
                if node.is_error() || node.is_missing() {
                    first_error = Some(node);
                    break;
                }

                // If there's no more children, even though some outer node has an error,
                // then that means that the first error is hidden, but the later error could be
                // visible. So, we walk back up to the child of the first node with an error,
                // and then check its siblings for errors.
                if !cursor.goto_first_child() {
                    let earliest = earliest_node_with_error.unwrap();
                    while cursor.goto_parent() {
                        if cursor.node().parent().is_some_and(|p| p == earliest) {
                            while cursor.goto_next_sibling() {
                                let sibling = cursor.node();
                                if sibling.is_error() || sibling.is_missing() {
                                    first_error = Some(sibling);
                                    break 'outer;
                                }
                                if sibling.has_error() && cursor.goto_first_child() {
                                    continue 'outer;
                                }
                            }
                            break;
                        }
                    }
                    break;
                }
            } else if !cursor.goto_next_sibling() {
                break;
            }
        }

        if first_error.is_some() || opts.print_time {
            let path = path.to_string_lossy();
            write!(
                &mut stdout,
                "{:width$}\tParse: {parse_duration_ms:>7.2} ms\t{:>6} bytes/ms",
                name,
                (source_code.len() as u128 * 1_000_000) / parse_duration.as_nanos(),
                width = max_path_length
            )?;
            if let Some(node) = first_error {
                let start = node.start_position();
                let end = node.end_position();
                let mut node_text = String::new();
                for c in node.kind().chars() {
                    if let Some(escaped) = escape_invisible(c) {
                        node_text += escaped;
                    } else {
                        node_text.push(c);
                    }
                }
                write!(&mut stdout, "\t(")?;
                if node.is_missing() {
                    if node.is_named() {
                        write!(&mut stdout, "MISSING {node_text}")?;
                    } else {
                        write!(&mut stdout, "MISSING \"{node_text}\"")?;
                    }
                } else {
                    write!(&mut stdout, "{node_text}")?;
                }
                write!(
                    &mut stdout,
                    " [{}, {}] - [{}, {}])",
                    start.row, start.column, end.row, end.column
                )?;
            }
            if !opts.edits.is_empty() {
                write!(
                    &mut stdout,
                    "\n{:width$}\tEdit:  {edit_duration_ms:>7.2} ms",
                    " ".repeat(path.len()),
                    width = max_path_length,
                )?;
            }
            writeln!(&mut stdout)?;
        }

        opts.stats.parse_summaries.push(ParseSummary {
            file: path.to_path_buf(),
            successful: first_error.is_none(),
            start: Some(tree.root_node().start_position().into()),
            end: Some(tree.root_node().end_position().into()),
            duration: Some(parse_duration),
            bytes: Some(source_code.len()),
        });

        return Ok(());
    }
    parser.stop_printing_dot_graphs();

    if opts.print_time {
        let duration = parse_time.elapsed();
        let duration_ms = duration.as_micros() as f64 / 1e3;
        writeln!(
            &mut stdout,
            "{:width$}\tParse: {duration_ms:>7.2} ms\t(timed out)",
            path.to_str().unwrap(),
            width = max_path_length
        )?;
    }

    opts.stats.parse_summaries.push(ParseSummary {
        file: path.to_path_buf(),
        successful: false,
        start: None,
        end: None,
        duration: None,
        bytes: Some(source_code.len()),
    });

    Ok(())
}

const fn escape_invisible(c: char) -> Option<&'static str> {
    Some(match c {
        '\n' => "\\n",
        '\r' => "\\r",
        '\t' => "\\t",
        '\0' => "\\0",
        '\\' => "\\\\",
        '\x0b' => "\\v",
        '\x0c' => "\\f",
        _ => return None,
    })
}

fn render_node_text(source: &str) -> String {
    source
        .chars()
        .fold(String::with_capacity(source.len()), |mut acc, c| {
            if let Some(esc) = escape_invisible(c) {
                acc.push_str(esc);
            } else {
                acc.push(c);
            }
            acc
        })
}

fn write_node_text(
    opts: &ParseFileOptions,
    stdout: &mut StdoutLock<'static>,
    cursor: &TreeCursor,
    is_named: bool,
    source: &str,
    color: Option<impl Into<Color> + Copy>,
    text_info: (usize, usize),
) -> Result<()> {
    let (total_width, indent_level) = text_info;
    let (quote, quote_color) = if is_named {
        ('`', opts.parse_theme.backtick)
    } else {
        ('\"', color.map(|c| c.into()))
    };

    if !is_named {
        write!(
            stdout,
            "{}{}{}",
            paint(quote_color, &String::from(quote)),
            paint(color, &render_node_text(source)),
            paint(quote_color, &String::from(quote)),
        )?;
    } else {
        let multiline = source.contains('\n');
        for (i, line) in source.split_inclusive('\n').enumerate() {
            if line.is_empty() {
                break;
            }
            let mut node_range = cursor.node().range();
            // For each line of text, adjust the row by shifting it down `i` rows,
            // and adjust the column by setting it to the length of *this* line.
            node_range.start_point.row += i;
            node_range.end_point.row = node_range.start_point.row;
            node_range.end_point.column = line.len()
                + if i == 0 {
                    node_range.start_point.column
                } else {
                    0
                };
            let formatted_line = render_line_feed(line, opts);
            if !opts.no_ranges {
                write!(
                    stdout,
                    "{}{}{}{}{}{}",
                    if multiline { "\n" } else { "" },
                    if multiline {
                        render_node_range(opts, cursor, is_named, true, total_width, node_range)
                    } else {
                        String::new()
                    },
                    if multiline {
                        "  ".repeat(indent_level + 1)
                    } else {
                        String::new()
                    },
                    paint(quote_color, &String::from(quote)),
                    &paint(color, &render_node_text(&formatted_line)),
                    paint(quote_color, &String::from(quote)),
                )?;
            } else {
                write!(
                    stdout,
                    "\n{}{}{}{}",
                    "  ".repeat(indent_level + 1),
                    paint(quote_color, &String::from(quote)),
                    &paint(color, &render_node_text(&formatted_line)),
                    paint(quote_color, &String::from(quote)),
                )?;
            }
        }
    }

    Ok(())
}

fn render_line_feed(source: &str, opts: &ParseFileOptions) -> String {
    if cfg!(windows) {
        source.replace("\r\n", &paint(opts.parse_theme.line_feed, "\r\n"))
    } else {
        source.replace('\n', &paint(opts.parse_theme.line_feed, "\n"))
    }
}

fn render_node_range(
    opts: &ParseFileOptions,
    cursor: &TreeCursor,
    is_named: bool,
    is_multiline: bool,
    total_width: usize,
    range: Range,
) -> String {
    let has_field_name = cursor.field_name().is_some();
    let range_color = if is_named && !is_multiline && !has_field_name {
        opts.parse_theme.row_color_named
    } else {
        opts.parse_theme.row_color
    };

    let remaining_width_start = (total_width
        - (range.start_point.row as f64).log10() as usize
        - (range.start_point.column as f64).log10() as usize)
        .max(1);
    let remaining_width_end = (total_width
        - (range.end_point.row as f64).log10() as usize
        - (range.end_point.column as f64).log10() as usize)
        .max(1);
    paint(
        range_color,
        &format!(
            "{}:{}{:remaining_width_start$}- {}:{}{:remaining_width_end$}",
            range.start_point.row,
            range.start_point.column,
            ' ',
            range.end_point.row,
            range.end_point.column,
            ' ',
        ),
    )
}

fn cst_render_node(
    opts: &ParseFileOptions,
    cursor: &mut TreeCursor,
    source_code: &[u8],
    stdout: &mut StdoutLock<'static>,
    total_width: usize,
    indent_level: usize,
    in_error: bool,
) -> Result<()> {
    let node = cursor.node();
    let is_named = node.is_named();
    if !opts.no_ranges {
        write!(
            stdout,
            "{}",
            render_node_range(opts, cursor, is_named, false, total_width, node.range())
        )?;
    }
    write!(
        stdout,
        "{}{}",
        "  ".repeat(indent_level),
        if in_error && !node.has_error() {
            " "
        } else {
            ""
        }
    )?;
    if is_named {
        if let Some(field_name) = cursor.field_name() {
            write!(
                stdout,
                "{}",
                paint(opts.parse_theme.field, &format!("{field_name}: "))
            )?;
        }

        if node.has_error() || node.is_error() {
            write!(stdout, "{}", paint(opts.parse_theme.error, "•"))?;
        }

        let kind_color = if node.is_error() {
            opts.parse_theme.error
        } else if node.is_extra() || node.parent().is_some_and(|p| p.is_extra() && !p.is_error()) {
            opts.parse_theme.extra
        } else {
            opts.parse_theme.node_kind
        };
        write!(stdout, "{} ", paint(kind_color, node.kind()))?;

        if node.child_count() == 0 {
            // Node text from a pattern or external scanner
            write_node_text(
                opts,
                stdout,
                cursor,
                is_named,
                &String::from_utf8_lossy(&source_code[node.start_byte()..node.end_byte()]),
                opts.parse_theme.node_text,
                (total_width, indent_level),
            )?;
        }
    } else if node.is_missing() {
        write!(stdout, "{}: ", paint(opts.parse_theme.missing, "MISSING"))?;
        write!(
            stdout,
            "\"{}\"",
            paint(opts.parse_theme.missing, node.kind())
        )?;
    } else {
        // Terminal literals, like "fn"
        write_node_text(
            opts,
            stdout,
            cursor,
            is_named,
            node.kind(),
            opts.parse_theme.literal,
            (total_width, indent_level),
        )?;
    }
    writeln!(stdout)?;

    Ok(())
}

pub fn perform_edit(tree: &mut Tree, input: &mut Vec<u8>, edit: &Edit) -> Result<InputEdit> {
    let start_byte = edit.position;
    let old_end_byte = edit.position + edit.deleted_length;
    let new_end_byte = edit.position + edit.inserted_text.len();
    let start_position = position_for_offset(input, start_byte)?;
    let old_end_position = position_for_offset(input, old_end_byte)?;
    input.splice(start_byte..old_end_byte, edit.inserted_text.iter().copied());
    let new_end_position = position_for_offset(input, new_end_byte)?;
    let edit = InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position,
        old_end_position,
        new_end_position,
    };
    tree.edit(&edit);
    Ok(edit)
}

fn parse_edit_flag(source_code: &[u8], flag: &str) -> Result<Edit> {
    let error = || {
        anyhow!(concat!(
            "Invalid edit string '{}'. ",
            "Edit strings must match the pattern '<START_BYTE_OR_POSITION> <REMOVED_LENGTH> <NEW_TEXT>'"
        ), flag)
    };

    // Three whitespace-separated parts:
    // * edit position
    // * deleted length
    // * inserted text
    let mut parts = flag.split(' ');
    let position = parts.next().ok_or_else(error)?;
    let deleted_length = parts.next().ok_or_else(error)?;
    let inserted_text = parts.collect::<Vec<_>>().join(" ").into_bytes();

    // Position can either be a byte_offset or row,column pair, separated by a comma
    let position = if position == "$" {
        source_code.len()
    } else if position.contains(',') {
        let mut parts = position.split(',');
        let row = parts.next().ok_or_else(error)?;
        let row = row.parse::<usize>().map_err(|_| error())?;
        let column = parts.next().ok_or_else(error)?;
        let column = column.parse::<usize>().map_err(|_| error())?;
        offset_for_position(source_code, Point { row, column })?
    } else {
        position.parse::<usize>().map_err(|_| error())?
    };

    // Deleted length must be a byte count.
    let deleted_length = deleted_length.parse::<usize>().map_err(|_| error())?;

    Ok(Edit {
        position,
        deleted_length,
        inserted_text,
    })
}

pub fn offset_for_position(input: &[u8], position: Point) -> Result<usize> {
    let mut row = 0;
    let mut offset = 0;
    let mut iter = memchr::memchr_iter(b'\n', input);
    loop {
        if let Some(pos) = iter.next() {
            if row < position.row {
                row += 1;
                offset = pos;
                continue;
            }
        }
        offset += 1;
        break;
    }
    if position.row - row > 0 {
        return Err(anyhow!("Failed to address a row: {}", position.row));
    }
    if let Some(pos) = iter.next() {
        if (pos - offset < position.column) || (input[offset] == b'\n' && position.column > 0) {
            return Err(anyhow!("Failed to address a column: {}", position.column));
        }
    } else if input.len() - offset < position.column {
        return Err(anyhow!("Failed to address a column over the end"));
    }
    Ok(offset + position.column)
}

pub fn position_for_offset(input: &[u8], offset: usize) -> Result<Point> {
    if offset > input.len() {
        return Err(anyhow!("Failed to address an offset: {offset}"));
    }
    let mut result = Point { row: 0, column: 0 };
    let mut last = 0;
    for pos in memchr::memchr_iter(b'\n', &input[..offset]) {
        result.row += 1;
        last = pos;
    }
    result.column = if result.row > 0 {
        offset - last - 1
    } else {
        offset
    };
    Ok(result)
}
