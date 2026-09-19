//! Presentation and new keyboard navigation checks for the local-provider startup picker.

use super::*;
use crate::test_backend::VT100Backend;
use pretty_assertions::assert_eq;
use ratatui::widgets::FrameExt;

#[test]
fn provider_picker_snapshots() {
    for (name, lmstudio, ollama, width, height, selected) in [
        (
            "oss_providers_stopped",
            ProviderStatus::NotRunning,
            ProviderStatus::NotRunning,
            80,
            24,
            0,
        ),
        (
            "oss_providers_running",
            ProviderStatus::Running,
            ProviderStatus::Running,
            80,
            24,
            1,
        ),
        (
            "oss_providers_unknown_narrow",
            ProviderStatus::Unknown,
            ProviderStatus::Unknown,
            40,
            16,
            0,
        ),
        (
            "oss_providers_mixed_short",
            ProviderStatus::NotRunning,
            ProviderStatus::Running,
            28,
            12,
            1,
        ),
    ] {
        let mut widget = OssSelectionWidget::new(lmstudio, ollama);
        widget.selected_option = selected;
        let mut terminal = Terminal::new(VT100Backend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&widget, frame.area()))
            .expect("draw");
        insta::assert_snapshot!(name, terminal.backend());
    }
}

#[test]
fn navigation_wraps_without_confirming() {
    for event in [
        KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
    ] {
        let mut widget = OssSelectionWidget::new(ProviderStatus::Running, ProviderStatus::Running);
        assert_eq!(widget.handle_key_event(event), None);
        assert_eq!((widget.selected_option, widget.is_complete()), (1, false));
        assert_eq!(widget.handle_key_event(event), None);
        assert_eq!((widget.selected_option, widget.is_complete()), (0, false));
    }
}

#[test]
fn shortcuts_select_the_provider_even_when_unavailable() {
    for (key, provider) in [
        ('1', LMSTUDIO_OSS_PROVIDER_ID),
        ('2', OLLAMA_OSS_PROVIDER_ID),
    ] {
        let mut widget =
            OssSelectionWidget::new(ProviderStatus::NotRunning, ProviderStatus::NotRunning);
        assert_eq!(
            widget.handle_key_event(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE)),
            Some(provider.to_string())
        );
        assert!(widget.is_complete());
    }
}
