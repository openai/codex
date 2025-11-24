use base64::prelude::*;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use std::fmt::Error as FormatterError;
use std::fmt::{Debug, Formatter};
#[cfg(feature = "timing-resistant-secret-traits")]
use std::hash::{Hash, Hasher};
use std::ops::Deref;

macro_rules! new_type {
    // Convenience pattern without an impl.
    (
        $(#[$attr:meta])*
        $name:ident(
            $(#[$type_attr:meta])*
            $type:ty
        )
    ) => {
        new_type![
            @new_type $(#[$attr])*,
            $name(
                $(#[$type_attr])*
                $type
            ),
            concat!(
                "Create a new `",
                stringify!($name),
                "` to wrap the given `",
                stringify!($type),
                "`."
            ),
            impl {}
        ];
    };
    // Main entry point with an impl.
    (
        $(#[$attr:meta])*
        $name:ident(
            $(#[$type_attr:meta])*
            $type:ty
        )
        impl {
            $($item:tt)*
        }
    ) => {
        new_type![
            @new_type $(#[$attr])*,
            $name(
                $(#[$type_attr])*
                $type
            ),
            concat!(
                "Create a new `",
                stringify!($name),
                "` to wrap the given `",
                stringify!($type),
                "`."
            ),
            impl {
                $($item)*
            }
        ];
    };
    // Actual implementation, after stringifying the #[doc] attr.
    (
        @new_type $(#[$attr:meta])*,
        $name:ident(
            $(#[$type_attr:meta])*
            $type:ty
        ),
        $new_doc:expr,
        impl {
            $($item:tt)*
        }
    ) => {
        $(#[$attr])*
        #[derive(Clone, Debug, PartialEq)]
        pub struct $name(
            $(#[$type_attr])*
            $type
        );
        impl $name {
            $($item)*

            #[doc = $new_doc]
            pub const fn new(s: $type) -> Self {
                $name(s)
            }
        }
        impl Deref for $name {
            type Target = $type;
            fn deref(&self) -> &$type {
                &self.0
            }
        }
        impl From<$name> for $type {
            fn from(t: $name) -> $type {
                t.0
            }
        }
    }
}

macro_rules! new_secret_type {
    (
        $(#[$attr:meta])*
        $name:ident($type:ty)
    ) => {
        new_secret_type![
            $(#[$attr])*
            $name($type)
            impl {}
        ];
    };
    (
        $(#[$attr:meta])*
        $name:ident($type:ty)
        impl {
            $($item:tt)*
        }
    ) => {
        new_secret_type![
            $(#[$attr])*,
            $name($type),
            concat!(
                "Create a new `",
                stringify!($name),
                "` to wrap the given `",
                stringify!($type),
                "`."
            ),
            concat!("Get the secret contained within this `", stringify!($name), "`."),
            impl {
                $($item)*
            }
        ];
    };
    (
        $(#[$attr:meta])*,
        $name:ident($type:ty),
        $new_doc:expr,
        $secret_doc:expr,
        impl {
            $($item:tt)*
        }
    ) => {
        $(
            #[$attr]
        )*
        #[cfg_attr(feature = "timing-resistant-secret-traits", derive(Eq))]
        pub struct $name($type);
        impl $name {
            $($item)*

            #[doc = $new_doc]
            pub fn new(s: $type) -> Self {
                $name(s)
            }

            #[doc = $secret_doc]
            ///
            /// # Security Warning
            ///
            /// Leaking this value may compromise the security of the OAuth2 flow.
            pub fn secret(&self) -> &$type { &self.0 }

            #[doc = $secret_doc]
            ///
            /// # Security Warning
            ///
            /// Leaking this value may compromise the security of the OAuth2 flow.
            pub fn into_secret(self) -> $type { self.0 }
        }
        impl Debug for $name {
            fn fmt(&self, f: &mut Formatter) -> Result<(), FormatterError> {
                write!(f, concat!(stringify!($name), "([redacted])"))
            }
        }

        #[cfg(feature = "timing-resistant-secret-traits")]
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                Sha256::digest(&self.0) == Sha256::digest(&other.0)
            }
        }

        #[cfg(feature = "timing-resistant-secret-traits")]
        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                Sha256::digest(&self.0).hash(state)
            }
        }

    };
}

/// Creates a URL-specific new type
///
/// Types created by this macro enforce during construction that the contained value represents a
/// syntactically valid URL. However, comparisons and hashes of these types are based on the string
/// representation given during construction, disregarding any canonicalization performed by the
/// underlying `Url` struct. OpenID Connect requires certain URLs (e.g., ID token issuers) to be
/// compared exactly, without canonicalization.
///
/// In addition to the raw string representation, these types include a `url` method to retrieve a
/// parsed `Url` struct.
macro_rules! new_url_type {
    // Convenience pattern without an impl.
    (
        $(#[$attr:meta])*
        $name:ident
    ) => {
        new_url_type![
            @new_type_pub $(#[$attr])*,
            $name,
            concat!("Create a new `", stringify!($name), "` from a `String` to wrap a URL."),
            concat!("Create a new `", stringify!($name), "` from a `Url` to wrap a URL."),
            concat!("Return this `", stringify!($name), "` as a parsed `Url`."),
            impl {}
        ];
    };
    // Main entry point with an impl.
    (
        $(#[$attr:meta])*
        $name:ident
        impl {
            $($item:tt)*
        }
    ) => {
        new_url_type![
            @new_type_pub $(#[$attr])*,
            $name,
            concat!("Create a new `", stringify!($name), "` from a `String` to wrap a URL."),
            concat!("Create a new `", stringify!($name), "` from a `Url` to wrap a URL."),
            concat!("Return this `", stringify!($name), "` as a parsed `Url`."),
            impl {
                $($item)*
            }
        ];
    };
    // Actual implementation, after stringifying the #[doc] attr.
    (
        @new_type_pub $(#[$attr:meta])*,
        $name:ident,
        $new_doc:expr,
        $from_url_doc:expr,
        $url_doc:expr,
        impl {
            $($item:tt)*
        }
    ) => {
        $(#[$attr])*
        #[derive(Clone)]
        pub struct $name(Url, String);
        impl $name {
            #[doc = $new_doc]
            pub fn new(url: String) -> Result<Self, ::url::ParseError> {
                Ok($name(Url::parse(&url)?, url))
            }
            #[doc = $from_url_doc]
            pub fn from_url(url: Url) -> Self {
                let s = url.to_string();
                Self(url, s)
            }
            #[doc = $url_doc]
            pub fn url(&self) -> &Url {
                return &self.0;
            }
            $($item)*
        }
        impl Deref for $name {
            type Target = String;
            fn deref(&self) -> &String {
                &self.1
            }
        }
        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
                write!(f, "{}", self.1)
            }
        }
        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
                let mut debug_trait_builder = f.debug_tuple(stringify!($name));
                debug_trait_builder.field(&self.1);
                debug_trait_builder.finish()
            }
        }
        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                struct UrlVisitor;
                impl<'de> ::serde::de::Visitor<'de> for UrlVisitor {
                    type Value = $name;

                    fn expecting(
                        &self,
                        formatter: &mut ::std::fmt::Formatter
                    ) -> ::std::fmt::Result {
                        formatter.write_str(stringify!($name))
                    }

                    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        $name::new(v.to_string()).map_err(E::custom)
                    }
                }
                deserializer.deserialize_str(UrlVisitor {})
            }
        }
        impl ::serde::Serialize for $name {
            fn serialize<SE>(&self, serializer: SE) -> Result<SE::Ok, SE::Error>
            where
                SE: ::serde::Serializer,
            {
                serializer.serialize_str(&self.1)
            }
        }
        impl ::std::hash::Hash for $name {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) -> () {
                ::std::hash::Hash::hash(&(self.1), state);
            }
        }
        impl Ord for $name {
            fn cmp(&self, other: &$name) -> ::std::cmp::Ordering {
                self.1.cmp(&other.1)
            }
        }
        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &$name) -> Option<::std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl PartialEq for $name {
            fn eq(&self, other: &$name) -> bool {
                self.1 == other.1
            }
        }
        impl Eq for $name {}
    };
}

new_type![
    /// Client identifier issued to the client during the registration process described by
    /// [Section 2.2](https://tools.ietf.org/html/rfc6749#section-2.2).
    #[derive(Deserialize, Serialize, Eq, Hash)]
    ClientId(String)
];

new_url_type![
    /// URL of the authorization server's authorization endpoint.
    AuthUrl
];
new_url_type![
    /// URL of the authorization server's token endpoint.
    TokenUrl
];
new_url_type![
    /// URL of the client's redirection endpoint.
    RedirectUrl
];
new_url_type![
    /// URL of the client's [RFC 7662 OAuth 2.0 Token Introspection](https://tools.ietf.org/html/rfc7662) endpoint.
    IntrospectionUrl
];
new_url_type![
    /// URL of the authorization server's RFC 7009 token revocation endpoint.
    RevocationUrl
];
new_url_type![
    /// URL of the client's device authorization endpoint.
    DeviceAuthorizationUrl
];
new_url_type![
    /// URL of the end-user verification URI on the authorization server.
    EndUserVerificationUrl
];
new_type![
    /// Authorization endpoint response (grant) type defined in
    /// [Section 3.1.1](https://tools.ietf.org/html/rfc6749#section-3.1.1).
    #[derive(Deserialize, Serialize, Eq, Hash)]
    ResponseType(String)
];
new_type![
    /// Resource owner's username used directly as an authorization grant to obtain an access
    /// token.
    #[derive(Deserialize, Serialize, Eq, Hash)]
    ResourceOwnerUsername(String)
];

new_type![
    /// Access token scope, as defined by the authorization server.
    #[derive(Deserialize, Serialize, Eq, Hash)]
    Scope(String)
];
impl AsRef<str> for Scope {
    fn as_ref(&self) -> &str {
        self
    }
}

new_type![
    /// Code Challenge Method used for [PKCE](https://tools.ietf.org/html/rfc7636) protection
    /// via the `code_challenge_method` parameter.
    #[derive(Deserialize, Serialize, Eq, Hash)]
    PkceCodeChallengeMethod(String)
];
// This type intentionally does not implement Clone in order to make it difficult to reuse PKCE
// challenges across multiple requests.
new_secret_type![
    /// Code Verifier used for [PKCE](https://tools.ietf.org/html/rfc7636) protection via the
    /// `code_verifier` parameter. The value must have a minimum length of 43 characters and a
    /// maximum length of 128 characters.  Each character must be ASCII alphanumeric or one of
    /// the characters "-" / "." / "_" / "~".
    #[derive(Deserialize, Serialize)]
    PkceCodeVerifier(String)
];

/// Code Challenge used for [PKCE](https://tools.ietf.org/html/rfc7636) protection via the
/// `code_challenge` parameter.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PkceCodeChallenge {
    code_challenge: String,
    code_challenge_method: PkceCodeChallengeMethod,
}
impl PkceCodeChallenge {
    /// Generate a new random, base64-encoded SHA-256 PKCE code.
    pub fn new_random_sha256() -> (Self, PkceCodeVerifier) {
        Self::new_random_sha256_len(32)
    }

    /// Generate a new random, base64-encoded SHA-256 PKCE challenge code and verifier.
    ///
    /// # Arguments
    ///
    /// * `num_bytes` - Number of random bytes to generate, prior to base64-encoding.
    ///   The value must be in the range 32 to 96 inclusive in order to generate a verifier
    ///   with a suitable length.
    ///
    /// # Panics
    ///
    /// This method panics if the resulting PKCE code verifier is not of a suitable length
    /// to comply with [RFC 7636](https://tools.ietf.org/html/rfc7636).
    pub fn new_random_sha256_len(num_bytes: u32) -> (Self, PkceCodeVerifier) {
        let code_verifier = Self::new_random_len(num_bytes);
        (
            Self::from_code_verifier_sha256(&code_verifier),
            code_verifier,
        )
    }

    /// Generate a new random, base64-encoded PKCE code verifier.
    ///
    /// # Arguments
    ///
    /// * `num_bytes` - Number of random bytes to generate, prior to base64-encoding.
    ///   The value must be in the range 32 to 96 inclusive in order to generate a verifier
    ///   with a suitable length.
    ///
    /// # Panics
    ///
    /// This method panics if the resulting PKCE code verifier is not of a suitable length
    /// to comply with [RFC 7636](https://tools.ietf.org/html/rfc7636).
    fn new_random_len(num_bytes: u32) -> PkceCodeVerifier {
        // The RFC specifies that the code verifier must have "a minimum length of 43
        // characters and a maximum length of 128 characters".
        // This implies 32-96 octets of random data to be base64 encoded.
        assert!((32..=96).contains(&num_bytes));
        let random_bytes: Vec<u8> = (0..num_bytes).map(|_| thread_rng().gen::<u8>()).collect();
        PkceCodeVerifier::new(BASE64_URL_SAFE_NO_PAD.encode(random_bytes))
    }

    /// Generate a SHA-256 PKCE code challenge from the supplied PKCE code verifier.
    ///
    /// # Panics
    ///
    /// This method panics if the supplied PKCE code verifier is not of a suitable length
    /// to comply with [RFC 7636](https://tools.ietf.org/html/rfc7636).
    pub fn from_code_verifier_sha256(code_verifier: &PkceCodeVerifier) -> Self {
        // The RFC specifies that the code verifier must have "a minimum length of 43
        // characters and a maximum length of 128 characters".
        assert!(code_verifier.secret().len() >= 43 && code_verifier.secret().len() <= 128);

        let digest = Sha256::digest(code_verifier.secret().as_bytes());
        let code_challenge = BASE64_URL_SAFE_NO_PAD.encode(digest);

        Self {
            code_challenge,
            code_challenge_method: PkceCodeChallengeMethod::new("S256".to_string()),
        }
    }

    /// Generate a new random, base64-encoded PKCE code.
    /// Use is discouraged unless the endpoint does not support SHA-256.
    ///
    /// # Panics
    ///
    /// This method panics if the supplied PKCE code verifier is not of a suitable length
    /// to comply with [RFC 7636](https://tools.ietf.org/html/rfc7636).
    #[cfg(feature = "pkce-plain")]
    pub fn new_random_plain() -> (Self, PkceCodeVerifier) {
        let code_verifier = Self::new_random_len(32);
        (
            Self::from_code_verifier_plain(&code_verifier),
            code_verifier,
        )
    }

    /// Generate a plain PKCE code challenge from the supplied PKCE code verifier.
    /// Use is discouraged unless the endpoint does not support SHA-256.
    ///
    /// # Panics
    ///
    /// This method panics if the supplied PKCE code verifier is not of a suitable length
    /// to comply with [RFC 7636](https://tools.ietf.org/html/rfc7636).
    #[cfg(feature = "pkce-plain")]
    pub fn from_code_verifier_plain(code_verifier: &PkceCodeVerifier) -> Self {
        // The RFC specifies that the code verifier must have "a minimum length of 43
        // characters and a maximum length of 128 characters".
        assert!(code_verifier.secret().len() >= 43 && code_verifier.secret().len() <= 128);

        let code_challenge = code_verifier.secret().clone();

        Self {
            code_challenge,
            code_challenge_method: PkceCodeChallengeMethod::new("plain".to_string()),
        }
    }

    /// Returns the PKCE code challenge as a string.
    pub fn as_str(&self) -> &str {
        &self.code_challenge
    }

    /// Returns the PKCE code challenge method as a string.
    pub fn method(&self) -> &PkceCodeChallengeMethod {
        &self.code_challenge_method
    }
}

new_secret_type![
    /// Client password issued to the client during the registration process described by
    /// [Section 2.2](https://tools.ietf.org/html/rfc6749#section-2.2).
    #[derive(Clone, Deserialize, Serialize)]
    ClientSecret(String)
];
new_secret_type![
    /// Value used for [CSRF](https://tools.ietf.org/html/rfc6749#section-10.12) protection
    /// via the `state` parameter.
    #[must_use]
    #[derive(Clone, Deserialize, Serialize)]
    CsrfToken(String)
    impl {
        /// Generate a new random, base64-encoded 128-bit CSRF token.
        pub fn new_random() -> Self {
            CsrfToken::new_random_len(16)
        }
        /// Generate a new random, base64-encoded CSRF token of the specified length.
        ///
        /// # Arguments
        ///
        /// * `num_bytes` - Number of random bytes to generate, prior to base64-encoding.
        pub fn new_random_len(num_bytes: u32) -> Self {
            let random_bytes: Vec<u8> = (0..num_bytes).map(|_| thread_rng().gen::<u8>()).collect();
            CsrfToken::new(BASE64_URL_SAFE_NO_PAD.encode(random_bytes))
        }
    }
];
new_secret_type![
    /// Authorization code returned from the authorization endpoint.
    #[derive(Clone, Deserialize, Serialize)]
    AuthorizationCode(String)
];
new_secret_type![
    /// Refresh token used to obtain a new access token (if supported by the authorization server).
    #[derive(Clone, Deserialize, Serialize)]
    RefreshToken(String)
];
new_secret_type![
    /// Access token returned by the token endpoint and used to access protected resources.
    #[derive(Clone, Deserialize, Serialize)]
    AccessToken(String)
];
new_secret_type![
    /// Resource owner's password used directly as an authorization grant to obtain an access
    /// token.
    #[derive(Clone)]
    ResourceOwnerPassword(String)
];
new_secret_type![
    /// Device code returned by the device authorization endpoint and used to query the token endpoint.
    #[derive(Clone, Deserialize, Serialize)]
    DeviceCode(String)
];
new_secret_type![
    /// Verification URI returned by the device authorization endpoint and visited by the user
    /// to authorize.  Contains the user code.
    #[derive(Clone, Deserialize, Serialize)]
    VerificationUriComplete(String)
];
new_secret_type![
    /// User code returned by the device authorization endpoint and used by the user to authorize at
    /// the verification URI.
    #[derive(Clone, Deserialize, Serialize)]
    UserCode(String)
];

#[cfg(test)]
mod tests {
    use crate::{ClientSecret, CsrfToken, PkceCodeChallenge, PkceCodeVerifier};

    #[test]
    fn test_secret_conversion() {
        let secret = CsrfToken::new("top_secret".into());
        assert_eq!(secret.into_secret().into_boxed_str(), "top_secret".into());
    }

    #[test]
    fn test_secret_redaction() {
        let secret = ClientSecret::new("top_secret".to_string());
        assert_eq!("ClientSecret([redacted])", format!("{secret:?}"));
    }

    #[test]
    #[should_panic]
    fn test_code_verifier_too_short() {
        PkceCodeChallenge::new_random_sha256_len(31);
    }

    #[test]
    #[should_panic]
    fn test_code_verifier_too_long() {
        PkceCodeChallenge::new_random_sha256_len(97);
    }

    #[test]
    fn test_code_verifier_min() {
        let code = PkceCodeChallenge::new_random_sha256_len(32);
        assert_eq!(code.1.secret().len(), 43);
    }

    #[test]
    fn test_code_verifier_max() {
        let code = PkceCodeChallenge::new_random_sha256_len(96);
        assert_eq!(code.1.secret().len(), 128);
    }

    #[test]
    fn test_code_verifier_challenge() {
        // Example from https://tools.ietf.org/html/rfc7636#appendix-B
        let code_verifier =
            PkceCodeVerifier::new("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string());
        assert_eq!(
            PkceCodeChallenge::from_code_verifier_sha256(&code_verifier).as_str(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        );
    }
}
