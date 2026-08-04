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
    super::aggregate_session_connector_candidates(detect_cla_session_connectors_by_source_path(
        sessions,
        connector_metadata_roots,
    ))
}

pub(crate) fn detect_cla_session_connectors_by_source_path(
    sessions: &[ExternalAgentSessionMigration],
    connector_metadata_roots: &[PathBuf],
) -> BTreeMap<PathBuf, Vec<DetectedConnectorCandidate>> {
    let session_attributions_by_source_path = sessions
        .iter()
        .filter_map(|session| {
            let source_path = fs::canonicalize(&session.path).ok()?;
            let attribution = session_connector_attribution(session)?;
            Some((source_path, attribution))
        })
        .collect::<BTreeMap<_, _>>();
    let connector_names_by_source_path = detect_imported_cla_session_connectors_by_source_path(
        &session_attributions_by_source_path,
        connector_metadata_roots,
    );

    connector_names_by_source_path
        .into_iter()
        .filter_map(|(source_path, names)| {
            let candidates = names
                .into_iter()
                .map(|name| DetectedConnectorCandidate {
                    name,
                    session_count: 1,
                    source: DetectedConnectorSource::RemoteMcpServersConfig,
                })
                .collect::<Vec<_>>();
            (!candidates.is_empty()).then_some((source_path, candidates))
        })
        .collect()
}

pub fn detect_imported_cla_session_connectors_by_source_path(
    session_attributions_by_source_path: &BTreeMap<PathBuf, ImportedSessionConnectorAttribution>,
    connector_metadata_roots: &[PathBuf],
) -> BTreeMap<PathBuf, Vec<String>> {
    detect_connector_names_by_key(
        session_attributions_by_source_path
            .iter()
            .map(|(source_path, attribution)| (source_path.clone(), attribution)),
        connector_metadata_roots,
    )
}

pub fn detect_imported_cla_session_connectors(
    session_attributions: &[ImportedSessionConnectorAttribution],
    connector_metadata_roots: &[PathBuf],
) -> BTreeMap<String, Vec<String>> {
    detect_connector_names_by_key(
        session_attributions
            .iter()
            .map(|attribution| (attribution.session_id.clone(), attribution)),
        connector_metadata_roots,
    )
}

fn detect_connector_names_by_key<'a, Key>(
    keyed_attributions: impl IntoIterator<Item = (Key, &'a ImportedSessionConnectorAttribution)>,
    connector_metadata_roots: &[PathBuf],
) -> BTreeMap<Key, Vec<String>>
where
    Key: Clone + Ord,
{
    // Session IDs come from file stems and can repeat across projects, so
    // preserve the caller's key while matching shared manifest metadata.
    let mut attributions_by_session_id = BTreeMap::<String, Vec<(Key, BTreeSet<String>)>>::new();
    for (key, attribution) in keyed_attributions {
        attributions_by_session_id
            .entry(attribution.session_id.clone())
            .or_default()
            .push((key, attribution.server_ids.clone()));
    }
    if attributions_by_session_id.is_empty() {
        return BTreeMap::new();
    }

    let mut connector_names_by_key = BTreeMap::<Key, BTreeMap<String, String>>::new();
    for metadata_root in connector_metadata_roots {
        let manifests_root = metadata_root.join(SESSION_MANIFESTS_DIR);
        for manifest_path in json_files_recursively(&manifests_root) {
            let Some(manifest) = read_session_manifest(&manifest_path) else {
                continue;
            };
            let Some(session_id) = manifest.cli_session_id else {
                continue;
            };
            let Some(keyed_attributions) = attributions_by_session_id.get(&session_id) else {
                continue;
            };
            for server in &manifest.remote_mcp_servers_config {
                let Some(name) =
                    crate::sessions::normalized_connector_display_name(server.name.as_deref())
                else {
                    continue;
                };
                for (key, attributed_server_ids) in keyed_attributions {
                    if attributed_server_ids.is_empty() {
                        continue;
                    }
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
                    connector_names_by_key
                        .entry(key.clone())
                        .or_default()
                        .entry(name.to_lowercase())
                        .or_insert_with(|| name.clone());
                }
            }
        }
    }

    connector_names_by_key
        .into_iter()
        .map(|(key, names)| (key, names.into_values().collect()))
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
