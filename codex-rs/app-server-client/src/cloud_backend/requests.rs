//! Maps conversation requests onto native Cloud operations.

use super::PreflightRejection;
use super::events;
use super::send_notification;
use crate::CloudUserMessageReceipt;
use codex_app_server_protocol as protocol;
use codex_cloud_client as cloud;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use codex_protocol::models::ImageDetail;
use color_eyre::eyre::Result;
use color_eyre::eyre::bail;
use color_eyre::eyre::eyre;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

#[derive(Debug, thiserror::Error)]
#[error("{0} is not supported by the cloud backend yet.")]
pub(super) struct UnsupportedOperation(pub(super) String);

pub(super) struct Requests {
    pub(super) client: std::sync::Arc<cloud::Client>,
    pub(super) account: protocol::GetAccountResponse,
    pub(super) loaded: Option<Box<protocol::ThreadResumeResponse>>,
    pub(super) warned_partial_history: Option<String>,
}

pub(super) enum Response {
    Value(Value),
    Attach(Box<protocol::ThreadResumeResponse>),
    Archived(String),
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

    pub(super) async fn handle(
        &mut self,
        request: protocol::ClientRequest,
        mode: cloud::SubmissionMode,
        project: Option<cloud::ProjectEnvironment>,
        output: &mpsc::UnboundedSender<crate::AppServerEvent>,
    ) -> Result<Response> {
        use protocol::ClientRequest as Request;

        let value = match request {
            Request::GetAccount { .. } => serde_json::to_value(&self.account)?,
            Request::ThreadList { params, .. } => {
                let query = thread_list_query(params)?;
                let mut page = self.client.list_threads(&query).await?;
                for thread in &mut page.data {
                    use_paginated_history(thread);
                }
                serde_json::to_value(page)?
            }
            Request::ThreadSetName { params, .. } => {
                let name = params.name.trim();
                self.client.set_thread_name(&params.thread_id, name).await?;
                if let Some(loaded) = &mut self.loaded
                    && loaded.thread.id == params.thread_id
                {
                    loaded.thread.name = Some(name.to_owned());
                }
                send_notification(
                    output,
                    protocol::ServerNotification::ThreadNameUpdated(
                        protocol::ThreadNameUpdatedNotification {
                            thread_id: params.thread_id,
                            thread_name: Some(name.to_owned()),
                        },
                    ),
                );
                serde_json::to_value(protocol::ThreadSetNameResponse {})?
            }
            Request::ThreadArchive { params, .. } => {
                self.client.archive_thread(&params.thread_id).await?;
                return Ok(Response::Archived(params.thread_id));
            }
            Request::ThreadUnarchive { params, .. } => {
                let mut thread = self.client.unarchive_thread(&params.thread_id).await?;
                use_paginated_history(&mut thread);
                let value = json!({ "thread": thread });
                send_notification(
                    output,
                    protocol::ServerNotification::ThreadUnarchived(
                        protocol::ThreadUnarchivedNotification {
                            thread_id: params.thread_id,
                        },
                    ),
                );
                value
            }
            Request::ThreadStart { mut params, .. } => {
                let model = params.model.take();
                let provider = params.model_provider.take();
                if params.ephemeral == Some(false) {
                    params.ephemeral = None;
                }
                if params != protocol::ThreadStartParams::default() {
                    bail!(
                        "Cloud conversations start with server defaults. Custom settings are not supported yet."
                    );
                }
                let settings = self.client.prewarm_thread(model, provider, project).await?;
                return attach_response(settings);
            }
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
                return attach_response(settings);
            }
            Request::ThreadTurnsList { params, .. } => {
                if params.items_view != Some(protocol::TurnItemsView::NotLoaded) {
                    bail!("Read cloud turn items separately with thread/items/list.");
                }
                let page = self
                    .client
                    .list_turns(&cloud::ListTurnsRequest {
                        thread_id: params.thread_id,
                        page: cloud::HistoryQuery {
                            cursor: params.cursor,
                            limit: params.limit,
                            order: history_order(
                                params
                                    .sort_direction
                                    .unwrap_or(protocol::SortDirection::Desc),
                            ),
                            ..Default::default()
                        },
                    })
                    .await?;
                serde_json::to_value(protocol::ThreadTurnsListResponse {
                    data: page
                        .data
                        .into_iter()
                        .map(|entry| events::turn(entry.summary))
                        .collect::<Result<_>>()?,
                    next_cursor: page.next_cursor,
                    backwards_cursor: page.backwards_cursor,
                })?
            }
            Request::ThreadItemsList { params, .. } => {
                let cursor = match params.cursor {
                    Some(protocol::ThreadItemsListCursor::Opaque(cursor)) => Some(cursor),
                    None => None,
                    Some(protocol::ThreadItemsListCursor::Anchor(_)) => {
                        bail!("Cloud item anchors are not supported yet.");
                    }
                };
                let page = self
                    .client
                    .list_items(&cloud::ListItemsRequest {
                        thread_id: params.thread_id.clone(),
                        turn_id: params.turn_id,
                        page: cloud::HistoryQuery {
                            cursor,
                            limit: params.limit,
                            order: history_order(
                                params
                                    .sort_direction
                                    .unwrap_or(protocol::SortDirection::Asc),
                            ),
                            ..Default::default()
                        },
                    })
                    .await?;
                let size = page.data.len();
                let data = page
                    .data
                    .into_iter()
                    .filter_map(|entry| {
                        Some(protocol::ThreadItemEntry {
                            turn_id: entry.turn_id,
                            item: events::item_summary(entry.summary?)?,
                            output_deferred: false,
                            started_at_ms: entry.started_at_ms,
                            completed_at_ms: entry.completed_at_ms,
                        })
                    })
                    .collect::<Vec<_>>();
                if data.len() != size
                    && self.warned_partial_history.as_ref() != Some(&params.thread_id)
                {
                    self.warned_partial_history = Some(params.thread_id.clone());
                    let warning = protocol::WarningNotification {
                        thread_id: Some(params.thread_id),
                        message: "Some Cloud history items cannot be displayed in this preview."
                            .to_owned(),
                    };
                    send_notification(output, protocol::ServerNotification::Warning(warning));
                }
                serde_json::to_value(protocol::ThreadItemsListResponse {
                    data,
                    next_cursor: page.next_cursor,
                    backwards_cursor: page.backwards_cursor,
                })?
            }
            Request::TurnStart { params, .. } => {
                let mut message = cloud_message(params)?;
                message.mode = mode;
                let admission = match self.client.add_user_message(&message).await {
                    Err(cloud::MutationError::InvalidRequest) => {
                        return Err(PreflightRejection(
                            "The Cloud message is empty, invalid, or too large.",
                        )
                        .into());
                    }
                    result => result?,
                };
                if matches!(mode, cloud::SubmissionMode::Steer) {
                    serde_json::to_value(admitted_message(admission.outcome)?)?
                } else {
                    let turn = admitted_turn(admission.outcome)?;
                    serde_json::to_value(protocol::TurnStartResponse { turn })?
                }
            }
            Request::TurnInterrupt { params, .. } => {
                self.client
                    .interrupt_turn(&cloud::InterruptTurnRequest {
                        thread_id: params.thread_id,
                        turn_id: params.turn_id,
                        shared_access: false,
                    })
                    .await?;
                serde_json::to_value(protocol::TurnInterruptResponse {})?
            }
            other => return Err(UnsupportedOperation(other.method_name().to_owned()).into()),
        };
        Ok(Response::Value(value))
    }
}

