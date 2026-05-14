use super::super::REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME;
use super::super::REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME;
use super::super::REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME;
use super::super::RemotePluginCatalogError;
use super::super::RemotePluginServiceConfig;
use super::local_paths;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_login::CodexAuth;
use codex_plugin::PluginId;
use codex_plugin::validate_plugin_segment;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Component;
use std::path::Path;

const PERSONAL_MARKETPLACE_NAME: &str = "codex-curated";
const PERSONAL_MARKETPLACE_DISPLAY_NAME: &str = "Personal";
const PERSONAL_MARKETPLACE_RELATIVE_PATH: &str = ".agents/plugins/marketplace.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePluginShareCheckoutResult {
    pub remote_plugin_id: String,
    pub plugin_id: String,
    pub plugin_name: String,
    pub plugin_path: AbsolutePathBuf,
    pub marketplace_name: String,
    pub marketplace_path: AbsolutePathBuf,
    pub remote_version: Option<String>,
}

pub async fn checkout_remote_plugin_share(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    codex_home: &Path,
    remote_plugin_id: &str,
) -> Result<RemotePluginShareCheckoutResult, RemotePluginCatalogError> {
    let detail = super::super::fetch_remote_plugin_detail_with_download_urls(
        config,
        auth,
        REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME,
        remote_plugin_id,
    )
    .await?;
    let plugin_name = detail.summary.name.clone();
    let remote_version = detail.release_version.clone();
    validate_plugin_segment(&plugin_name, "plugin name").map_err(|reason| {
        RemotePluginCatalogError::UnexpectedResponse(format!(
            "remote plugin `{remote_plugin_id}` returned invalid plugin name: {reason}"
        ))
    })?;
    if !is_checkout_supported_share_marketplace(&detail.marketplace_name)
        || detail.summary.share_context.is_none()
    {
        return Err(RemotePluginCatalogError::PluginShareCheckoutNotAvailable {
            remote_plugin_id: remote_plugin_id.to_string(),
        });
    }

    let home = crate::marketplace::home_dir().ok_or_else(|| {
        RemotePluginCatalogError::UnexpectedResponse(
            "could not determine home directory for personal plugin marketplace".to_string(),
        )
    })?;
    let home = AbsolutePathBuf::try_from(home).map_err(|err| {
        RemotePluginCatalogError::UnexpectedResponse(format!(
            "failed to resolve home directory for personal plugin marketplace: {err}"
        ))
    })?;

    let local_paths = load_share_local_paths_for_checkout(codex_home)?;
    let (local_plugin_path, already_checked_out) =
        editable_plugin_path_for_checkout(&home, &plugin_name, remote_plugin_id, &local_paths)?;

    let mut created_checkout_path = false;
    if !already_checked_out {
        let bundle = crate::remote_bundle::validate_remote_plugin_bundle(
            remote_plugin_id,
            &detail.marketplace_name,
            &plugin_name,
            detail.release_version.as_deref(),
            detail.bundle_download_url.as_deref(),
        )
        .map_err(|err| {
            RemotePluginCatalogError::UnexpectedResponse(format!(
                "failed to prepare remote plugin bundle checkout: {err}"
            ))
        })?;
        crate::remote_bundle::download_and_extract_remote_plugin_bundle_to_path(
            bundle,
            local_plugin_path.clone(),
        )
        .await
        .map_err(|err| {
            RemotePluginCatalogError::UnexpectedResponse(format!(
                "failed to check out remote plugin bundle: {err}"
            ))
        })?;
        created_checkout_path = true;
    }

    let marketplace = match update_personal_marketplace(
        &home,
        &plugin_name,
        &local_plugin_path,
        detail.summary.install_policy,
        detail.summary.auth_policy,
        detail
            .summary
            .interface
            .as_ref()
            .and_then(|interface| interface.category.clone()),
    ) {
        Ok(marketplace) => marketplace,
        Err(err) => {
            return Err(clean_up_created_checkout_path(
                created_checkout_path,
                &local_plugin_path,
                err,
            ));
        }
    };

    if let Err(err) = local_paths::record_plugin_share_local_path(
        codex_home,
        remote_plugin_id,
        local_plugin_path.clone(),
    ) {
        let err = RemotePluginCatalogError::UnexpectedResponse(format!(
            "failed to record plugin share local path mapping: {err}"
        ));
        return Err(clean_up_created_checkout_path(
            created_checkout_path,
            &local_plugin_path,
            err,
        ));
    }

    let plugin_id = PluginId::new(plugin_name.clone(), marketplace.name.clone())
        .map_err(|err| {
            RemotePluginCatalogError::UnexpectedResponse(format!(
                "failed to build checked out plugin id: {err}"
            ))
        })?
        .as_key();

    Ok(RemotePluginShareCheckoutResult {
        remote_plugin_id: remote_plugin_id.to_string(),
        plugin_id,
        plugin_name,
        plugin_path: local_plugin_path,
        marketplace_name: marketplace.name,
        marketplace_path: marketplace.path,
        remote_version,
    })
}

