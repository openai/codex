//! Components.
#![allow(dead_code)]

use core::fmt;

use iri_string::build::Builder;

/// Test case.
#[derive(Debug, Clone, Copy)]
pub struct TestCase<'a> {
    /// Test case name.
    pub name: Option<&'a str>,
    /// Test case description.
    pub description: Option<&'a str>,
    /// Composed string.
    pub composed: &'a str,
    /// Components.
    pub components: Components<'a>,
    /// Normalized string as URI.
    pub normalized_uri: &'a str,
    /// Normalized string as IRI.
    pub normalized_iri: &'a str,
    /// Normalized (by WHATWG-like algorithm) string as URI.
    pub normalized_uri_whatwg_like: Option<&'a str>,
    /// Normalized (by WHATWG-like algorithm) string as IRI.
    pub normalized_iri_whatwg_like: Option<&'a str>,
    /// Different IRIs.
    pub different_iris: &'a [&'a str],
}

impl TestCase<'_> {
    #[inline]
    #[must_use]
    pub fn is_uri_class(&self) -> bool {
        self.composed.is_ascii()
    }

    #[inline]
    #[must_use]
    pub const fn is_iri_class(&self) -> bool {
        true
    }

    #[inline]
    #[must_use]
    pub const fn is_absolute(&self) -> bool {
        self.components.is_absolute()
    }

    #[inline]
    #[must_use]
    pub const fn is_absolute_without_fragment(&self) -> bool {
        self.components.is_absolute_without_fragment()
    }

    #[inline]
    #[must_use]
    pub const fn is_relative(&self) -> bool {
        self.components.is_relative()
    }

    #[inline]
    #[must_use]
    pub fn is_rfc3986_normalizable(&self) -> bool {
        match self.normalized_iri.find('/') {
            Some(pos) => !self.normalized_iri[(pos + 1)..].starts_with("./"),
            None => true,
        }
    }
}

/// Components.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Components<'a> {
    /// `scheme`.
    pub scheme: Option<&'a str>,
    /// User part (string before the first colon) of `userinfo`.
    ///
    /// Note that `host` should also be `Some(_)` if this is `Some(_)`.
    pub user: Option<&'a str>,
    /// Password part (string after the first colon) of `userinfo`.
    ///
    /// Note that `host` should also be `Some(_)` if this is `Some(_)`.
    pub password: Option<&'a str>,
    /// `host`.
    pub host: Option<&'a str>,
    /// `port`.
    ///
    /// Note that `host` should also be `Some(_)` if this is `Some(_)`.
    pub port: Option<&'a str>,
    /// `path`.
    pub path: &'a str,
    /// `query`.
    pub query: Option<&'a str>,
    /// `fragment`.
    pub fragment: Option<&'a str>,
}

impl<'a> Components<'a> {
    #[inline]
    #[must_use]
    const fn const_default() -> Self {
        Self {
            scheme: None,
            user: None,
            password: None,
            host: None,
            port: None,
            path: "",
            query: None,
            fragment: None,
        }
    }

    pub fn feed_builder(&self, builder: &mut Builder<'a>, clean: bool) {
        if let Some(scheme) = self.scheme {
            builder.scheme(scheme);
        } else if clean {
            builder.unset_scheme();
        }

        if let Some(host) = self.host {
            if self.user.is_some() || self.password.is_some() {
                builder.userinfo((self.user.unwrap_or(""), self.password));
            } else if clean {
                builder.unset_userinfo();
            }

            builder.host(host);

            if let Some(port) = self.port {
                builder.port(port);
            } else if clean {
                builder.unset_port();
            }
        } else if clean {
            builder.unset_authority();
        }

        builder.path(self.path);

        if let Some(query) = self.query {
            builder.query(query);
        } else if clean {
            builder.unset_query();
        }

        if let Some(fragment) = self.fragment {
            builder.fragment(fragment);
        } else if clean {
            builder.unset_fragment();
        }
    }

    #[inline]
    #[must_use]
    pub const fn is_absolute(&self) -> bool {
        self.scheme.is_some()
    }

    #[inline]
    #[must_use]
    pub const fn is_absolute_without_fragment(&self) -> bool {
        self.scheme.is_some() && self.fragment.is_none()
    }

    #[inline]
    #[must_use]
    pub const fn is_relative(&self) -> bool {
        self.scheme.is_none()
    }
}

impl fmt::Display for Components<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(scheme) = self.scheme {
            write!(f, "{scheme}:")?;
        }

        assert!(
            self.host.is_some()
                || (self.user.is_none() && self.password.is_none() && self.port.is_none()),
            "`user`, `password`, and `port` requires `host` to be present"
        );
        if let Some(host) = self.host {
            if let Some(user) = self.user {
                f.write_str(user)?;
            }
            if let Some(password) = self.password {
                write!(f, ":{password}")?;
            }
            if self.user.is_some() || self.password.is_some() {
                write!(f, "@")?;
            }
            f.write_str(host)?;
            if let Some(port) = self.port {
                write!(f, ":{port}")?;
            }
        }

        f.write_str(self.path)?;

        if let Some(query) = self.query {
            write!(f, "#{query}")?;
        }

        if let Some(fragment) = self.fragment {
            write!(f, "#{fragment}")?;
        }

        Ok(())
    }
}

macro_rules! components {
    () => {
        Components::default()
    };
    ($($field:ident: $expr:expr),* $(,)?) => {
        Components {
            $( $field: components!(@field; $field: $expr) ),*,
            .. Components::const_default()
        }
    };
    (@field; path: $expr:expr) => {
        $expr
    };
    (@field; $field:ident: None) => {
        None
    };
    (@field; $field:ident: $expr:expr) => {
        Some($expr)
    };
}

