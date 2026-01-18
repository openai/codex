use crate::client::lsp_client::LspClient;
use crate::config::LspConfig;
use crate::config::LspDiagnosticsInPrompt;
use crate::config::LspMode;
use crate::detect::detect_servers_for_file;
use crate::detect::find_root_with_markers;
use crate::diagnostics::DiagnosticEntry;
use crate::diagnostics::DiagnosticStore;
use crate::diagnostics::SeverityFilter;
use crate::registry::InstallStrategy;
use crate::registry::LanguageServerId;
use crate::registry::ServerRegistry;
use crate::registry::ServerSpec;
use crate::text::position_for_offset;
use crate::uri::uri_from_directory_path;
use crate::uri::uri_from_file_path;
use crate::uri::uri_to_file_path;
use crate::workspace_edit::WorkspaceEditError;
use crate::workspace_edit::WorkspaceEditResult;
use crate::workspace_edit::workspace_edit_to_apply_patch;
use anyhow::Context;
use anyhow::Result;
use lsp_types::Position;
use lsp_types::TextDocumentIdentifier;
use lsp_types::Uri;
use lsp_types::WorkspaceEdit;
use serde_json::Value;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum LspError {
    #[error("LSP is disabled")]
    Disabled,
    #[error("no language server detected for {0}")]
    NotDetected(String),
    #[error("language server {0} is disabled")]
    ServerDisabled(LanguageServerId),
    #[error("language server {0} is not installed")]
    NotInstalled(LanguageServerId),
    #[error("invalid position {line}:{character} for {path}")]
    InvalidPosition {
        path: String,
        line: u32,
        character: u32,
    },
    #[error("language server error: {0}")]
    ServerError(String),
    #[error(transparent)]
    WorkspaceEdit(#[from] WorkspaceEditError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct LspManagerStatusEntry {
    pub id: LanguageServerId,
    pub enabled: bool,
    pub detected: bool,
    pub running: bool,
    pub installed: bool,
    pub root: Option<PathBuf>,
    pub command: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LspManagerStatus {
    pub entries: Vec<LspManagerStatusEntry>,
}

#[derive(Clone)]
pub struct LspManager {
    config: LspConfig,
    root: PathBuf,
    registry: ServerRegistry,
    diagnostics: DiagnosticStore,
    state: std::sync::Arc<Mutex<LspState>>,
}

struct LspState {
    servers: HashMap<ServerKey, ServerHandle>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ServerKey {
    id: LanguageServerId,
    root: PathBuf,
}

struct ServerHandle {
    spec: ServerSpec,
    root: PathBuf,
    client: LspClient,
    open_docs: HashMap<Uri, OpenDocument>,
    command: Vec<String>,
}

#[derive(Debug, Clone)]
struct OpenDocument {
    version: i32,
}

impl LspManager {
    pub fn new(config: LspConfig, root: PathBuf) -> Self {
        Self {
            config,
            root,
            registry: ServerRegistry::default(),
            diagnostics: DiagnosticStore::default(),
            state: std::sync::Arc::new(Mutex::new(LspState {
                servers: HashMap::new(),
            })),
        }
    }

    pub fn diagnostics_store(&self) -> DiagnosticStore {
        self.diagnostics.clone()
    }

    pub fn prompt_diagnostics_summary(&self) -> Option<String> {
        let filter = match self.config.diagnostics_in_prompt {
            LspDiagnosticsInPrompt::Off => return None,
            LspDiagnosticsInPrompt::Errors => SeverityFilter::Errors,
            LspDiagnosticsInPrompt::ErrorsAndWarnings => SeverityFilter::ErrorsAndWarnings,
        };
        let summary = self.diagnostics.summarize(
            filter,
            self.config.max_files,
            self.config.max_diagnostics_per_file,
        );
        if summary.lines.is_empty() {
            None
        } else {
            Some(summary.render())
        }
    }

    pub async fn status(&self) -> Result<LspManagerStatus> {
        let mut entries = Vec::new();
        let state = self.state.lock().await;
        for spec in self.registry.specs() {
            let server_config = self.config.server_config(spec.id);
            let enabled = server_config.enabled;
            let detected = find_root_with_markers(&self.root, &spec.markers).is_some();
            let mut running_root = None;
            let mut running_command = None;
            for (key, handle) in &state.servers {
                if key.id == spec.id {
                    running_root = Some(handle.root.clone());
                    running_command = Some(handle.command.clone());
                    break;
                }
            }
            let running = running_root.is_some();
            let installed = self.is_installed(spec, &server_config);
            let command = running_command.unwrap_or_else(|| {
                self.resolve_command(spec, &server_config)
                    .unwrap_or_default()
            });
            entries.push(LspManagerStatusEntry {
                id: spec.id,
                enabled,
                detected,
                running,
                installed,
                root: running_root.or_else(|| find_root_with_markers(&self.root, &spec.markers)),
                command,
            });
        }
        Ok(LspManagerStatus { entries })
    }

    pub async fn install(&self, id: Option<LanguageServerId>) -> Result<Vec<LanguageServerId>> {
        let mut installed = Vec::new();
        let specs: Vec<ServerSpec> = match id {
            Some(id) => self.registry.spec(id).cloned().into_iter().collect(),
            None => self.registry.specs().to_vec(),
        };

        for spec in specs {
            self.install_server(&spec).await?;
            installed.push(spec.id);
        }

        Ok(installed)
    }

    pub async fn on_files_changed(&self, paths: Vec<PathBuf>) -> Result<(), LspError> {
        for path in paths {
            if self.path_missing(&path).await? {
                self.handle_missing_path(&path).await?;
                continue;
            }
            let Some(server_key) = self.ensure_server_for_path(&path).await? else {
                continue;
            };
            match self.open_or_change(&path, &server_key).await {
                Ok(_) => {}
                Err(LspError::Io(err)) if err.kind() == ErrorKind::NotFound => {
                    self.handle_missing_path(&path).await?;
                }
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    async fn path_missing(&self, path: &Path) -> Result<bool, LspError> {
        match tokio::fs::metadata(path).await {
            Ok(_) => Ok(false),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(true),
            Err(err) => Err(err.into()),
        }
    }

    async fn handle_missing_path(&self, path: &Path) -> Result<(), LspError> {
        self.diagnostics.clear(path);
        let Some(uri) = uri_from_file_path(path) else {
            return Ok(());
        };
        let clients = {
            let mut state = self.state.lock().await;
            let mut clients = Vec::new();
            for handle in state.servers.values_mut() {
                if handle.open_docs.remove(&uri).is_some() {
                    clients.push(handle.client.clone());
                }
            }
            clients
        };
        for client in clients {
            client
                .notify_did_close(uri.clone())
                .await
                .map_err(|err| LspError::ServerError(err.to_string()))?;
        }
        Ok(())
    }

    pub async fn diagnostics_for(
        &self,
        path: Option<PathBuf>,
        filter: SeverityFilter,
        wait: Option<std::time::Duration>,
    ) -> Result<Vec<DiagnosticEntry>, LspError> {
        if let Some(path) = path {
            let Some(server_key) = self.ensure_server_for_path(&path).await? else {
                return Err(LspError::NotDetected(path.display().to_string()));
            };
            self.open_or_change(&path, &server_key).await?;
            if let Some(wait) = wait {
                let _ = self.diagnostics.wait_for_path(&path, wait).await;
            }
            let diagnostics = self.diagnostics.diagnostics_for(&path).unwrap_or_default();
            let entries = diagnostics
                .into_iter()
                .filter(|d| filter.matches(d))
                .map(|diagnostic| DiagnosticEntry {
                    path: path.clone(),
                    diagnostic,
                })
                .collect();
            Ok(entries)
        } else {
            let entries = self
                .diagnostics
                .all_diagnostics()
                .into_iter()
                .filter(|entry| filter.matches(&entry.diagnostic))
                .collect();
            Ok(entries)
        }
    }

    pub async fn definition(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LocationInfo>, LspError> {
        let (client, uri, position) = self.position_for_path(path, line, character).await?;
        let value = client
            .request_definition(TextDocumentIdentifier { uri: uri.clone() }, position)
            .await
            .map_err(|err| LspError::ServerError(err.message))?;
        parse_locations(value)
    }

    pub async fn references(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Vec<LocationInfo>, LspError> {
        let (client, uri, position) = self.position_for_path(path, line, character).await?;
        let value = client
            .request_references(
                TextDocumentIdentifier { uri: uri.clone() },
                position,
                include_declaration,
            )
            .await
            .map_err(|err| LspError::ServerError(err.message))?;
        parse_locations(value)
    }

    pub async fn rename(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<WorkspaceEditResult, LspError> {
        let (client, uri, position) = self.position_for_path(path, line, character).await?;
        let value = client
            .request_rename(
                TextDocumentIdentifier { uri: uri.clone() },
                position,
                new_name,
            )
            .await
            .map_err(|err| LspError::ServerError(err.message))?;
        let edit: Option<WorkspaceEdit> = serde_json::from_value(value)
            .context("deserialize workspace edit")
            .map_err(|err| LspError::ServerError(err.to_string()))?;
        let Some(edit) = edit else {
            return Err(LspError::ServerError(
                "rename returned no edits".to_string(),
            ));
        };
        let encoding = client.position_encoding();
        let result = workspace_edit_to_apply_patch(edit, &self.root, encoding).await?;
        Ok(result)
    }

    async fn ensure_server_for_path(&self, path: &Path) -> Result<Option<ServerKey>, LspError> {
        match self.config.mode {
            LspMode::Off => return Err(LspError::Disabled),
            LspMode::Auto => {}
            LspMode::On => {}
        }

        let detected = if matches!(self.config.mode, LspMode::Auto) {
            detect_servers_for_file(&self.registry, path)
                .into_iter()
                .next()
        } else {
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(str::to_ascii_lowercase);
            let Some(extension) = extension else {
                return Ok(None);
            };
            self.registry
                .specs()
                .iter()
                .filter(|spec| {
                    spec.extensions
                        .iter()
                        .any(|ext| ext.eq_ignore_ascii_case(&extension))
                })
                .map(|spec| {
                    let root = find_root_with_markers(path, &spec.markers)
                        .unwrap_or_else(|| self.root.clone());
                    crate::detect::DetectedServer { spec, root }
                })
                .next()
        };

        let Some(detected) = detected else {
            return Ok(None);
        };

        let server_config = self.config.server_config(detected.spec.id);
        if !server_config.enabled {
            return Err(LspError::ServerDisabled(detected.spec.id));
        }

        self.ensure_server_running(detected.spec, detected.root, server_config)
            .await
    }

    async fn ensure_server_running(
        &self,
        spec: &ServerSpec,
        root: PathBuf,
        server_config: crate::config::LspServerConfig,
    ) -> Result<Option<ServerKey>, LspError> {
        let key = ServerKey {
            id: spec.id,
            root: root.clone(),
        };
        {
            let state = self.state.lock().await;
            if state.servers.contains_key(&key) {
                return Ok(Some(key));
            }
        }

        if !self.is_installed(spec, &server_config) && self.config.auto_install {
            self.install_server(spec)
                .await
                .map_err(|err| LspError::ServerError(err.to_string()))?;
        }
        if !self.is_installed(spec, &server_config) {
            return Err(LspError::NotInstalled(spec.id));
        }
        let command = self.resolve_command(spec, &server_config)?;
        let (command_bin, args) = command
            .split_first()
            .ok_or_else(|| LspError::ServerError("invalid command".to_string()))?;
        let client = LspClient::start(
            command_bin,
            &args.iter().map(ToString::to_string).collect::<Vec<_>>(),
            None,
            Some(&root),
            uri_from_directory_path(&root)
                .ok_or_else(|| LspError::ServerError("invalid root uri".to_string()))?,
            self.diagnostics.clone(),
        )
        .await
        .map_err(|err| LspError::ServerError(err.to_string()))?;

        let mut inserted = false;
        {
            let mut state = self.state.lock().await;
            if !state.servers.contains_key(&key) {
                state.servers.insert(
                    key.clone(),
                    ServerHandle {
                        spec: spec.clone(),
                        root,
                        client: client.clone(),
                        open_docs: HashMap::new(),
                        command,
                    },
                );
                inserted = true;
            }
        }
        if !inserted {
            let _ = client.shutdown().await;
        }
        Ok(Some(key))
    }

    async fn open_or_change(
        &self,
        path: &Path,
        server_key: &ServerKey,
    ) -> Result<(LspClient, Uri), LspError> {
        let uri = uri_from_file_path(path)
            .ok_or_else(|| LspError::ServerError("invalid file uri".to_string()))?;
        let text = tokio::fs::read_to_string(path).await?;
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let (client, language_id, version, is_change) = {
            let mut state = self.state.lock().await;
            let handle = state
                .servers
                .get_mut(server_key)
                .ok_or_else(|| LspError::ServerError("server missing".to_string()))?;
            let language_id = handle
                .spec
                .language_id_for_extension(&extension)
                .to_string();
            let (version, is_change) = if let Some(open) = handle.open_docs.get_mut(&uri) {
                open.version += 1;
                (open.version, true)
            } else {
                handle
                    .open_docs
                    .insert(uri.clone(), OpenDocument { version: 1 });
                (1, false)
            };
            (handle.client.clone(), language_id, version, is_change)
        };

        if is_change {
            client
                .notify_did_change(uri.clone(), version, text)
                .await
                .map_err(|err| LspError::ServerError(err.to_string()))?;
        } else {
            client
                .notify_did_open(uri.clone(), &language_id, version, text)
                .await
                .map_err(|err| LspError::ServerError(err.to_string()))?;
        }
        Ok((client, uri))
    }

    fn resolve_command(
        &self,
        spec: &ServerSpec,
        config: &crate::config::LspServerConfig,
    ) -> Result<Vec<String>, LspError> {
        if let Some(command) = &config.command {
            let parts = shlex::split(command)
                .ok_or_else(|| LspError::ServerError("invalid command override".to_string()))?;
            return Ok(parts);
        }

        let mut command = Vec::new();
        if let Some(installed) = self.installed_path(spec) {
            command.push(installed.to_string_lossy().to_string());
        } else {
            command.push(spec.bin_name.to_string());
        }
        command.extend(spec.default_args.iter().map(ToString::to_string));
        Ok(command)
    }

    fn installed_path(&self, spec: &ServerSpec) -> Option<PathBuf> {
        let base = match spec.install {
            InstallStrategy::Npm { .. } => self
                .config
                .install_dir
                .join("node_modules")
                .join(".bin")
                .join(spec.bin_name),
            InstallStrategy::GoInstall { .. } | InstallStrategy::RustupComponent { .. } => {
                self.config.install_dir.join("bin").join(spec.bin_name)
            }
            InstallStrategy::SystemOnly => return None,
        };
        installed_candidate(&base)
    }

    fn is_installed(&self, spec: &ServerSpec, config: &crate::config::LspServerConfig) -> bool {
        if config.command.is_some() {
            return true;
        }
        if self.installed_path(spec).is_some() {
            return true;
        }
        which::which(spec.bin_name).is_ok()
    }

    async fn install_server(&self, spec: &ServerSpec) -> Result<()> {
        match spec.install {
            InstallStrategy::SystemOnly => {
                let id = spec.id;
                anyhow::bail!("{id} is system-only and cannot be auto-installed")
            }
            InstallStrategy::Npm { package } => {
                tokio::fs::create_dir_all(&self.config.install_dir).await?;
                let mut cmd = tokio::process::Command::new("npm");
                cmd.arg("install")
                    .arg("--prefix")
                    .arg(&self.config.install_dir)
                    .arg(package);
                let status = cmd.status().await?;
                if !status.success() {
                    anyhow::bail!("npm install failed for {package}");
                }
            }
            InstallStrategy::GoInstall { module } => {
                let bin_dir = self.config.install_dir.join("bin");
                tokio::fs::create_dir_all(&bin_dir).await?;
                let mut cmd = tokio::process::Command::new("go");
                cmd.arg("install").arg(module);
                cmd.env("GOBIN", &bin_dir);
                let status = cmd.status().await?;
                if !status.success() {
                    anyhow::bail!("go install failed for {module}");
                }
            }
            InstallStrategy::RustupComponent { component } => {
                tokio::fs::create_dir_all(self.config.install_dir.join("bin")).await?;
                let status = tokio::process::Command::new("rustup")
                    .arg("component")
                    .arg("add")
                    .arg(component)
                    .status()
                    .await?;
                if !status.success() {
                    anyhow::bail!("rustup component add failed for {component}");
                }
                let output = tokio::process::Command::new("rustup")
                    .arg("which")
                    .arg(spec.bin_name)
                    .output()
                    .await?;
                if !output.status.success() {
                    anyhow::bail!("rustup which failed for {}", spec.bin_name);
                }
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let source = PathBuf::from(path);
                let dest = if let Some(ext) = source.extension() {
                    self.config
                        .install_dir
                        .join("bin")
                        .join(spec.bin_name)
                        .with_extension(ext)
                } else {
                    self.config.install_dir.join("bin").join(spec.bin_name)
                };
                tokio::fs::copy(&source, &dest).await?;
            }
        }
        Ok(())
    }

    async fn position_for_path(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<(LspClient, Uri, Position), LspError> {
        let server_key = self
            .ensure_server_for_path(path)
            .await?
            .ok_or_else(|| LspError::NotDetected(path.display().to_string()))?;
        let (client, uri) = self.open_or_change(path, &server_key).await?;
        let text = tokio::fs::read_to_string(path).await?;
        let offset = byte_offset_for_line_character(&text, line, character).ok_or_else(|| {
            LspError::InvalidPosition {
                path: path.display().to_string(),
                line,
                character,
            }
        })?;
        let position =
            position_for_offset(&text, offset, client.position_encoding()).ok_or_else(|| {
                LspError::InvalidPosition {
                    path: path.display().to_string(),
                    line,
                    character,
                }
            })?;
        Ok((client, uri, position))
    }
}

fn installed_candidate(base: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        let cmd = base.with_extension("cmd");
        if cmd.exists() {
            return Some(cmd);
        }
        let exe = base.with_extension("exe");
        if exe.exists() {
            return Some(exe);
        }
    }
    if base.exists() {
        Some(base.to_path_buf())
    } else {
        None
    }
}

fn byte_offset_for_line_character(text: &str, line: u32, character: u32) -> Option<usize> {
    if line == 0 || character == 0 {
        return None;
    }
    let target_line = (line - 1) as usize;
    let target_character = (character - 1) as usize;
    let mut offset = 0usize;

    for (idx, line_text) in text.split('\n').enumerate() {
        if idx == target_line {
            let mut count = 0usize;
            for (byte_idx, _) in line_text.char_indices() {
                if count == target_character {
                    return Some(offset + byte_idx);
                }
                count += 1;
            }
            if count == target_character {
                return Some(offset + line_text.len());
            }
            return None;
        }
        offset = offset.saturating_add(line_text.len() + 1);
    }
    None
}

#[derive(Debug, Clone)]
pub struct LocationInfo {
    pub path: PathBuf,
    pub line: u32,
    pub character: u32,
}

fn parse_locations(value: Value) -> Result<Vec<LocationInfo>, LspError> {
    let response: Option<lsp_types::GotoDefinitionResponse> = serde_json::from_value(value)
        .context("parse definition response")
        .map_err(|err| LspError::ServerError(err.to_string()))?;
    let Some(response) = response else {
        return Ok(Vec::new());
    };
    let mut locations = Vec::new();
    match response {
        lsp_types::GotoDefinitionResponse::Scalar(location) => {
            push_location(&mut locations, location);
        }
        lsp_types::GotoDefinitionResponse::Array(items) => {
            for location in items {
                push_location(&mut locations, location);
            }
        }
        lsp_types::GotoDefinitionResponse::Link(links) => {
            for link in links {
                let location = lsp_types::Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                };
                push_location(&mut locations, location);
            }
        }
    }
    Ok(locations)
}

fn push_location(locations: &mut Vec<LocationInfo>, location: lsp_types::Location) {
    if let Some(path) = uri_to_file_path(&location.uri) {
        locations.push(LocationInfo {
            path,
            line: location.range.start.line + 1,
            character: location.range.start.character + 1,
        });
    }
}
