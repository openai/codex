// Copyright 2016 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Getters and setters for URL components implemented per <https://url.spec.whatwg.org/#api>
//!
//! Unless you need to be interoperable with web browsers,
//! you probably want to use `Url` method instead.

use crate::parser::{default_port, Context, Input, Parser, SchemeType};
use crate::{Host, ParseError, Position, Url};
use alloc::string::String;
use alloc::string::ToString;

/// Internal components / offsets of a URL.
///
/// https://user@pass:example.com:1234/foo/bar?baz#quux
///      |      |    |          | ^^^^|       |   |
///      |      |    |          | |   |       |   `----- fragment_start
///      |      |    |          | |   |       `--------- query_start
///      |      |    |          | |   `----------------- path_start
///      |      |    |          | `--------------------- port
///      |      |    |          `----------------------- host_end
///      |      |    `---------------------------------- host_start
///      |      `--------------------------------------- username_end
///      `---------------------------------------------- scheme_end
#[derive(Copy, Clone)]
#[cfg(feature = "expose_internals")]
pub struct InternalComponents {
    pub scheme_end: u32,
    pub username_end: u32,
    pub host_start: u32,
    pub host_end: u32,
    pub port: Option<u16>,
    pub path_start: u32,
    pub query_start: Option<u32>,
    pub fragment_start: Option<u32>,
}

/// Internal component / parsed offsets of the URL.
///
/// This can be useful for implementing efficient serialization
/// for the URL.
#[cfg(feature = "expose_internals")]
pub fn internal_components(url: &Url) -> InternalComponents {
    InternalComponents {
        scheme_end: url.scheme_end,
        username_end: url.username_end,
        host_start: url.host_start,
        host_end: url.host_end,
        port: url.port,
        path_start: url.path_start,
        query_start: url.query_start,
        fragment_start: url.fragment_start,
    }
}

/// <https://url.spec.whatwg.org/#dom-url-domaintoascii>
pub fn domain_to_ascii(domain: &str) -> String {
    match Host::parse(domain) {
        Ok(Host::Domain(domain)) => domain,
        _ => String::new(),
    }
}

