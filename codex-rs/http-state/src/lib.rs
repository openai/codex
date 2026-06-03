use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::RwLock;

use codex_utils_path::write_atomically;
use serde::Deserialize;
use serde::Serialize;

const STATE_DIR_NAME: &str = "state";

/// Native Codex surface that owns one local HTTP-state file.
///
/// Unknown app-server clients intentionally share the CLI state file. This
/// preserves the default classification while first-party clients opt into a
/// more specific surface by setting `clientInfo.name` during initialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpStateSurface {
    CodexCli,
    CodexTui,
    CodexExec,
    CodexVscode,
    CodexDesktop,
    CodexDesktopSsh,
    CodexRemoteControl,
}

impl HttpStateSurface {
    pub fn try_from_app_server_client_name(client_name: &str) -> Option<Self> {
        match client_name {
            "codex_cli" => Some(Self::CodexCli),
            "codex-tui" => Some(Self::CodexTui),
            "codex_exec" => Some(Self::CodexExec),
            "codex_vscode" => Some(Self::CodexVscode),
            "codex_desktop" => Some(Self::CodexDesktop),
            "codex_desktop_ssh" => Some(Self::CodexDesktopSsh),
            "codex_remote_control" => Some(Self::CodexRemoteControl),
            _ => None,
        }
    }

    pub fn from_app_server_client_name(client_name: &str) -> Self {
        Self::try_from_app_server_client_name(client_name).unwrap_or(Self::CodexCli)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CodexCli => "codex_cli",
            Self::CodexTui => "codex_tui",
            Self::CodexExec => "codex_exec",
            Self::CodexVscode => "codex_vscode",
            Self::CodexDesktop => "codex_desktop",
            Self::CodexDesktopSsh => "codex_desktop_ssh",
            Self::CodexRemoteControl => "codex_remote_control",
        }
    }
}

impl fmt::Display for HttpStateSurface {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Deserialize, Serialize)]
struct HttpStateFile {
    state: String,
}

#[derive(Clone, Debug)]
pub struct HttpStateStore {
    codex_home: PathBuf,
}

impl HttpStateStore {
    pub fn new(codex_home: PathBuf) -> Self {
        Self { codex_home }
    }

    fn state_path(&self, surface: HttpStateSurface) -> PathBuf {
        self.codex_home
            .join(STATE_DIR_NAME)
            .join(format!("{surface}.json"))
    }

    pub fn get(&self, surface: HttpStateSurface) -> io::Result<Option<String>> {
        let path = self.state_path(surface);
        let contents = match fs::read(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let state_file: HttpStateFile =
            serde_json::from_slice(&contents).map_err(io::Error::other)?;
        Ok(Some(state_file.state))
    }

    pub fn set(&self, surface: HttpStateSurface, state: String) -> io::Result<()> {
        self.write(surface, &HttpStateFile { state })
    }

    pub fn clear(&self, surface: HttpStateSurface) -> io::Result<()> {
        match fs::remove_file(self.state_path(surface)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Stores `next_state` only if the local file still contains `expected_state`.
    ///
    /// This is intentionally lock-free. Concurrent writers may both observe
    /// the same prior value, in which case atomic replacement keeps the file
    /// well-formed and the last writer wins.
    pub fn compare_and_set(
        &self,
        surface: HttpStateSurface,
        expected_state: &str,
        next_state: String,
    ) -> io::Result<bool> {
        let Some(current) = self.get(surface)? else {
            return Ok(false);
        };
        if current != expected_state {
            return Ok(false);
        }

        self.write(surface, &HttpStateFile { state: next_state })?;
        Ok(true)
    }

    fn write(&self, surface: HttpStateSurface, state_file: &HttpStateFile) -> io::Result<()> {
        let path = self.state_path(surface);
        let contents = serde_json::to_string_pretty(state_file).map_err(io::Error::other)?;
        write_atomically(&path, &contents)
    }
}

/// Shared surface selection for one native Codex client session.
///
/// App-server clients may update the selected surface after the model client is
/// constructed. Requests snapshot the current value when they create their
/// auth decorator, so clones of the model client stay in sync without moving
/// in-flight rotations to another surface.
#[derive(Clone, Debug)]
pub struct HttpStateContext {
    store: HttpStateStore,
    surface: Arc<RwLock<HttpStateSurface>>,
}

impl HttpStateContext {
    pub fn new(codex_home: PathBuf, surface: HttpStateSurface) -> Self {
        Self {
            store: HttpStateStore::new(codex_home),
            surface: Arc::new(RwLock::new(surface)),
        }
    }

    pub fn surface(&self) -> HttpStateSurface {
        *self.surface.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn set_surface(&self, surface: HttpStateSurface) -> bool {
        let mut selected_surface = self.surface.write().unwrap_or_else(PoisonError::into_inner);
        if *selected_surface == surface {
            return false;
        }
        *selected_surface = surface;
        true
    }

    pub fn get_for_surface(&self, surface: HttpStateSurface) -> io::Result<Option<String>> {
        self.store.get(surface)
    }

    pub fn set_for_surface(&self, surface: HttpStateSurface, state: String) -> io::Result<()> {
        self.store.set(surface, state)
    }

    pub fn compare_and_set_for_surface(
        &self,
        surface: HttpStateSurface,
        expected_state: &str,
        next_state: String,
    ) -> io::Result<bool> {
        self.store
            .compare_and_set(surface, expected_state, next_state)
    }
}

#[cfg(test)]
mod tests;
