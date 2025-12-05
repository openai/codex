//! Convert between [`syntect`](https://lib.rs/syntect) highlighting and
//! [generic styling types][anstyle::Style]

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

/// Convert highlighting style to general style
pub fn to_anstyle(style: syntect::highlighting::Style) -> anstyle::Style {
    anstyle::Style::new()
        .fg_color(Some(to_anstyle_color(style.foreground)))
        .bg_color(Some(to_anstyle_color(style.background)))
        .effects(to_anstyle_effects(style.font_style))
}

/// Convert highlighting color to general color
pub fn to_anstyle_color(color: syntect::highlighting::Color) -> anstyle::Color {
    anstyle::RgbColor(color.r, color.g, color.b).into()
}

/// Convert highlighting style to general effects
pub fn to_anstyle_effects(style: syntect::highlighting::FontStyle) -> anstyle::Effects {
    let mut effects = anstyle::Effects::new();

    if style.contains(syntect::highlighting::FontStyle::BOLD) {
        effects |= anstyle::Effects::BOLD;
    }
    if style.contains(syntect::highlighting::FontStyle::ITALIC) {
        effects |= anstyle::Effects::ITALIC;
    }
    if style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
        effects |= anstyle::Effects::UNDERLINE;
    }

    effects
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
