#![deny(clippy::print_stdout, clippy::print_stderr)]

mod admin;
mod config;
mod http_proxy;
mod init;
mod mitm;
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
pub use proxy::Command;
pub use proxy::NetworkProxy;
pub use proxy::NetworkProxyBuilder;
pub use proxy::NetworkProxyHandle;
pub use proxy::run_init;

pub async fn run_main(args: Args) -> Result<()> {
    if let Some(Command::Init) = args.command {
        run_init()?;
        return Ok(());
    }

    let proxy = NetworkProxy::builder().build().await?;
    proxy.run().await?.wait().await
}
