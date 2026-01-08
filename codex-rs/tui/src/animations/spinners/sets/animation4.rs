use std::sync::OnceLock;

use ratatui::style::Stylize;
use ratatui::text::Span;

use super::SpinnerKind;
use super::SpinnerTheme;
use crate::animations::spinners::SpinnerStyle;

const SCALE: usize = 1;
const TEXT_MS: u128 = 1400;

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
static DEFINITION_ARCS_SCALED: OnceLock<&'static [&'static [&'static str]]> = OnceLock::new();

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

pub(crate) struct Animation4Frame {
    pub(crate) text: &'static str,
    pub(crate) face: &'static str,
}

pub(crate) fn frame(
    _kind: SpinnerKind,
    elapsed_ms: u128,
    animations_enabled: bool,
    seed: u64,
) -> Animation4Frame {
    let elapsed_ms = if animations_enabled { elapsed_ms } else { 0 };
    let text_ms = jittered_text_ms(seed);
    let step = elapsed_ms / text_ms.max(1);
    let step = step.saturating_add(start_offset_step(seed, scaled_definition_arcs()));
    let faces = scaled_face_sequences();
    let arcs = scaled_definition_arcs();
    let (arc_idx, arc_step_offset) = arc_index(step, arcs);
    let text = arcs
        .get(arc_idx)
        .and_then(|arc| arc.get(text_index(arc_step_offset, arc.len())))
        .copied()
        .unwrap_or("Ok, focus...");
    let face = faces
        .get(face_sequence_index(step, seed, faces.len()))
        .and_then(|seq| seq.get(face_frame_index(arc_step_offset, seq.len())))
        .copied()
        .unwrap_or("._.");

    Animation4Frame { text, face }
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

fn scale_definition_arcs(
    base: &'static [&'static [&'static str]],
) -> &'static [&'static [&'static str]] {
    let scaled: Vec<&'static [&'static str]> = base
        .iter()
        .map(|arc| {
            let lines: Vec<&'static str> = arc
                .iter()
                .map(|line| Box::leak(scale_frame(line, SCALE).into_boxed_str()) as &'static str)
                .collect();
            Box::leak(lines.into_boxed_slice()) as &'static [&'static str]
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

fn scaled_definition_arcs() -> &'static [&'static [&'static str]] {
    DEFINITION_ARCS_SCALED.get_or_init(|| scale_definition_arcs(DEFINITION_ARCS))
}

fn arc_index(step: u128, arcs: &[&[&str]]) -> (usize, u128) {
    if arcs.is_empty() {
        return (0, 0);
    }
    let mut remaining = step;
    for (idx, arc) in arcs.iter().enumerate() {
        let arc_len = arc.len().max(1) as u128;
        if remaining < arc_len {
            return (idx, remaining);
        }
        remaining = remaining.saturating_sub(arc_len);
    }
    ((step as usize) % arcs.len(), 0)
}

fn text_index(arc_step_offset: u128, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (arc_step_offset as usize) % len
}

fn jittered_text_ms(seed: u64) -> u128 {
    let percent = 25 + (mix(seed) % 125) as u128;
    TEXT_MS.saturating_mul(percent).max(1) / 100
}

fn start_offset_step(seed: u64, arcs: &[&[&str]]) -> u128 {
    let total = total_steps(arcs);
    if total == 0 {
        return 0;
    }
    let offset = mix(seed) % total as u64;
    offset as u128
}

fn total_steps(arcs: &[&[&str]]) -> u128 {
    arcs.iter().map(|arc| arc.len() as u128).sum()
}

fn face_sequence_index(step: u128, seed: u64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let mut idx = (mix(seed ^ step as u64) as usize) % len;
    if len > 1 {
        let prev_step = step.saturating_sub(1);
        let prev = (mix(seed ^ prev_step as u64) as usize) % len;
        if idx == prev {
            idx = (idx + 1) % len;
        }
    }
    idx
}

fn face_frame_index(arc_step_offset: u128, seq_len: usize) -> usize {
    if seq_len == 0 {
        return 0;
    }
    (arc_step_offset as usize) % seq_len
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
    &["._.", "^_^", "^-^", "#_#"],
    &["o_o", "O_O", "o_O", "O_o"],
    &["o_o", "x_x", "-_-", "._."],
    &["#_#", "$_$", "^_^", "._."],
    &[">_>", "<_<", ">_<", "^_^"],
];

const EXPLORING_BASE: &[&[&str]] = &[
    &[">_>", "._.", "^-^", "o_o"],
    &["^_^", "o_o", "O_O", "._."],
    &["/o\\", "\\o/", "/o\\", "\\o/"],
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

const DEFINITION_ARCS: &[&[&str]] = &[
    &[
        "And now, the moment.",
        "I am doing the thing.",
        "On that stubborn page.",
        "To calm the spinner.",
        "With one better check.",
        "And one sweeter line.",
        "Here we go again.",
        "For real this time.",
    ],
    &[
        "No more looping.",
        "No more coping.",
        "Promise.",
        "Pinky swear.",
        "Cross my heart.",
        "If it loops, I'll cry.",
        "If it works, I'll fly.",
        "Ok, focus.",
    ],
    &[
        "Starting vibes...",
        "Starting logic...",
        "Starting regret...",
        "Spinning politely.",
        "Caching bravely.",
        "Fetching gently.",
        "Retrying softly.",
        "Still retrying.",
    ],
    &[
        "This is fine.",
        "This is code.",
        "This is hope.",
        "This is rope.",
        "Tugging the thread.",
        "Oops, it's dread.",
        "Kidding. Mostly.",
    ],
    &[
        "Compiling courage.",
        "Linking feelings.",
        "Bundling dreams.",
        "Shipping screams.",
        "Hydrating hopes.",
        "Revalidating jokes.",
    ],
    &[
        "Negotiating with React.",
        "Begging the router.",
        "Asking state nicely.",
        "State said \"no.\"",
        "State said \"lol.\"",
        "Ok that's rude.",
    ],
    &[
        "Back to build.",
        "Build is life.",
        "Build is love.",
        "Build is joy.",
    ],
    &[
        "No more looping.",
        "No more snooping.",
        "No more duping.",
        "Serious promise.",
        "Serious-serious.",
        "Double pinky.",
        "Triple pinky.",
        "Tap the keyboard.",
        "Seal the commit.",
        "Ok I'm calm.",
        "I'm not calm.",
        "I'm calm again.",
    ],
    &[
        "Optimism loaded.",
        "Optimism unloaded.",
        "Joy is async.",
        "Sadness is sync.",
        "Hope is pending.",
        "Dread is trending.",
        "It passed locally.",
        "Eventually.",
        "I trust the tests.",
        "The tests hate me.",
        "Ok that got dark.",
        "Ok that got funny.",
    ],
    &[
        "Back to coding.",
        "Coding is light.",
        "Coding is life.",
        "Coding is joy.",
    ],
];
