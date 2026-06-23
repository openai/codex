use super::*;

pub(super) struct RequestScopedProxy {
    pub(super) environment_id: String,
    http_task: JoinHandle<Result<()>>,
    socks_task: Option<JoinHandle<Result<()>>>,
}

impl Drop for RequestScopedProxy {
    fn drop(&mut self) {
        self.http_task.abort();
        if let Some(socks_task) = self.socks_task.as_ref() {
            socks_task.abort();
        }
    }
}

impl NetworkProxy {
    /// Creates an execution-scoped proxy that lives until all returned clones are dropped.
    /// Windows, and macOS with local binding, fall back because loopback listeners are shared.
    pub fn scope_for_request(&self, environment_id: &str, request_origin: String) -> Result<Self> {
        #[cfg(target_os = "windows")]
        {
            // Windows' firewall is shared, so distinct endpoints cannot be isolated between execs.
            let _ = (environment_id, request_origin);
            Ok(self.clone())
        }

        #[cfg(target_os = "macos")]
        if self.allow_local_binding() {
            return Ok(self.clone());
        }

        #[cfg(not(target_os = "windows"))]
        {
            anyhow::ensure!(
                self.request_scope.is_none(),
                "cannot create a request scope from an already scoped network proxy"
            );

            let runtime = tokio::runtime::Handle::try_current()?;
            let listeners = reserve_loopback_ephemeral_listeners(self.socks_enabled)?;
            let http_addr = listeners.http_addr()?;
            let socks_addr = listeners.socks_addr(self.socks_addr)?;
            let ReservedListenerSet {
                http_listener,
                socks_listener,
            } = listeners;

            let state = Arc::new(self.state.with_request_origin(request_origin));
            let http_state = Arc::clone(&state);
            let http_decider = self.policy_decider.clone();
            let http_environment_id = Some(environment_id.to_string());
            let http_task = runtime.spawn(async move {
                http_proxy::run_http_proxy_with_std_listener(
                    http_state,
                    http_listener,
                    http_decider,
                    http_environment_id,
                )
                .await
            });

            let socks_task = if self.socks_enabled {
                let socks_state = Arc::clone(&state);
                let socks_decider = self.policy_decider.clone();
                let socks_environment_id = Some(environment_id.to_string());
                let socks5_udp_enabled = self.socks5_udp_enabled;
                socks_listener.map(|listener| {
                    runtime.spawn(async move {
                        socks5::run_socks5_with_std_listener(
                            socks_state,
                            listener,
                            socks_decider,
                            socks_environment_id,
                            socks5_udp_enabled,
                        )
                        .await
                    })
                })
            } else {
                None
            };

            Ok(Self {
                state,
                http_addr,
                socks_addr,
                socks_enabled: self.socks_enabled,
                socks5_udp_enabled: self.socks5_udp_enabled,
                runtime_settings: Arc::clone(&self.runtime_settings),
                reserved_listeners: None,
                policy_decider: self.policy_decider.clone(),
                environment_proxies: Arc::clone(&self.environment_proxies),
                request_scope: Some(Arc::new(RequestScopedProxy {
                    environment_id: environment_id.to_string(),
                    http_task,
                    socks_task,
                })),
            })
        }
    }
}

#[cfg(all(test, not(target_os = "windows")))]
#[path = "request_scope_tests.rs"]
mod tests;
