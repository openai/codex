//! HTTP header types
//!
//! The module provides [`HeaderName`], [`HeaderMap`], and a number of types
//! used for interacting with `HeaderMap`. These types allow representing both
//! HTTP/1 and HTTP/2 headers.
//!
//! # `HeaderName`
//!
//! The `HeaderName` type represents both standard header names as well as
//! custom header names. The type handles the case insensitive nature of header
//! names and is used as the key portion of `HeaderMap`. Header names are
//! normalized to lower case. In other words, when creating a `HeaderName` with
//! a string, even if upper case characters are included, when getting a string
//! representation of the `HeaderName`, it will be all lower case. This allows
//! for faster `HeaderMap` comparison operations.
//!
//! The internal representation is optimized to efficiently handle the cases
//! most commonly encountered when working with HTTP. Standard header names are
//! special cased and are represented internally as an enum. Short custom
//! headers will be stored directly in the `HeaderName` struct and will not
//! incur any allocation overhead, however longer strings will require an
//! allocation for storage.
//!
//! ## Limitations
//!
//! `HeaderName` has a max length of 32,768 for header names. Attempting to
//! parse longer names will result in a panic.
//!
//! # `HeaderMap`
//!
//! `HeaderMap` is a map structure of header names highly optimized for use
//! cases common with HTTP. It is a [multimap] structure, where each header name
//! may have multiple associated header values. Given this, some of the APIs
//! diverge from [`HashMap`].
//!
//! ## Overview
//!
//! Just like `HashMap` in Rust's stdlib, `HeaderMap` is based on [Robin Hood
//! hashing]. This algorithm tends to reduce the worst case search times in the
//! table and enables high load factors without seriously affecting performance.
//! Internally, keys and values are stored in vectors. As such, each insertion
//! will not incur allocation overhead. However, once the underlying vector
//! storage is full, a larger vector must be allocated and all values copied.
//!
//! ## Deterministic ordering
//!
//! Unlike Rust's `HashMap`, values in `HeaderMap` are deterministically
//! ordered. Roughly, values are ordered by insertion. This means that a
//! function that deterministically operates on a header map can rely on the
//! iteration order to remain consistent across processes and platforms.
//!
//! ## Adaptive hashing
//!
//! `HeaderMap` uses an adaptive hashing strategy in order to efficiently handle
//! most common cases. All standard headers have statically computed hash values
//! which removes the need to perform any hashing of these headers at runtime.
//! The default hash function emphasizes performance over robustness. However,
//! `HeaderMap` detects high collision rates and switches to a secure hash
//! function in those events. The threshold is set such that only denial of
//! service attacks should trigger it.
//!
//! ## Limitations
//!
//! `HeaderMap` can store a maximum of 32,768 headers (header name / value
//! pairs). Attempting to insert more will result in a panic.
//!
//! [`HeaderName`]: struct.HeaderName.html
//! [`HeaderMap`]: struct.HeaderMap.html
//! [multimap]: https://en.wikipedia.org/wiki/Multimap
//! [`HashMap`]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
//! [Robin Hood hashing]: https://en.wikipedia.org/wiki/Hash_table#Robin_Hood_hashing

mod map;
mod name;
mod value;

pub use self::map::{
    AsHeaderName, Drain, Entry, GetAll, HeaderMap, IntoHeaderName, IntoIter, Iter, IterMut, Keys,
    MaxSizeReached, OccupiedEntry, VacantEntry, ValueDrain, ValueIter, ValueIterMut, Values,
    ValuesMut,
};
pub use self::name::{HeaderName, InvalidHeaderName};
pub use self::value::{HeaderValue, InvalidHeaderValue, ToStrError};

// Use header name constants
#[rustfmt::skip]
pub use self::name::{
    ACCEPT,
    ACCEPT_CHARSET,
    ACCEPT_ENCODING,
    ACCEPT_LANGUAGE,
    ACCEPT_RANGES,
    ACCESS_CONTROL_ALLOW_CREDENTIALS,
    ACCESS_CONTROL_ALLOW_HEADERS,
    ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_EXPOSE_HEADERS,
    ACCESS_CONTROL_MAX_AGE,
    ACCESS_CONTROL_REQUEST_HEADERS,
    ACCESS_CONTROL_REQUEST_METHOD,
    AGE,
    ALLOW,
    ALT_SVC,
    AUTHORIZATION,
    CACHE_CONTROL,
    CACHE_STATUS,
    CDN_CACHE_CONTROL,
    CONNECTION,
    CONTENT_DISPOSITION,
    CONTENT_ENCODING,
    CONTENT_LANGUAGE,
    CONTENT_LENGTH,
    CONTENT_LOCATION,
    CONTENT_RANGE,
    CONTENT_SECURITY_POLICY,
    CONTENT_SECURITY_POLICY_REPORT_ONLY,
    CONTENT_TYPE,
    COOKIE,
    DNT,
    DATE,
    ETAG,
    EXPECT,
    EXPIRES,
    FORWARDED,
    FROM,
    HOST,
    IF_MATCH,
    IF_MODIFIED_SINCE,
    IF_NONE_MATCH,
    IF_RANGE,
    IF_UNMODIFIED_SINCE,
    LAST_MODIFIED,
    LINK,
    LOCATION,
    MAX_FORWARDS,
    ORIGIN,
    PRAGMA,
    PROXY_AUTHENTICATE,
    PROXY_AUTHORIZATION,
    PUBLIC_KEY_PINS,
    PUBLIC_KEY_PINS_REPORT_ONLY,
    RANGE,
    REFERER,
    REFERRER_POLICY,
    REFRESH,
    RETRY_AFTER,
    SEC_WEBSOCKET_ACCEPT,
    SEC_WEBSOCKET_EXTENSIONS,
    SEC_WEBSOCKET_KEY,
    SEC_WEBSOCKET_PROTOCOL,
    SEC_WEBSOCKET_VERSION,
    SERVER,
    SET_COOKIE,
    STRICT_TRANSPORT_SECURITY,
    TE,
    TRAILER,
    TRANSFER_ENCODING,
    UPGRADE,
    UPGRADE_INSECURE_REQUESTS,
    USER_AGENT,
    VARY,
    VIA,
    WARNING,
    WWW_AUTHENTICATE,
    X_CONTENT_TYPE_OPTIONS,
    X_DNS_PREFETCH_CONTROL,
    X_FRAME_OPTIONS,
    X_XSS_PROTECTION,
};

/// Maximum length of a header name
///
/// Generally, 64kb for a header name is WAY too much than would ever be needed
/// in practice. Restricting it to this size enables using `u16` values to
/// represent offsets when dealing with header names.
const MAX_HEADER_NAME_LEN: usize = (1 << 16) - 1;
