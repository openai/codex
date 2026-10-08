#[cfg(test)]
use crate::config::NetworkMode;
use crate::domain_matcher::DomainPatternSet;
use crate::domain_matcher::GlobalWildcard;
pub(crate) use crate::domain_matcher::is_global_wildcard_domain_pattern;
pub use crate::domain_matcher::normalize_host;
pub(crate) use crate::domain_matcher::unscoped_ip_literal;
use anyhow::Result;
use anyhow::ensure;
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
    let host = unscoped_ip_literal(host).unwrap_or(host);
    if host == "localhost" {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return ip.is_loopback();
    }
    false
}

/// Matches the destinations in `DEFAULT_NO_PROXY_VALUE`, without resolving DNS.
pub(crate) fn is_default_proxy_bypass_host(host: &str) -> bool {
    let host = normalize_host(host);
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip == Ipv4Addr::LOCALHOST || ip.is_private(),
        Ok(IpAddr::V6(ip)) => ip == Ipv6Addr::LOCALHOST,
        Err(_) => host == "localhost" || host.ends_with(".localhost"),
    }
}

/// Classifies IP literals excluded from public network destinations; does not resolve DNS.
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
        || ipv4_in_cidr(ip, [0, 0, 0, 0], /*prefix*/ 8) // "this network" (RFC 1122)
        || ipv4_in_cidr(ip, [100, 64, 0, 0], /*prefix*/ 10) // CGNAT (RFC 6598)
        || ipv4_in_cidr(ip, [192, 0, 0, 0], /*prefix*/ 24) // IETF Protocol Assignments (RFC 6890)
        || ipv4_in_cidr(ip, [192, 0, 2, 0], /*prefix*/ 24) // TEST-NET-1 (RFC 5737)
        || ipv4_in_cidr(ip, [198, 18, 0, 0], /*prefix*/ 15) // Benchmarking (RFC 2544)
        || ipv4_in_cidr(ip, [198, 51, 100, 0], /*prefix*/ 24) // TEST-NET-2 (RFC 5737)
        || ipv4_in_cidr(ip, [203, 0, 113, 0], /*prefix*/ 24) // TEST-NET-3 (RFC 5737)
        || ipv4_in_cidr(ip, [240, 0, 0, 0], /*prefix*/ 4) // Reserved (RFC 6890)
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

/// Private unicast ranges that may be reachable through an upstream VPN proxy.
/// Loopback, link-local, and other special-use addresses retain their normal routing.
pub(crate) fn is_private_network_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ipv4_in_cidr(ip, [100, 64, 0, 0], /*prefix*/ 10),
        IpAddr::V6(ip) => match ip.to_ipv4() {
            Some(ip) => is_private_network_ip(IpAddr::V4(ip)),
            None => ip.is_unique_local(),
        },
    }
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

pub(crate) fn compile_allowlist(patterns: &[String]) -> Result<DomainPatternSet> {
    DomainPatternSet::new(patterns, GlobalWildcard::Allow)
}

pub(crate) fn compile_denylist(patterns: &[String]) -> Result<DomainPatternSet> {
    DomainPatternSet::new(patterns, GlobalWildcard::Reject)
}

#[derive(Debug, Clone)]
pub(crate) enum DomainPattern {
    ApexAndSubdomains(String),
    SubdomainsOnly(String),
    Exact(String),
}

impl DomainPattern {
    /// Parse a policy pattern for constraint comparisons, validating domain parts with `url`.
    pub(crate) fn parse_for_constraints(input: &str) -> Self {
        let input = input.trim();
        if input.is_empty() {
            return Self::Exact(String::new());
        }
        if let Some(domain) = input.strip_prefix("**.") {
            return Self::ApexAndSubdomains(parse_domain_for_constraints(domain));
        }
        if let Some(domain) = input.strip_prefix("*.") {
            return Self::SubdomainsOnly(parse_domain_for_constraints(domain));
        }
        Self::Exact(parse_domain_for_constraints(input))
    }

