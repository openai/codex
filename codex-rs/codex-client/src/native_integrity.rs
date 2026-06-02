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

pub const INTEGRITY_STATE_HEADER_NAME: &str = "X-OAI-IS";
pub const INTEGRITY_STATE_UPDATE_HEADER_NAME: &str = "X-OAI-IS-Update";
const MAX_INTEGRITY_STATE_ENVELOPE_BYTES: usize = 2048;

const STATE_DIR_NAME: &str = "state";

/// Native Codex surface that owns one local integrity-state file.
///
/// Unknown app-server clients intentionally share the CLI state file. This
/// preserves the default classification while first-party clients opt into a
/// more specific surface by setting `clientInfo.name` during initialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeIntegritySurface {
    CodexCli,
    CodexTui,
    CodexExec,
    CodexVscode,
    CodexDesktop,
    CodexDesktopSsh,
    CodexRemoteControl,
}

impl NativeIntegritySurface {
    pub fn from_app_server_client_name(client_name: &str) -> Self {
        match client_name {
            "codex-tui" => Self::CodexTui,
            "codex_exec" => Self::CodexExec,
            "codex_vscode" => Self::CodexVscode,
            "codex_desktop" => Self::CodexDesktop,
            "codex_desktop_ssh" => Self::CodexDesktopSsh,
            "codex_remote_control" => Self::CodexRemoteControl,
            _ => Self::CodexCli,
        }
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

impl fmt::Display for NativeIntegritySurface {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeIntegrityStateFile {
    pub state: String,
}

#[derive(Clone, Debug)]
pub struct NativeIntegrityStateStore {
    codex_home: PathBuf,
}

impl NativeIntegrityStateStore {
    pub fn new(codex_home: PathBuf) -> Self {
        Self { codex_home }
    }

    fn state_path(&self, surface: NativeIntegritySurface) -> PathBuf {
        self.codex_home
            .join(STATE_DIR_NAME)
            .join(format!("{surface}.json"))
    }

    pub fn load(
        &self,
        surface: NativeIntegritySurface,
    ) -> io::Result<Option<NativeIntegrityStateFile>> {
        let path = self.state_path(surface);
        let contents = match fs::read(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let state_file: NativeIntegrityStateFile =
            serde_json::from_slice(&contents).map_err(io::Error::other)?;
        validate_integrity_state_envelope(&state_file.state)?;
        Ok(Some(state_file))
    }

    pub fn replace(&self, surface: NativeIntegritySurface, state: String) -> io::Result<()> {
        validate_integrity_state_envelope(&state)?;
        self.write(surface, &NativeIntegrityStateFile { state })
    }

    pub fn clear(&self, surface: NativeIntegritySurface) -> io::Result<()> {
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
    pub fn compare_and_store(
        &self,
        surface: NativeIntegritySurface,
        expected_state: &str,
        next_state: String,
    ) -> io::Result<bool> {
        validate_integrity_state_envelope(&next_state)?;
        let Some(current) = self.load(surface)? else {
            return Ok(false);
        };
        if current.state != expected_state {
            return Ok(false);
        }

        self.write(surface, &NativeIntegrityStateFile { state: next_state })?;
        Ok(true)
    }

    fn write(
        &self,
        surface: NativeIntegritySurface,
        state_file: &NativeIntegrityStateFile,
    ) -> io::Result<()> {
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
pub struct NativeIntegrityStateContext {
    store: NativeIntegrityStateStore,
    surface: Arc<RwLock<NativeIntegritySurface>>,
}

impl NativeIntegrityStateContext {
    pub fn new(codex_home: PathBuf, surface: NativeIntegritySurface) -> Self {
        Self {
            store: NativeIntegrityStateStore::new(codex_home),
            surface: Arc::new(RwLock::new(surface)),
        }
    }

    pub fn surface(&self) -> NativeIntegritySurface {
        *self.surface.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn set_surface(&self, surface: NativeIntegritySurface) -> bool {
        let mut selected_surface = self.surface.write().unwrap_or_else(PoisonError::into_inner);
        if *selected_surface == surface {
            return false;
        }
        *selected_surface = surface;
        true
    }

    pub fn load_for_surface(
        &self,
        surface: NativeIntegritySurface,
    ) -> io::Result<Option<NativeIntegrityStateFile>> {
        self.store.load(surface)
    }

    pub fn compare_and_store_for_surface(
        &self,
        surface: NativeIntegritySurface,
        expected_state: &str,
        next_state: String,
    ) -> io::Result<bool> {
        self.store
            .compare_and_store(surface, expected_state, next_state)
    }
}

fn is_valid_integrity_state_envelope(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_INTEGRITY_STATE_ENVELOPE_BYTES || value.trim() != value
    {
        return false;
    }

    let mut parts = value.split('.');
    if parts.next() != Some("ois1") {
        return false;
    }

    let valid_part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    };

    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(header), Some(nonce), Some(ciphertext), None)
            if valid_part(header) && valid_part(nonce) && valid_part(ciphertext)
    )
}

fn validate_integrity_state_envelope(value: &str) -> io::Result<()> {
    if is_valid_integrity_state_envelope(value) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "integrity state envelope is malformed",
        ))
    }
}

#[cfg(test)]
#[path = "native_integrity_tests.rs"]
mod tests;
