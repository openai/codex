use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::time::Instant;

use codex_core::features::Feature;
use codex_core::features::Features;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::text::Span;

mod sets;

#[derive(Clone, Copy, Debug)]
pub(crate) enum SpinnerKind {
    Thinking,
    Exploring,
    Executing,
    Waiting,
    Tool,
}

impl SpinnerKind {
    pub(crate) fn from_header(header: &str) -> Self {
        let header = header.to_ascii_lowercase();
        if header.contains("explor") {
            Self::Exploring
        } else if header.contains("wait") {
            Self::Waiting
        } else if header.contains("run") || header.contains("execut") {
            Self::Executing
        } else {
            Self::Thinking
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpinnerSet {
    Default,
    Animation1,
    Animation2,
    Animation3,
    Animation4,
}

impl SpinnerSet {
    pub(crate) fn from_features(features: &Features) -> Self {
        if features.enabled(Feature::Animation4) {
            Self::Animation4
        } else if features.enabled(Feature::Animation3) {
            Self::Animation3
        } else if features.enabled(Feature::Animation2) {
            Self::Animation2
        } else if features.enabled(Feature::Animation1) {
            Self::Animation1
        } else {
            Self::Default
        }
    }
}

struct SpinnerTheme {
    variants: &'static [&'static [&'static str]],
    tick_ms: u128,
    idle_frame: &'static str,
    style: SpinnerStyle,
}

enum SpinnerStyle {
    Fixed(fn(&'static str) -> Span<'static>),
    Cycle {
        colors: &'static [Color],
        tick_ms: u128,
        dim_every: u8,
    },
}

impl SpinnerStyle {
    fn render(&self, frame: &'static str, elapsed_ms: u128, seed: u64) -> Span<'static> {
        match self {
            SpinnerStyle::Fixed(style) => style(frame),
            SpinnerStyle::Cycle {
                colors,
                tick_ms,
                dim_every,
            } => {
                if colors.is_empty() {
                    return Span::from(frame).bold();
                }
                let step = elapsed_ms / (*tick_ms).max(1);
                let idx = ((step as u64).wrapping_add(seed) as usize) % colors.len();
                let mut span = Span::from(frame).fg(colors[idx]).bold();
                if *dim_every > 0 && (idx as u8) % dim_every == 0 {
                    span = span.dim();
                }
                span
            }
        }
    }
}

pub(crate) fn spinner_seed(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn spinner(
    set: SpinnerSet,
    kind: SpinnerKind,
    start_time: Option<Instant>,
    animations_enabled: bool,
    seed: u64,
) -> Span<'static> {
    let theme = sets::theme(set, kind);
    if !animations_enabled {
        return theme.idle_frame.dim();
    }

    let elapsed_ms = start_time.map(|st| st.elapsed().as_millis()).unwrap_or(0);
    let variants = theme.variants;
    if variants.is_empty() {
        return theme.idle_frame.dim();
    }
    let variant_idx = ((seed ^ kind_seed(kind)) as usize) % variants.len();
    let frames = variants[variant_idx];
    if frames.is_empty() {
        return theme.idle_frame.dim();
    }

    let tick_ms = theme.tick_ms.max(1);
    let frame_idx = ((elapsed_ms / tick_ms) % frames.len() as u128) as usize;
    theme
        .style
        .render(frames[frame_idx], elapsed_ms, seed ^ kind_seed(kind))
}

pub(crate) struct Animation3Spans {
    pub(crate) text: Span<'static>,
    pub(crate) face: Span<'static>,
}

pub(crate) fn animation3_spans(
    kind: SpinnerKind,
    start_time: Option<Instant>,
    animations_enabled: bool,
    seed: u64,
) -> Animation3Spans {
    let elapsed_ms = start_time.map(|st| st.elapsed().as_millis()).unwrap_or(0);
    let frame = sets::animation3::frame(kind, elapsed_ms, animations_enabled, seed);
    let style = sets::animation3::style_for_kind(kind);
    let text = style.render(frame.text, elapsed_ms, seed);
    let face = style.render(frame.face, elapsed_ms, seed);
    Animation3Spans { text, face }
}

pub(crate) fn animation4_spans(
    kind: SpinnerKind,
    start_time: Option<Instant>,
    animations_enabled: bool,
    seed: u64,
) -> Animation3Spans {
    let elapsed_ms = start_time.map(|st| st.elapsed().as_millis()).unwrap_or(0);
    let frame = sets::animation4::frame(kind, elapsed_ms, animations_enabled, seed);
    let style = sets::animation4::style_for_kind(kind);
    let text = style.render(frame.text, elapsed_ms, seed);
    let face = style.render(frame.face, elapsed_ms, seed);
    Animation3Spans { text, face }
}

fn kind_seed(kind: SpinnerKind) -> u64 {
    match kind {
        SpinnerKind::Thinking => 1,
        SpinnerKind::Exploring => 2,
        SpinnerKind::Executing => 3,
        SpinnerKind::Waiting => 4,
        SpinnerKind::Tool => 5,
    }
}

pub(super) fn style_thinking(frame: &'static str) -> Span<'static> {
    frame.magenta().bold()
}

pub(super) fn style_exploring(frame: &'static str) -> Span<'static> {
    frame.cyan().bold()
}

pub(super) fn style_executing(frame: &'static str) -> Span<'static> {
    frame.green().bold()
}

pub(super) fn style_waiting(frame: &'static str) -> Span<'static> {
    frame.yellow().bold()
}

pub(super) fn style_tool(frame: &'static str) -> Span<'static> {
    frame.blue().bold()
}
