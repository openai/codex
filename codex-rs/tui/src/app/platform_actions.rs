//! Platform-specific app actions and small global shortcuts.
//!
//! This module owns platform state used by `App`, the side-conversation return shortcut predicate,
//! and Windows sandbox helper actions that are compiled only on Windows.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WindowsSandboxHost {
    Local,
    Mixed,
    Remote,
}

#[derive(Default)]
pub(super) struct WindowsSandboxState {
    pub(super) setup_started_at: Option<Instant>,
}

pub(super) fn windows_sandbox_host(
    target: &AppServerTarget,
    environment_manager: &EnvironmentManager,
) -> WindowsSandboxHost {
    if target.uses_remote_workspace() {
        WindowsSandboxHost::Remote
    } else if environment_manager
        .default_environment_ids()
        .into_iter()
        .filter_map(|id| environment_manager.get_environment(&id))
        .any(|environment| environment.is_remote())
    {
        if environment_manager.try_local_environment().is_some() {
            WindowsSandboxHost::Mixed
        } else {
            WindowsSandboxHost::Remote
        }
    } else {
        WindowsSandboxHost::Local
    }
}

#[cfg(target_os = "windows")]
pub(super) async fn windows_sandbox_ready(app_server: &mut AppServerSession) -> bool {
    let request_id = app_server.next_request_id();
    matches!(
        tokio::time::timeout(
            Duration::from_secs(5),
            app_server
                .request_handle()
                .request_typed(ClientRequest::WindowsSandboxReadiness {
                    request_id,
                    params: None,
                }),
        )
        .await,
        Ok(Ok(
            codex_app_server_protocol::WindowsSandboxReadinessResponse {
                status: codex_app_server_protocol::WindowsSandboxReadiness::Ready,
            }
        ))
    )
}

impl App {
    pub(super) fn windows_sandbox_host(&self) -> WindowsSandboxHost {
        windows_sandbox_host(&self.app_server_target, self.environment_manager.as_ref())
    }

    /// A local app server owns setup for both embedded and daemon connections.
    pub(super) fn windows_sandbox_setup_is_local(&self) -> bool {
        self.windows_sandbox_host() == WindowsSandboxHost::Local
    }
}

pub(super) fn side_return_shortcut_matches(key_event: KeyEvent) -> bool {
    matches!(
        key_event,
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            kind: KeyEventKind::Press,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL)
            && (c.eq_ignore_ascii_case(&'c') || c.eq_ignore_ascii_case(&'d'))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_return_shortcuts_match_ctrl_c_and_ctrl_d() {
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
            KeyEventKind::Press,
        )));
        assert!(!side_return_shortcut_matches(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )));
    }
}
