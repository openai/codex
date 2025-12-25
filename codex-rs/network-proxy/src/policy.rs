use crate::config::NetworkMode;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;

pub fn method_allowed(mode: NetworkMode, method: &str) -> bool {
    match mode {
        NetworkMode::Full => true,
        NetworkMode::Limited => matches!(method, "GET" | "HEAD" | "OPTIONS"),
    }
}

pub fn is_loopback_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host == "localhost." {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return ip.is_loopback();
    }
    false
}

pub fn is_non_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_non_public_ipv4(ip),
        IpAddr::V6(ip) => is_non_public_ipv6(ip),
    }
}

fn is_non_public_ipv4(ip: Ipv4Addr) -> bool {
    // Use the standard library classification helpers where possible; they encode the intent more
    // clearly than hand-rolled range checks.
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
}

fn is_non_public_ipv6(ip: Ipv6Addr) -> bool {
    // Treat anything that isn't globally routable as "local" for SSRF prevention. In particular:
    //  - `::1` loopback
    //  - `fc00::/7` unique-local (RFC 4193)
    //  - `fe80::/10` link-local
    //  - `::` unspecified
    //  - multicast ranges
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
}

pub fn normalize_host(host: &str) -> String {
    let host = host.trim();
    if host.starts_with('[')
        && let Some(end) = host.find(']')
    {
        return host[1..end].to_ascii_lowercase();
    }

    // The proxy stack should typically hand us a host without a port, but be
    // defensive and strip `:port` when there is exactly one `:`.
    if host.bytes().filter(|b| *b == b':').count() == 1 {
        return host
            .split(':')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
    }

    // Avoid mangling unbracketed IPv6 literals.
    host.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn method_allowed_full_allows_everything() {
        assert!(method_allowed(NetworkMode::Full, "GET"));
        assert!(method_allowed(NetworkMode::Full, "POST"));
        assert!(method_allowed(NetworkMode::Full, "CONNECT"));
    }

    #[test]
    fn method_allowed_limited_allows_only_safe_methods() {
        assert!(method_allowed(NetworkMode::Limited, "GET"));
        assert!(method_allowed(NetworkMode::Limited, "HEAD"));
        assert!(method_allowed(NetworkMode::Limited, "OPTIONS"));
        assert!(!method_allowed(NetworkMode::Limited, "POST"));
        assert!(!method_allowed(NetworkMode::Limited, "CONNECT"));
    }

    #[test]
    fn is_loopback_host_handles_localhost_variants() {
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("localhost."));
        assert!(is_loopback_host("LOCALHOST"));
        assert!(!is_loopback_host("notlocalhost"));
    }

    #[test]
    fn is_loopback_host_handles_ip_literals() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("::1"));
        assert!(!is_loopback_host("1.2.3.4"));
    }

    #[test]
    fn is_non_public_ip_rejects_private_and_loopback_ranges() {
        assert!(is_non_public_ip("127.0.0.1".parse().unwrap()));
        assert!(is_non_public_ip("10.0.0.1".parse().unwrap()));
        assert!(is_non_public_ip("192.168.0.1".parse().unwrap()));
        assert!(!is_non_public_ip("8.8.8.8".parse().unwrap()));

        assert!(is_non_public_ip("::1".parse().unwrap()));
        assert!(is_non_public_ip("fe80::1".parse().unwrap()));
        assert!(is_non_public_ip("fc00::1".parse().unwrap()));
    }

    #[test]
    fn normalize_host_lowercases_and_trims() {
        assert_eq!(normalize_host("  ExAmPlE.CoM  "), "example.com");
    }

    #[test]
    fn normalize_host_strips_port_for_host_port() {
        assert_eq!(normalize_host("example.com:1234"), "example.com");
    }

    #[test]
    fn normalize_host_preserves_unbracketed_ipv6() {
        assert_eq!(normalize_host("2001:db8::1"), "2001:db8::1");
    }

    #[test]
    fn normalize_host_strips_brackets_for_ipv6() {
        assert_eq!(normalize_host("[::1]"), "::1");
        assert_eq!(normalize_host("[::1]:443"), "::1");
    }
}
