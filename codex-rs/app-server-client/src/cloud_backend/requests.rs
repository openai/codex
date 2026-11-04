//! Maps conversation requests onto native Cloud operations.

use codex_app_server_protocol as protocol;
use codex_cloud_client as cloud;
use color_eyre::eyre::Result;
use color_eyre::eyre::bail;
use color_eyre::eyre::eyre;
use serde_json::Value;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
#[error("{0} is not supported by the cloud backend yet.")]
pub(super) struct UnsupportedOperation(pub(super) String);

pub(super) struct Requests {
    pub(super) client: cloud::Client,
    pub(super) account: protocol::GetAccountResponse,
    pub(super) loaded: Option<Box<protocol::ThreadResumeResponse>>,
}

pub(super) enum Response {
    Value(Value),
    Attach(Box<protocol::ThreadResumeResponse>),
}

impl Requests {
    pub(super) fn read_thread(
        &self,
        params: &protocol::ThreadReadParams,
        status: Option<&cloud::ThreadStatus>,
    ) -> Result<Value> {
        if params.include_turns {
            bail!("Use paginated history for cloud conversations.");
        }
        let loaded = self
            .loaded
            .as_deref()
            .filter(|loaded| loaded.thread.id == params.thread_id)
            .ok_or_else(|| eyre!("Open this cloud conversation before reading its transcript."))?;
        let mut thread = serde_json::to_value(&loaded.thread)?;
        if let Some(status) = status {
            thread["status"] = serde_json::to_value(status)?;
        }
        Ok(json!({"thread": thread, "readState": null}))
    }

    pub(super) async fn handle(&self, request: protocol::ClientRequest) -> Result<Response> {
        use protocol::ClientRequest as Request;

        let value = match request {
            Request::GetAccount { .. } => serde_json::to_value(&self.account)?,
            Request::ThreadResume { params, .. } => {
                let supported = protocol::ThreadResumeParams {
                    thread_id: params.thread_id.clone(),
                    exclude_turns: true,
                    ..Default::default()
                };
                if params != supported {
                    bail!(
                        "Cloud conversations resume by ID with saved settings. Set excludeTurns to true and load history separately."
                    );
                }
                let settings = self.client.activate(&params.thread_id).await?;
                let resolved = serde_json::from_value(serde_json::to_value(settings)?)
                    .map_err(|_| eyre!("The cloud returned unsupported conversation settings."))?;
                return Ok(Response::Attach(resolved));
            }
            Request::ThreadLoadedList { .. } => {
                let ids = self
                    .loaded
                    .iter()
                    .map(|loaded| &loaded.thread.id)
                    .collect::<Vec<_>>();
                json!({"data": ids, "nextCursor": null})
            }
            other => return Err(UnsupportedOperation(other.method_name().to_owned()).into()),
        };
        Ok(Response::Value(value))
    }
}
