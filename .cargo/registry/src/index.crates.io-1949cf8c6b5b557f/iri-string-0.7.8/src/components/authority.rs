//! Subcomponents of authority.

use crate::parser::trusted as trusted_parser;
use crate::spec::Spec;
use crate::types::RiReferenceStr;

/// Subcomponents of authority.
///
/// This is a return type of the `authority_components` method of the string
/// types (for example [`RiStr::authority_components`].
///
/// [`RiStr::authority_components`]: `crate::types::RiStr::authority_components`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthorityComponents<'a> {
    /// Authority string, excluding the leading `//`.
    pub(crate) authority: &'a str,
    /// Start position of the `host`.
    pub(crate) host_start: usize,
    /// End position of the `host`.
    pub(crate) host_end: usize,
}

impl<'a> AuthorityComponents<'a> {
    /// Creates a new `AuthorityComponents` from the IRI.
    pub fn from_iri<S: Spec>(iri: &'a RiReferenceStr<S>) -> Option<Self> {
        iri.authority_str()
            .map(trusted_parser::authority::decompose_authority)
    }

    /// Returns the `userinfo` part, excluding the following `@`.
    #[must_use]
    pub fn userinfo(&self) -> Option<&'a str> {
        let userinfo_at = self.host_start.checked_sub(1)?;
        debug_assert_eq!(self.authority.as_bytes()[userinfo_at], b'@');
        Some(&self.authority[..userinfo_at])
    }

    /// Returns the `host` part.
    #[inline]
    #[must_use]
    pub fn host(&self) -> &'a str {
        // NOTE: RFC 6874 support may need the internal logic to change.
        &self.authority[self.host_start..self.host_end]
    }

    /// Returns the `port` part, excluding the following `:`.
    #[must_use]
    pub fn port(&self) -> Option<&'a str> {
        if self.host_end == self.authority.len() {
            return None;
        }
        let port_colon = self.host_end;
        debug_assert_eq!(self.authority.as_bytes()[port_colon], b':');
        Some(&self.authority[(port_colon + 1)..])
    }
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
    use super::*;

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    use alloc::string::String;

    use crate::types::IriReferenceStr;

    const USERINFO: &[&str] = &["", "user:password", "user"];

    const PORT: &[&str] = &[
        "",
        "0",
        "0000",
        "80",
        "1234567890123456789012345678901234567890",
    ];

    const HOST: &[&str] = &[
        "",
        "localhost",
        "example.com",
        "192.0.2.0",
        "[2001:db8::1]",
        "[2001:0db8:0:0:0:0:0:1]",
        "[2001:0db8::192.0.2.255]",
        "[v9999.this-is-futuristic-ip-address]",
    ];

    fn compose_to_relative_iri(userinfo: Option<&str>, host: &str, port: Option<&str>) -> String {
        let mut buf = String::from("//");
        if let Some(userinfo) = userinfo {
            buf.push_str(userinfo);
            buf.push('@');
        }
        buf.push_str(host);
        if let Some(port) = port {
            buf.push(':');
            buf.push_str(port);
        }
        buf
    }

    #[test]
    fn test_decompose_authority() {
        for host in HOST.iter().copied() {
            for userinfo in USERINFO.iter().map(|s| Some(*s)).chain(None) {
                for port in PORT.iter().map(|s| Some(*s)).chain(None) {
                    let authority = compose_to_relative_iri(userinfo, host, port);
                    let authority =
                        IriReferenceStr::new(&authority).expect("test case should be valid");
                    let components = AuthorityComponents::from_iri(authority)
                        .expect("relative path composed for this test should contain authority");

                    assert_eq!(components.host(), host);
                    assert_eq!(components.userinfo(), userinfo);
                    assert_eq!(components.port(), port);
                }
            }
        }
    }
}
