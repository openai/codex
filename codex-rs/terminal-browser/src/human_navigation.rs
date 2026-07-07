use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;

use crate::navigation;
use crate::navigation::LoadState;
use crate::navigation::NavigateRequest;
use crate::navigation::NavigationAction;
use crate::session::TerminalBrowser;
use crate::tool_dispatch::validate_browser_url;

/// A navigation action initiated from the Codex-owned browser chrome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HumanNavigationAction {
    Goto(String),
    Back,
    Forward,
    Reload,
}

impl TerminalBrowser {
    /// Navigates the live page without yielding human control back to the model.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the operation and session guards serialize browser chrome actions with lifecycle changes"
    )]
    pub async fn navigate_for_human(&self, action: HumanNavigationAction) -> Result<()> {
        anyhow::ensure!(
            self.is_human_control_active(),
            "human control is not active"
        );
        let _operation = self
            .inner
            .operation
            .try_lock()
            .map_err(|_| anyhow!("browser_busy: another terminal browser action is running"))?;
        self.flush_human_handle_invalidation().await;

        let (action, url) = match action {
            HumanNavigationAction::Goto(raw_url) => (
                NavigationAction::Goto,
                Some(validate_browser_url(&raw_url)?.into()),
            ),
            HumanNavigationAction::Back => (NavigationAction::Back, None),
            HumanNavigationAction::Forward => (NavigationAction::Forward, None),
            HumanNavigationAction::Reload => (NavigationAction::Reload, None),
        };
        let request = NavigateRequest {
            action,
            url,
            wait_until: LoadState::Load,
            timeout_ms: None,
        };
        let mut session = self.inner.session.lock().await;
        let session = session.as_mut().context("terminal browser is not open")?;
        session.handles.clear();
        navigation::navigate_request(&session.cdp, &request).await?;
        self.refresh_page_metadata(session).await
    }
}
