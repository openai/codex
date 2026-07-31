use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::BufRead;
use std::path::Path;
use std::path::PathBuf;

use crate::model::DetectedConnectorCandidate;
use crate::model::DetectedConnectorSource;
use crate::sessions::ExternalAgentSessionMigration;

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

pub(crate) fn detect_cla_session_connectors(
    sessions: &[ExternalAgentSessionMigration],
    connector_metadata_roots: &[PathBuf],
) -> Vec<DetectedConnectorCandidate> {
    let session_attributions = sessions
        .iter()
        .filter_map(session_connector_attribution)
        .collect::<Vec<_>>();
    let connector_names_by_session =
        detect_imported_cla_session_connectors(&session_attributions, connector_metadata_roots);

    let mut candidates = BTreeMap::<String, DetectedConnectorCandidate>::new();
    for names in connector_names_by_session.into_values() {
        for name in names {
            let key = name.to_lowercase();
            let candidate = candidates.entry(key).or_insert(DetectedConnectorCandidate {
                name,
                session_count: 0,
                source: DetectedConnectorSource::RemoteMcpServersConfig,
            });
            candidate.session_count = candidate.session_count.saturating_add(1);
        }
    }
    candidates.into_values().collect()
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
                let Some(name) =
                    crate::sessions::normalized_connector_display_name(server.name.as_deref())
                else {
                    continue;
                };
                // Depending on the source client, attributionMcpServer contains either the
                // manifest UUID or its configured server name.
                let matches_uuid = server
                    .uuid
                    .as_deref()
                    .is_some_and(|uuid| attributed_server_ids.contains(uuid));
                let matches_name = attributed_server_ids
                    .iter()
                    .any(|server_id| server_id.eq_ignore_ascii_case(&name));
                if !matches_uuid && !matches_name {
                    continue;
                }
                connector_names.entry(name.to_lowercase()).or_insert(name);
            }
        }
    }

    connector_names_by_session
        .into_iter()
        .map(|(session_id, names)| (session_id, names.into_values().collect()))
        .collect()
}

fn session_connector_attribution(
    session: &ExternalAgentSessionMigration,
) -> Option<ImportedSessionConnectorAttribution> {
    let session_id = session
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|session_id| !session_id.is_empty())?
        .to_string();
    let file = fs::File::open(&session.path).ok()?;
    let server_ids = std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<JsonValue>(line.trim()).ok())
        .filter_map(|record| {
            record
                .get("attributionMcpServer")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .filter(|server_id| !server_id.is_empty())
                .map(ToOwned::to_owned)
        })
        .collect::<BTreeSet<_>>();
    (!server_ids.is_empty()).then_some(ImportedSessionConnectorAttribution {
        session_id,
        server_ids,
    })
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

#[cfg(test)]
#[path = "connectors_cla_tests.rs"]
mod tests;
