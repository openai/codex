use std::io;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;

use url::Url;

/// Replaces exactly one non-program argument with the effective loopback HTTP proxy endpoint.
///
/// This runs only after the network-namespace bridge rewrites `HTTP_PROXY`. The argument prefix
/// comes from trusted direct-spawn orchestration metadata; no shell interpolation occurs.
pub(crate) fn rewrite_http_proxy_argument_from_env(
    command: &mut [String],
    argument_prefix: &str,
) -> io::Result<()> {
    let proxy_url = std::env::var("HTTP_PROXY").map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "runtime proxy argument rewrite requires HTTP_PROXY",
        )
    })?;
    rewrite_http_proxy_argument(command, argument_prefix, &proxy_url)
}

fn rewrite_http_proxy_argument(
    command: &mut [String],
    argument_prefix: &str,
    proxy_url: &str,
) -> io::Result<()> {
    if argument_prefix.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime proxy argument prefix must not be empty",
        ));
    }
    let endpoint = parse_loopback_http_proxy_endpoint(proxy_url).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime HTTP proxy must be a parseable nonzero loopback endpoint",
        )
    })?;
    let matching_indices = command
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(index, argument)| argument.starts_with(argument_prefix).then_some(index))
        .collect::<Vec<_>>();
    let [argument_index] = matching_indices.as_slice() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "runtime proxy argument rewrite expected exactly one `{argument_prefix}` argument"
            ),
        ));
    };
    command[*argument_index] = format!("{argument_prefix}http://{endpoint}");
    Ok(())
}

fn parse_loopback_http_proxy_endpoint(proxy_url: &str) -> Option<SocketAddr> {
    let parsed = Url::parse(proxy_url).ok()?;
    if parsed.scheme() != "http" {
        return None;
    }
    let host = parsed.host_str()?;
    let port = parsed.port_or_known_default()?;
    if port == 0 {
        return None;
    }
    let ip = if host.eq_ignore_ascii_case("localhost") {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        host.parse::<IpAddr>().ok()?
    };
    ip.is_loopback().then_some(SocketAddr::new(ip, port))
}

#[cfg(test)]
#[path = "runtime_proxy_argument_tests.rs"]
mod tests;
