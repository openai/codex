use std::sync::PoisonError;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::accessibility;
use crate::actions;
use crate::actions::BrowserToolOutput;
use crate::navigation;
use crate::navigation::NavigateRequest;
use crate::navigation::NavigationAction;
use crate::navigation::WaitRequest;
use crate::network::BrowserNetworkPolicy;
use crate::process::BrowserSession;
use crate::screen::BrowserStatus;
use crate::session::RenderMode;
use crate::session::SessionConfig;
use crate::session::TerminalBrowser;
use crate::url_policy::is_allowed_browser_url;

const MAX_URL_INPUT_BYTES: usize = 4 * 1024;
const MAX_FILL_TEXT_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenArgs {
    url: String,
    visible: Option<bool>,
    render_mode: Option<RenderMode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NodeArgs {
    node_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FillArgs {
    node_id: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PressArgs {
    key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScrollArgs {
    #[serde(default)]
    delta_x: i64,
    #[serde(default)]
    delta_y: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VisibilityArgs {
    visible: bool,
}

impl TerminalBrowser {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the operation and session guards serialize stateful CDP calls with lifecycle changes"
    )]
    pub async fn execute(
        &self,
        session_key: &str,
        tool: &str,
        arguments: Value,
    ) -> Result<BrowserToolOutput> {
        if self.is_human_control_active() && !matches!(tool, "set_visibility" | "close") {
            anyhow::bail!("human_control_active: return control to the agent before using tools");
        }
        let _operation = self
            .inner
            .operation
            .try_lock()
            .map_err(|_| anyhow!("browser_busy: another terminal browser action is running"))?;
        self.flush_human_handle_invalidation().await;
        let session_changed = {
            let mut current = self
                .inner
                .session_key
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if current.as_deref() == Some(session_key) {
                false
            } else {
                *current = Some(session_key.to_string());
                true
            }
        };
        if session_changed {
            self.inner.close_session().await;
        }
        if !matches!(tool, "open" | "set_visibility" | "close") {
            let network_policy = self.inner.network_policy();
            let policy_changed = self
                .inner
                .session
                .lock()
                .await
                .as_ref()
                .is_some_and(|session| session.config.network_policy != network_policy);
            if policy_changed {
                self.inner.close_session().await;
                anyhow::bail!("browser network policy changed; reopen the page before continuing");
            }
        }
        match tool {
            "open" => self.open(serde_json::from_value(arguments)?).await,
            "navigate" => {
                let request: NavigateRequest = serde_json::from_value(arguments)?;
                if request.action == NavigationAction::Goto {
                    let url = request
                        .url
                        .as_deref()
                        .context("navigate goto requires a URL")?;
                    validate_browser_url(url)?;
                }
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                session.handles.clear();
                let output = navigation::navigate_request(&session.cdp, &request).await?;
                self.refresh_page_metadata(session).await?;
                Ok(output)
            }
            "wait" => {
                let request: WaitRequest = serde_json::from_value(arguments)?;
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = navigation::wait(&session.cdp, &session.handles, &request).await?;
                self.refresh_page_metadata(session).await?;
                Ok(output)
            }
            "profile" => self.execute_profile_tool(arguments),
            "snapshot" => {
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = accessibility::snapshot(&session.cdp, &mut session.handles).await?;
                self.refresh_page_metadata(session).await?;
                Ok(output)
            }
            "click" => {
                let args: NodeArgs = serde_json::from_value(arguments)?;
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output =
                    accessibility::click(&session.cdp, &session.handles, &args.node_id).await?;
                self.refresh_page_metadata(session).await?;
                Ok(output)
            }
            "fill" => {
                let args: FillArgs = serde_json::from_value(arguments)?;
                anyhow::ensure!(
                    args.text.len() <= MAX_FILL_TEXT_BYTES,
                    "fill text exceeds the 64 KiB input limit"
                );
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output =
                    accessibility::fill(&session.cdp, &session.handles, &args.node_id, &args.text)
                        .await?;
                self.refresh_page_metadata(session).await?;
                Ok(output)
            }
            "press" => {
                let args: PressArgs = serde_json::from_value(arguments)?;
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = actions::press(&session.cdp, &args.key).await?;
                self.refresh_page_metadata(session).await?;
                Ok(output)
            }
            "scroll" => {
                let args: ScrollArgs = serde_json::from_value(arguments)?;
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = actions::scroll(&session.cdp, args.delta_x, args.delta_y).await?;
                self.refresh_page_metadata(session).await?;
                Ok(output)
            }
            "screenshot" => {
                let mut session = self.inner.session.lock().await;
                let session = session.as_mut().context("terminal browser is not open")?;
                let output = actions::screenshot(&session.cdp).await?;
                self.refresh_page_metadata(session).await?;
                Ok(output)
            }
            "set_visibility" => {
                let args: VisibilityArgs = serde_json::from_value(arguments)?;
                self.set_visibility(args.visible);
                self.flush_human_handle_invalidation().await;
                Ok(BrowserToolOutput::Text(format!(
                    "terminal browser visibility set to {}",
                    args.visible
                )))
            }
            "close" => {
                self.inner.close_session().await;
                Ok(BrowserToolOutput::Text(
                    "terminal browser closed".to_string(),
                ))
            }
            _ => anyhow::bail!("unknown terminal browser tool: {tool}"),
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the session guard keeps navigation and metadata refresh on the same CDP client"
    )]
    async fn open(&self, args: OpenArgs) -> Result<BrowserToolOutput> {
        let url = validate_browser_url(&args.url)?;
        let network_policy = self.inner.network_policy();
        anyhow::ensure!(
            !matches!(&network_policy, BrowserNetworkPolicy::Disabled),
            "terminal browser network access is disabled by the active permission profile"
        );
        let config = SessionConfig {
            render_mode: args.render_mode.unwrap_or_default(),
            network_policy,
        };
        let visible = args.visible.unwrap_or(/*default*/ true);
        self.inner.update_view(|view| {
            view.status = BrowserStatus::Starting;
            view.visible = visible;
            view.url = Some(url.as_str().to_string());
        });
        if let Err(error) = self.inner.ensure_session(config).await {
            self.inner.set_crashed(error.to_string());
            return Err(error);
        }

        let mut session = self.inner.session.lock().await;
        let session = session.as_mut().context("terminal browser did not start")?;
        session.handles.clear();
        let metadata = match async {
            navigation::navigate(&session.cdp, url.as_str()).await?;
            let metadata = actions::page_metadata(&session.cdp).await?;
            if let Some(final_url) = metadata.url.as_deref()
                && !is_allowed_browser_url(final_url)
            {
                blank_disallowed_page(session).await;
                anyhow::bail!("browser navigation blocked by the active permission policy");
            }
            Ok::<_, anyhow::Error>(metadata)
        }
        .await
        {
            Ok(metadata) => metadata,
            Err(error) => {
                self.inner
                    .update_view(|view| view.status = BrowserStatus::Running);
                return Err(error);
            }
        };
        let final_url = metadata.url.unwrap_or_else(|| url.as_str().to_string());
        self.inner.update_view(|view| {
            view.status = BrowserStatus::Running;
            view.title = metadata.title;
            view.url = Some(final_url);
            view.visible = visible;
        });
        Ok(BrowserToolOutput::Text(format!(
            "opened {} in the terminal browser",
            url.as_str()
        )))
    }

    async fn refresh_page_metadata(&self, session: &mut BrowserSession) -> Result<()> {
        let metadata = actions::page_metadata(&session.cdp).await?;
        if let Some(url) = metadata.url.as_deref()
            && !is_allowed_browser_url(url)
        {
            blank_disallowed_page(session).await;
            anyhow::bail!("browser navigation blocked by the active permission policy");
        }
        self.inner.update_view(|view| {
            view.url = metadata.url;
            view.title = metadata.title;
        });
        Ok(())
    }
}

async fn blank_disallowed_page(session: &mut BrowserSession) {
    let _ = session
        .cdp
        .call("Page.stopLoading", serde_json::json!({}))
        .await;
    let _ = session
        .cdp
        .call("Page.navigate", serde_json::json!({ "url": "about:blank" }))
        .await;
    session.handles.clear();
}

fn validate_browser_url(raw_url: &str) -> Result<Url> {
    anyhow::ensure!(
        raw_url.len() <= MAX_URL_INPUT_BYTES,
        "URL exceeds the 4 KiB input limit"
    );
    let url = Url::parse(raw_url).context("parse terminal browser URL")?;
    anyhow::ensure!(
        is_allowed_browser_url(url.as_str()),
        "terminal_browser only supports http and https URLs"
    );
    Ok(url)
}
