//! Embedded ASCII animation frames for the TUI loading indicator.
//!
//! Each variant is stored as a fixed set of 36 text frames included at compile time, so the
//! runtime never touches the filesystem when animating. The module exposes the per-variant
//! frame arrays, the ordered list of variants, and the default tick duration that drives the
//! animation cadence.

use std::time::Duration;

/// Expands to the 36 frame strings for a specific frame directory.
///
/// This macro is used to include fixed-size animation sequences at compile time so that the
/// spinner implementation can index into the frames without doing any runtime IO.
macro_rules! frames_for {
    ($dir:literal) => {
        [
            include_str!(concat!("../frames/", $dir, "/frame_1.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_2.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_3.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_4.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_5.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_6.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_7.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_8.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_9.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_10.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_11.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_12.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_13.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_14.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_15.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_16.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_17.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_18.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_19.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_20.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_21.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_22.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_23.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_24.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_25.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_26.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_27.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_28.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_29.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_30.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_31.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_32.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_33.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_34.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_35.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_36.txt")),
        ]
    };
}

/// Default spinner frames.
pub(crate) const FRAMES_DEFAULT: [&str; 36] = frames_for!("default");
/// Codex-branded spinner frames.
pub(crate) const FRAMES_CODEX: [&str; 36] = frames_for!("codex");
/// OpenAI-branded spinner frames.
pub(crate) const FRAMES_OPENAI: [&str; 36] = frames_for!("openai");
/// Block-style spinner frames.
pub(crate) const FRAMES_BLOCKS: [&str; 36] = frames_for!("blocks");
/// Dot-style spinner frames.
pub(crate) const FRAMES_DOTS: [&str; 36] = frames_for!("dots");
/// Hash-style spinner frames.
pub(crate) const FRAMES_HASH: [&str; 36] = frames_for!("hash");
/// Horizontal bar spinner frames.
pub(crate) const FRAMES_HBARS: [&str; 36] = frames_for!("hbars");
/// Vertical bar spinner frames.
pub(crate) const FRAMES_VBARS: [&str; 36] = frames_for!("vbars");
/// Shape-based spinner frames.
pub(crate) const FRAMES_SHAPES: [&str; 36] = frames_for!("shapes");
/// Slug animation frames.
pub(crate) const FRAMES_SLUG: [&str; 36] = frames_for!("slug");

/// Ordered list of all available frame variants.
///
/// The order is stable so callers can treat indices as persistent choices.
pub(crate) const ALL_VARIANTS: &[&[&str]] = &[
    &FRAMES_DEFAULT,
    &FRAMES_CODEX,
    &FRAMES_OPENAI,
    &FRAMES_BLOCKS,
    &FRAMES_DOTS,
    &FRAMES_HASH,
    &FRAMES_HBARS,
    &FRAMES_VBARS,
    &FRAMES_SHAPES,
    &FRAMES_SLUG,
];

/// Default frame tick duration used by the animation driver.
///
/// This is tuned for readable cadence across the bundled 36-frame sequences.
pub(crate) const FRAME_TICK_DEFAULT: Duration = Duration::from_millis(80);
