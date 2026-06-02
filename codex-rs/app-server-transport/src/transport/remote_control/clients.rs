use super::auth::RemoteControlConnectionAuth;
use super::auth::load_remote_control_auth;
use super::auth::recover_remote_control_auth;
use super::enroll::REMOTE_CONTROL_ACCOUNT_ID_HEADER;
use super::enroll::format_headers;
use super::enroll::preview_remote_control_response_body;
use super::protocol::normalize_remote_control_base_url;
use axum::http::HeaderMap;
use codex_app_server_protocol::RemoteControlClient;
use codex_app_server_protocol::RemoteControlClientsListOrder;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsListResponse;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlClientsRevokeResponse;
use codex_login::AuthManager;
use codex_login::default_client::build_reqwest_client;
use serde::Deserialize;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use url::Url;

const REMOTE_CONTROL_CLIENT_MANAGEMENT_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct ListRemoteControlClientsResponse {
    items: Vec<RemoteControlClientResponse>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteControlClientResponse {
    client_id: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    device_type: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    os_version: Option<String>,
    #[serde(default)]
    device_model: Option<String>,
    #[serde(default)]
    app_version: Option<String>,
    #[serde(default)]
    last_seen_at: Option<String>,
}

enum ClientManagementRequest<'a> {
    List {
        url: &'a Url,
        params: &'a RemoteControlClientsListParams,
    },
    Revoke {
        url: &'a Url,
    },
}

struct ClientManagementResponse {
    status: axum::http::StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

pub(super) async fn list_remote_control_clients(
    remote_control_url: &str,
    auth_manager: &Arc<AuthManager>,
    params: RemoteControlClientsListParams,
) -> io::Result<RemoteControlClientsListResponse> {
    if params.environment_id.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "remote control client list requires environmentId",
        ));
    }
    if params
        .limit
        .is_some_and(|limit| !(1..=100).contains(&limit))
    {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "remote control client list limit must be between 1 and 100",
        ));
    }
    let url = environment_clients_url(remote_control_url, &params.environment_id)?;
    let response = send_client_management_request(
        auth_manager,
        ClientManagementRequest::List {
            url: &url,
            params: &params,
        },
        "list remote control clients",
    )
    .await?;
    let ClientManagementResponse {
        status,
        headers,
        body,
    } = response;
    let body_preview = preview_remote_control_response_body(&body);
    ensure_success_response(status, &headers, &url, &body_preview, "client list")?;
    let response = serde_json::from_slice::<ListRemoteControlClientsResponse>(&body).map_err(
        |err| {
            io::Error::other(format!(
                "failed to parse remote control client list response from `{url}`: HTTP {status}, {}, body: {body_preview}, decode error: {err}",
                format_headers(&headers)
            ))
        },
    )?;
    Ok(RemoteControlClientsListResponse {
        data: response
            .items
            .into_iter()
            .map(RemoteControlClient::try_from)
            .collect::<io::Result<_>>()?,
        next_cursor: response.cursor,
    })
}

pub(super) async fn revoke_remote_control_client(
    remote_control_url: &str,
    auth_manager: &Arc<AuthManager>,
    params: RemoteControlClientsRevokeParams,
) -> io::Result<RemoteControlClientsRevokeResponse> {
    if params.environment_id.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "remote control client revoke requires environmentId",
        ));
    }
    if params.client_id.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "remote control client revoke requires clientId",
        ));
    }
    let url = environment_client_url(
        remote_control_url,
        &params.environment_id,
        &params.client_id,
    )?;
    let response = send_client_management_request(
        auth_manager,
        ClientManagementRequest::Revoke { url: &url },
        "revoke remote control client",
    )
    .await?;
    let ClientManagementResponse {
        status,
        headers,
        body,
    } = response;
    let body_preview = preview_remote_control_response_body(&body);
    ensure_success_response(status, &headers, &url, &body_preview, "client revoke")?;
    Ok(RemoteControlClientsRevokeResponse {})
}

async fn send_client_management_request(
    auth_manager: &Arc<AuthManager>,
    request: ClientManagementRequest<'_>,
    action: &str,
) -> io::Result<ClientManagementResponse> {
    let mut auth_recovery = auth_manager.unauthorized_recovery();
    let mut auth_change_rx = auth_manager.auth_change_receiver();
    let auth = load_remote_control_auth(auth_manager).await?;
    let response = send_client_management_request_once(&auth, &request, action).await?;
    if response.status.as_u16() != 401
        || !recover_remote_control_auth(&mut auth_recovery, &mut auth_change_rx).await
    {
        return Ok(response);
    }
    let auth = load_remote_control_auth(auth_manager).await?;
    send_client_management_request_once(&auth, &request, action).await
}

