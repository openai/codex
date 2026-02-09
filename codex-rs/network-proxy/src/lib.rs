#![deny(clippy::print_stdout, clippy::print_stderr)]

mod admin;
mod config;
mod http_proxy;
mod network_policy;
mod policy;
mod proxy;
mod reasons;
mod responses;
mod runtime;
mod socks5;
mod state;
mod upstream;

pub use config::NetworkMode;
pub use config::NetworkProxyConfig;
pub use network_policy::NetworkDecision;
pub use network_policy::NetworkPolicyDecider;
pub use network_policy::NetworkPolicyRequest;
pub use network_policy::NetworkPolicyRequestArgs;
pub use network_policy::NetworkProtocol;
pub use proxy::Args;
pub use proxy::NetworkProxy;
pub use proxy::NetworkProxyBuilder;
pub use proxy::NetworkProxyHandle;
pub use runtime::ConfigReloader;
pub use runtime::ConfigState;
pub use runtime::NetworkProxyState;
pub use state::NetworkProxyConstraintError;
pub use state::NetworkProxyConstraints;
pub use state::PartialNetworkConfig;
pub use state::PartialNetworkProxyConfig;
pub use state::build_config_state;
pub use state::validate_policy_against_constraints;
