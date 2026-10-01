//! Resolve the cyber treatment for an exec turn from the connected server's catalog.

use crate::RequestIdSequencer;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_protocol::Account;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CyberAccessProgram;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::GetAccountResponse;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_protocol::openai_models::ModelAccessPrograms;

pub(crate) async fn program_for_turn(
    client: &InProcessAppServerClient,
    request_ids: &mut RequestIdSequencer,
    model: &str,
    provider: &str,
    enabled: bool,
) -> anyhow::Result<Option<CyberAccessProgram>> {
    if provider != "openai" {
        anyhow::ensure!(!enabled, "Daybreak requires the OpenAI model provider");
        return Ok(None);
    }

    if enabled {
        anyhow::ensure!(
            has_chatgpt_account(client, request_ids).await?,
            "Daybreak requires a signed-in ChatGPT account"
        );
    }

    let mut cursor = None;
    loop {
        let result = client
            .request_typed::<ModelListResponse>(ClientRequest::ModelList {
                request_id: request_ids.next(),
                params: ModelListParams {
                    cursor,
                    limit: None,
                    include_hidden: Some(true),
                },
            })
            .await;
        let Ok(result) = result else {
            anyhow::ensure!(!enabled, "Daybreak availability could not be determined");
            return Ok(None);
        };
        if let Some(selected) = result.data.into_iter().find(|entry| entry.model == model) {
            let programs = selected
                .available_access_programs
                .map(ModelAccessPrograms::from);
            if enabled {
                return programs
                    .and_then(|programs| programs.daybreak())
                    .map(|program| Some(program.into()))
                    .ok_or_else(|| anyhow::anyhow!("Daybreak is unavailable for model {model}; turn it off or choose a compatible model"));
            }
            let Some(standard) = programs.and_then(|programs| programs.standard()) else {
                return Ok(None);
            };
            return Ok(has_chatgpt_account(client, request_ids)
                .await
                .unwrap_or(false)
                .then_some(standard.into()));
        }
        match result.next_cursor {
            Some(next) => cursor = Some(next),
            None => {
                anyhow::ensure!(
                    !enabled,
                    "Daybreak availability could not be determined for model {model}"
                );
                return Ok(None);
            }
        }
    }
}

async fn has_chatgpt_account(
    client: &InProcessAppServerClient,
    request_ids: &mut RequestIdSequencer,
) -> anyhow::Result<bool> {
    let account = client
        .request_typed::<GetAccountResponse>(ClientRequest::GetAccount {
            request_id: request_ids.next(),
            params: GetAccountParams {
                refresh_token: false,
            },
        })
        .await
        .map_err(|error| {
            anyhow::anyhow!("Daybreak account availability could not be determined: {error}")
        })?;
    Ok(matches!(account.account, Some(Account::Chatgpt { .. })))
}
