#[cfg(unix)]
use dns_lookup::AddrInfoHints;
#[cfg(unix)]
use dns_lookup::getaddrinfo;
use std::sync::LazyLock;
#[cfg(windows)]
use winapi_util::sysinfo::ComputerNameKind;
#[cfg(windows)]
use winapi_util::sysinfo::get_computer_name;

static REQUIREMENTS_HOSTNAME: LazyLock<Option<String>> =
    LazyLock::new(compute_requirements_hostname);

pub fn requirements_hostname() -> Option<String> {
    REQUIREMENTS_HOSTNAME.clone()
}

fn compute_requirements_hostname() -> Option<String> {
    let kernel_hostname = gethostname::gethostname();
    let kernel_hostname = normalize_requirements_hostname(&kernel_hostname.to_string_lossy())?;

    // Remote sandbox requirements are meant to target remote hosts by DNS name,
    // so prefer the canonical FQDN when the local resolver can provide one.
    if let Some(fqdn) = local_fqdn_for_hostname(&kernel_hostname) {
        return Some(fqdn);
    }

    // Some machines have only a short local hostname or resolver setup that
    // does not return AI_CANONNAME. Keep matching behavior best-effort by
    // falling back to the cleaned kernel hostname instead of returning None.
    Some(kernel_hostname)
}

fn normalize_requirements_hostname(hostname: &str) -> Option<String> {
    let hostname = hostname.trim().trim_end_matches('.');
    (!hostname.is_empty()).then(|| hostname.to_ascii_lowercase())
}

#[cfg(unix)]
fn local_fqdn_for_hostname(hostname: &str) -> Option<String> {
    let hints = AddrInfoHints {
        flags: libc::AI_CANONNAME,
        ..AddrInfoHints::default()
    };

    getaddrinfo(Some(hostname), /*service*/ None, Some(hints))
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|addr| addr.canonname)
        // getaddrinfo may return the short hostname as canonname when no FQDN
        // is available. Treat only DNS-qualified names as an FQDN result.
        .find_map(|hostname| normalize_fqdn_candidate(&hostname))
}

#[cfg(windows)]
fn local_fqdn_for_hostname(_hostname: &str) -> Option<String> {
    get_computer_name(ComputerNameKind::PhysicalDnsFullyQualified)
        .ok()
        .and_then(|hostname| hostname.into_string().ok())
        .and_then(|hostname| normalize_fqdn_candidate(&hostname))
}

#[cfg(not(any(unix, windows)))]
fn local_fqdn_for_hostname(_hostname: &str) -> Option<String> {
    None
}

fn normalize_fqdn_candidate(hostname: &str) -> Option<String> {
    normalize_requirements_hostname(hostname).filter(|hostname| hostname.contains('.'))
}

#[cfg(test)]
mod tests {
    use super::normalize_fqdn_candidate;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_fqdn_candidate_accepts_dns_qualified_name() {
        assert_eq!(
            normalize_fqdn_candidate("runner-01.ci.example.com"),
            Some("runner-01.ci.example.com".to_string())
        );
    }

    #[test]
    fn normalize_fqdn_candidate_rejects_short_name() {
        assert_eq!(normalize_fqdn_candidate("runner-01"), None);
    }

    #[test]
    fn normalize_fqdn_candidate_trims_trailing_dot_and_normalizes_case() {
        assert_eq!(
            normalize_fqdn_candidate("RUNNER-01.CI.EXAMPLE.COM."),
            Some("runner-01.ci.example.com".to_string())
        );
    }
}