fn attach_response(mut settings: cloud::ThreadActivation) -> Result<Response> {
    use_paginated_history(&mut settings.thread);
    let resolved = serde_json::from_value(serde_json::to_value(settings)?)
        .map_err(|_| eyre!("The cloud returned unsupported conversation settings."))?;
    Ok(Response::Attach(resolved))
}

// Cloud serves history in pages even when an older thread used the legacy rollout format.
fn use_paginated_history(thread: &mut cloud::ThreadSnapshot) {
    thread.history_mode = cloud::ThreadHistoryMode::Paginated;
}

fn thread_list_query(params: protocol::ThreadListParams) -> Result<cloud::ThreadListQuery> {
    use protocol::ThreadSourceKind::Cli;
    use protocol::ThreadSourceKind::VsCode;
    // The resume picker's ordinary interactive list maps to Cloud's own interactive default.
    // Its explicit CLI/VS Code defaults are conventional source names.
    let sources = params.source_kinds.as_deref().unwrap_or_default();
    if !matches!(sources, [] | [Cli, VsCode] | [VsCode, Cli])
        || params.cwd.is_some()
        || params.search_term.is_some()
        || params.section_id.is_some()
        || params.project_id.is_some()
        || params.parent_thread_id.is_some()
        || params.ancestor_thread_id.is_some()
        || params.attached_to_thread_id.is_some()
        || params
            .model_providers
            .as_ref()
            .is_some_and(|values| !values.is_empty())
        || params
            .excluded_thread_ids
            .as_ref()
            .is_some_and(|values| !values.is_empty())
    {
        bail!("These cloud conversation filters are not supported yet.");
    }
    Ok(cloud::ThreadListQuery {
        cursor: params.cursor,
        limit: params.limit,
        archived: params.archived.unwrap_or_default(),
        originators: params.originators.unwrap_or_default(),
        sort_key: match params.sort_key {
            Some(protocol::ThreadSortKey::CreatedAt) | None => cloud::ThreadSortKey::CreatedAt,
            Some(protocol::ThreadSortKey::UpdatedAt) => cloud::ThreadSortKey::UpdatedAt,
            Some(protocol::ThreadSortKey::RecencyAt) => cloud::ThreadSortKey::RecencyAt,
            Some(protocol::ThreadSortKey::SectionPosition) => {
                bail!("Cloud section ordering is not supported yet.");
            }
        },
        sort_direction: match params.sort_direction {
            Some(protocol::SortDirection::Asc) => cloud::ThreadSortDirection::Asc,
            Some(protocol::SortDirection::Desc) | None => cloud::ThreadSortDirection::Desc,
        },
    })
}

