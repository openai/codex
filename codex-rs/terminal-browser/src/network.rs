use std::net::SocketAddr;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BrowserNetworkPolicy {
    #[default]
    Disabled,
    Direct,
    ManagedProxy {
        http_addr: SocketAddr,
    },
}
