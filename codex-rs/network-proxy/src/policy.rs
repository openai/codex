use crate::config::NetworkMode;
use hyper::Method;
use std::net::IpAddr;

pub fn method_allowed(mode: NetworkMode, method: &Method) -> bool {
    match mode {
        NetworkMode::Full => true,
        NetworkMode::Limited => matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS),
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

pub fn normalize_host(host: &str) -> String {
    let host = host.trim();
    if host.starts_with('[') {
        if let Some(end) = host.find(']') {
            return host[1..end].to_ascii_lowercase();
        }
    }
    host.split(':').next().unwrap_or("").to_ascii_lowercase()
}