fn history_order(direction: protocol::SortDirection) -> cloud::HistoryOrder {
    match direction {
        protocol::SortDirection::Asc => cloud::HistoryOrder::Ascending,
        protocol::SortDirection::Desc => cloud::HistoryOrder::Descending,
    }
}

fn cloud_message(
    mut params: protocol::TurnStartParams,
) -> Result<cloud::AddUserMessageRequest, PreflightRejection> {
    let thread_id = std::mem::take(&mut params.thread_id);
    let client_user_message_id = params
        .client_user_message_id
        .take()
        .ok_or(PreflightRejection(
            "A cloud message needs a stable submission ID.",
        ))?;
    let input = std::mem::take(&mut params.input);
    let turn_trigger = params.turn_trigger.take();
    let (model, effort, service_tier) = if turn_trigger.as_deref() == Some("user") {
        let tier = match params.service_tier.take() {
            Some(Some(tier)) if tier == SERVICE_TIER_DEFAULT_REQUEST_VALUE => Some(None),
            Some(Some(tier))
                if !matches!(tier.as_str(), "fast" | "priority" | "flex" | "ultrafast") =>
            {
                return Err(PreflightRejection(
                    "This service tier is unavailable in Cloud.",
                ));
            }
            tier => tier,
        };
        (params.model.take(), params.effort.take(), tier)
    } else {
        (None, None, None)
    };
    if params != protocol::TurnStartParams::default() {
        return Err(PreflightRejection(
            "Changing cloud settings or sending structured input is not supported yet.",
        ));
    }
    let mut content = Vec::with_capacity(input.len());
    for item in input {
        let item = match item {
            protocol::UserInput::Text {
                text,
                text_elements,
            } => {
                // Keep the existing line separation for consecutive plain text parts.
                if text_elements.is_empty()
                    && let Some(cloud::UserMessageInput::Text {
                        text: previous,
                        text_elements: previous_elements,
                    }) = content.last_mut()
                    && previous_elements.is_empty()
                {
                    previous.push('\n');
                    previous.push_str(&text);
                    continue;
                }
                cloud::UserMessageInput::Text {
                    text,
                    text_elements: text_elements
                        .into_iter()
                        .map(|element| cloud::UserMessageTextElement {
                            placeholder: element.placeholder().map(str::to_owned),
                            start: element.byte_range.start,
                            end: element.byte_range.end,
                        })
                        .collect(),
                }
            }
            protocol::UserInput::Image { image, detail } => cloud::UserMessageInput::Image {
                image: match image {
                    protocol::ImageReference::Inline { url } => cloud::ImageReference::Url(url),
                    protocol::ImageReference::File { file_id } => {
                        cloud::ImageReference::FileId(file_id)
                    }
                },
                detail: detail.map(|detail| match detail {
                    ImageDetail::Auto => cloud::ImageDetail::Auto,
                    ImageDetail::Low => cloud::ImageDetail::Low,
                    ImageDetail::High => cloud::ImageDetail::High,
                    ImageDetail::Original => cloud::ImageDetail::Original,
                }),
            },
            protocol::UserInput::LocalImage { .. } => {
                return Err(PreflightRejection(
                    "Prepare local images before sending them to Cloud.",
                ));
            }
            protocol::UserInput::Audio { .. }
            | protocol::UserInput::LocalAudio { .. }
            | protocol::UserInput::Skill { .. }
            | protocol::UserInput::Mention { .. } => {
                return Err(PreflightRejection(
                    "Cloud conversations currently accept text and images.",
                ));
            }
        };
        content.push(item);
    }
    Ok(cloud::AddUserMessageRequest {
        thread_id,
        client_user_message_id,
        input: content,
        turn_trigger,
        model,
        effort: effort.map(|effort| effort.to_string()),
        service_tier,
        mode: cloud::SubmissionMode::StartIfIdle,
        shared_access: false,
    })
}

fn admitted_message(outcome: cloud::AdmissionOutcome) -> Result<CloudUserMessageReceipt> {
    Ok(match outcome {
        cloud::AdmissionOutcome::Started(turn) => {
            CloudUserMessageReceipt::Started(events::turn(turn)?)
        }
        cloud::AdmissionOutcome::Steered(turn) => {
            CloudUserMessageReceipt::Steered(events::turn(turn)?)
        }
        cloud::AdmissionOutcome::Duplicate { turn_id } => {
            CloudUserMessageReceipt::Duplicate { turn_id }
        }
        cloud::AdmissionOutcome::Queued { .. } => {
            bail!(
                "Cloud already accepted this message without returning a turn. Reopen the conversation to see its status before sending again."
            );
        }
    })
}

fn admitted_turn(outcome: cloud::AdmissionOutcome) -> Result<protocol::Turn> {
    match admitted_message(outcome)? {
        CloudUserMessageReceipt::Started(turn) | CloudUserMessageReceipt::Steered(turn) => Ok(turn),
        CloudUserMessageReceipt::Duplicate { .. } => {
            bail!(
                "Cloud already accepted this message without returning a turn. Reopen the conversation to see its status before sending again."
            );
        }
    }
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
