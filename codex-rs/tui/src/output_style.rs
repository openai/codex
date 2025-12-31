//! Output style selection popup.
//!
//! Extension module to reduce upstream merge conflicts.
//! Contains the bulk of output style TUI logic.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use codex_core::config::output_style::OutputStyle;
use codex_core::config::output_style::OutputStyleSource;
use codex_core::config::output_style_loader::load_all_styles;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use std::path::Path;

/// Build SelectionViewParams for the output style popup.
pub(crate) fn output_style_selection_params(
    cwd: &Path,
    codex_home: &Path,
    current_style: &str,
    tx: AppEventSender,
) -> SelectionViewParams {
    let styles_map = load_all_styles(cwd, codex_home);

    // Convert HashMap to sorted Vec for consistent ordering
    let mut styles: Vec<OutputStyle> = styles_map.into_values().collect();
    // Sort: default first, then built-in, then user, then project
    styles.sort_by_key(|s| {
        (
            !s.name.eq_ignore_ascii_case("default"),
            match s.source {
                OutputStyleSource::BuiltIn => 0,
                OutputStyleSource::UserSettings => 1,
                OutputStyleSource::ProjectSettings => 2,
            },
            s.name.to_lowercase(),
        )
    });

    let items: Vec<SelectionItem> = styles
        .into_iter()
        .map(|style| {
            let is_current = style.name.eq_ignore_ascii_case(current_style);
            let style_name = style.name.clone();
            let tx_clone = tx.clone();

            let actions: Vec<SelectionAction> = vec![Box::new(move |_| {
                tx_clone.send(AppEvent::SetOutputStyle {
                    style_name: style_name.clone(),
                });
            })];

            SelectionItem {
                name: format_style_name(&style),
                description: Some(style.description.clone()),
                is_current,
                is_default: style.name.eq_ignore_ascii_case("default"),
                actions,
                dismiss_on_select: true,
                ..Default::default()
            }
        })
        .collect();

    SelectionViewParams {
        title: Some("Output Style".to_string()),
        subtitle: Some("Choose how Claude responds".to_string()),
        footer_hint: Some(standard_popup_hint_line()),
        items,
        ..Default::default()
    }
}

/// Format style name with source indicator.
fn format_style_name(style: &OutputStyle) -> String {
    match style.source {
        OutputStyleSource::BuiltIn => style.name.clone(),
        OutputStyleSource::UserSettings => format!("{} (user)", style.name),
        OutputStyleSource::ProjectSettings => format!("{} (project)", style.name),
    }
}

/// Standard hint line for popup footer.
fn standard_popup_hint_line() -> Line<'static> {
    Line::from(vec![
        Span::raw("↑↓").dim(),
        Span::raw(" navigate  ").dim(),
        Span::raw("Enter").dim(),
        Span::raw(" select  ").dim(),
        Span::raw("Esc").dim(),
        Span::raw(" cancel").dim(),
    ])
}
