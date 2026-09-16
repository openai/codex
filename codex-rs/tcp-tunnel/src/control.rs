//! Bounded parsing of HTTP/3 CONNECT metadata and bearer input.
//! Extension values remain sensitive and may never replace authorization or forwarding headers.
use std::io::BufRead;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::ensure;
use http::HeaderMap;
use http::HeaderValue;
use http::header;

pub(super) const MAX_TOKEN_BYTES: usize = 64 * 1024;
pub(super) const MAX_CONNECT_METADATA_BYTES: usize = 16 * 1024;
pub(super) const MAX_CONNECT_HEADERS: usize = 16;
pub(super) const MAX_CONNECT_HEADER_VALUE_BYTES: usize = 4 * 1024;

pub(super) fn read_connect_headers(reader: &mut impl BufRead) -> Result<HeaderMap> {
    let mut line = Vec::new();
    std::io::Read::take(&mut *reader, (MAX_CONNECT_METADATA_BYTES + 1) as u64)
        .read_until(b'\n', &mut line)
        .context("reading CONNECT metadata")?;
    ensure!(
        !line.is_empty() && line.len() <= MAX_CONNECT_METADATA_BYTES && line.ends_with(b"\n"),
        "invalid or oversized CONNECT metadata"
    );
    let pairs: Vec<(String, String)> =
        serde_json::from_slice(&line).map_err(|_| anyhow!("invalid CONNECT metadata"))?;
    ensure!(
        pairs.len() <= MAX_CONNECT_HEADERS,
        "too many CONNECT metadata headers"
    );
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        let name = header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| anyhow!("invalid CONNECT metadata header name"))?;
        ensure!(
            !headers.contains_key(&name),
            "duplicate CONNECT metadata header"
        );
        let extension = name.as_str();
        ensure!(
            extension.starts_with("x-")
                && !extension.starts_with("x-forwarded-")
                && extension != "x-real-ip",
            "CONNECT metadata must use non-forwarding extension headers"
        );
        let mut value = HeaderValue::from_str(&value)
            .map_err(|_| anyhow!("invalid CONNECT metadata header value"))?;
        ensure!(
            value.as_bytes().len() <= MAX_CONNECT_HEADER_VALUE_BYTES,
            "CONNECT metadata header value is too long"
        );
        value.set_sensitive(true);
        headers.insert(name, value);
    }
    Ok(headers)
}

pub(super) fn read_auth_token(reader: &mut impl BufRead) -> Result<Option<HeaderValue>> {
    let mut line = Vec::new();
    std::io::Read::take(&mut *reader, (MAX_TOKEN_BYTES + 1) as u64)
        .read_until(b'\n', &mut line)
        .context("reading MASQUE token")?;
    if line.is_empty() {
        return Ok(None);
    }
    ensure!(line.len() <= MAX_TOKEN_BYTES, "MASQUE token is too long");
    let token = std::str::from_utf8(&line)
        .context("invalid MASQUE token encoding")?
        .trim();
    ensure!(!token.is_empty(), "empty MASQUE token");
    let mut auth =
        HeaderValue::from_str(&format!("Bearer {token}")).context("invalid MASQUE token header")?;
    auth.set_sensitive(true);
    Ok(Some(auth))
}