async fn send_client_management_request_once(
    auth: &RemoteControlConnectionAuth,
    request: &ClientManagementRequest<'_>,
    action: &str,
) -> io::Result<ClientManagementResponse> {
    let client = build_reqwest_client();
    let mut auth_headers = HeaderMap::new();
    auth.auth_provider.add_auth_headers(&mut auth_headers);
    let request = match request {
        ClientManagementRequest::List { url, params } => {
            let mut query = Vec::new();
            if let Some(cursor) = &params.cursor {
                query.push(("cursor", cursor.clone()));
            }
            if let Some(limit) = params.limit {
                query.push(("limit", limit.to_string()));
            }
            if let Some(order) = params.order {
                query.push(("order", clients_list_order_as_str(order).to_string()));
            }
            client.get((*url).clone()).query(&query)
        }
        ClientManagementRequest::Revoke { url } => client.delete((*url).clone()),
    };
    let response = request
        .timeout(REMOTE_CONTROL_CLIENT_MANAGEMENT_TIMEOUT)
        .headers(auth_headers)
        .header(REMOTE_CONTROL_ACCOUNT_ID_HEADER, &auth.account_id)
        .send()
        .await
        .map_err(|err| io::Error::other(format!("failed to {action}: {err}")))?;
    let headers = response.headers().clone();
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|err| io::Error::other(format!("failed to read {action} response: {err}")))?
        .to_vec();
    Ok(ClientManagementResponse {
        status,
        headers,
        body,
    })
}

fn ensure_success_response(
    status: axum::http::StatusCode,
    headers: &HeaderMap,
    url: &Url,
    body_preview: &str,
    response_kind: &str,
) -> io::Result<()> {
    if status.is_success() {
        return Ok(());
    }
    let error_kind = match status.as_u16() {
        400 => ErrorKind::InvalidInput,
        401 | 403 => ErrorKind::PermissionDenied,
        404 => ErrorKind::NotFound,
        _ => ErrorKind::Other,
    };
    Err(io::Error::new(
        error_kind,
        format!(
            "remote control {response_kind} failed at `{url}`: HTTP {status}, {}, body: {body_preview}",
            format_headers(headers)
        ),
    ))
}

fn environment_clients_url(remote_control_url: &str, environment_id: &str) -> io::Result<Url> {
    let mut url = normalize_remote_control_base_url(remote_control_url)?
        .join("wham/remote/control/environments")
        .map_err(io::Error::other)?;
    url.path_segments_mut()
        .map_err(|()| {
            io::Error::new(
                ErrorKind::InvalidInput,
                "remote control URL cannot be a base",
            )
        })?
        .push(environment_id)
        .push("clients");
    Ok(url)
}

fn environment_client_url(
    remote_control_url: &str,
    environment_id: &str,
    client_id: &str,
) -> io::Result<Url> {
    let mut url = environment_clients_url(remote_control_url, environment_id)?;
    url.path_segments_mut()
        .map_err(|()| {
            io::Error::new(
                ErrorKind::InvalidInput,
                "remote control URL cannot be a base",
            )
        })?
        .push(client_id);
    Ok(url)
}

fn clients_list_order_as_str(order: RemoteControlClientsListOrder) -> &'static str {
    match order {
        RemoteControlClientsListOrder::Asc => "asc",
        RemoteControlClientsListOrder::Desc => "desc",
    }
}

impl TryFrom<RemoteControlClientResponse> for RemoteControlClient {
    type Error = io::Error;

    fn try_from(client: RemoteControlClientResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            client_id: client.client_id,
            display_name: client.display_name,
            device_type: client.device_type,
            platform: client.platform,
            os_version: client.os_version,
            device_model: client.device_model,
            app_version: client.app_version,
            last_seen_at: client
                .last_seen_at
                .map(|last_seen_at| {
                    OffsetDateTime::parse(&last_seen_at, &Rfc3339)
                        .map(OffsetDateTime::unix_timestamp)
                        .map_err(|err| {
                            io::Error::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "failed to parse remote control client last_seen_at `{last_seen_at}`: {err}"
                                ),
                            )
                        })
                })
                .transpose()?,
        })
    }
}
