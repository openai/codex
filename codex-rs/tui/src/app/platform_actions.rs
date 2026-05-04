//! Platform-specific app actions and small global shortcuts.
//!
//! This module owns platform state used by `App`, the side-conversation return shortcut predicate,
//! and Windows sandbox helper actions that are compiled only on Windows.

use super::*;

#[derive(Default)]
pub(super) struct WindowsSandboxState {
    pub(super) setup_started_at: Option<Instant>,
    // One-shot suppression of the next world-writable scan after user confirmation.
    pub(super) skip_world_writable_scan_once: bool,
}

impl App {
    #[cfg(target_os = "windows")]
    pub(super) fn spawn_world_writable_scan(
        cwd: AbsolutePathBuf,
        env_map: std::collections::HashMap<String, String>,
        logs_base_dir: AbsolutePathBuf,
        permission_profile: PermissionProfile,
        tx: AppEventSender,
    ) {
        let Ok(sandbox_policy) = permission_profile.to_legacy_sandbox_policy(cwd.as_path()) else {
            send_world_writable_scan_failed(&tx);
            return;
        };

        tokio::task::spawn_blocking(move || {
            let logs_base_dir_path = logs_base_dir.as_path();
            let result = codex_windows_sandbox::apply_world_writable_scan_and_denies(
                logs_base_dir_path,
                cwd.as_path(),
                &env_map,
                &sandbox_policy,
                Some(logs_base_dir_path),
            );
            if result.is_err() {
                // Scan failed: warn without examples.
                send_world_writable_scan_failed(&tx);
            }
        });
    }
}

#[cfg(target_os = "windows")]
fn send_world_writable_scan_failed(tx: &AppEventSender) {
    tx.send(AppEvent::OpenWorldWritableWarningConfirmation {
        preset: None,
        sample_paths: Vec::new(),
        extra_count: 0usize,
        failed_scan: true,
    });
}

pub(super) fn side_return_shortcut_matches(key_event: KeyEvent) -> bool {
    match key_event {
        KeyEvent {
            code: KeyCode::Esc,
            kind: KeyEventKind::Press | KeyEventKind::Repeat,
            ..
        } => true,
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            kind: KeyEventKind::Press,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL)
            && (c.eq_ignore_ascii_case(&'c') || c.eq_ignore_ascii_case(&'d')) =>
        {
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_return_shortcuts_match_esc_ctrl_c_and_ctrl_d() {
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('D'),
            KeyModifiers::CONTROL,
        )));
        assert!(!side_return_shortcut_matches(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )));
    }
}
