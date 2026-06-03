use std::io;
use std::path::PathBuf;

use codex_app_server_protocol::HttpStateClearResponse;
use codex_app_server_protocol::HttpStateGetResponse;
use codex_app_server_protocol::HttpStateSetParams;
use codex_app_server_protocol::HttpStateSetResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_http_state::HttpStateStore;
use codex_http_state::HttpStateSurface;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;

#[derive(Clone)]
pub(crate) struct HttpStateRequestProcessor {
    store: HttpStateStore,
}

impl HttpStateRequestProcessor {
    pub(crate) fn new(codex_home: PathBuf) -> Self {
        Self {
            store: HttpStateStore::new(codex_home),
        }
    }

    pub(crate) fn get(
        &self,
        app_server_client_name: Option<&str>,
    ) -> Result<HttpStateGetResponse, JSONRPCErrorError> {
        let surface = surface_for_client_name(app_server_client_name)?;
        let state = self.store.get(surface).map_err(map_get_error)?;
        Ok(HttpStateGetResponse { state })
    }

    pub(crate) fn set(
        &self,
        app_server_client_name: Option<&str>,
        params: HttpStateSetParams,
    ) -> Result<HttpStateSetResponse, JSONRPCErrorError> {
        let surface = surface_for_client_name(app_server_client_name)?;
        let written = match params.expected_state {
            Some(expected_state) => {
                self.store
                    .compare_and_set(surface, &expected_state, params.state)
            }
            None => self.store.set(surface, params.state).map(|()| true),
        }
        .map_err(map_set_error)?;
        Ok(HttpStateSetResponse { written })
    }

    pub(crate) fn clear(
        &self,
        app_server_client_name: Option<&str>,
    ) -> Result<HttpStateClearResponse, JSONRPCErrorError> {
        let surface = surface_for_client_name(app_server_client_name)?;
        self.store.clear(surface).map_err(map_clear_error)?;
        Ok(HttpStateClearResponse {})
    }
}

fn surface_for_client_name(
    app_server_client_name: Option<&str>,
) -> Result<HttpStateSurface, JSONRPCErrorError> {
    let client_name = app_server_client_name
        .ok_or_else(|| invalid_request("HTTP state requires an initialized client"))?;
    HttpStateSurface::try_from_app_server_client_name(client_name).ok_or_else(|| {
        invalid_request(format!(
            "HTTP state is unavailable for app-server client {client_name:?}"
        ))
    })
}

fn map_get_error(error: io::Error) -> JSONRPCErrorError {
    internal_error(format!("failed to read HTTP state: {error}"))
}

fn map_set_error(error: io::Error) -> JSONRPCErrorError {
    internal_error(format!("failed to write HTTP state: {error}"))
}

fn map_clear_error(error: io::Error) -> JSONRPCErrorError {
    internal_error(format!("failed to clear HTTP state: {error}"))
}
