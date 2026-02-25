#[cfg(test)]
use crate::config::NetworkMode;
use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use globset::GlobBuilder;
use globset::GlobSet;
use globset::GlobSetBuilder;
use std::collections::HashSet;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use url::Host as UrlHost;

/// A normalized host string for policy evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Host(String);

impl Host {
    pub fn parse(input: &str) -> Result<Self> {
        let normalized = normalize_host(input);
        ensure!(!normalized.is_empty(), "host is empty");
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Returns true if the host is a loopback hostname or IP literal.
pub fn is_loopback_host(host: &Host) -> bool {
    let host = host.as_str();
    let host = host.split_once('%').map(|(ip, _)| ip).unwrap_or(host);
    if host == "localhost" {
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
    // clearly than hand-rolled range checks. Some non-public ranges (e.g., CGNAT and TEST-NET
    // blocks) are not covered by stable stdlib helpers yet, so we fall back to CIDR checks.
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ipv4_in_cidr(ip, [0, 0, 0, 0], 8) // "this network" (RFC 1122)
        || ipv4_in_cidr(ip, [100, 64, 0, 0], 10) // CGNAT (RFC 6598)
        || ipv4_in_cidr(ip, [192, 0, 0, 0], 24) // IETF Protocol Assignments (RFC 6890)
        || ipv4_in_cidr(ip, [192, 0, 2, 0], 24) // TEST-NET-1 (RFC 5737)
        || ipv4_in_cidr(ip, [198, 18, 0, 0], 15) // Benchmarking (RFC 2544)
        || ipv4_in_cidr(ip, [198, 51, 100, 0], 24) // TEST-NET-2 (RFC 5737)
        || ipv4_in_cidr(ip, [203, 0, 113, 0], 24) // TEST-NET-3 (RFC 5737)
        || ipv4_in_cidr(ip, [240, 0, 0, 0], 4) // Reserved (RFC 6890)
}

fn ipv4_in_cidr(ip: Ipv4Addr, base: [u8; 4], prefix: u8) -> bool {
    let ip = u32::from(ip);
    let base = u32::from(Ipv4Addr::from(base));
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (ip & mask) == (base & mask)
}

fn is_non_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4() {
        return is_non_public_ipv4(v4) || ip.is_loopback();
    }
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

/// Normalize host fragments for policy matching (trim whitespace, strip ports/brackets, lowercase).
pub fn normalize_host(host: &str) -> String {
    let host = host.trim();
    if host.starts_with('[')
        && let Some(end) = host.find(']')
    {
        return normalize_dns_host(&host[1..end]);
    }

    // The proxy stack should typically hand us a host without a port, but be
    // defensive and strip `:port` when there is exactly one `:`.
    if host.bytes().filter(|b| *b == b':').count() == 1 {
        let host = host.split(':').next().unwrap_or_default();
        return normalize_dns_host(host);
    }

    // Avoid mangling unbracketed IPv6 literals, but strip trailing dots so fully qualified domain
    // names are treated the same as their dotless variants.
    normalize_dns_host(host)
}

fn normalize_dns_host(host: &str) -> String {
    let host = host.to_ascii_lowercase();
    host.trim_end_matches('.').to_string()
}

fn normalize_pattern(pattern: &str) -> String {
    let pattern = pattern.trim();
    if pattern == "*" {
        return "*".to_string();
    }

    let (prefix, remainder) = if let Some(domain) = pattern.strip_prefix("**.") {
        ("**.", domain)
    } else if let Some(domain) = pattern.strip_prefix("*.") {
        ("*.", domain)
    } else {
        ("", pattern)
    };

    let remainder = normalize_host(remainder);
    if prefix.is_empty() {
        remainder
    } else {
        format!("{prefix}{remainder}")
    }
}

pub(crate) fn compile_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut seen = HashSet::new();
    for pattern in patterns {
        let pattern = normalize_pattern(pattern);
        // Supported domain patterns:
        // - "example.com": match the exact host
        // - "*.example.com": match any subdomain (not the apex)
        // - "**.example.com": match the apex and any subdomain
        // - "*": match any host
        for candidate in expand_domain_pattern(&pattern) {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            let glob = GlobBuilder::new(&candidate)
                .case_insensitive(true)
                .build()
                .with_context(|| format!("invalid glob pattern: {candidate}"))?;
            builder.add(glob);
        }
    }
    Ok(builder.build()?)
}

#[derive(Debug, Clone)]
pub(crate) enum DomainPattern {
    Any,
    ApexAndSubdomains(String),
    SubdomainsOnly(String),
    Exact(String),
}

impl DomainPattern {
    /// Parse a policy pattern for constraint comparisons.
    ///
    /// Validation of glob syntax happens when building the globset; here we only
    /// decode the wildcard prefixes to keep constraint checks lightweight.
    pub(crate) fn parse(input: &str) -> Self {
        let input = input.trim();
        if input.is_empty() {
            return Self::Exact(String::new());
        }
        if input == "*" {
            Self::Any
        } else if let Some(domain) = input.strip_prefix("**.") {
            Self::parse_domain(domain, Self::ApexAndSubdomains)
        } else if let Some(domain) = input.strip_prefix("*.") {
            Self::parse_domain(domain, Self::SubdomainsOnly)
        } else {
            Self::Exact(input.to_string())
        }
    }

    /// Parse a policy pattern for constraint comparisons, validating domain parts with `url`.
    pub(crate) fn parse_for_constraints(input: &str) -> Self {
        let input = input.trim();
        if input.is_empty() {
            return Self::Exact(String::new());
        }
        if input == "*" {
            return Self::Any;
        }
        if let Some(domain) = input.strip_prefix("**.") {
            return Self::ApexAndSubdomains(parse_domain_for_constraints(domain));
        }
        if let Some(domain) = input.strip_prefix("*.") {
            return Self::SubdomainsOnly(parse_domain_for_constraints(domain));
        }
        Self::Exact(parse_domain_for_constraints(input))
    }

    fn parse_domain(domain: &str, build: impl FnOnce(String) -> Self) -> Self {
        let domain = domain.trim();
        if domain.is_empty() {
            return Self::Exact(String::new());
        }
        build(domain.to_string())
    }

    pub(crate) fn allows(&self, candidate: &DomainPattern) -> bool {
        match self {
            DomainPattern::Any => true,
            DomainPattern::Exact(domain) => match candidate {
                DomainPattern::Exact(candidate) => domain_eq(candidate, domain),
                _ => false,
            },
            DomainPattern::SubdomainsOnly(domain) => match candidate {
                DomainPattern::Any => false,
                DomainPattern::Exact(candidate) => is_strict_subdomain(candidate, domain),
                DomainPattern::SubdomainsOnly(candidate) => {
                    is_subdomain_or_equal(candidate, domain)
                }
                DomainPattern::ApexAndSubdomains(candidate) => {
                    is_strict_subdomain(candidate, domain)
                }
            },
            DomainPattern::ApexAndSubdomains(domain) => match candidate {
                DomainPattern::Any => false,
                DomainPattern::Exact(candidate) => is_subdomain_or_equal(candidate, domain),
                DomainPattern::SubdomainsOnly(candidate) => {
                    is_subdomain_or_equal(candidate, domain)
                }
                DomainPattern::ApexAndSubdomains(candidate) => {
                    is_subdomain_or_equal(candidate, domain)
                }
            },
        }
    }
}

fn parse_domain_for_constraints(domain: &str) -> String {
    let domain = domain.trim().trim_end_matches('.');
    if domain.is_empty() {
        return String::new();
    }
    let host = if domain.starts_with('[') && domain.ends_with(']') {
        &domain[1..domain.len().saturating_sub(1)]
    } else {
        domain
    };
    if host.contains('*') || host.contains('?') || host.contains('%') {
        return domain.to_string();
    }
    match UrlHost::parse(host) {
        Ok(host) => host.to_string(),
        Err(_) => String::new(),
    }
}

fn expand_domain_pattern(pattern: &str) -> Vec<String> {
    match DomainPattern::parse(pattern) {
        DomainPattern::Any => vec![pattern.to_string()],
        DomainPattern::Exact(domain) => vec![domain],
        DomainPattern::SubdomainsOnly(domain) => {
            vec![format!("?*.{domain}")]
        }
        DomainPattern::ApexAndSubdomains(domain) => {
            vec![domain.clone(), format!("?*.{domain}")]
        }
    }
}

fn normalize_domain(domain: &str) -> String {
    domain.trim_end_matches('.').to_ascii_lowercase()
}

fn domain_eq(left: &str, right: &str) -> bool {
    normalize_domain(left) == normalize_domain(right)
}

fn is_subdomain_or_equal(child: &str, parent: &str) -> bool {
    let child = normalize_domain(child);
    let parent = normalize_domain(parent);
    if child == parent {
        return true;
    }
    child.ends_with(&format!(".{parent}"))
}

fn is_strict_subdomain(child: &str, parent: &str) -> bool {
    let child = normalize_domain(child);
    let parent = normalize_domain(parent);
    child != parent && child.ends_with(&format!(".{parent}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn method_allowed_full_allows_everything() {
        assert!(NetworkMode::Full.allows_method("GET"));
        assert!(NetworkMode::Full.allows_method("POST"));
        assert!(NetworkMode::Full.allows_method("CONNECT"));
    }

    #[test]
    fn method_allowed_limited_allows_only_safe_methods() {
        assert!(NetworkMode::Limited.allows_method("GET"));
        assert!(NetworkMode::Limited.allows_method("HEAD"));
        assert!(NetworkMode::Limited.allows_method("OPTIONS"));
        assert!(!NetworkMode::Limited.allows_method("POST"));
        assert!(!NetworkMode::Limited.allows_method("CONNECT"));
    }

    #[test]
    fn compile_globset_normalizes_trailing_dots() {
        let set = compile_globset(&["Example.COM.".to_string()]).unwrap();

        assert_eq!(true, set.is_match("example.com"));
        assert_eq!(false, set.is_match("api.example.com"));
    }

    #[test]
    fn compile_globset_normalizes_wildcards() {
        let set = compile_globset(&["*.Example.COM.".to_string()]).unwrap();

        assert_eq!(true, set.is_match("api.example.com"));
        assert_eq!(false, set.is_match("example.com"));
    }

    #[test]
    fn compile_globset_normalizes_apex_and_subdomains() {
        let set = compile_globset(&["**.Example.COM.".to_string()]).unwrap();

        assert_eq!(true, set.is_match("example.com"));
        assert_eq!(true, set.is_match("api.example.com"));
    }

    #[test]
    fn compile_globset_normalizes_bracketed_ipv6_literals() {
        let set = compile_globset(&["[::1]".to_string()]).unwrap();

        assert_eq!(true, set.is_match("::1"));
    }

    #[test]
    fn is_loopback_host_handles_localhost_variants() {
        assert!(is_loopback_host(&Host::parse("localhost").unwrap()));
        assert!(is_loopback_host(&Host::parse("localhost.").unwrap()));
        assert!(is_loopback_host(&Host::parse("LOCALHOST").unwrap()));
        assert!(!is_loopback_host(&Host::parse("notlocalhost").unwrap()));
    }

    #[test]
    fn is_loopback_host_handles_ip_literals() {
        assert!(is_loopback_host(&Host::parse("127.0.0.1").unwrap()));
        assert!(is_loopback_host(&Host::parse("::1").unwrap()));
        assert!(!is_loopback_host(&Host::parse("1.2.3.4").unwrap()));
    }

    #[test]
    fn is_non_public_ip_rejects_private_and_loopback_ranges() {
        assert!(is_non_public_ip("127.0.0.1".parse().unwrap()));
        assert!(is_non_public_ip("10.0.0.1".parse().unwrap()));
        assert!(is_non_public_ip("192.168.0.1".parse().unwrap()));
        assert!(is_non_public_ip("100.64.0.1".parse().unwrap()));
        assert!(is_non_public_ip("192.0.0.1".parse().unwrap()));
        assert!(is_non_public_ip("192.0.2.1".parse().unwrap()));
        assert!(is_non_public_ip("198.18.0.1".parse().unwrap()));
        assert!(is_non_public_ip("198.51.100.1".parse().unwrap()));
        assert!(is_non_public_ip("203.0.113.1".parse().unwrap()));
        assert!(is_non_public_ip("240.0.0.1".parse().unwrap()));
        assert!(is_non_public_ip("0.1.2.3".parse().unwrap()));
        assert!(!is_non_public_ip("8.8.8.8".parse().unwrap()));

        assert!(is_non_public_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(is_non_public_ip("::ffff:10.0.0.1".parse().unwrap()));
        assert!(!is_non_public_ip("::ffff:8.8.8.8".parse().unwrap()));

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
    fn normalize_host_strips_trailing_dot() {
        assert_eq!(normalize_host("example.com."), "example.com");
        assert_eq!(normalize_host("ExAmPlE.CoM."), "example.com");
    }

    #[test]
    fn normalize_host_strips_trailing_dot_with_port() {
        assert_eq!(normalize_host("example.com.:443"), "example.com");
    }

    #[test]
    fn normalize_host_strips_brackets_for_ipv6() {
        assert_eq!(normalize_host("[::1]"), "::1");
        assert_eq!(normalize_host("[::1]:443"), "::1");
    }
}
