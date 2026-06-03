use std::io;
use std::path::PathBuf;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::NativeIntegrityStateClearResponse;
use codex_app_server_protocol::NativeIntegrityStateReadResponse;
use codex_app_server_protocol::NativeIntegrityStateWriteParams;
use codex_app_server_protocol::NativeIntegrityStateWriteResponse;
use codex_client::NativeIntegrityStateStore;
use codex_client::NativeIntegritySurface;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;

#[derive(Clone)]
pub(crate) struct NativeIntegrityStateRequestProcessor {
    store: NativeIntegrityStateStore,
}

impl NativeIntegrityStateRequestProcessor {
    pub(crate) fn new(codex_home: PathBuf) -> Self {
        Self {
            store: NativeIntegrityStateStore::new(codex_home),
        }
    }

    pub(crate) fn read(
        &self,
        app_server_client_name: Option<&str>,
    ) -> Result<NativeIntegrityStateReadResponse, JSONRPCErrorError> {
        let surface = surface_for_client_name(app_server_client_name)?;
        let state = self
            .store
            .load(surface)
            .map_err(map_read_error)?
            .map(|state_file| state_file.state);
        Ok(NativeIntegrityStateReadResponse { state })
    }

    pub(crate) fn write(
        &self,
        app_server_client_name: Option<&str>,
        params: NativeIntegrityStateWriteParams,
    ) -> Result<NativeIntegrityStateWriteResponse, JSONRPCErrorError> {
        let surface = surface_for_client_name(app_server_client_name)?;
        let written = match params.expected_state {
            Some(expected_state) => {
                self.store
                    .compare_and_store(surface, &expected_state, params.state)
            }
            None => self.store.replace(surface, params.state).map(|()| true),
        }
        .map_err(map_write_error)?;
        Ok(NativeIntegrityStateWriteResponse { written })
    }

    pub(crate) fn clear(
        &self,
        app_server_client_name: Option<&str>,
    ) -> Result<NativeIntegrityStateClearResponse, JSONRPCErrorError> {
        let surface = surface_for_client_name(app_server_client_name)?;
        self.store.clear(surface).map_err(map_clear_error)?;
        Ok(NativeIntegrityStateClearResponse {})
    }
}

fn surface_for_client_name(
    app_server_client_name: Option<&str>,
) -> Result<NativeIntegritySurface, JSONRPCErrorError> {
    let client_name = app_server_client_name
        .ok_or_else(|| invalid_request("native integrity state requires an initialized client"))?;
    NativeIntegritySurface::try_from_app_server_client_name(client_name).ok_or_else(|| {
        invalid_request(format!(
            "native integrity state is unavailable for app-server client {client_name:?}"
        ))
    })
}

fn map_read_error(error: io::Error) -> JSONRPCErrorError {
    internal_error(format!("failed to read native integrity state: {error}"))
}

fn map_write_error(error: io::Error) -> JSONRPCErrorError {
    if error.kind() == io::ErrorKind::InvalidData {
        invalid_request(error.to_string())
    } else {
        internal_error(format!("failed to write native integrity state: {error}"))
    }
}

fn map_clear_error(error: io::Error) -> JSONRPCErrorError {
    internal_error(format!("failed to clear native integrity state: {error}"))
}
