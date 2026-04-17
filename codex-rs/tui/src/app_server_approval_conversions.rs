use codex_app_server_protocol::AdditionalFileSystemPermissions;
use codex_app_server_protocol::AdditionalNetworkPermissions;
use codex_app_server_protocol::GrantedPermissionProfile;
use codex_app_server_protocol::NetworkApprovalContext as AppServerNetworkApprovalContext;
use codex_protocol::protocol::NetworkApprovalContext;
use codex_protocol::protocol::NetworkApprovalProtocol;
use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;

pub(crate) fn network_approval_context_to_core(
    value: AppServerNetworkApprovalContext,
) -> NetworkApprovalContext {
    NetworkApprovalContext {
        host: value.host,
        protocol: match value.protocol {
            codex_app_server_protocol::NetworkApprovalProtocol::Http => {
                NetworkApprovalProtocol::Http
            }
            codex_app_server_protocol::NetworkApprovalProtocol::Https => {
                NetworkApprovalProtocol::Https
            }
            codex_app_server_protocol::NetworkApprovalProtocol::Socks5Tcp => {
                NetworkApprovalProtocol::Socks5Tcp
            }
            codex_app_server_protocol::NetworkApprovalProtocol::Socks5Udp => {
                NetworkApprovalProtocol::Socks5Udp
            }
        },
    }
}

pub(crate) fn granted_permission_profile_from_request(
    value: CoreRequestPermissionProfile,
) -> GrantedPermissionProfile {
    GrantedPermissionProfile {
        network: value.network.map(|network| AdditionalNetworkPermissions {
            enabled: network.enabled,
        }),
        file_system: value.file_system.map(|file_system| {
            let (read, write) = file_system
                .legacy_read_write_roots()
                .unwrap_or((None, None));
            AdditionalFileSystemPermissions { read, write }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::granted_permission_profile_from_request;
    use super::network_approval_context_to_core;
    use codex_protocol::models::FileSystemPermissions;
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::protocol::NetworkApprovalContext;
    use codex_protocol::protocol::NetworkApprovalProtocol;
    use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn absolute_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(PathBuf::from(path)).expect("path must be absolute")
    }

    #[test]
    fn converts_app_server_network_approval_context_to_core() {
        assert_eq!(
            network_approval_context_to_core(codex_app_server_protocol::NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: codex_app_server_protocol::NetworkApprovalProtocol::Socks5Tcp,
            }),
            NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Socks5Tcp,
            }
        );
    }

    #[test]
    fn converts_request_permissions_into_granted_permissions() {
        assert_eq!(
            granted_permission_profile_from_request(CoreRequestPermissionProfile {
                network: Some(NetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(FileSystemPermissions::from_read_write_roots(
                    Some(vec![absolute_path("/tmp/read-only")]),
                    Some(vec![absolute_path("/tmp/write")]),
                )),
            }),
            codex_app_server_protocol::GrantedPermissionProfile {
                network: Some(codex_app_server_protocol::AdditionalNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(codex_app_server_protocol::AdditionalFileSystemPermissions {
                    read: Some(vec![absolute_path("/tmp/read-only")]),
                    write: Some(vec![absolute_path("/tmp/write")]),
                }),
            }
        );
    }
}
