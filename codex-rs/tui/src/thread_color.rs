//! Stateless thread identity colors drawn from the active syntax theme.
//!
//! Only the theme palette is cached. Assignments depend on the full thread ID,
//! never on names, list order, or other threads; palette collisions are allowed.

use std::cell::RefCell;

use codex_protocol::ThreadId;
use ratatui::style::Color;
use syntect::highlighting::Highlighter;
use syntect::highlighting::Theme;
use syntect::parsing::Scope;

use crate::render::highlight::convert_syntect_color;
use crate::render::highlight::current_syntax_theme;
use crate::render::highlight::syntax_theme_revision;

thread_local! {
    static PALETTE: RefCell<Option<(u64, Vec<Color>)>> = const { RefCell::new(None) };
}

pub(crate) fn thread_color(thread_id: ThreadId) -> Color {
    PALETTE.with_borrow_mut(|cached| {
        let revision = syntax_theme_revision();
        let (cached_revision, colors) =
            cached.get_or_insert_with(|| (revision, theme_accents(&current_syntax_theme())));
        if *cached_revision != revision {
            *colors = theme_accents(&current_syntax_theme());
            *cached_revision = revision;
        }
        color_for_id(thread_id, colors)
    })
}

fn theme_accents(theme: &Theme) -> Vec<Color> {
    // Syntect stores scope rules, not a named palette. Scan every rule so rare
    // accents remain available, excluding base text and generic comment/punctuation colors.
    // A foreground paired with a special background is not a standalone accent.
    let highlighter = Highlighter::new(theme);
    let mut excluded = vec![theme.settings.foreground, theme.settings.background];
    for scope in ["comment", "punctuation"] {
        if let Ok(scope) = Scope::new(scope) {
            excluded.push(highlighter.style_mod_for_stack(&[scope]).foreground);
        }
    }

    let mut colors = Vec::new();
    for color in theme
        .scopes
        .iter()
        .filter(|rule| {
            rule.style.background.is_none() || rule.style.background == theme.settings.background
        })
        .filter_map(|rule| rule.style.foreground)
        .chain(theme.settings.accent)
    {
        if !excluded.contains(&Some(color))
            && let Some(color) = convert_syntect_color(color)
            && !colors.contains(&color)
        {
            colors.push(color);
        }
    }
    colors
}

fn color_for_id(thread_id: ThreadId, colors: &[Color]) -> Color {
    // Explicit FNV-1a, rather than a process-random or Rust-version-specific hasher.
    let hash = thread_id
        .to_string()
        .bytes()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    colors
        .get((hash % colors.len().max(1) as u64) as usize)
        .copied()
        .unwrap_or(Color::Reset)
}

#[cfg(test)]
#[path = "thread_color_tests.rs"]
mod tests;