fn is_checkout_supported_share_marketplace(marketplace_name: &str) -> bool {
    matches!(
        marketplace_name,
        REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME
            | REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME
            | REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME
    )
}

fn load_share_local_paths_for_checkout(
    codex_home: &Path,
) -> Result<BTreeMap<String, AbsolutePathBuf>, RemotePluginCatalogError> {
    match local_paths::load_plugin_share_local_paths(codex_home) {
        Ok(paths) => Ok(paths),
        Err(err) if err.kind() == io::ErrorKind::InvalidData => Ok(BTreeMap::new()),
        Err(err) => Err(RemotePluginCatalogError::UnexpectedResponse(format!(
            "failed to load plugin share local path mapping: {err}"
        ))),
    }
}

fn editable_plugin_path_for_checkout(
    home: &AbsolutePathBuf,
    plugin_name: &str,
    remote_plugin_id: &str,
    local_paths: &BTreeMap<String, AbsolutePathBuf>,
) -> Result<(AbsolutePathBuf, bool), RemotePluginCatalogError> {
    if let Some(existing_path) = local_paths.get(remote_plugin_id)
        && existing_path.as_path().exists()
    {
        ensure_path_can_be_listed_in_personal_marketplace(home, existing_path)?;
        return Ok((existing_path.clone(), true));
    }

    let local_plugin_path = local_paths
        .get(remote_plugin_id)
        .cloned()
        .unwrap_or_else(|| home.join("plugins").join(plugin_name));
    ensure_path_can_be_listed_in_personal_marketplace(home, &local_plugin_path)?;

    if local_plugin_path.as_path().exists() {
        return Err(RemotePluginCatalogError::InvalidPluginPath {
            path: local_plugin_path.to_path_buf(),
            reason: format!(
                "cannot check out remote plugin `{remote_plugin_id}` because the local plugin path already exists"
            ),
        });
    }

    Ok((local_plugin_path, false))
}

fn clean_up_created_checkout_path(
    created_checkout_path: bool,
    local_plugin_path: &AbsolutePathBuf,
    original_err: RemotePluginCatalogError,
) -> RemotePluginCatalogError {
    if !created_checkout_path {
        return original_err;
    }

    match remove_created_checkout_path(local_plugin_path) {
        Ok(()) => original_err,
        Err(cleanup_err) => RemotePluginCatalogError::UnexpectedResponse(format!(
            "{original_err}; additionally failed to clean up checked out plugin path `{}`: {cleanup_err}",
            local_plugin_path.display()
        )),
    }
}

fn remove_created_checkout_path(local_plugin_path: &AbsolutePathBuf) -> io::Result<()> {
    if local_plugin_path.as_path().is_dir() {
        fs::remove_dir_all(local_plugin_path.as_path())
    } else {
        fs::remove_file(local_plugin_path.as_path())
    }
}

fn ensure_path_can_be_listed_in_personal_marketplace(
    home: &AbsolutePathBuf,
    path: &AbsolutePathBuf,
) -> Result<(), RemotePluginCatalogError> {
    personal_marketplace_relative_plugin_path(home, path).map(|_| ())
}

struct PersonalMarketplaceUpdate {
    name: String,
    path: AbsolutePathBuf,
}

fn update_personal_marketplace(
    home: &AbsolutePathBuf,
    plugin_name: &str,
    local_plugin_path: &AbsolutePathBuf,
    install_policy: PluginInstallPolicy,
    auth_policy: PluginAuthPolicy,
    category: Option<String>,
) -> Result<PersonalMarketplaceUpdate, RemotePluginCatalogError> {
    let marketplace_path = home.join(PERSONAL_MARKETPLACE_RELATIVE_PATH);
    let relative_plugin_path = personal_marketplace_relative_plugin_path(home, local_plugin_path)?;
    let mut marketplace = read_or_create_personal_marketplace(marketplace_path.as_path())?;
    let Some(marketplace_object) = marketplace.as_object_mut() else {
        return Err(invalid_marketplace_file(
            marketplace_path.as_path(),
            "personal marketplace file must contain a JSON object",
        ));
    };
    let marketplace_name = marketplace_object
        .entry("name")
        .or_insert_with(|| json!(PERSONAL_MARKETPLACE_NAME))
        .as_str()
        .ok_or_else(|| {
            invalid_marketplace_file(
                marketplace_path.as_path(),
                "marketplace name must be a string",
            )
        })?
        .to_string();
    validate_plugin_segment(&marketplace_name, "marketplace name").map_err(|reason| {
        invalid_marketplace_file(
            marketplace_path.as_path(),
            &format!("marketplace name is invalid: {reason}"),
        )
    })?;

    let plugins = marketplace_object
        .entry("plugins")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| {
            invalid_marketplace_file(
                marketplace_path.as_path(),
                "marketplace plugins must be an array",
            )
        })?;

    let new_entry = personal_marketplace_plugin_entry(
        plugin_name,
        &relative_plugin_path,
        install_policy,
        auth_policy,
        category,
    );

    if let Some(existing_entry) = plugins
        .iter_mut()
        .find(|entry| entry.get("name").and_then(JsonValue::as_str) == Some(plugin_name))
    {
        let existing_path = existing_entry
            .get("source")
            .and_then(|source| source.get("path"))
            .and_then(JsonValue::as_str);
        if existing_path != Some(relative_plugin_path.as_str()) {
            return Err(invalid_marketplace_file(
                marketplace_path.as_path(),
                &format!(
                    "marketplace already contains plugin `{plugin_name}` with a different source path"
                ),
            ));
        }
        *existing_entry = new_entry;
    } else {
        plugins.push(new_entry);
    }

    let contents = serde_json::to_string_pretty(&marketplace)
        .map_err(|err| RemotePluginCatalogError::UnexpectedResponse(err.to_string()))?;
    write_json_atomically(marketplace_path.as_path(), &format!("{contents}\n")).map_err(|err| {
        RemotePluginCatalogError::UnexpectedResponse(format!(
            "failed to update personal plugin marketplace: {err}"
        ))
    })?;

    Ok(PersonalMarketplaceUpdate {
        name: marketplace_name,
        path: marketplace_path,
    })
}

