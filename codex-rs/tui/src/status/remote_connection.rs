use crate::AppServerTarget;
use crate::RemoteAppServerEndpoint;
use crate::update_versions::ServerVersionNoticeKind;
use sha2::Digest;
use sha2::Sha256;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RemoteConnectionStatus {
    pub(crate) address: String,
    pub(crate) version: String,
    pub(crate) is_local_daemon: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ServerVersionNotice {
    pub(crate) message: String,
    pub(crate) offer_update: bool,
}

pub(crate) fn remote_connection_status_value(
    app_server_target: &AppServerTarget,
    server_version: Option<&str>,
) -> Option<RemoteConnectionStatus> {
    let endpoint = match app_server_target {
        AppServerTarget::Embedded => return None,
        AppServerTarget::LocalDaemon { endpoint, .. } | AppServerTarget::Remote { endpoint } => {
            endpoint
        }
    };
    let address = match endpoint {
        RemoteAppServerEndpoint::WebSocket { websocket_url, .. } => {
            sanitized_websocket_url(websocket_url)
                .map(|url| url.to_string())
                .unwrap_or_else(|| "<invalid websocket URL>".to_string())
        }
        RemoteAppServerEndpoint::UnixSocket { socket_path } => {
            format!("unix://{}", socket_path.display())
        }
    };
    let version = server_version
        .map(|version| format!("v{version}"))
        .unwrap_or_else(|| "unknown".to_string());
    Some(RemoteConnectionStatus {
        address,
        version,
        is_local_daemon: matches!(app_server_target, AppServerTarget::LocalDaemon { .. }),
    })
}

pub(crate) fn server_version_notice(client: &str, server: Option<&str>) -> Option<String> {
    let server = server?;
    let comparison = match crate::update_versions::server_version_notice_kind(client, server)? {
        ServerVersionNoticeKind::Older => "older than",
        ServerVersionNoticeKind::Different => "different from",
    };
    Some(format!(
        "A background Codex service is running v{server}, {comparison} your Codex CLI v{client}."
    ))
}

pub(crate) fn server_version_notice_for_tui(
    settings: &codex_config::types::Tui,
    client: &str,
    server: Option<&str>,
) -> Option<String> {
    if settings.show_server_version_notice {
        server_version_notice(client, server)
    } else {
        None
    }
}

fn hash_identity_part(hasher: &mut Sha256, tag: &[u8], value: &[u8]) {
    hasher.update(tag);
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

pub(crate) fn server_version_notice_key(
    target: &AppServerTarget,
    server_home: Option<&str>,
    client: &str,
    server: Option<&str>,
) -> Option<String> {
    let server = server?;
    server_version_notice(client, Some(server))?;
    let mut hasher = Sha256::new();
    let endpoint = match target {
        AppServerTarget::Embedded => None,
        AppServerTarget::LocalDaemon { endpoint, .. } | AppServerTarget::Remote { endpoint } => {
            Some(endpoint)
        }
    };
    if let Some(server_home) = server_home {
        hash_identity_part(&mut hasher, b"server-home:", server_home.as_bytes());
    }
    match endpoint {
        Some(RemoteAppServerEndpoint::UnixSocket { socket_path }) => {
            hash_identity_part(
                &mut hasher,
                b"unix:",
                socket_path.as_path().as_os_str().as_encoded_bytes(),
            );
        }
        Some(RemoteAppServerEndpoint::WebSocket { websocket_url, .. }) => {
            if let Ok(mut url) = Url::parse(websocket_url) {
                let routing = url
                    .query_pairs()
                    .filter(|(name, _)| {
                        matches!(
                            name.to_ascii_lowercase().as_str(),
                            "workspace"
                                | "project"
                                | "tenant"
                                | "environment"
                                | "namespace"
                                | "organization"
                                | "org"
                                | "account"
                                | "route"
                                | "instance"
                                | "region"
                                | "deployment"
                                | "cluster"
                        )
                    })
                    .map(|(name, value)| (name.into_owned(), value.into_owned()))
                    .collect::<Vec<_>>();
                let _ = url.set_username("");
                let _ = url.set_password(None);
                url.set_query(None);
                url.set_fragment(None);
                if !routing.is_empty() {
                    url.query_pairs_mut().extend_pairs(routing);
                }
                hash_identity_part(&mut hasher, b"websocket:", url.as_str().as_bytes());
            }
        }
        None => hash_identity_part(&mut hasher, b"embedded:", b""),
    }
    Some(format!("{:x}:{client}-{server}", hasher.finalize()))
}

pub(crate) fn pending_server_version_notice(
    settings: &codex_config::types::Tui,
    target: &AppServerTarget,
    server_home: Option<&str>,
    client: &str,
    server: Option<&str>,
    last_shown: Option<&str>,
) -> Option<(ServerVersionNotice, String)> {
    let message = server_version_notice_for_tui(settings, client, server)?;
    let notice = ServerVersionNotice {
        message,
        offer_update: matches!(target, AppServerTarget::LocalDaemon { .. }),
    };
    let key = server_version_notice_key(target, server_home, client, server)?;
    (last_shown != Some(key.as_str())).then_some((notice, key))
}

pub(crate) fn sanitized_websocket_url(raw: &str) -> Option<Url> {
    let mut url = Url::parse(raw).ok()?;
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    #[test]
    fn remote_connection_status_value_formats_display_value() -> color_eyre::Result<()> {
        assert_eq!(
            remote_connection_status_value(&AppServerTarget::Embedded, Some("1.2.3")),
            None
        );

        let websocket_target = AppServerTarget::Remote {
            endpoint: RemoteAppServerEndpoint::WebSocket {
                websocket_url: "ws://user:secret@127.0.0.1:4500/?token=abc#frag".to_string(),
                auth_token: Some("abc".to_string()),
            },
        };
        assert_eq!(
            remote_connection_status_value(&websocket_target, Some("1.2.3")),
            Some(RemoteConnectionStatus {
                address: "ws://127.0.0.1:4500/".to_string(),
                version: "v1.2.3".to_string(),
                is_local_daemon: false,
            })
        );

        let socket_path = AbsolutePathBuf::relative_to_current_dir("codex.sock")?;
        let daemon_target = AppServerTarget::LocalDaemon {
            allow_embedded_fallback: true,
            endpoint: RemoteAppServerEndpoint::UnixSocket {
                socket_path: socket_path.clone(),
            },
        };
        assert_eq!(
            remote_connection_status_value(&daemon_target, /*server_version*/ None),
            Some(RemoteConnectionStatus {
                address: format!("unix://{}", socket_path.display()),
                version: "unknown".to_string(),
                is_local_daemon: true,
            })
        );
        let remote_socket_target = AppServerTarget::Remote {
            endpoint: RemoteAppServerEndpoint::UnixSocket {
                socket_path: socket_path.clone(),
            },
        };
        assert_eq!(
            remote_connection_status_value(&remote_socket_target, Some("1.2.3")),
            Some(RemoteConnectionStatus {
                address: format!("unix://{}", socket_path.display()),
                version: "v1.2.3".to_string(),
                is_local_daemon: false,
            })
        );
        Ok(())
    }

    #[test]
    fn server_version_notice_uses_client_release_policy() {
        assert_eq!(
            server_version_notice("0.153.0", Some("0.152.1")),
            Some("A background Codex service is running v0.152.1, older than your Codex CLI v0.153.0.".to_string())
        );
        assert_eq!(server_version_notice("0.153.0", Some("0.153.0")), None);
        assert_eq!(
            server_version_notice("0.0.0", Some("0.152.1")),
            Some("A background Codex service is running v0.152.1, different from your Codex CLI v0.0.0.".to_string())
        );
        assert_eq!(server_version_notice("0.153.0", /*server*/ None), None);
        assert_eq!(
            server_version_notice("0.153.0-alpha.10", Some("0.153.0-alpha.9")),
            Some("A background Codex service is running v0.153.0-alpha.9, older than your Codex CLI v0.153.0-alpha.10.".to_string())
        );
        for server in ["0.153.0-alpha.10", "0.153.0-alpha.11", "0.153.0"] {
            assert_eq!(
                server_version_notice("0.153.0-alpha.10", Some(server)),
                None
            );
        }
        assert_eq!(
            server_version_notice("0.155.0-alpha.12", Some("0.156.0")),
            Some("A background Codex service is running v0.156.0, different from your Codex CLI v0.155.0-alpha.12.".to_string())
        );
    }

    #[test]
    fn update_command_is_only_suggested_for_implicit_local_daemon() -> color_eyre::Result<()> {
        let endpoint = RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        };
        let local = AppServerTarget::LocalDaemon {
            allow_embedded_fallback: true,
            endpoint: endpoint.clone(),
        };
        let remote = AppServerTarget::Remote { endpoint };
        let settings = codex_config::types::Tui {
            show_server_version_notice: true,
            ..Default::default()
        };
        let notice = |target| {
            pending_server_version_notice(
                &settings,
                target,
                /*server_home*/ None,
                "0.153.0-alpha.10.1",
                Some("0.153.0-alpha.9.2"),
                /*last_shown*/ None,
            )
            .map(|(notice, _)| notice)
        };
        let remote_notice =
            server_version_notice("0.153.0-alpha.10.1", Some("0.153.0-alpha.9.2")).unwrap();
        assert_eq!(
            notice(&remote),
            Some(ServerVersionNotice {
                message: remote_notice.clone(),
                offer_update: false,
            })
        );
        assert_eq!(
            notice(&local),
            Some(ServerVersionNotice {
                message: remote_notice,
                offer_update: true,
            })
        );
        Ok(())
    }

    #[test]
    fn notice_key_identifies_socket_and_server_home_across_connection_modes() -> anyhow::Result<()>
    {
        let socket = |path| -> anyhow::Result<RemoteAppServerEndpoint> {
            Ok(RemoteAppServerEndpoint::UnixSocket {
                socket_path: AbsolutePathBuf::from_absolute_path(path)?,
            })
        };
        let local = AppServerTarget::LocalDaemon {
            allow_embedded_fallback: true,
            endpoint: socket("/c")?,
        };
        let remote = AppServerTarget::Remote {
            endpoint: socket("/c")?,
        };
        let key = |target: &AppServerTarget, server_home| {
            server_version_notice_key(target, server_home, "0.153.0", Some("0.152.0"))
        };
        assert_eq!(key(&local, Some("/a")), key(&remote, Some("/a")));
        let other = AppServerTarget::Remote {
            endpoint: socket("/bunix:/c")?,
        };
        assert_ne!(key(&remote, Some("/aunix:/b")), key(&other, Some("/a")));
        Ok(())
    }
}
