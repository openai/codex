use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use codex_app_server_protocol::CodexAvatarAdminAwardGrantParams;
use codex_app_server_protocol::CodexAvatarAdminCapabilitiesReadResponse;
use codex_app_server_protocol::CodexAvatarDefinition;
use codex_app_server_protocol::CodexAvatarEquipParams;
use codex_app_server_protocol::CodexAvatarInventoryReadResponse;
use codex_app_server_protocol::CodexAvatarOwnership;
use codex_app_server_protocol::CodexAvatarRarity;
use codex_app_server_protocol::CodexAvatarStatus;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_backend_client::Client as BackendClient;
use codex_backend_client::CodexAvatarAdminAwardGrantRequest as BackendAvatarAdminAwardGrantRequest;
use codex_backend_client::CodexAvatarAdminCapabilitiesResponse as BackendAvatarAdminCapabilitiesResponse;
use codex_backend_client::CodexAvatarDefinition as BackendAvatarDefinition;
use codex_backend_client::CodexAvatarInventoryResponse as BackendAvatarInventoryResponse;
use codex_backend_client::CodexAvatarOwnership as BackendAvatarOwnership;
use codex_backend_client::CodexAvatarRarity as BackendAvatarRarity;
use codex_backend_client::CodexAvatarStatus as BackendAvatarStatus;
use codex_backend_client::RequestError;
use codex_core::AuthManager;
use serde_json::Value;

pub(crate) async fn read_avatar_inventory(
    auth_manager: &AuthManager,
    chatgpt_base_url: &str,
) -> Result<CodexAvatarInventoryReadResponse, JSONRPCErrorError> {
    let client = avatar_backend_client(auth_manager, chatgpt_base_url).await?;
    let response = client
        .get_avatar_inventory()
        .await
        .map_err(|err| backend_avatar_error("read avatar inventory", err))?;
    Ok(map_avatar_inventory_response(response))
}

pub(crate) async fn equip_avatar(
    auth_manager: &AuthManager,
    chatgpt_base_url: &str,
    params: CodexAvatarEquipParams,
) -> Result<CodexAvatarInventoryReadResponse, JSONRPCErrorError> {
    let client = avatar_backend_client(auth_manager, chatgpt_base_url).await?;
    let response = client
        .equip_avatar(params.avatar_id)
        .await
        .map_err(|err| backend_avatar_error("equip avatar", err))?;
    Ok(map_avatar_inventory_response(response))
}

pub(crate) async fn grant_admin_avatar_award(
    auth_manager: &AuthManager,
    chatgpt_base_url: &str,
    params: CodexAvatarAdminAwardGrantParams,
) -> Result<CodexAvatarInventoryReadResponse, JSONRPCErrorError> {
    let client = avatar_backend_client(auth_manager, chatgpt_base_url).await?;
    let response = client
        .grant_admin_avatar_award(BackendAvatarAdminAwardGrantRequest {
            account_user_id: params.account_user_id,
            award_id: params.award_id,
            avatar_id: params.avatar_id,
            source_type: params.source_type,
            source_ref: params.source_ref,
            awarded_at: params.awarded_at,
            awarded_by: params.awarded_by,
            metadata_json: params.metadata_json,
            source_summary: params.source_summary,
        })
        .await
        .map_err(|err| backend_avatar_error("grant avatar award", err))?;
    Ok(map_avatar_inventory_response(response))
}

pub(crate) async fn read_avatar_admin_capabilities(
    auth_manager: &AuthManager,
    chatgpt_base_url: &str,
) -> Result<CodexAvatarAdminCapabilitiesReadResponse, JSONRPCErrorError> {
    let client = avatar_backend_client(auth_manager, chatgpt_base_url).await?;
    let response = client
        .get_avatar_admin_capabilities()
        .await
        .map_err(|err| backend_avatar_error("read avatar admin capabilities", err))?;
    Ok(map_avatar_admin_capabilities_response(response))
}

async fn avatar_backend_client(
    auth_manager: &AuthManager,
    chatgpt_base_url: &str,
) -> Result<BackendClient, JSONRPCErrorError> {
    let Some(auth) = auth_manager.auth().await else {
        return Err(JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: "codex account authentication required to manage avatars".to_string(),
            data: None,
        });
    };

    if !auth.is_chatgpt_auth() {
        return Err(JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: "chatgpt authentication required to manage avatars".to_string(),
            data: None,
        });
    }

    BackendClient::from_auth(chatgpt_base_url.to_string(), &auth).map_err(|err| JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message: format!("failed to construct backend client: {err}"),
        data: None,
    })
}

fn backend_avatar_error(action: &str, err: RequestError) -> JSONRPCErrorError {
    match &err {
        RequestError::UnexpectedStatus { status, body, .. }
            if status.as_u16() == 400 || status.as_u16() == 403 =>
        {
            JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: avatar_error_detail(body)
                    .unwrap_or_else(|| format!("failed to {action}: {err}")),
                data: None,
            }
        }
        _ => JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: format!("failed to {action}: {err}"),
            data: None,
        },
    }
}

fn avatar_error_detail(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    value
        .get("detail")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn map_avatar_inventory_response(
    response: BackendAvatarInventoryResponse,
) -> CodexAvatarInventoryReadResponse {
    CodexAvatarInventoryReadResponse {
        account_user_id: response.account_user_id,
        avatar_definitions: response
            .avatar_definitions
            .into_iter()
            .map(map_avatar_definition)
            .collect(),
        owned_avatars: response
            .owned_avatars
            .into_iter()
            .map(map_avatar_ownership)
            .collect(),
        equipped_avatar_id: response.equipped_avatar_id,
        equipped_at: response.equipped_at,
        updated_at: response.updated_at,
        synced_at: response.synced_at,
        catalog_version: response.catalog_version,
    }
}

fn map_avatar_admin_capabilities_response(
    response: BackendAvatarAdminCapabilitiesResponse,
) -> CodexAvatarAdminCapabilitiesReadResponse {
    CodexAvatarAdminCapabilitiesReadResponse {
        can_grant_awards: response.can_grant_awards,
    }
}

fn map_avatar_definition(definition: BackendAvatarDefinition) -> CodexAvatarDefinition {
    CodexAvatarDefinition {
        avatar_id: definition.avatar_id,
        slug: definition.slug,
        display_name: definition.display_name,
        description: definition.description,
        rarity: match definition.rarity {
            BackendAvatarRarity::Common => CodexAvatarRarity::Common,
            BackendAvatarRarity::Rare => CodexAvatarRarity::Rare,
            BackendAvatarRarity::Epic => CodexAvatarRarity::Epic,
            BackendAvatarRarity::Legendary => CodexAvatarRarity::Legendary,
        },
        asset_ref: definition.asset_ref,
        status: match definition.status {
            BackendAvatarStatus::Active => CodexAvatarStatus::Active,
            BackendAvatarStatus::Hidden => CodexAvatarStatus::Hidden,
            BackendAvatarStatus::Retired => CodexAvatarStatus::Retired,
        },
        sort_order: definition.sort_order,
        created_at: definition.created_at,
        updated_at: definition.updated_at,
    }
}

fn map_avatar_ownership(ownership: BackendAvatarOwnership) -> CodexAvatarOwnership {
    CodexAvatarOwnership {
        account_user_id: ownership.account_user_id,
        avatar_id: ownership.avatar_id,
        first_unlocked_at: ownership.first_unlocked_at,
        last_awarded_at: ownership.last_awarded_at,
        source_summary: ownership.source_summary,
    }
}
