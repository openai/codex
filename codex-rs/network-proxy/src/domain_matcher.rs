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
    restricted_ip_literals: Arc<HashSet<IpAddr>>,
}

impl DomainPatternSet {
    pub(crate) fn new(patterns: &[String], global_wildcard: GlobalWildcard) -> Result<Self> {
        let mut compiled = Vec::new();
        let mut restricted_ip_literals = HashSet::new();
        let mut seen = HashSet::new();
        for pattern in patterns {
            let pattern = validated_pattern(pattern, global_wildcard)?;
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
                // Keep exact address equivalence separate from ordinary allowlist
                // matching. A scoped IP rule must never apply to other interfaces.
                if !candidate.contains('%')
                    && let Some(ip) = crate::policy::numeric_host_address(&candidate)
                {
                    restricted_ip_literals.insert(ip.to_canonical());
                }
                compiled.push(candidate.into_bytes());
            }
        }
        Ok(Self {
            patterns: compiled.into(),
            restricted_ip_literals: Arc::new(restricted_ip_literals),
        })
    }

    pub fn is_match(&self, host: impl AsRef<str>) -> bool {
        let host = host.as_ref().as_bytes();
        self.patterns.iter().any(|pattern| matches(pattern, host))
    }

    /// Conservatively include wildcard patterns that could match a canonical IP spelling.
    pub(crate) fn may_restrict_ip(&self) -> bool {
        !self.restricted_ip_literals.is_empty()
            || self.patterns.iter().any(|p| {
                p.iter().any(|b| matches!(b, b'*' | b'?'))
                    && p.iter()
                        .all(|b| b.is_ascii_hexdigit() || matches!(b, b':' | b'.' | b'*' | b'?'))
                    // Dots occur only in the decimal IPv4 tail, including mapped IPv6.
                    && p.iter().skip_while(|b| **b != b'.')
                        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'*' | b'?'))
            })
    }

    /// An IPv4 address and its IPv4-mapped IPv6 form can dial the same peer.
    /// Check both spellings for restrictive rules, without expanding allowlists.
    pub(crate) fn restricts_ip(&self, ip: IpAddr) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        let ip = ip.to_canonical();
        if self.restricted_ip_literals.contains(&ip) {
            return true;
        }
        match ip {
            IpAddr::V4(ip) => {
                self.is_match(ip.to_string()) || self.is_match(ip.to_ipv6_mapped().to_string())
            }
            IpAddr::V6(ip) => self.is_match(ip.to_string()),
        }
    }
}

/// Validate and normalize an allow selector using the proxy matcher's syntax.
pub fn canonical_allow_pattern(pattern: &str) -> Result<String> {
    let host = canonical_pattern(pattern, GlobalWildcard::Allow)?;
    // A wildcard in a stripped port still disqualifies this rule from the local-binding
    // exception. Retain a wildcard port in the comparison key for that case.
    if pattern.contains(['*', '?'])
        && !host.contains(['*', '?'])
        && crate::policy::is_local_literal(&host)
    {
        return Ok(if host.contains(':') {
            format!("[{host}]:*")
        } else {
            format!("{host}:*")
        });
    }
    Ok(host)
}

/// Validate and normalize a deny selector, including the restriction on global wildcards.
pub fn canonical_deny_pattern(pattern: &str) -> Result<String> {
    canonical_pattern(pattern, GlobalWildcard::Reject).map(restrictive_pattern_identity)
}

/// Review rules allow global wildcards and, like denials, compare unscoped IPv6 literals by address.
pub fn canonical_review_pattern(pattern: &str) -> Result<String> {
    canonical_pattern(pattern, GlobalWildcard::Allow).map(restrictive_pattern_identity)
}

fn restrictive_pattern_identity(pattern: String) -> String {
    match pattern.parse::<Ipv6Addr>() {
        Ok(ip) => ip.to_string(),
        Err(_) => pattern,
    }
}

fn canonical_pattern(pattern: &str, global_wildcard: GlobalWildcard) -> Result<String> {
    let pattern = validated_pattern(pattern, global_wildcard)?;
    // Subdomains-only selectors compile to one byte glob with this prefix. Treat the
    // explicitly written glob as the same selector for list/constraint comparisons.
    if let Some(domain) = pattern.strip_prefix("?*.") {
        return Ok(format!("*.{domain}"));
    }
    Ok(pattern)
}

fn validated_pattern(pattern: &str, global_wildcard: GlobalWildcard) -> Result<String> {
    if global_wildcard == GlobalWildcard::Reject && is_global_wildcard_domain_pattern(pattern) {
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
    ensure!(
        !matches!(pattern.as_str(), "" | "*." | "**."),
        "domain pattern is empty"
    );
    Ok(pattern)
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
