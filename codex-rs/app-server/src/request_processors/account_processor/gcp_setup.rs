//! Validates and persists GCP setup without changing saved login credentials or existing threads.

use super::super::config_processor::map_error as map_config_error;
use super::AccountRequestProcessor;
use super::CloudLoginProvider;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::GcpSetupParams;
use codex_app_server_protocol::GcpSetupResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_config::apply_default_gcp_auth;
use codex_login::RefreshTokenError;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::GCP_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::merge_gcp_provider_overrides;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::MapAccess;
use serde::de::Visitor;
use serde::de::value::MapAccessDeserializer;
use serde_json::json;
use std::fmt;
use std::io::BufReader;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs::File;

pub(crate) async fn handle_gcp_setup(
    processor: &AccountRequestProcessor,
    params: GcpSetupParams,
) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
    processor.ensure_provider_login_allowed(CloudLoginProvider::Gcp)?;
    let network_policy = processor
        .auth_manager
        .application_network_policy()
        .for_current_account();
    let layers = processor
        .model_catalog
        .load_startup_config_layers()
        .await
        .map_err(|err| internal_error(format!("failed to load configuration layers: {err}")))?;
    let (write, provider) = processor
        .config_manager
        .prepare_provider_config(
            &layers,
            GCP_PROVIDER_ID,
            [
                ("project_id", Some(params.project_id.trim())),
                (
                    "location",
                    Some(params.location.as_deref().unwrap_or("global").trim()),
                ),
                ("credential_file", params.credential_file.as_deref()),
            ],
        )
        .map_err(map_config_error)?;
    if provider.base_url.is_some() {
        return Err(invalid_request(
            "Google Cloud has a custom `base_url`. Clear `model_providers.gcp.base_url` before configuring a project and location.",
        ));
    }
    // Even checking a Windows UNC credential path can contact the remote host.
    let permit = network_policy
        .acquire_for_unsupported_sdk()
        .map_err(|err| invalid_request(err.to_string()))?;
    permit
        .run(verify_credentials(provider, &processor.config.cwd))
        .await
        .map_err(|err| invalid_request(err.to_string()))??;

    processor
        .config_manager
        .commit_provider_config(write)
        .await
        .map_err(map_config_error)?;
    processor.cancel_active_login().await;
    processor.model_catalog.invalidate().await;
    Ok(Some(GcpSetupResponse {}.into()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum AuthErrorReason {
    GcloudNotInstalled,
    CredentialFileInvalid,
    AuthCommandFailed,
}

impl AuthErrorReason {
    fn error(self, message: impl Into<String>) -> JSONRPCErrorError {
        let mut error = invalid_request(message);
        error.data = Some(json!({ "reason": self }));
        error
    }
}

async fn open_credential_file(path: &Path) -> std::io::Result<File> {
    if !tokio::fs::metadata(path).await?.is_file() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "expected a regular file",
        ));
    }
    File::open(path).await
}

// Require a JSON object. Serde also accepts positional arrays when deserializing a struct.
struct AdcObject;

impl<'de> Visitor<'de> for AdcObject {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Google Cloud credential object")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<(), A::Error> {
        #[derive(Deserialize)]
        struct Adc {
            #[serde(rename = "type")]
            kind: String,
        }
        let adc = Adc::deserialize(MapAccessDeserializer::new(map))?;
        if adc.kind.trim().is_empty() {
            return Err(serde::de::Error::custom(
                "expected a nonempty credential type",
            ));
        }
        Ok(())
    }
}

async fn validate_adc_file(file: File) -> std::io::Result<()> {
    let file = file.into_std().await;
    tokio::task::spawn_blocking(move || {
        // Stream past type-specific fields without retaining the credential contents.
        let mut json = serde_json::Deserializer::from_reader(BufReader::new(file));
        json.deserialize_map(AdcObject)?;
        json.end()?;
        Ok(())
    })
    .await?
}

async fn verify_credentials(
    configured: ModelProviderInfo,
    cwd: &AbsolutePathBuf,
) -> Result<(), JSONRPCErrorError> {
    configured
        .validate_gcp_credential_file()
        .map_err(|err| AuthErrorReason::CredentialFileInvalid.error(err))?;
    let default_gcloud = configured.auth.is_none();
    let mut candidate = ModelProviderInfo::create_gcp_provider();
    merge_gcp_provider_overrides(Some(&mut candidate), configured).map_err(invalid_request)?;
    apply_default_gcp_auth(&mut candidate, cwd);
    candidate.validate().map_err(invalid_request)?;
    let auth_file = candidate.credential_file.clone().or_else(|| {
        if default_gcloud {
            std::env::var_os("GOOGLE_APPLICATION_CREDENTIALS").map(PathBuf::from)
        } else {
            None
        }
    });
    if let Some(path) = auth_file {
        let path = cwd.as_path().join(path);
        let file = open_credential_file(&path).await.map_err(|err| {
            AuthErrorReason::CredentialFileInvalid.error(format!(
                "Cannot read the Google Cloud credential file: {err}"
            ))
        })?;
        if default_gcloud {
            validate_adc_file(file).await.map_err(|_| AuthErrorReason::CredentialFileInvalid.error(
                "Expected a Google Cloud credential file containing a JSON object with a nonempty string type.",
            ))?;
        }
    }
    let provider = create_model_provider(candidate, /*auth_manager*/ None);
    provider
        .runtime_base_url()
        .await
        .map_err(|err| invalid_request(err.to_string()))?;
    // This fresh provider owns a command-backed manager. A refresh runs that command
    // once and preserves errors that the inference auth path reduces to missing auth.
    let auth_manager = provider
        .auth_manager()
        .ok_or_else(|| internal_error("Missing GCP auth manager."))?;
    if let Err(err) = auth_manager.refresh_token().await {
        if default_gcloud
            && let RefreshTokenError::Transient(err) = err
            && err.kind() == ErrorKind::NotFound
            && cwd.as_path().is_dir()
        {
            return Err(AuthErrorReason::GcloudNotInstalled.error(
                "Cannot find gcloud on the app-server host. Check its installation and PATH.",
            ));
        }
        return Err(AuthErrorReason::AuthCommandFailed
            .error("The Google Cloud auth command could not obtain an access token."));
    }

    Ok(())
}
