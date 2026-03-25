use std::collections::BTreeMap;
use std::io;

use toml_edit::value;

use crate::codex::Session;
use crate::config::Config;
use crate::config::deserialize_config_toml_with_base;
use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::request_permissions::PermissionProfilePersistence;

pub(crate) fn persistence_target_for_permissions(
    config: &Config,
    permissions: &PermissionProfile,
) -> Option<PermissionProfilePersistence> {
    if !is_supported_filesystem_only_request(permissions) {
        return None;
    }

    let user_layer = config.config_layer_stack.get_user_layer()?;
    let user_config =
        deserialize_config_toml_with_base(user_layer.config.clone(), &config.codex_home).ok()?;
    let profile_name = user_config.default_permissions?;
    let permissions = user_config.permissions?;
    permissions
        .entries
        .contains_key(profile_name.as_str())
        .then_some(PermissionProfilePersistence { profile_name })
}

pub(crate) async fn persist_permissions_for_profile(
    sess: &Session,
    action: &codex_protocol::protocol::PersistPermissionProfileAction,
) -> io::Result<()> {
    let codex_home = sess.codex_home().await;

    let edits = filesystem_permission_edits(
        action.profile_name.as_str(),
        action.permissions.file_system.as_ref(),
    );
    if edits.is_empty() {
        return Ok(());
    }

    ConfigEditsBuilder::new(&codex_home)
        .with_edits(edits)
        .apply()
        .await
        .map_err(|err| io::Error::other(format!("failed to persist permission profile: {err}")))?;
    sess.reload_user_config_layer().await;
    Ok(())
}

fn is_supported_filesystem_only_request(permissions: &PermissionProfile) -> bool {
    let Some(file_system) = permissions.file_system.as_ref() else {
        return false;
    };

    if file_system.is_empty() {
        return false;
    }

    if permissions
        .network
        .as_ref()
        .and_then(|network| network.enabled)
        .unwrap_or(false)
    {
        return false;
    }

    permissions.macos.is_none()
}

fn filesystem_permission_edits(
    profile_name: &str,
    file_system: Option<&FileSystemPermissions>,
) -> Vec<ConfigEdit> {
    let Some(file_system) = file_system else {
        return Vec::new();
    };

    let mut path_access = BTreeMap::new();

    if let Some(read_roots) = file_system.read.as_ref() {
        for path in read_roots {
            path_access
                .entry(path.display().to_string())
                .or_insert(FileSystemAccessMode::Read);
        }
    }

    if let Some(write_roots) = file_system.write.as_ref() {
        for path in write_roots {
            path_access.insert(path.display().to_string(), FileSystemAccessMode::Write);
        }
    }

    path_access
        .into_iter()
        .map(|(path, access)| ConfigEdit::SetPath {
            segments: vec![
                "permissions".to_string(),
                profile_name.to_string(),
                "filesystem".to_string(),
                path,
            ],
            value: value(access.to_string()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use codex_utils_absolute_path::AbsolutePathBuf;

    fn absolute_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::from_absolute_path(path).expect("absolute path")
    }

    #[test]
    fn filesystem_permission_edits_upgrade_write_access() {
        let edits = filesystem_permission_edits(
            "workspace",
            Some(&FileSystemPermissions {
                read: Some(vec![
                    absolute_path("/tmp/read"),
                    absolute_path("/tmp/write"),
                ]),
                write: Some(vec![absolute_path("/tmp/write")]),
            }),
        );

        assert_eq!(edits.len(), 2);
        match &edits[0] {
            ConfigEdit::SetPath { segments, value } => {
                assert_eq!(
                    segments,
                    &[
                        "permissions".to_string(),
                        "workspace".to_string(),
                        "filesystem".to_string(),
                        "/tmp/read".to_string(),
                    ]
                );
                assert_eq!(
                    value.as_value().and_then(toml_edit::Value::as_str),
                    Some("read")
                );
            }
            other => panic!("unexpected edit: {other:?}"),
        }
        match &edits[1] {
            ConfigEdit::SetPath { segments, value } => {
                assert_eq!(
                    segments,
                    &[
                        "permissions".to_string(),
                        "workspace".to_string(),
                        "filesystem".to_string(),
                        "/tmp/write".to_string(),
                    ]
                );
                assert_eq!(
                    value.as_value().and_then(toml_edit::Value::as_str),
                    Some("write")
                );
            }
            other => panic!("unexpected edit: {other:?}"),
        }
    }
}