    pub(crate) fn allows(&self, candidate: &DomainPattern) -> bool {
        match self {
            DomainPattern::Exact(domain) => match candidate {
                DomainPattern::Exact(candidate) => domain_eq(candidate, domain),
                _ => false,
            },
            DomainPattern::SubdomainsOnly(domain) => match candidate {
                DomainPattern::Exact(candidate) => is_strict_subdomain(candidate, domain),
                DomainPattern::SubdomainsOnly(candidate) => {
                    is_subdomain_or_equal(candidate, domain)
                }
                DomainPattern::ApexAndSubdomains(candidate) => {
                    is_strict_subdomain(candidate, domain)
                }
            },
            DomainPattern::ApexAndSubdomains(domain) => match candidate {
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
        let set = compile_denylist(&["Example.COM.".to_string()]).unwrap();

        assert_eq!(true, set.is_match("example.com"));
        assert_eq!(false, set.is_match("api.example.com"));
    }

    #[test]
    fn compile_globset_normalizes_wildcards() {
        let set = compile_denylist(&["*.Example.COM.".to_string()]).unwrap();

        assert_eq!(true, set.is_match("api.example.com"));
        assert_eq!(false, set.is_match("example.com"));
    }

    #[test]
    fn compile_globset_supports_mid_label_wildcards() {
        let set = compile_denylist(&["region*.v2.argotunnel.com".to_string()]).unwrap();

        assert_eq!(true, set.is_match("region1.v2.argotunnel.com"));
        assert_eq!(true, set.is_match("region.v2.argotunnel.com"));
        assert_eq!(false, set.is_match("xregion1.v2.argotunnel.com"));
        assert_eq!(false, set.is_match("foo.region1.v2.argotunnel.com"));
    }

    // Keep this table one-for-one with the browser network-policy matchers'
    // question-mark contract cases so grammar changes are checked on both sides.
    #[test]
    fn compile_globset_supports_question_mark_wildcards() -> Result<()> {
        for (pattern, host, expected) in [
            ("api?.example.com", "api1.example.com", true),
            ("api?.example.com", "api.example.com", false),
            ("api?.example.com", "api12.example.com", false),
            ("api??.example.com", "api12.example.com", true),
            ("api??.example.com", "api1.example.com", false),
            ("api*?.example.com", "api.example.com", false),
            ("api*?.example.com", "api1.example.com", true),
            ("api*?.example.com", "api123.example.com", true),
            ("api?example.com", "api.example.com", true),
            ("*.api?.example.com", "api1.example.com", false),
            ("*.api?.example.com", "www.api1.example.com", true),
            ("*.api?.example.com", "nested.www.api1.example.com", true),
            ("*.api?.example.com", "www.api12.example.com", false),
            ("**.api?.example.com", "api1.example.com", true),
            ("**.api?.example.com", "www.api1.example.com", true),
            ("**.api?.example.com", "nested.www.api1.example.com", true),
            ("**.api?.example.com", "api12.example.com", false),
            ("**.api?.example.com", "www.api12.example.com", false),
            (" API?.EXAMPLE.COM. ", "API1.EXAMPLE.COM.", true),
        ] {
            let patterns = [pattern.to_string()];
            for set in [compile_allowlist(&patterns)?, compile_denylist(&patterns)?] {
                assert_eq!(
                    set.is_match(normalize_host(host)),
                    expected,
                    "pattern {pattern}, host {host}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn compile_globset_normalizes_apex_and_subdomains() {
        let set = compile_denylist(&["**.Example.COM.".to_string()]).unwrap();

        assert_eq!(true, set.is_match("example.com"));
        assert_eq!(true, set.is_match("api.example.com"));
    }

    #[test]
    fn compile_globset_normalizes_bracketed_ipv6_literals() {
        let set = compile_denylist(&["[::1]".to_string()]).unwrap();

        assert_eq!(true, set.is_match("::1"));
    }

    #[test]
    fn compile_globset_preserves_scoped_ipv6_literals() {
        let set = compile_denylist(&["[fe80::1%25lo0]".to_string()]).unwrap();

        assert_eq!(true, set.is_match("fe80::1%lo0"));
        assert_eq!(false, set.is_match("fe80::1%lo1"));
        assert_eq!(false, set.is_match("fe80::1"));
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
}
