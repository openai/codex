use super::SpinnerKind;
use super::SpinnerTheme;
use crate::animations::spinners::SpinnerStyle;
use crate::animations::spinners::style_executing;
use crate::animations::spinners::style_exploring;
use crate::animations::spinners::style_thinking;
use crate::animations::spinners::style_tool;
use crate::animations::spinners::style_waiting;

pub(super) fn theme(kind: SpinnerKind) -> SpinnerTheme {
    match kind {
        SpinnerKind::Thinking => SpinnerTheme {
            variants: THINKING_VARIANTS,
            tick_ms: 200,
            idle_frame: "....",
            style: SpinnerStyle::Fixed(style_thinking),
        },
        SpinnerKind::Exploring => SpinnerTheme {
            variants: EXPLORING_VARIANTS,
            tick_ms: 140,
            idle_frame: "....",
            style: SpinnerStyle::Fixed(style_exploring),
        },
        SpinnerKind::Executing => SpinnerTheme {
            variants: EXECUTING_VARIANTS,
            tick_ms: 120,
            idle_frame: "====",
            style: SpinnerStyle::Fixed(style_executing),
        },
        SpinnerKind::Waiting => SpinnerTheme {
            variants: WAITING_VARIANTS,
            tick_ms: 220,
            idle_frame: "....",
            style: SpinnerStyle::Fixed(style_waiting),
        },
        SpinnerKind::Tool => SpinnerTheme {
            variants: TOOL_VARIANTS,
            tick_ms: 160,
            idle_frame: "[--]",
            style: SpinnerStyle::Fixed(style_tool),
        },
    }
}

const THINKING_VARIANTS: &[&[&str]] = &[
    &["o...", ".o..", "..o.", "...o"],
    &[".o..", "..o.", "...o", "o..."],
];

const EXPLORING_VARIANTS: &[&[&str]] = &[
    &[">...", ".>..", "..>.", "...>", "..>.", ".>.."],
    &["<...", ".<..", "..<.", "...<", "..<.", ".<.."],
];

const EXECUTING_VARIANTS: &[&[&str]] = &[&[
    "====", "-===", "--==", "---=", "----", "=---", "==--", "===-",
]];

const WAITING_VARIANTS: &[&[&str]] = &[
    &[". . ", " .. ", "  . ", " .. "],
    &["....", " .. ", "....", " .. "],
];

const TOOL_VARIANTS: &[&[&str]] = &[
    &["[##]", "[# ]", "[ #]", "[##]"],
    &["[<>]", "[><]", "[<>]", "[><]"],
];
