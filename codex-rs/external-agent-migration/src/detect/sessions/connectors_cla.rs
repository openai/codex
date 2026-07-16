use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

const SESSION_MANIFESTS_DIR: &str = "claude-code-sessions";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSessionConnectorAttribution {
    pub session_id: String,
    pub server_ids: BTreeSet<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionManifest {
    cli_session_id: Option<String>,
    #[serde(default)]
    remote_mcp_servers_config: Vec<RemoteMcpServerConfig>,
}

#[derive(Deserialize)]
struct RemoteMcpServerConfig {
    name: Option<String>,
    uuid: Option<String>,
}

pub fn detect_imported_cla_session_connectors(
    session_attributions: &[ImportedSessionConnectorAttribution],
    connector_metadata_roots: &[PathBuf],
) -> BTreeMap<String, Vec<String>> {
    if session_attributions.is_empty() {
        return BTreeMap::new();
    }

    let attributed_server_ids_by_session = session_attributions
        .iter()
        .map(|attribution| {
            (
                attribution.session_id.clone(),
                attribution.server_ids.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut connector_names_by_session = BTreeMap::<String, BTreeMap<String, String>>::new();

    for metadata_root in connector_metadata_roots {
        let manifests_root = metadata_root.join(SESSION_MANIFESTS_DIR);
        for manifest_path in json_files_recursively(&manifests_root) {
            let Some(manifest) = read_session_manifest(&manifest_path) else {
                continue;
            };
            let Some(session_id) = manifest.cli_session_id else {
                continue;
            };
            let Some(attributed_server_ids) = attributed_server_ids_by_session.get(&session_id)
            else {
                continue;
            };
            if attributed_server_ids.is_empty() {
                continue;
            }

            let connector_names = connector_names_by_session.entry(session_id).or_default();
            for server in manifest.remote_mcp_servers_config {
                let Some(uuid) = server.uuid else {
                    continue;
                };
                if !attributed_server_ids.contains(&uuid) {
                    continue;
                }
                let Some(name) =
                    crate::sessions::normalized_connector_display_name(server.name.as_deref())
                else {
                    continue;
                };
                connector_names.entry(name.to_lowercase()).or_insert(name);
            }
        }
    }

    connector_names_by_session
        .into_iter()
        .map(|(session_id, names)| (session_id, names.into_values().collect()))
        .collect()
}

fn read_session_manifest(path: &Path) -> Option<SessionManifest> {
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn json_files_recursively(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file()
                && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            {
                files.push(entry.path());
            }
        }
    }
    files
}