fn read_or_create_personal_marketplace(
    marketplace_path: &Path,
) -> Result<JsonValue, RemotePluginCatalogError> {
    match std::fs::read_to_string(marketplace_path) {
        Ok(contents) => serde_json::from_str(&contents).map_err(|err| {
            invalid_marketplace_file(
                marketplace_path,
                &format!("failed to parse personal marketplace file: {err}"),
            )
        }),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(json!({
            "name": PERSONAL_MARKETPLACE_NAME,
            "interface": {
                "displayName": PERSONAL_MARKETPLACE_DISPLAY_NAME,
            },
            "plugins": [],
        })),
        Err(err) => Err(RemotePluginCatalogError::UnexpectedResponse(format!(
            "failed to read personal plugin marketplace: {err}"
        ))),
    }
}

fn personal_marketplace_plugin_entry(
    plugin_name: &str,
    relative_plugin_path: &str,
    install_policy: PluginInstallPolicy,
    auth_policy: PluginAuthPolicy,
    category: Option<String>,
) -> JsonValue {
    let mut entry = json!({
        "name": plugin_name,
        "source": {
            "source": "local",
            "path": relative_plugin_path,
        },
        "policy": {
            "installation": plugin_install_policy_value(install_policy),
            "authentication": plugin_auth_policy_value(auth_policy),
        },
    });
    if let Some(category) = category
        && !category.trim().is_empty()
        && let Some(object) = entry.as_object_mut()
    {
        object.insert("category".to_string(), json!(category));
    }
    entry
}

fn plugin_install_policy_value(policy: PluginInstallPolicy) -> &'static str {
    match policy {
        PluginInstallPolicy::NotAvailable => "NOT_AVAILABLE",
        PluginInstallPolicy::Available => "AVAILABLE",
        PluginInstallPolicy::InstalledByDefault => "INSTALLED_BY_DEFAULT",
    }
}

fn plugin_auth_policy_value(policy: PluginAuthPolicy) -> &'static str {
    match policy {
        PluginAuthPolicy::OnInstall => "ON_INSTALL",
        PluginAuthPolicy::OnUse => "ON_USE",
    }
}

fn personal_marketplace_relative_plugin_path(
    home: &AbsolutePathBuf,
    local_plugin_path: &AbsolutePathBuf,
) -> Result<String, RemotePluginCatalogError> {
    let relative = local_plugin_path
        .as_path()
        .strip_prefix(home.as_path())
        .map_err(|_| RemotePluginCatalogError::InvalidPluginPath {
            path: local_plugin_path.to_path_buf(),
            reason: "local plugin path must be inside the home directory to be listed in the personal marketplace".to_string(),
        })?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => {
                let segment = segment.to_str().ok_or_else(|| {
                    RemotePluginCatalogError::InvalidPluginPath {
                        path: local_plugin_path.to_path_buf(),
                        reason: "local plugin path contains non-UTF-8 segments".to_string(),
                    }
                })?;
                segments.push(segment.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(RemotePluginCatalogError::InvalidPluginPath {
                    path: local_plugin_path.to_path_buf(),
                    reason:
                        "local plugin path cannot be represented as a personal marketplace path"
                            .to_string(),
                });
            }
        }
    }
    if segments.is_empty() {
        return Err(RemotePluginCatalogError::InvalidPluginPath {
            path: local_plugin_path.to_path_buf(),
            reason: "local plugin path must not be the home directory".to_string(),
        });
    }
    Ok(format!("./{}", segments.join("/")))
}

fn invalid_marketplace_file(path: &Path, message: &str) -> RemotePluginCatalogError {
    RemotePluginCatalogError::InvalidPluginPath {
        path: path.to_path_buf(),
        reason: message.to_string(),
    }
}

fn write_json_atomically(write_path: &Path, contents: &str) -> io::Result<()> {
    let parent = write_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path {} has no parent directory", write_path.display()),
        )
    })?;
    std::fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(contents.as_bytes())?;
    tmp.persist(write_path).map_err(|err| err.error)?;
    Ok(())
}