macro_rules! test_case {
    // Name.
    (@field=name; name: $value:expr, $($rest:tt)*) => {
        $value
    };
    // Description.
    (@field=description; description: $value:expr, $($rest:tt)*) => {
        Some($value)
    };
    (@field=description;) => {
        None
    };
    // Composed.
    (@field=composed; composed: $value:expr, $($rest:tt)*) => {
        $value
    };
    // Components.
    (@field=components; components: { $($toks:tt)* }, $($rest:tt)*) => {
        components! { $($toks)* }
    };
    // Normalized URI.
    (@field=normalized_uri; normalized_uri: $value:expr, $($rest:tt)*) => {
        $value
    };
    // Normalized IRI.
    (@field=normalized_iri; normalized_iri: $value:expr, $($rest:tt)*) => {
        $value
    };
    // Normalized URI (WHATWG-like).
    (@field=normalized_uri_whatwg_like; normalized_uri_whatwg_like: $value:expr, $($rest:tt)*) => {
        Some($value)
    };
    (@field=normalized_uri_whatwg_like;) => {
        None
    };
    // Normalized IRI (WHATWG-like).
    (@field=normalized_iri_whatwg_like; normalized_iri_whatwg_like: $value:expr, $($rest:tt)*) => {
        Some($value)
    };
    (@field=normalized_iri_whatwg_like;) => {
        None
    };
    // Different IRIs.
    (@field=different_iris; different_iris: $value:expr, $($rest:tt)*) => {
        $value
    };
    (@field=different_iris;) => {
        &[]
    };
    // Fallback.
    (@field=$name:ident; $field:ident: { $($toks:tt)* }, $($rest:tt)*) => {
        test_case!(@field=$name; $($rest)*)
    };
    // Fallback.
    (@field=$name:ident; $field:ident: $value:expr, $($rest:tt)*) => {
        test_case!(@field=$name; $($rest)*)
    };
    ($($args:tt)*) => {
        TestCase {
            name: Some(test_case!(@field=name; $($args)*)),
            description: test_case!(@field=description; $($args)*),
            composed: test_case!(@field=composed; $($args)*),
            components: test_case!(@field=components; $($args)*),
            normalized_uri: test_case!(@field=normalized_uri; $($args)*),
            normalized_iri: test_case!(@field=normalized_iri; $($args)*),
            normalized_uri_whatwg_like: test_case!(@field=normalized_uri_whatwg_like; $($args)*),
            normalized_iri_whatwg_like: test_case!(@field=normalized_iri_whatwg_like; $($args)*),
            different_iris: test_case!(@field=different_iris; $($args)*),
        }
    };
}

macro_rules! test_cases {
    ($({$($toks:tt)*}),* $(,)?) => {
        &[ $( test_case! { $($toks)* } ),* ]
    }
}

