use std::sync::OnceLock;

use ratatui::style::Stylize;
use ratatui::text::Span;

use super::SpinnerKind;
use super::SpinnerTheme;
use crate::animations::spinners::SpinnerStyle;

const SCALE: usize = 1;
const FACE_FRAME_MS: u128 = 840;
const FACE_PAUSE_MS: u128 = 1040;
const TEXT_MS: u128 = 800;

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

static FACE_SEQUENCES_SCALED: OnceLock<&'static [&'static [&'static str]]> = OnceLock::new();
static THINKING_DEFS_SCALED: OnceLock<&'static [&'static str]> = OnceLock::new();
static EXPLORING_DEFS_SCALED: OnceLock<&'static [&'static str]> = OnceLock::new();
static EXECUTING_DEFS_SCALED: OnceLock<&'static [&'static str]> = OnceLock::new();
static WAITING_DEFS_SCALED: OnceLock<&'static [&'static str]> = OnceLock::new();
static TOOL_DEFS_SCALED: OnceLock<&'static [&'static str]> = OnceLock::new();

pub(super) fn theme(kind: SpinnerKind) -> SpinnerTheme {
    match kind {
        SpinnerKind::Thinking => SpinnerTheme {
            variants: scaled_thinking(),
            tick_ms: 150,
            idle_frame: scaled_idle("._.", &THINKING_IDLE),
            style: style_for_kind(kind),
        },
        SpinnerKind::Exploring => SpinnerTheme {
            variants: scaled_exploring(),
            tick_ms: 120,
            idle_frame: scaled_idle("o_o", &EXPLORING_IDLE),
            style: style_for_kind(kind),
        },
        SpinnerKind::Executing => SpinnerTheme {
            variants: scaled_executing(),
            tick_ms: 90,
            idle_frame: scaled_idle("RUN", &EXECUTING_IDLE),
            style: style_for_kind(kind),
        },
        SpinnerKind::Waiting => SpinnerTheme {
            variants: scaled_waiting(),
            tick_ms: 160,
            idle_frame: scaled_idle("zzz", &WAITING_IDLE),
            style: style_for_kind(kind),
        },
        SpinnerKind::Tool => SpinnerTheme {
            variants: scaled_tool(),
            tick_ms: 120,
            idle_frame: scaled_idle("[]", &TOOL_IDLE),
            style: style_for_kind(kind),
        },
    }
}

pub(crate) fn style_for_kind(kind: SpinnerKind) -> SpinnerStyle {
    match kind {
        SpinnerKind::Thinking
        | SpinnerKind::Exploring
        | SpinnerKind::Executing
        | SpinnerKind::Waiting
        | SpinnerKind::Tool => SpinnerStyle::Fixed(style_default),
    }
}

fn style_default(frame: &'static str) -> Span<'static> {
    Span::from(frame).bold()
}

pub(crate) struct Animation3Frame {
    pub(crate) text: &'static str,
    pub(crate) face: &'static str,
}

pub(crate) fn frame(
    kind: SpinnerKind,
    elapsed_ms: u128,
    animations_enabled: bool,
    seed: u64,
) -> Animation3Frame {
    let elapsed_ms = if animations_enabled { elapsed_ms } else { 0 };
    let definitions = scaled_definitions_for(kind);
    let text = definitions
        .get(text_index(elapsed_ms, seed, definitions.len()))
        .copied()
        .unwrap_or("...");

    let faces = scaled_face_sequences();
    let face = faces
        .get(face_sequence_index(elapsed_ms, seed, faces.len()))
        .and_then(|seq| seq.get(face_frame_index(elapsed_ms, seq.len())))
        .copied()
        .unwrap_or("._.");

    Animation3Frame { text, face }
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

fn scale_definitions(base: &'static [&'static str]) -> &'static [&'static str] {
    let scaled: Vec<&'static str> = base
        .iter()
        .map(|value| {
            let value = format!("{value}...");
            Box::leak(scale_frame(&value, SCALE).into_boxed_str()) as &'static str
        })
        .collect();
    Box::leak(scaled.into_boxed_slice())
}

fn scale_frame(frame: &str, scale: usize) -> String {
    if scale <= 1 {
        return frame.to_string();
    }
    let mut scaled = String::with_capacity(frame.len() * scale);
    let mut first = true;
    for ch in frame.chars() {
        if !first {
            for _ in 1..scale {
                scaled.push(' ');
            }
        }
        scaled.push(ch);
        first = false;
    }
    scaled
}

fn scaled_face_sequences() -> &'static [&'static [&'static str]] {
    FACE_SEQUENCES_SCALED.get_or_init(|| scale_variants(FACE_SEQUENCES))
}

fn scaled_definitions_for(kind: SpinnerKind) -> &'static [&'static str] {
    match kind {
        SpinnerKind::Thinking => {
            THINKING_DEFS_SCALED.get_or_init(|| scale_definitions(THINKING_DEFS))
        }
        SpinnerKind::Exploring => {
            EXPLORING_DEFS_SCALED.get_or_init(|| scale_definitions(EXPLORING_DEFS))
        }
        SpinnerKind::Executing => {
            EXECUTING_DEFS_SCALED.get_or_init(|| scale_definitions(EXECUTING_DEFS))
        }
        SpinnerKind::Waiting => WAITING_DEFS_SCALED.get_or_init(|| scale_definitions(WAITING_DEFS)),
        SpinnerKind::Tool => TOOL_DEFS_SCALED.get_or_init(|| scale_definitions(TOOL_DEFS)),
    }
}

fn text_index(elapsed_ms: u128, seed: u64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let step = elapsed_ms / TEXT_MS.max(1);
    (mix(seed ^ step as u64) as usize) % len
}

