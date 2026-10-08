//! Network-domain wildcards over UTF-8 bytes, independent of filesystem semantics.
//! Construction validates and normalizes patterns and expands domain prefixes.
//! Callers normalize candidate hosts before matching. No IDNA conversion is performed.
//! Byte matching preserves the historical globset wildcard semantics.

use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use std::collections::HashSet;
use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlobalWildcard {
    Allow,
    Reject,
}

#[derive(Clone)]
pub struct DomainPatternSet {
    patterns: Arc<[Vec<u8>]>,
}

impl DomainPatternSet {
    pub(crate) fn new(patterns: &[String], global_wildcard: GlobalWildcard) -> Result<Self> {
        let mut compiled = Vec::new();
        let mut seen = HashSet::new();
        for pattern in patterns {
            if global_wildcard == GlobalWildcard::Reject
                && is_global_wildcard_domain_pattern(pattern)
            {
                bail!(
                    "unsupported global wildcard domain pattern \"*\"; use exact hosts or scoped wildcards like *.example.com or **.example.com"
                );
            }
            let raw = pattern.trim();
            ensure!(
                !raw.contains(['{', '}', '\\', '/']) && !raw.chars().any(char::is_whitespace),
                "unsupported domain pattern syntax: {pattern}"
            );
            let host = raw
                .strip_prefix("**.")
                .or_else(|| raw.strip_prefix("*."))
                .unwrap_or(raw);
            ensure!(
                !host.contains("**"),
                "unsupported repeated wildcard: {pattern}"
            );
            if host.contains(['[', ']']) {
                let literal = host.strip_prefix('[').and_then(|s| s.split_once(']'));
                ensure!(
                    literal.is_some_and(|(ip, suffix)| {
                        let normalized = normalize_dns_host_or_ip_literal(ip);
                        let unscoped = normalized.split('%').next().unwrap_or_default();
                        !ip.contains(['[', ']'])
                            && (normalized == "*" || unscoped.parse::<Ipv6Addr>().is_ok())
                            && !suffix.contains(['[', ']'])
                            && (suffix.chars().all(|c| c == '.') || suffix.starts_with(':'))
                    }),
                    "brackets are only supported for IPv6 literals or [*]: {pattern}"
                );
            }
            let pattern = normalize_pattern(pattern);
            // Supported domain patterns:
            // - "example.com": match the exact host
            // - "*.example.com": match any subdomain (not the apex)
            // - "**.example.com": match the apex and any subdomain
            // - "api?.example.com": match exactly one byte after "api"
            // - "*": match every host when explicitly enabled for allowlist compilation
            for candidate in expand_domain_pattern(&pattern) {
                if !seen.insert(candidate.clone()) {
                    continue;
                }
                ensure!(!candidate.is_empty(), "domain pattern is empty");
                compiled.push(candidate.into_bytes());
            }
        }
        Ok(Self {
            patterns: compiled.into(),
        })
    }

    pub fn is_match(&self, host: impl AsRef<str>) -> bool {
        let host = host.as_ref().as_bytes();
        self.patterns.iter().any(|pattern| matches(pattern, host))
    }
}

/// Normalize host fragments for policy matching (trim whitespace, strip ports/brackets, lowercase).
pub fn normalize_host(host: &str) -> String {
    let host = host.trim();
    if host.starts_with('[')
        && let Some(end) = host.find(']')
    {
        return normalize_dns_host_or_ip_literal(&host[1..end]);
    }

    // The proxy stack should typically hand us a host without a port, but be
    // defensive and strip `:port` when there is exactly one `:`.
    if host.bytes().filter(|b| *b == b':').count() == 1 {
        let host = host.split(':').next().unwrap_or_default();
        return normalize_dns_host_or_ip_literal(host);
    }

    // Avoid mangling unbracketed IPv6 literals, but strip trailing dots so fully qualified domain
    // names are treated the same as their dotless variants.
    normalize_dns_host_or_ip_literal(host)
}

fn normalize_dns_host_or_ip_literal(host: &str) -> String {
    let host = host.to_ascii_lowercase();
    let host = host.trim_end_matches('.');
    if let Some(ip) = normalize_ip_literal(host) {
        return ip;
    }
    host.to_string()
}

pub(crate) fn unscoped_ip_literal(host: &str) -> Option<&str> {
    let (ip, _) = host.split_once('%')?;
    ip.parse::<IpAddr>().ok()?;
    Some(ip)
}

fn normalize_ip_literal(host: &str) -> Option<String> {
    if host.parse::<IpAddr>().is_ok() {
        return Some(host.to_string());
    }
    for delimiter in ["%25", "%"] {
        if let Some((ip, scope)) = host.split_once(delimiter)
            && ip.parse::<IpAddr>().is_ok()
        {
            return Some(format!("{ip}%{scope}"));
        }
    }
    None
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

pub(crate) fn is_global_wildcard_domain_pattern(pattern: &str) -> bool {
    let normalized = normalize_pattern(pattern);
    expand_domain_pattern(&normalized)
        .iter()
        .any(|candidate| candidate == "*")
}

fn expand_domain_pattern(pattern: &str) -> Vec<String> {
    let pattern = pattern.trim();
    let Some(domain) = pattern
        .strip_prefix("**.")
        .or_else(|| pattern.strip_prefix("*."))
    else {
        return vec![pattern.to_string()];
    };
    let domain = domain.trim();
    if domain.is_empty() {
        return vec![String::new()];
    }
    if pattern.starts_with("**.") {
        vec![domain.to_string(), format!("?*.{domain}")]
    } else {
        vec![format!("?*.{domain}")]
    }
}

// Only the latest star needs retrying: it can absorb any extension that an earlier star could.
// Its retry position advances monotonically, bounding work by O(pattern length * host length).
fn matches(pattern: &[u8], host: &[u8]) -> bool {
    let (mut p, mut h) = (0, 0);
    let mut star = None;
    while h < host.len() {
        match pattern.get(p) {
            Some(b'*') => {
                star = Some((p + 1, h));
                p += 1;
            }
            Some(c) if *c == b'?' || c.eq_ignore_ascii_case(&host[h]) => {
                p += 1;
                h += 1;
            }
            _ => match &mut star {
                Some((resume, consumed)) => {
                    *consumed += 1;
                    p = *resume;
                    h = *consumed;
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|c| *c == b'*')
}

#[cfg(test)]
#[path = "domain_matcher_tests.rs"]
mod tests;