/// <https://url.spec.whatwg.org/#dom-url-domaintounicode>
pub fn domain_to_unicode(domain: &str) -> String {
    match Host::parse(domain) {
        Ok(Host::Domain(ref domain)) => {
            let (unicode, _errors) = idna::domain_to_unicode(domain);
            unicode
        }
        _ => String::new(),
    }
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-href>
pub fn href(url: &Url) -> &str {
    url.as_str()
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-href>
pub fn set_href(url: &mut Url, value: &str) -> Result<(), ParseError> {
    *url = Url::parse(value)?;
    Ok(())
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-origin>
pub fn origin(url: &Url) -> String {
    url.origin().ascii_serialization()
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-protocol>
#[inline]
pub fn protocol(url: &Url) -> &str {
    &url.as_str()[..url.scheme().len() + ":".len()]
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-protocol>
#[allow(clippy::result_unit_err)]
pub fn set_protocol(url: &mut Url, mut new_protocol: &str) -> Result<(), ()> {
    // The scheme state in the spec ignores everything after the first `:`,
    // but `set_scheme` errors if there is more.
    if let Some(position) = new_protocol.find(':') {
        new_protocol = &new_protocol[..position];
    }
    url.set_scheme(new_protocol)
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-username>
#[inline]
pub fn username(url: &Url) -> &str {
    url.username()
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-username>
#[allow(clippy::result_unit_err)]
pub fn set_username(url: &mut Url, new_username: &str) -> Result<(), ()> {
    url.set_username(new_username)
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-password>
#[inline]
pub fn password(url: &Url) -> &str {
    url.password().unwrap_or("")
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-password>
#[allow(clippy::result_unit_err)]
pub fn set_password(url: &mut Url, new_password: &str) -> Result<(), ()> {
    url.set_password(if new_password.is_empty() {
        None
    } else {
        Some(new_password)
    })
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-host>
#[inline]
pub fn host(url: &Url) -> &str {
    &url[Position::BeforeHost..Position::AfterPort]
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-host>
#[allow(clippy::result_unit_err)]
pub fn set_host(url: &mut Url, new_host: &str) -> Result<(), ()> {
    // If context object’s url’s cannot-be-a-base-URL flag is set, then return.
    if url.cannot_be_a_base() {
        return Err(());
    }
    // Host parsing rules are strict,
    // We don't want to trim the input
    let input = Input::new_no_trim(new_host);
    let host;
    let opt_port;
    {
        let scheme = url.scheme();
        let scheme_type = SchemeType::from(scheme);
        if scheme_type == SchemeType::File && new_host.is_empty() {
            url.set_host_internal(Host::Domain(String::new()), None);
            return Ok(());
        }

        if let Ok((h, remaining)) = Parser::parse_host(input, scheme_type) {
            host = h;
            opt_port = if let Some(remaining) = remaining.split_prefix(':') {
                if remaining.is_empty() {
                    None
                } else {
                    Parser::parse_port(remaining, || default_port(scheme), Context::Setter)
                        .ok()
                        .map(|(port, _remaining)| port)
                }
            } else {
                None
            };
        } else {
            return Err(());
        }
    }
    // Make sure we won't set an empty host to a url with a username or a port
    if host == Host::Domain("".to_string())
        && (!username(url).is_empty() || matches!(opt_port, Some(Some(_))) || url.port().is_some())
    {
        return Err(());
    }
    url.set_host_internal(host, opt_port);
    Ok(())
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-hostname>
#[inline]
pub fn hostname(url: &Url) -> &str {
    url.host_str().unwrap_or("")
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-hostname>
#[allow(clippy::result_unit_err)]
pub fn set_hostname(url: &mut Url, new_hostname: &str) -> Result<(), ()> {
    if url.cannot_be_a_base() {
        return Err(());
    }
    // Host parsing rules are strict we don't want to trim the input
    let input = Input::new_no_trim(new_hostname);
    let scheme_type = SchemeType::from(url.scheme());
    if scheme_type == SchemeType::File && new_hostname.is_empty() {
        url.set_host_internal(Host::Domain(String::new()), None);
        return Ok(());
    }

    if let Ok((host, _remaining)) = Parser::parse_host(input, scheme_type) {
        if let Host::Domain(h) = &host {
            if h.is_empty() {
                // Empty host on special not file url
                if SchemeType::from(url.scheme()) == SchemeType::SpecialNotFile
                    // Port with an empty host
                    ||!port(url).is_empty()
                    // Empty host that includes credentials
                    || !url.username().is_empty()
                    || !url.password().unwrap_or("").is_empty()
                {
                    return Err(());
                }
            }
        }
        url.set_host_internal(host, None);
        Ok(())
    } else {
        Err(())
    }
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-port>
#[inline]
pub fn port(url: &Url) -> &str {
    &url[Position::BeforePort..Position::AfterPort]
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-port>
#[allow(clippy::result_unit_err)]
pub fn set_port(url: &mut Url, new_port: &str) -> Result<(), ()> {
    let result;
    {
        // has_host implies !cannot_be_a_base
        let scheme = url.scheme();
        if !url.has_host() || url.host() == Some(Host::Domain("")) || scheme == "file" {
            return Err(());
        }
        result = Parser::parse_port(
            Input::new_no_trim(new_port),
            || default_port(scheme),
            Context::Setter,
        )
    }
    if let Ok((new_port, _remaining)) = result {
        url.set_port_internal(new_port);
        Ok(())
    } else {
        Err(())
    }
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-pathname>
#[inline]
pub fn pathname(url: &Url) -> &str {
    url.path()
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-pathname>
pub fn set_pathname(url: &mut Url, new_pathname: &str) {
    if url.cannot_be_a_base() {
        return;
    }
    if new_pathname.starts_with('/')
        || (SchemeType::from(url.scheme()).is_special()
            // \ is a segment delimiter for 'special' URLs"
            && new_pathname.starts_with('\\'))
    {
        url.set_path(new_pathname)
    } else if SchemeType::from(url.scheme()).is_special()
        || !new_pathname.is_empty()
        || !url.has_host()
    {
        let mut path_to_set = String::from("/");
        path_to_set.push_str(new_pathname);
        url.set_path(&path_to_set)
    } else {
        url.set_path(new_pathname)
    }
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-search>
pub fn search(url: &Url) -> &str {
    trim(&url[Position::AfterPath..Position::AfterQuery])
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-search>
pub fn set_search(url: &mut Url, new_search: &str) {
    url.set_query(match new_search {
        "" => None,
        _ if new_search.starts_with('?') => Some(&new_search[1..]),
        _ => Some(new_search),
    })
}

/// Getter for <https://url.spec.whatwg.org/#dom-url-hash>
pub fn hash(url: &Url) -> &str {
    trim(&url[Position::AfterQuery..])
}

/// Setter for <https://url.spec.whatwg.org/#dom-url-hash>
pub fn set_hash(url: &mut Url, new_hash: &str) {
    url.set_fragment(match new_hash {
        // If the given value is the empty string,
        // then set context object’s url’s fragment to null and return.
        "" => None,
        // Let input be the given value with a single leading U+0023 (#) removed, if any.
        _ if new_hash.starts_with('#') => Some(&new_hash[1..]),
        _ => Some(new_hash),
    })
}

fn trim(s: &str) -> &str {
    if s.len() == 1 {
        ""
    } else {
        s
    }
}