#[allow(clippy::needless_update)] // For `components!` macro.
pub static TEST_CASES: &[TestCase<'static>] = test_cases![
    {
        name: "typical example URI",
        composed: "http://example.com/",
        components: {
            scheme: "http",
            host: "example.com",
            path: "/",
        },
        normalized_uri: "http://example.com/",
        normalized_iri: "http://example.com/",
    },
    {
        name: "typical example URI with user and password",
        composed: "http://user:password@example.com/",
        components: {
            scheme: "http",
            user: "user",
            password: "password",
            host: "example.com",
            path: "/",
        },
        normalized_uri: "http://user:password@example.com/",
        normalized_iri: "http://user:password@example.com/",
    },
    {
        name: "URI with ASCII-only hostname with capital letters",
        description: "ASCII-only hostname should be normalized to lower letters",
        composed: "http://usER:passWORD@eXAMPLe.CoM/",
        components: {
            scheme: "http",
            user: "usER",
            password: "passWORD",
            host: "eXAMPLe.CoM",
            path: "/",
        },
        normalized_uri: "http://usER:passWORD@example.com/",
        normalized_iri: "http://usER:passWORD@example.com/",
    },
    {
        name: "IRI with non-ASCII hostname with capital letters",
        description: "hostname with non-ASCII characters should not be normalized to lower letters",
        composed: "http://usER:passWORD@\u{03B1}.CoM/",
        components: {
            scheme: "http",
            user: "usER",
            password: "passWORD",
            host: "\u{03B1}.CoM",
            path: "/",
        },
        // The RFC 3986 (not 3987) spec is ambiguous: if the host contains percent-encoded
        // non-ASCII characters, should other part of the host be lowercased?
        // In this crate for now, the operations is implemented based on RFC 3987, i.e.
        // even URI type internally checks whether the percent-encoded characters
        // would be decoded to ASCII or not.
        normalized_uri: "http://usER:passWORD@%CE%B1.CoM/",
        normalized_iri: "http://usER:passWORD@\u{03B1}.CoM/",
    },
    {
        name: "URI with all components set",
        composed: "http://user:password@example.com:80/path/to/somewhere?query#fragment",
        components: {
            scheme: "http",
            user: "user",
            password: "password",
            host: "example.com",
            port: "80",
            path: "/path/to/somewhere",
            query: "query",
            fragment: "fragment",
        },
        normalized_uri: "http://user:password@example.com:80/path/to/somewhere?query#fragment",
        normalized_iri: "http://user:password@example.com:80/path/to/somewhere?query#fragment",
    },
    {
        name: "URI that cannot be normalized by pure RFC 3986",
        composed: "scheme:/.//not-a-host",
        components: {
            scheme: "scheme",
            path: "/.//not-a-host",
        },
        normalized_uri: "scheme:/.//not-a-host",
        normalized_iri: "scheme:/.//not-a-host",
    },
    {
        name: "URI that cannot be normalized by pure RFC 3986",
        composed: "scheme:..///not-a-host",
        components: {
            scheme: "scheme",
            path: "..///not-a-host",
        },
        normalized_uri: "scheme:/.//not-a-host",
        normalized_iri: "scheme:/.//not-a-host",
        normalized_uri_whatwg_like: "scheme:..///not-a-host",
        normalized_iri_whatwg_like: "scheme:..///not-a-host",
    },
    {
        name: "Relative URI reference as a relative path `..`",
        description: "Relative path without scheme and authority should not be normalized",
        composed: "..",
        components: {
            path: "..",
        },
        normalized_uri: "..",
        normalized_iri: "..",
    },
    {
        name: "Relative URI reference as a relative path",
        description: "Relative path without scheme and authority should not be normalized",
        composed: "../foo/..",
        components: {
            path: "../foo/..",
        },
        normalized_uri: "../foo/..",
        normalized_iri: "../foo/..",
    },
    {
        name: "Relative URI reference as a relative path",
        description: "Relative path without scheme and authority should not be normalized",
        composed: "foo/../p%61th",
        components: {
            path: "foo/../p%61th",
        },
        normalized_uri: "foo/../path",
        normalized_iri: "foo/../path",
    },
    {
        name: "Relative path in an absolute URI",
        composed: "scheme:foo/../p%61th",
        components: {
            scheme: "scheme",
            path: "foo/../p%61th",
        },
        normalized_uri: "scheme:/path",
        normalized_iri: "scheme:/path",
        normalized_uri_whatwg_like: "scheme:foo/../path",
        normalized_iri_whatwg_like: "scheme:foo/../path",
    },
    {
        name: "Non-normalized URI",
        composed: "HTTPs://EXaMPLE.COM/pA/Th?Query#Frag",
        components: {
            scheme: "HTTPs",
            host: "EXaMPLE.COM",
            path: "/pA/Th",
            query: "Query",
            fragment: "Frag",
        },
        normalized_uri: "https://example.com/pA/Th?Query#Frag",
        normalized_iri: "https://example.com/pA/Th?Query#Frag",
        different_iris: &[
            "https://example.com/pa/th?Query#Frag",
            "https://example.com/pA/Th?query#Frag",
            "https://example.com/pA/Th?Query#frag",
        ],
    },
    {
        name: "UUID URN",
        composed: "urn:uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        components: {
            scheme: "urn",
            path: "uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        },
        normalized_uri: "urn:uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        normalized_iri: "urn:uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        different_iris: &[
            "urn:UUID:7f1450df-6678-465b-a881-188f9b6ec822",
            "urn:uuid:7F1450DF-6678-465B-A881-188F9B6EC822",
        ],
    },
    {
        name: "UUID URN",
        composed: "URN:uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        components: {
            scheme: "URN",
            path: "uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        },
        normalized_uri: "urn:uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        normalized_iri: "urn:uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        different_iris: &[
            "urn:UUID:7f1450df-6678-465b-a881-188f9b6ec822",
            "urn:uuid:7F1450DF-6678-465B-A881-188F9B6EC822",
        ],
    },
    {
        name: "UUID URN",
        composed: "URN:uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        components: {
            scheme: "URN",
            path: "uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        },
        normalized_uri: "urn:uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        normalized_iri: "urn:uuid:7f1450df-6678-465b-a881-188f9b6ec822",
        different_iris: &[
            "urn:UUID:7f1450df-6678-465b-a881-188f9b6ec822",
            "urn:uuid:7F1450DF-6678-465B-A881-188F9B6EC822",
        ],
    },
    {
        name: "IRI with percent-encoded unreserved characters and non-valid UTF-8 bytes",
        composed: "http://example.com/?a=%CE%B1&b=%CE%CE%B1%B1",
        components: {
            scheme: "http",
            host: "example.com",
            path: "/",
            query: "a=%CE%B1&b=%CE%CE%B1%B1",
        },
        normalized_uri: "http://example.com/?a=%CE%B1&b=%CE%CE%B1%B1",
        normalized_iri: "http://example.com/?a=\u{03B1}&b=%CE\u{03B1}%B1",
    },
    {
        name: "not ASCII-only host",
        composed: "SCHEME://Alpha%ce%b1/",
        components: {
            scheme: "SCHEME",
            host: "Alpha%ce%b1",
            path: "/",
        },
        normalized_uri: "scheme://Alpha%CE%B1/",
        normalized_iri: "scheme://Alpha\u{03B1}/",
    },
    {
        name: "URI with percent-encoded unreserevd and reserved characters",
        description: "Tilde character (0x7e) is unreserved and bang (0x21) is reserved",
        composed: "http://example.com/%7E%41%73%63%69%69%21",
        components: {
            scheme: "http",
            host: "example.com",
            path: "/%7E%41%73%63%69%69%21",
        },
        normalized_uri: "http://example.com/~Ascii%21",
        normalized_iri: "http://example.com/~Ascii%21",
    },
    {
        name: "not ASCII-only host",
        description: "Plus character (0x2B) is reserved (sub-delim), so it should not be decoded in host part",
        composed: "SCHEME://PLUS%2bPLUS/",
        components: {
            scheme: "SCHEME",
            host: "PLUS%2bPLUS",
            path: "/",
        },
        normalized_uri: "scheme://plus%2Bplus/",
        normalized_iri: "scheme://plus%2Bplus/",
    },
    {
        name: "empty port",
        // <https://www.rfc-editor.org/rfc/rfc3986.html#section-3.2.3>:
        //
        // > URI producers and normalizers should omit the port component
        // > and its ":" delimiter if port is empty or if its value would
        // > be the same as that of the scheme's default.
        description: "According to RFC 3986 section 3.2.3, empty port should be omitted by normalization",
        composed: "https://example.com:/",
        components: {
            scheme: "https",
            host: "example.com",
            port: "",
            path: "/",
        },
        normalized_uri: "https://example.com/",
        normalized_iri: "https://example.com/",
    },
    {
        name: "URI with a dot-dot segment",
        composed: "http://example.com/a/b/c/%2e%2e/d/e",
        components: {
            scheme: "http",
            host: "example.com",
            path: "/a/b/c/%2e%2e/d/e",
        },
        normalized_uri: "http://example.com/a/b/d/e",
        normalized_iri: "http://example.com/a/b/d/e",
    },
    {
        name: "URI with a dot-dot segment",
        composed: "http://example.com/a/b/c/%2E%2E/d/e",
        components: {
            scheme: "http",
            host: "example.com",
            path: "/a/b/c/%2E%2E/d/e",
        },
        normalized_uri: "http://example.com/a/b/d/e",
        normalized_iri: "http://example.com/a/b/d/e",
    },
    {
        name: "URI with a dot-dot segment",
        composed: "http://example.com/a/b/c/../d/e",
        components: {
            scheme: "http",
            host: "example.com",
            path: "/a/b/c/../d/e",
        },
        normalized_uri: "http://example.com/a/b/d/e",
        normalized_iri: "http://example.com/a/b/d/e",
    },
    {
        name: "URI with a dot-dot segment",
        composed: "http://example.com/a/b/c/.%2e/d/e",
        components: {
            scheme: "http",
            host: "example.com",
            path: "/a/b/c/.%2e/d/e",
        },
        normalized_uri: "http://example.com/a/b/d/e",
        normalized_iri: "http://example.com/a/b/d/e",
    },
    {
        name: "URI with dot segments",
        composed: "http://example.com/a/./././././b/c/.%2e/d/e",
        components: {
            scheme: "http",
            host: "example.com",
            path: "/a/./././././b/c/.%2e/d/e",
        },
        normalized_uri: "http://example.com/a/b/d/e",
        normalized_iri: "http://example.com/a/b/d/e",
    },
    // START: Combination.
    {
        name: "Empty relative IRI",
        composed: "",
        components: {
            path: "",
        },
        normalized_uri: "",
        normalized_iri: "",
    },
    {
        name: "Combination: fragment",
        composed: "#fragment",
        components: {
            fragment: "fragment",
        },
        normalized_uri: "#fragment",
        normalized_iri: "#fragment",
    },
    {
        name: "Combination: query",
        composed: "?query",
        components: {
            query: "query",
        },
        normalized_uri: "?query",
        normalized_iri: "?query",
    },
    {
        name: "Combination: query+fragment",
        composed: "?query#fragment",
        components: {
            query: "query",
            fragment: "fragment",
        },
        normalized_uri: "?query#fragment",
        normalized_iri: "?query#fragment",
    },
    {
        name: "Combination: path",
        composed: "/pa/th",
        components: {
            path: "/pa/th",
        },
        normalized_uri: "/pa/th",
        normalized_iri: "/pa/th",
    },
    {
        name: "Combination: path+fragment",
        composed: "/pa/th#fragment",
        components: {
            path: "/pa/th",
            fragment: "fragment",
        },
        normalized_uri: "/pa/th#fragment",
        normalized_iri: "/pa/th#fragment",
    },
    {
        name: "Combination: path+query",
        composed: "/pa/th?query",
        components: {
            path: "/pa/th",
            query: "query",
        },
        normalized_uri: "/pa/th?query",
        normalized_iri: "/pa/th?query",
    },
    {
        name: "Combination: path+query+fragment",
        composed: "/pa/th?query#fragment",
        components: {
            path: "/pa/th",
            query: "query",
            fragment: "fragment",
        },
        normalized_uri: "/pa/th?query#fragment",
        normalized_iri: "/pa/th?query#fragment",
    },
    {
        name: "Combination: authority",
        composed: "//authority",
        components: {
            host: "authority",
        },
        normalized_uri: "//authority",
        normalized_iri: "//authority",
    },
    {
        name: "Combination: authority+fragment",
        composed: "//authority#fragment",
        components: {
            host: "authority",
            fragment: "fragment",
        },
        normalized_uri: "//authority#fragment",
        normalized_iri: "//authority#fragment",
    },
    {
        name: "Combination: authority+query",
        composed: "//authority?query",
        components: {
            host: "authority",
            query: "query",
        },
        normalized_uri: "//authority?query",
        normalized_iri: "//authority?query",
    },
    {
        name: "Combination: authority+query+fragment",
        composed: "//authority?query#fragment",
        components: {
            host: "authority",
            query: "query",
            fragment: "fragment",
        },
        normalized_uri: "//authority?query#fragment",
        normalized_iri: "//authority?query#fragment",
    },
    {
        name: "Combination: authority+path",
        composed: "//authority/pa/th",
        components: {
            host: "authority",
            path: "/pa/th",
        },
        normalized_uri: "//authority/pa/th",
        normalized_iri: "//authority/pa/th",
    },
    {
        name: "Combination: authority+path+fragment",
        composed: "//authority/pa/th#fragment",
        components: {
            host: "authority",
            path: "/pa/th",
            fragment: "fragment",
        },
        normalized_uri: "//authority/pa/th#fragment",
        normalized_iri: "//authority/pa/th#fragment",
    },
    {
        name: "Combination: authority+path+query",
        composed: "//authority/pa/th?query",
        components: {
            host: "authority",
            path: "/pa/th",
            query: "query",
        },
        normalized_uri: "//authority/pa/th?query",
        normalized_iri: "//authority/pa/th?query",
    },
    {
        name: "Combination: authority+path+query+fragment",
        composed: "//authority/pa/th?query#fragment",
        components: {
            host: "authority",
            path: "/pa/th",
            query: "query",
            fragment: "fragment",
        },
        normalized_uri: "//authority/pa/th?query#fragment",
        normalized_iri: "//authority/pa/th?query#fragment",
    },
    {
        name: "Combination: scheme",
        composed: "scheme:",
        components: {
            scheme: "scheme",
        },
        normalized_uri: "scheme:",
        normalized_iri: "scheme:",
    },
    {
        name: "Combination: scheme+fragment",
        composed: "scheme:#fragment",
        components: {
            scheme: "scheme",
            fragment: "fragment",
        },
        normalized_uri: "scheme:#fragment",
        normalized_iri: "scheme:#fragment",
    },
    {
        name: "Combination: scheme+query",
        composed: "scheme:?query",
        components: {
            scheme: "scheme",
            query: "query",
        },
        normalized_uri: "scheme:?query",
        normalized_iri: "scheme:?query",
    },
    {
        name: "Combination: scheme+query+fragment",
        composed: "scheme:?query#fragment",
        components: {
            scheme: "scheme",
            query: "query",
            fragment: "fragment",
        },
        normalized_uri: "scheme:?query#fragment",
        normalized_iri: "scheme:?query#fragment",
    },
    {
        name: "Combination: scheme+path",
        composed: "scheme:/pa/th",
        components: {
            scheme: "scheme",
            path: "/pa/th",
        },
        normalized_uri: "scheme:/pa/th",
        normalized_iri: "scheme:/pa/th",
    },
    {
        name: "Combination: scheme+path+fragment",
        composed: "scheme:/pa/th#fragment",
        components: {
            scheme: "scheme",
            path: "/pa/th",
            fragment: "fragment",
        },
        normalized_uri: "scheme:/pa/th#fragment",
        normalized_iri: "scheme:/pa/th#fragment",
    },
    {
        name: "Combination: scheme+path+query",
        composed: "scheme:/pa/th?query",
        components: {
            scheme: "scheme",
            path: "/pa/th",
            query: "query",
        },
        normalized_uri: "scheme:/pa/th?query",
        normalized_iri: "scheme:/pa/th?query",
    },
    {
        name: "Combination: scheme+path+query+fragment",
        composed: "scheme:/pa/th?query#fragment",
        components: {
            scheme: "scheme",
            path: "/pa/th",
            query: "query",
            fragment: "fragment",
        },
        normalized_uri: "scheme:/pa/th?query#fragment",
        normalized_iri: "scheme:/pa/th?query#fragment",
    },
    {
        name: "Combination: scheme+authority",
        composed: "scheme://authority",
        components: {
            scheme: "scheme",
            host: "authority",
        },
        normalized_uri: "scheme://authority",
        normalized_iri: "scheme://authority",
    },
    {
        name: "Combination: scheme+authority+fragment",
        composed: "scheme://authority#fragment",
        components: {
            scheme: "scheme",
            host: "authority",
            fragment: "fragment",
        },
        normalized_uri: "scheme://authority#fragment",
        normalized_iri: "scheme://authority#fragment",
    },
    {
        name: "Combination: scheme+authority+query",
        composed: "scheme://authority?query",
        components: {
            scheme: "scheme",
            host: "authority",
            query: "query",
        },
        normalized_uri: "scheme://authority?query",
        normalized_iri: "scheme://authority?query",
    },
    {
        name: "Combination: scheme+authority+query+fragment",
        composed: "scheme://authority?query#fragment",
        components: {
            scheme: "scheme",
            host: "authority",
            query: "query",
            fragment: "fragment",
        },
        normalized_uri: "scheme://authority?query#fragment",
        normalized_iri: "scheme://authority?query#fragment",
    },
    {
        name: "Combination: scheme+authority+path",
        composed: "scheme://authority/pa/th",
        components: {
            scheme: "scheme",
            host: "authority",
            path: "/pa/th",
        },
        normalized_uri: "scheme://authority/pa/th",
        normalized_iri: "scheme://authority/pa/th",
    },
    {
        name: "Combination: scheme+authority+path+fragment",
        composed: "scheme://authority/pa/th#fragment",
        components: {
            scheme: "scheme",
            host: "authority",
            path: "/pa/th",
            fragment: "fragment",
        },
        normalized_uri: "scheme://authority/pa/th#fragment",
        normalized_iri: "scheme://authority/pa/th#fragment",
    },
    {
        name: "Combination: scheme+authority+path+query",
        composed: "scheme://authority/pa/th?query",
        components: {
            scheme: "scheme",
            host: "authority",
            path: "/pa/th",
            query: "query",
        },
        normalized_uri: "scheme://authority/pa/th?query",
        normalized_iri: "scheme://authority/pa/th?query",
    },
    {
        name: "Combination: scheme+authority+path+query+fragment",
        composed: "scheme://authority/pa/th?query#fragment",
        components: {
            scheme: "scheme",
            host: "authority",
            path: "/pa/th",
            query: "query",
            fragment: "fragment",
        },
        normalized_uri: "scheme://authority/pa/th?query#fragment",
        normalized_iri: "scheme://authority/pa/th?query#fragment",
    },
    // END: Combination.
    {
        name: "1 slash following to the scheme",
        composed: "scheme:/",
        components: {
            scheme: "scheme",
            path: "/",
        },
        normalized_uri: "scheme:/",
        normalized_iri: "scheme:/",
    },
    {
        name: "2 slashes following to the scheme",
        composed: "scheme://",
        components: {
            scheme: "scheme",
            host: "",
        },
        normalized_uri: "scheme://",
        normalized_iri: "scheme://",
    },
    {
        name: "3 slashes following to the scheme",
        composed: "scheme:///",
        components: {
            scheme: "scheme",
            host: "",
            path: "/",
        },
        normalized_uri: "scheme:///",
        normalized_iri: "scheme:///",
    },
    {
        name: "4 slashes following to the scheme",
        composed: "scheme:////",
        components: {
            scheme: "scheme",
            host: "",
            path: "//",
        },
        normalized_uri: "scheme:////",
        normalized_iri: "scheme:////",
    },
    {
        name: "5 slashes following to the scheme",
        composed: "scheme://///",
        components: {
            scheme: "scheme",
            host: "",
            path: "///",
        },
        normalized_uri: "scheme://///",
        normalized_iri: "scheme://///",
    },
    {
        name: "1 slash",
        composed: "/",
        components: {
            path: "/",
        },
        normalized_uri: "/",
        normalized_iri: "/",
    },
    {
        name: "2 slash",
        composed: "//",
        components: {
            host: "",
        },
        normalized_uri: "//",
        normalized_iri: "//",
    },
    {
        name: "3 slash",
        composed: "///",
        components: {
            host: "",
            path: "/",
        },
        normalized_uri: "///",
        normalized_iri: "///",
    },
    {
        name: "4 slash",
        composed: "////",
        components: {
            host: "",
            path: "//",
        },
        normalized_uri: "////",
        normalized_iri: "////",
    },
    {
        name: "5 slash",
        composed: "/////",
        components: {
            host: "",
            path: "///",
        },
        normalized_uri: "/////",
        normalized_iri: "/////",
    },
    {
        name: "IPv4 address",
        composed: "//192.0.2.0",
        components: {
            host: "192.0.2.0",
        },
        normalized_uri: "//192.0.2.0",
        normalized_iri: "//192.0.2.0",
    },
    {
        name: "IPv4 address with port",
        composed: "//192.0.2.0:80",
        components: {
            host: "192.0.2.0",
            port: "80",
        },
        normalized_uri: "//192.0.2.0:80",
        normalized_iri: "//192.0.2.0:80",
    },
    {
        name: "IPv4 address",
        composed: "//255.255.255.255",
        components: {
            host: "255.255.255.255",
        },
        normalized_uri: "//255.255.255.255",
        normalized_iri: "//255.255.255.255",
    },
    {
        name: "IPv4 address with port",
        composed: "//255.255.255.255:65536",
        components: {
            host: "255.255.255.255",
            port: "65536",
        },
        normalized_uri: "//255.255.255.255:65536",
        normalized_iri: "//255.255.255.255:65536",
    },
    {
        name: "IPv4 address",
        composed: "//0.0.0.0",
        components: {
            host: "0.0.0.0",
        },
        normalized_uri: "//0.0.0.0",
        normalized_iri: "//0.0.0.0",
    },
    {
        name: "IPv4 address with port",
        composed: "//0.0.0.0:0",
        components: {
            host: "0.0.0.0",
            port: "0",
        },
        normalized_uri: "//0.0.0.0:0",
        normalized_iri: "//0.0.0.0:0",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:db8::]",
        components: {
            host: "[2001:db8::]",
        },
        normalized_uri: "//[2001:db8::]",
        normalized_iri: "//[2001:db8::]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:db8::]:80",
        components: {
            host: "[2001:db8::]",
            port: "80",
        },
        normalized_uri: "//[2001:db8::]:80",
        normalized_iri: "//[2001:db8::]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8::]",
        components: {
            host: "[2001:0db8::]",
        },
        normalized_uri: "//[2001:0db8::]",
        normalized_iri: "//[2001:0db8::]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8::]:80",
        components: {
            host: "[2001:0db8::]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8::]:80",
        normalized_iri: "//[2001:0db8::]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8:0:0:0:0:0:ffff]",
        components: {
            host: "[2001:0db8:0:0:0:0:0:ffff]",
        },
        normalized_uri: "//[2001:0db8:0:0:0:0:0:ffff]",
        normalized_iri: "//[2001:0db8:0:0:0:0:0:ffff]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8:0:0:0:0:0:ffff]:80",
        components: {
            host: "[2001:0db8:0:0:0:0:0:ffff]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8:0:0:0:0:0:ffff]:80",
        normalized_iri: "//[2001:0db8:0:0:0:0:0:ffff]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0DB8:0000:0000:0000:000A:BCDE:FFFF]",
        components: {
            host: "[2001:0DB8:0000:0000:0000:000A:BCDE:FFFF]",
        },
        normalized_uri: "//[2001:0db8:0000:0000:0000:000a:bcde:ffff]",
        normalized_iri: "//[2001:0db8:0000:0000:0000:000a:bcde:ffff]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0DB8:0000:0000:0000:000A:BCDE:FFFF]:80",
        components: {
            host: "[2001:0DB8:0000:0000:0000:000A:BCDE:FFFF]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8:0000:0000:0000:000a:bcde:ffff]:80",
        normalized_iri: "//[2001:0db8:0000:0000:0000:000a:bcde:ffff]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8::]",
        components: {
            host: "[2001:0db8::]",
        },
        normalized_uri: "//[2001:0db8::]",
        normalized_iri: "//[2001:0db8::]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8::]:80",
        components: {
            host: "[2001:0db8::]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8::]:80",
        normalized_iri: "//[2001:0db8::]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0DB8:0:0:0:0::1]",
        components: {
            host: "[2001:0DB8:0:0:0:0::1]",
        },
        normalized_uri: "//[2001:0db8:0:0:0:0::1]",
        normalized_iri: "//[2001:0db8:0:0:0:0::1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0DB8:0:0:0:0::1]:80",
        components: {
            host: "[2001:0DB8:0:0:0:0::1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8:0:0:0:0::1]:80",
        normalized_iri: "//[2001:0db8:0:0:0:0::1]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8::89ab:cdef:89AB:CDEF]",
        components: {
            host: "[2001:0db8::89ab:cdef:89AB:CDEF]",
        },
        normalized_uri: "//[2001:0db8::89ab:cdef:89ab:cdef]",
        normalized_iri: "//[2001:0db8::89ab:cdef:89ab:cdef]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8::89ab:cdef:89AB:CDEF]:80",
        components: {
            host: "[2001:0db8::89ab:cdef:89AB:CDEF]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8::89ab:cdef:89ab:cdef]:80",
        normalized_iri: "//[2001:0db8::89ab:cdef:89ab:cdef]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8::1]",
        components: {
            host: "[2001:0db8::1]",
        },
        normalized_uri: "//[2001:0db8::1]",
        normalized_iri: "//[2001:0db8::1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8::1]:80",
        components: {
            host: "[2001:0db8::1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8::1]:80",
        normalized_iri: "//[2001:0db8::1]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8:0::1]",
        components: {
            host: "[2001:0db8:0::1]",
        },
        normalized_uri: "//[2001:0db8:0::1]",
        normalized_iri: "//[2001:0db8:0::1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8:0::1]:80",
        components: {
            host: "[2001:0db8:0::1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8:0::1]:80",
        normalized_iri: "//[2001:0db8:0::1]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8:0:0::1]",
        components: {
            host: "[2001:0db8:0:0::1]",
        },
        normalized_uri: "//[2001:0db8:0:0::1]",
        normalized_iri: "//[2001:0db8:0:0::1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8:0:0::1]:80",
        components: {
            host: "[2001:0db8:0:0::1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8:0:0::1]:80",
        normalized_iri: "//[2001:0db8:0:0::1]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8:0:0:0::1]",
        components: {
            host: "[2001:0db8:0:0:0::1]",
        },
        normalized_uri: "//[2001:0db8:0:0:0::1]",
        normalized_iri: "//[2001:0db8:0:0:0::1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8:0:0:0::1]:80",
        components: {
            host: "[2001:0db8:0:0:0::1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8:0:0:0::1]:80",
        normalized_iri: "//[2001:0db8:0:0:0::1]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8:0:0:0:0::1]",
        components: {
            host: "[2001:0db8:0:0:0:0::1]",
        },
        normalized_uri: "//[2001:0db8:0:0:0:0::1]",
        normalized_iri: "//[2001:0db8:0:0:0:0::1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8:0:0:0:0::1]:80",
        components: {
            host: "[2001:0db8:0:0:0:0::1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8:0:0:0:0::1]:80",
        normalized_iri: "//[2001:0db8:0:0:0:0::1]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8::0:1]",
        components: {
            host: "[2001:0db8::0:1]",
        },
        normalized_uri: "//[2001:0db8::0:1]",
        normalized_iri: "//[2001:0db8::0:1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8::0:1]:80",
        components: {
            host: "[2001:0db8::0:1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8::0:1]:80",
        normalized_iri: "//[2001:0db8::0:1]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8::0:0:1]",
        components: {
            host: "[2001:0db8::0:0:1]",
        },
        normalized_uri: "//[2001:0db8::0:0:1]",
        normalized_iri: "//[2001:0db8::0:0:1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8::0:0:1]:80",
        components: {
            host: "[2001:0db8::0:0:1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8::0:0:1]:80",
        normalized_iri: "//[2001:0db8::0:0:1]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8::0:0:0:1]",
        components: {
            host: "[2001:0db8::0:0:0:1]",
        },
        normalized_uri: "//[2001:0db8::0:0:0:1]",
        normalized_iri: "//[2001:0db8::0:0:0:1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8::0:0:0:1]:80",
        components: {
            host: "[2001:0db8::0:0:0:1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8::0:0:0:1]:80",
        normalized_iri: "//[2001:0db8::0:0:0:1]:80",
    },
    {
        name: "IPv6 address",
        composed: "//[2001:0db8::0:0:0:0:1]",
        components: {
            host: "[2001:0db8::0:0:0:0:1]",
        },
        normalized_uri: "//[2001:0db8::0:0:0:0:1]",
        normalized_iri: "//[2001:0db8::0:0:0:0:1]",
    },
    {
        name: "IPv6 address with port",
        composed: "//[2001:0db8::0:0:0:0:1]:80",
        components: {
            host: "[2001:0db8::0:0:0:0:1]",
            port: "80",
        },
        normalized_uri: "//[2001:0db8::0:0:0:0:1]:80",
        normalized_iri: "//[2001:0db8::0:0:0:0:1]:80",
    },
    {
        name: "IPvFuture address",
        composed: "//[v9999.this-is-future-version-of-ip-address:::::::::]",
        components: {
            host: "[v9999.this-is-future-version-of-ip-address:::::::::]",
        },
        normalized_uri: "//[v9999.this-is-future-version-of-ip-address:::::::::]",
        normalized_iri: "//[v9999.this-is-future-version-of-ip-address:::::::::]",
    },
    {
        name: "IPvFuture address with port",
        composed: "//[v9999.this-is-future-version-of-ip-address:::::::::]:80",
        components: {
            host: "[v9999.this-is-future-version-of-ip-address:::::::::]",
            port: "80",
        },
        normalized_uri: "//[v9999.this-is-future-version-of-ip-address:::::::::]:80",
        normalized_iri: "//[v9999.this-is-future-version-of-ip-address:::::::::]:80",
    },
    {
        name: "Too large port",
        description: "RFC 3986 accepts `*DIGIT` as `port` component",
        composed: "//localhost:999999999",
        components: {
            host: "localhost",
            port: "999999999",
        },
        normalized_uri: "//localhost:999999999",
        normalized_iri: "//localhost:999999999",
    },
    {
        name: "Port only",
        description: "`host` can be empty",
        composed: "//:999999999",
        components: {
            host: "",
            port: "999999999",
        },
        normalized_uri: "//:999999999",
        normalized_iri: "//:999999999",
    },
    {
        name: "Trailing slash should remain after normalization",
        description: "Trailing slash should remain after normalization if the path ends with slash",
        composed: "https://example.com/../../",
        components: {
            scheme: "https",
            host: "example.com",
            path: "/../../",
        },
        normalized_uri: "https://example.com/",
        normalized_iri: "https://example.com/",
    },
    {
        name: "Slash should remain",
        description: "Slash should remain after normalization if the IRI ends with a dot segment",
        composed: "https://example.com/..",
        components: {
            scheme: "https",
            host: "example.com",
            path: "/..",
        },
        normalized_uri: "https://example.com/",
        normalized_iri: "https://example.com/",
    },
    {
        name: "Slash should remain",
        description: "Slash should remain after normalization if the IRI ends with a dot segment",
        composed: "https://example.com/.",
        components: {
            scheme: "https",
            host: "example.com",
            path: "/.",
        },
        normalized_uri: "https://example.com/",
        normalized_iri: "https://example.com/",
    },
    {
        name: "WHATWG URL Standard serialization",
        composed: "scheme:/a/b/../..//c",
        components: {
            scheme: "scheme",
            path: "/a/b/../..//c",
        },
        normalized_uri: "scheme:/.//c",
        normalized_iri: "scheme:/.//c",
    },
    {
        name: "WHATWG URL Standard serialization",
        composed: "scheme:/a/b/../..//c",
        components: {
            scheme: "scheme",
            path: "/a/b/../..//c",
        },
        normalized_uri: "scheme:/.//c",
        normalized_iri: "scheme:/.//c",
    },
    {
        name: "redundant UTF-8 encoding (1 byte inflated to 2 bytes)",
        composed: "scheme:/%C0%AE",
        components: {
            scheme: "scheme",
            path: "/%C0%AE",
        },
        normalized_uri: "scheme:/%C0%AE",
        normalized_iri: "scheme:/%C0%AE",
    },
    {
        name: "redundant UTF-8 encoding (1 byte inflated to 3 bytes)",
        composed: "scheme:/%E0%80%AE",
        components: {
            scheme: "scheme",
            path: "/%E0%80%AE",
        },
        normalized_uri: "scheme:/%E0%80%AE",
        normalized_iri: "scheme:/%E0%80%AE",
    },
    {
        name: "redundant UTF-8 encoding (1 byte inflated to 4 bytes)",
        composed: "scheme:/%F0%80%80%AE",
        components: {
            scheme: "scheme",
            path: "/%F0%80%80%AE",
        },
        normalized_uri: "scheme:/%F0%80%80%AE",
        normalized_iri: "scheme:/%F0%80%80%AE",
    },
    {
        name: "redundant UTF-8 encoding (2 byte inflated to 3 bytes)",
        composed: "scheme:/%E0%8E%B1",
        components: {
            scheme: "scheme",
            path: "/%E0%8E%B1",
        },
        normalized_uri: "scheme:/%E0%8E%B1",
        normalized_iri: "scheme:/%E0%8E%B1",
    },
    {
        name: "redundant UTF-8 encoding (2 byte inflated to 4 bytes)",
        composed: "scheme:/%F0%80%8E%B1",
        components: {
            scheme: "scheme",
            path: "/%F0%80%8E%B1",
        },
        normalized_uri: "scheme:/%F0%80%8E%B1",
        normalized_iri: "scheme:/%F0%80%8E%B1",
    },
    {
        name: "redundant UTF-8 encoding (3 byte inflated to 4 bytes)",
        composed: "scheme:/%F0%83%82%A4",
        components: {
            scheme: "scheme",
            path: "/%F0%83%82%A4",
        },
        normalized_uri: "scheme:/%F0%83%82%A4",
        normalized_iri: "scheme:/%F0%83%82%A4",
    },
    {
        name: "non-UTF-8 percent encoding (starts with invaild byte)",
        composed: "scheme:/%FF",
        components: {
            scheme: "scheme",
            path: "/%FF",
        },
        normalized_uri: "scheme:/%FF",
        normalized_iri: "scheme:/%FF",
    },
    {
        name: "non-UTF-8 percent encoding (starts with continue byte)",
        composed: "scheme:/%BF%BF",
        components: {
            scheme: "scheme",
            path: "/%BF%BF",
        },
        normalized_uri: "scheme:/%BF%BF",
        normalized_iri: "scheme:/%BF%BF",
    },
    {
        name: "non-UTF-8 percent encoding (expected 2 bytes, invalid at 2nd byte)",
        composed: "scheme:/%CE%FF",
        components: {
            scheme: "scheme",
            path: "/%CE%FF",
        },
        normalized_uri: "scheme:/%CE%FF",
        normalized_iri: "scheme:/%CE%FF",
    },
    {
        name: "non-UTF-8 percent encoding (expected 2 bytes, starts again at 2nd byte)",
        composed: "scheme:/%CE%CE%B1",
        components: {
            scheme: "scheme",
            path: "/%CE%CE%B1",
        },
        normalized_uri: "scheme:/%CE%CE%B1",
        normalized_iri: "scheme:/%CE\u{03B1}",
    },
    {
        name: "non-UTF-8 percent encoding (expected 3 bytes, invalid at 2nd byte)",
        composed: "scheme:/%E3%FF%A4",
        components: {
            scheme: "scheme",
            path: "/%E3%FF%A4",
        },
        normalized_uri: "scheme:/%E3%FF%A4",
        normalized_iri: "scheme:/%E3%FF%A4",
    },
    {
        name: "non-UTF-8 percent encoding (expected 3 bytes, starts again at 2nd byte)",
        composed: "scheme:/%E3%E3%82%A4",
        components: {
            scheme: "scheme",
            path: "/%E3%E3%82%A4",
        },
        normalized_uri: "scheme:/%E3%E3%82%A4",
        normalized_iri: "scheme:/%E3\u{30A4}",
    },
    {
        name: "non-UTF-8 percent encoding (expected 3 bytes, invalid at 3rd byte)",
        composed: "scheme:/%E3%82%FF",
        components: {
            scheme: "scheme",
            path: "/%E3%82%FF",
        },
        normalized_uri: "scheme:/%E3%82%FF",
        normalized_iri: "scheme:/%E3%82%FF",
    },
    {
        name: "non-UTF-8 percent encoding (expected 3 bytes, starts again at 3rd byte)",
        composed: "scheme:/%E3%82%E3%82%A4",
        components: {
            scheme: "scheme",
            path: "/%E3%82%E3%82%A4",
        },
        normalized_uri: "scheme:/%E3%82%E3%82%A4",
        normalized_iri: "scheme:/%E3%82\u{30A4}",
    },
    {
        name: "non-UTF-8 percent encoding (expected 4 bytes, invalid at 2nd byte)",
        composed: "scheme:/%F0%FF%8D%A3",
        components: {
            scheme: "scheme",
            path: "/%F0%FF%8D%A3",
        },
        normalized_uri: "scheme:/%F0%FF%8D%A3",
        normalized_iri: "scheme:/%F0%FF%8D%A3",
    },
    {
        name: "non-UTF-8 percent encoding (expected 4 bytes, starts again at 2nd byte)",
        composed: "scheme:/%F0%F0%9F%8D%A3",
        components: {
            scheme: "scheme",
            path: "/%F0%F0%9F%8D%A3",
        },
        normalized_uri: "scheme:/%F0%F0%9F%8D%A3",
        normalized_iri: "scheme:/%F0\u{1F363}",
    },
    {
        name: "non-UTF-8 percent encoding (expected 4 bytes, invalid at 3rd byte)",
        composed: "scheme:/%F0%9F%FF%A3",
        components: {
            scheme: "scheme",
            path: "/%F0%9F%FF%A3",
        },
        normalized_uri: "scheme:/%F0%9F%FF%A3",
        normalized_iri: "scheme:/%F0%9F%FF%A3",
    },
    {
        name: "non-UTF-8 percent encoding (expected 4 bytes, starts again at 3rd byte)",
        composed: "scheme:/%F0%9F%F0%9F%8D%A3",
        components: {
            scheme: "scheme",
            path: "/%F0%9F%F0%9F%8D%A3",
        },
        normalized_uri: "scheme:/%F0%9F%F0%9F%8D%A3",
        normalized_iri: "scheme:/%F0%9F\u{1F363}",
    },
    {
        name: "non-UTF-8 percent encoding (expected 4 bytes, invalid at 4th byte)",
        composed: "scheme:/%F0%9F%8D%FF",
        components: {
            scheme: "scheme",
            path: "/%F0%9F%8D%FF",
        },
        normalized_uri: "scheme:/%F0%9F%8D%FF",
        normalized_iri: "scheme:/%F0%9F%8D%FF",
    },
    {
        name: "non-UTF-8 percent encoding (expected 4 bytes, starts again at 4th byte)",
        composed: "scheme:/%F0%9F%8D%F0%9F%8D%A3",
        components: {
            scheme: "scheme",
            path: "/%F0%9F%8D%F0%9F%8D%A3",
        },
        normalized_uri: "scheme:/%F0%9F%8D%F0%9F%8D%A3",
        normalized_iri: "scheme:/%F0%9F%8D\u{1F363}",
    },
    {
        name: "non-UTF-8 percent encoding (high-surrogate)",
        composed: "scheme:/%ED%A0%A0",
        components: {
            scheme: "scheme",
            path: "/%ED%A0%A0",
        },
        normalized_uri: "scheme:/%ED%A0%A0",
        normalized_iri: "scheme:/%ED%A0%A0",
    },
    {
        name: "non-UTF-8 percent encoding (out of range, larger than U+10FFFF)",
        composed: "scheme:/%F4%90%80%80",
        components: {
            scheme: "scheme",
            path: "/%F4%90%80%80",
        },
        normalized_uri: "scheme:/%F4%90%80%80",
        normalized_iri: "scheme:/%F4%90%80%80",
    },
    {
        name: "non-UTF-8 percent encoding, followed by valid pct encoding",
        composed: "scheme:/%CE%2E%2E",
        components: {
            scheme: "scheme",
            path: "/%CE%2E%2E",
        },
        normalized_uri: "scheme:/%CE..",
        normalized_iri: "scheme:/%CE..",
    },
    {
        name: "non-UTF-8 percent encoding, followed by valid pct encoding",
        composed: "scheme:/%CE%FF%2E",
        components: {
            scheme: "scheme",
            path: "/%CE%FF%2E",
        },
        normalized_uri: "scheme:/%CE%FF.",
        normalized_iri: "scheme:/%CE%FF.",
    },
];