fn face_sequence_index(elapsed_ms: u128, seed: u64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let cycle_ms = FACE_FRAME_MS
        .saturating_mul(FACE_SEQUENCE_LEN as u128)
        .saturating_add(FACE_PAUSE_MS);
    let cycle_idx = elapsed_ms / cycle_ms.max(1);
    let mut idx = (mix(seed ^ cycle_idx as u64) as usize) % len;
    if len > 1 {
        let prev_cycle = cycle_idx.saturating_sub(1);
        let prev = (mix(seed ^ prev_cycle as u64) as usize) % len;
        if idx == prev {
            idx = (idx + 1) % len;
        }
    }
    idx
}

fn face_frame_index(elapsed_ms: u128, seq_len: usize) -> usize {
    if seq_len == 0 {
        return 0;
    }
    let cycle_ms = FACE_FRAME_MS
        .saturating_mul(seq_len as u128)
        .saturating_add(FACE_PAUSE_MS);
    let phase_ms = elapsed_ms % cycle_ms.max(1);
    let active_ms = FACE_FRAME_MS.saturating_mul(seq_len as u128);
    if phase_ms < active_ms {
        return (phase_ms / FACE_FRAME_MS.max(1)) as usize;
    }
    seq_len.saturating_sub(1)
}

fn mix(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51afd7ed558ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ceb9fe1a85ec53);
    value ^= value >> 33;
    value
}

const THINKING_BASE: &[&[&str]] = &[
    &["._.", "^_^", "^-^", "^o^"],
    &["o_o", "O_O", "o_O", "O_o"],
    &["@_@", "x_x", "-_-", "._."],
    &["#_#", "$_$", "^_^", "._."],
    &[">_>", "<_<", ">_<", "^_^"],
];

const EXPLORING_BASE: &[&[&str]] = &[
    &[">_>", "._.", "^-^", "o_o"],
    &["^_^", "o_o", "O_O", "._."],
    &["/o\\", "\\o/", "/o\\", "\\o/"],
    &["[]]", "[]<", "<[]>", "[][]"],
    &["^w^", "^_^", "._.", "!_!"],
];

const EXECUTING_BASE: &[&[&str]] = &[
    &["RUN", "PUSH", "SHIP", "DONE"],
    &["DO!", "GO!", "NOW", "YEP"],
    &["MOVE", "JUMP", "ROLL", "FLIP"],
    &["SYNC", "MERG", "COMM", "PUSH"],
    &["BOOT", "TEST", "LINT", "PASS"],
];

const WAITING_BASE: &[&[&str]] = &[
    &["zzz", "z z", "  z", "z  "],
    &["...", ".. ", ".  ", " .."],
    &["T_T", "-_-", "._.", "o_o"],
    &["WAIT", "HOLD", "PAUS", "REST"],
    &["SIGH", "HMM", "UMM", "OK?"],
];

const TOOL_BASE: &[&[&str]] = &[
    &["[]", "][", "[]", "]["],
    &["< >", "><", "<>", "><"],
    &["/\\", "\\/", "/\\", "\\/"],
    &["TOOL", "WREN", "PICK", "FIX!"],
    &["CUT", "COPY", "PAST", "DONE"],
];

const FACE_SEQUENCE_LEN: usize = 3;
const FACE_SEQUENCES: &[&[&str]] = &[
    &["._.", "^_^", "^-^"],
    &["^-^", "^_^", "^o^"],
    &["^_^", "o_o", "O_O"],
    &["o_o", "O_o", "o_O"],
    &["o_O", "@_@", "x_x"],
    &["x_x", "-_-", "._."],
    &["._.", "-_-", ">_>"],
    &[">_>", "<_<", ">_<"],
    &[">_<", "^_^", "-_-"],
    &["#_#", "^_^", "._."],
    &["$_$", "o_O", "._."],
    &["._.", "._.", "^_^"],
    &["#_#", "^_^", "^.^"],
    &["^.^", "^_^", "^-^"],
    &["^_^", "T_T", "^_^"],
    &["^_^", "@_@", "^_^"],
    &["0_0", "o_o", "O_O"],
    &["O_O", "o_o", "._."],
    &["^_^", "^-^", "^o^"],
    &["O_O", "^w^", "^_^"],
    &["._.", "!_!", "^_^"],
    &["-_-", "T_T", "._."],
    &["@_@", "0_0", "o_o"],
    &[">_>", "._.", "^-^"],
    &["o_o", "._.", "^_^"],
];

const THINKING_DEFS: &[&str] = &[
    "Boop the Bit",
    "Pet the Bit",
    "Flip the Bit",
    "Debug a Bit",
    "Cache a Bit",
    "Weep a Bit",
    "Smile a Bit",
    "Sigh then Submit",
    "Zen Commit",
    "Sad Commit",
    "Glad Commit",
    "Emit",
    "Admit",
    "Omit",
];

const EXPLORING_DEFS: &[&str] = &[
    "Nudge the Git",
    "Curse the Git",
    "Bless the Git",
    "Tame the JIT",
    "Blame the JIT",
    "Merge then Commit",
    "Push and Commit",
    "Pull then Commit",
    "Patch then Commit",
];

const EXECUTING_DEFS: &[&str] = &[
    "Test and Submit",
    "Lint and Submit",
    "Merge then Commit",
    "Push and Commit",
    "Patch then Commit",
    "Panic Commit",
    "Debug a Bit",
];

const WAITING_DEFS: &[&str] = &[
    "Weep a Bit",
    "Sigh then Submit",
    "Sad Commit",
    "Zen Commit",
    "Glad Commit",
    "Omit",
];

const TOOL_DEFS: &[&str] = &[
    "Debug a Bit",
    "Cache a Bit",
    "Tame the JIT",
    "Blame the JIT",
    "Patch then Commit",
    "Merge then Commit",
];
