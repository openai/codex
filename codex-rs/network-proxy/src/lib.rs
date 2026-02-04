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

use anyhow::Result;
pub use network_policy::NetworkDecision;
pub use network_policy::NetworkPolicyDecider;
pub use network_policy::NetworkPolicyRequest;
pub use network_policy::NetworkPolicyRequestArgs;
pub use network_policy::NetworkProtocol;
pub use proxy::Args;
pub use proxy::NetworkProxy;
pub use proxy::NetworkProxyBuilder;
pub use proxy::NetworkProxyHandle;

pub async fn run_main(args: Args) -> Result<()> {
    let _ = args;
    let proxy = NetworkProxy::builder().build().await?;
    proxy.run().await?.wait().await
}
