use std::sync::OnceLock;

use ratatui::style::Color;

use super::SpinnerKind;
use super::SpinnerTheme;
use crate::animations::spinners::SpinnerStyle;

const SCALE: usize = 2;

const THINKING_COLORS: &[Color] = &[
    Color::LightMagenta,
    Color::Magenta,
    Color::LightBlue,
    Color::LightCyan,
];
const EXPLORING_COLORS: &[Color] = &[
    Color::LightCyan,
    Color::Cyan,
    Color::LightBlue,
    Color::LightGreen,
];
const EXECUTING_COLORS: &[Color] = &[
    Color::LightGreen,
    Color::Green,
    Color::Yellow,
    Color::LightRed,
];
const WAITING_COLORS: &[Color] = &[
    Color::LightBlue,
    Color::Blue,
    Color::LightCyan,
    Color::DarkGray,
];
const TOOL_COLORS: &[Color] = &[
    Color::LightYellow,
    Color::Yellow,
    Color::LightRed,
    Color::Red,
];

static THINKING_SCALED: OnceLock<&'static [&'static [&'static str]]> = OnceLock::new();
static EXPLORING_SCALED: OnceLock<&'static [&'static [&'static str]]> = OnceLock::new();
static EXECUTING_SCALED: OnceLock<&'static [&'static [&'static str]]> = OnceLock::new();
static WAITING_SCALED: OnceLock<&'static [&'static [&'static str]]> = OnceLock::new();
static TOOL_SCALED: OnceLock<&'static [&'static [&'static str]]> = OnceLock::new();

static THINKING_IDLE: OnceLock<&'static str> = OnceLock::new();
static EXPLORING_IDLE: OnceLock<&'static str> = OnceLock::new();
static EXECUTING_IDLE: OnceLock<&'static str> = OnceLock::new();
static WAITING_IDLE: OnceLock<&'static str> = OnceLock::new();
static TOOL_IDLE: OnceLock<&'static str> = OnceLock::new();

pub(super) fn theme(kind: SpinnerKind) -> SpinnerTheme {
    match kind {
        SpinnerKind::Thinking => SpinnerTheme {
            variants: scaled_thinking(),
            tick_ms: 120,
            idle_frame: scaled_idle("..", &THINKING_IDLE),
            style: SpinnerStyle::Cycle {
                colors: THINKING_COLORS,
                tick_ms: 260,
                dim_every: 3,
            },
        },
        SpinnerKind::Exploring => SpinnerTheme {
            variants: scaled_exploring(),
            tick_ms: 110,
            idle_frame: scaled_idle("..", &EXPLORING_IDLE),
            style: SpinnerStyle::Cycle {
                colors: EXPLORING_COLORS,
                tick_ms: 210,
                dim_every: 4,
            },
        },
        SpinnerKind::Executing => SpinnerTheme {
            variants: scaled_executing(),
            tick_ms: 80,
            idle_frame: scaled_idle("==", &EXECUTING_IDLE),
            style: SpinnerStyle::Cycle {
                colors: EXECUTING_COLORS,
                tick_ms: 140,
                dim_every: 0,
            },
        },
        SpinnerKind::Waiting => SpinnerTheme {
            variants: scaled_waiting(),
            tick_ms: 150,
            idle_frame: scaled_idle("..", &WAITING_IDLE),
            style: SpinnerStyle::Cycle {
                colors: WAITING_COLORS,
                tick_ms: 300,
                dim_every: 2,
            },
        },
        SpinnerKind::Tool => SpinnerTheme {
            variants: scaled_tool(),
            tick_ms: 110,
            idle_frame: scaled_idle("[]", &TOOL_IDLE),
            style: SpinnerStyle::Cycle {
                colors: TOOL_COLORS,
                tick_ms: 180,
                dim_every: 3,
            },
        },
    }
}

fn scaled_thinking() -> &'static [&'static [&'static str]] {
    THINKING_SCALED.get_or_init(|| scale_variants(THINKING_BASE))
}

fn scaled_exploring() -> &'static [&'static [&'static str]] {
    EXPLORING_SCALED.get_or_init(|| scale_variants(EXPLORING_BASE))
}

fn scaled_executing() -> &'static [&'static [&'static str]] {
    EXECUTING_SCALED.get_or_init(|| scale_variants(EXECUTING_BASE))
}

fn scaled_waiting() -> &'static [&'static [&'static str]] {
    WAITING_SCALED.get_or_init(|| scale_variants(WAITING_BASE))
}

fn scaled_tool() -> &'static [&'static [&'static str]] {
    TOOL_SCALED.get_or_init(|| scale_variants(TOOL_BASE))
}

fn scaled_idle(base: &str, cache: &OnceLock<&'static str>) -> &'static str {
    cache.get_or_init(|| Box::leak(scale_frame(base, SCALE).into_boxed_str()))
}

fn scale_variants(base: &'static [&'static [&'static str]]) -> &'static [&'static [&'static str]] {
    let scaled: Vec<&'static [&'static str]> = base
        .iter()
        .map(|variant| {
            let frames: Vec<&'static str> = variant
                .iter()
                .map(|frame| Box::leak(scale_frame(frame, SCALE).into_boxed_str()) as &'static str)
                .collect();
            Box::leak(frames.into_boxed_slice()) as &'static [&'static str]
        })
        .collect();
    Box::leak(scaled.into_boxed_slice())
}

fn scale_frame(frame: &str, scale: usize) -> String {
    if scale <= 1 {
        return frame.to_string();
    }
    let mut scaled = String::with_capacity(frame.len() * scale);
    for ch in frame.chars() {
        for _ in 0..scale {
            scaled.push(ch);
        }
    }
    scaled
}

const THINKING_BASE: &[&[&str]] = &[
    &["..", "o.", "oo", ".o", "..", ".o", "oo", "o."],
    &["<>", "><", "<>", "><"],
    &["()", ")(", "()", ")("],
    &["??", "!!", "??", "!!"],
    &["░░", "▒▒", "▓▓", "▒▒"],
];

const EXPLORING_BASE: &[&[&str]] = &[
    &[">.", ".>", "..", "<.", ".<", ".."],
    &["^.", ".^", "v.", ".v", "^.", ".^"],
    &["/\\", "\\/", "/\\", "\\/"],
    &["[]", "][", "[]", "]["],
    &["..", "::", ";;", "::"],
];

const EXECUTING_BASE: &[&[&str]] = &[
    &["==", "=-", "--", "-=", "=="],
    &["++", "+*", "**", "*+", "++"],
    &["##", "%%", "@@", "%%"],
    &["[]", "[=", "[]", "=]"],
    &["▣▣", "▣▢", "▢▣", "▣▣"],
];

const WAITING_BASE: &[&[&str]] = &[
    &["..", ". ", "  ", " .", ".."],
    &["--", " -", "  ", "- ", "--"],
    &["..", "o.", "oo", ".o"],
    &["zz", "z ", "  ", " z"],
    &["::", ".:", "..", ":."],
];

const TOOL_BASE: &[&[&str]] = &[
    &["/\\", "\\/", "/\\", "\\/"],
    &["[]", "][", "[]", "]["],
    &["<>", "><", "<>", "><"],
    &["**", "x*", "*x", "**"],
    &["⚙⚙", "⚙⚙", "⚙⚙", "⚙⚙"],
];
