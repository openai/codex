//! This macOS-only module implements DNS resolution
//! using macOS's native system resolver. We have to
//! do this rather than relying on the default behavior
//! (which reads /etc/resolv.conf) since default behavior
//! breaks Codex's ability to interact with network paths
//! that are provided by applications that modify / hook
//! into macOS DNS behavior (i.e. proxies, VPNs, etc)
use rama_dns::DnsResolver;
use rama_net::address::Domain;
use std::io;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use tokio::net::lookup_host;

/// Resolve TCP destinations through macOS's native resolver, including supplemental
/// resolvers and VPN split DNS that are not represented in `/etc/resolv.conf`.
#[derive(Clone)]
pub(crate) struct SystemDnsResolver;

impl DnsResolver for SystemDnsResolver {
    type Error = io::Error;

    async fn ipv4_lookup(&self, domain: Domain) -> io::Result<Vec<Ipv4Addr>> {
        Ok(lookup_host((domain.as_str(), 0))
            .await?
            .filter_map(|addr| match addr.ip() {
                IpAddr::V4(ip) => Some(ip),
                IpAddr::V6(_) => None,
            })
            .collect())
    }

    async fn ipv6_lookup(&self, domain: Domain) -> io::Result<Vec<Ipv6Addr>> {
        Ok(lookup_host((domain.as_str(), 0))
            .await?
            .filter_map(|addr| match addr.ip() {
                IpAddr::V4(_) => None,
                IpAddr::V6(ip) => Some(ip),
            })
            .collect())
    }

    async fn txt_lookup(&self, _domain: Domain) -> io::Result<Vec<Vec<u8>>> {
        // TCP connectors only need address lookups. Do not silently fall back to
        // a resolver with different DNS routing for unsupported record types.
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the system address resolver does not support TXT lookups",
        ))
    }
}

#[cfg(test)]
#[path = "system_dns_tests.rs"]
mod tests;
