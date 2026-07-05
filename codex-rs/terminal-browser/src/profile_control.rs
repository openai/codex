use std::sync::PoisonError;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use serde::Deserialize;
use serde_json::Value;

use crate::actions::BrowserToolOutput;
use crate::profile::BrowserProfileName;
use crate::session::TerminalBrowser;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum ProfileAction {
    List,
    RequestCreate,
    RequestSelect,
    RequestEphemeral,
    RequestForget,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileArgs {
    action: ProfileAction,
    name: Option<String>,
}

impl TerminalBrowser {
    pub fn profiles(&self) -> Result<Vec<String>> {
        Ok(self
            .inner
            .profile_store
            .as_ref()
            .context("named terminal-browser profiles are unavailable")?
            .list()?
            .profiles)
    }

    pub fn selected_profile(&self) -> Option<String> {
        self.inner
            .selected_profile
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|name| name.as_str().to_string())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "profile changes must serialize with browser actions and lifecycle transitions"
    )]
    pub async fn create_profile(&self, name: &str) -> Result<()> {
        let _operation = self.inner.operation.lock().await;
        let name = BrowserProfileName::parse(name)?;
        let store = self
            .inner
            .profile_store
            .as_ref()
            .context("named terminal-browser profiles are unavailable")?;
        store.create(&name)?;
        self.inner.close_session().await;
        *self
            .inner
            .selected_profile
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(name);
        Ok(())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "profile changes must serialize with browser actions and lifecycle transitions"
    )]
    pub async fn select_profile(&self, name: &str) -> Result<()> {
        let _operation = self.inner.operation.lock().await;
        let name = BrowserProfileName::parse(name)?;
        let store = self
            .inner
            .profile_store
            .as_ref()
            .context("named terminal-browser profiles are unavailable")?;
        store.existing_path(&name)?;
        self.inner.close_session().await;
        *self
            .inner
            .selected_profile
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(name);
        Ok(())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "profile changes must serialize with browser actions and lifecycle transitions"
    )]
    pub async fn select_ephemeral_profile(&self) -> Result<()> {
        let _operation = self.inner.operation.lock().await;
        self.inner.close_session().await;
        self.inner
            .selected_profile
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        Ok(())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "profile changes must serialize with browser actions and lifecycle transitions"
    )]
    pub async fn forget_profile(&self, name: &str) -> Result<()> {
        let _operation = self.inner.operation.lock().await;
        let name = BrowserProfileName::parse(name)?;
        let selected = self
            .inner
            .selected_profile
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            == Some(&name);
        if selected {
            self.inner.close_session().await;
            self.inner
                .selected_profile
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
        }
        self.inner
            .profile_store
            .as_ref()
            .context("named terminal-browser profiles are unavailable")?
            .forget(&name)
    }

    pub(crate) fn execute_profile_tool(&self, arguments: Value) -> Result<BrowserToolOutput> {
        let args: ProfileArgs = serde_json::from_value(arguments)?;
        match args.action {
            ProfileAction::List => {
                anyhow::ensure!(args.name.is_none(), "profile list does not accept a name");
                let listing = self
                    .inner
                    .profile_store
                    .as_ref()
                    .context("named terminal-browser profiles are unavailable")?
                    .list()?;
                Ok(BrowserToolOutput::Text(serde_json::to_string(
                    &serde_json::json!({
                        "profiles": listing.profiles,
                        "total": listing.total,
                        "truncated": listing.truncated,
                        "selected": self.selected_profile(),
                    }),
                )?))
            }
            ProfileAction::RequestEphemeral => {
                anyhow::ensure!(
                    args.name.is_none(),
                    "profile requestEphemeral does not accept a name"
                );
                approval_required("ephemeral", "/browser profile ephemeral")
            }
            action => {
                let name = args
                    .name
                    .as_deref()
                    .context("profile mutation requests require a name")?;
                let name = BrowserProfileName::parse(name)?;
                let command = match action {
                    ProfileAction::RequestCreate => {
                        format!("/browser profile create {}", name.as_str())
                    }
                    ProfileAction::RequestSelect => {
                        format!("/browser profile use {}", name.as_str())
                    }
                    ProfileAction::RequestForget => {
                        format!("/browser profile forget {} --confirm", name.as_str())
                    }
                    ProfileAction::List | ProfileAction::RequestEphemeral => {
                        return Err(anyhow!("invalid profile action"));
                    }
                };
                approval_required(name.as_str(), &command)
            }
        }
    }
}

fn approval_required(profile: &str, command: &str) -> Result<BrowserToolOutput> {
    Ok(BrowserToolOutput::Text(serde_json::to_string(
        &serde_json::json!({
            "status": "approvalRequired",
            "profile": profile,
            "command": command,
        }),
    )?))
}
