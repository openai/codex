use crate::byte_str::ByteStr;
use bytes::{Bytes, BytesMut};

use std::borrow::Borrow;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::str::FromStr;

/// Represents an HTTP header field name
///
/// Header field names identify the header. Header sets may include multiple
/// headers with the same name. The HTTP specification defines a number of
/// standard headers, but HTTP messages may include non-standard header names as
/// well as long as they adhere to the specification.
///
/// `HeaderName` is used as the [`HeaderMap`] key. Constants are available for
/// all standard header names in the [`header`] module.
///
/// # Representation
///
/// `HeaderName` represents standard header names using an `enum`, as such they
/// will not require an allocation for storage. All custom header names are
/// lower cased upon conversion to a `HeaderName` value. This avoids the
/// overhead of dynamically doing lower case conversion during the hash code
/// computation and the comparison operation.
///
/// [`HeaderMap`]: struct.HeaderMap.html
/// [`header`]: index.html
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct HeaderName {
    inner: Repr<Custom>,
}

// Almost a full `HeaderName`
#[derive(Debug, Hash)]
pub struct HdrName<'a> {
    inner: Repr<MaybeLower<'a>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum Repr<T> {
    Standard(StandardHeader),
    Custom(T),
}

// Used to hijack the Hash impl
#[derive(Debug, Clone, Eq, PartialEq)]
struct Custom(ByteStr);

#[derive(Debug, Clone)]
// Invariant: If lower then buf is valid UTF-8.
struct MaybeLower<'a> {
    buf: &'a [u8],
    lower: bool,
}

/// A possible error when converting a `HeaderName` from another type.
pub struct InvalidHeaderName {
    _priv: (),
}

macro_rules! standard_headers {
    (
        $(
            $(#[$docs:meta])*
            ($konst:ident, $upcase:ident, $name_bytes:literal);
        )+
    ) => {
        #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
        enum StandardHeader {
            $(
                $konst,
            )+
        }

        $(
            $(#[$docs])*
            pub const $upcase: HeaderName = HeaderName {
                inner: Repr::Standard(StandardHeader::$konst),
            };
        )+

        impl StandardHeader {
            #[inline]
            fn as_str(&self) -> &'static str {
                match *self {
                    // Safety: test_parse_standard_headers ensures these &[u8]s are &str-safe.
                    $(
                    StandardHeader::$konst => unsafe { std::str::from_utf8_unchecked( $name_bytes ) },
                    )+
                }
            }

            const fn from_bytes(name_bytes: &[u8]) -> Option<StandardHeader> {
                match name_bytes {
                    $(
                        $name_bytes => Some(StandardHeader::$konst),
                    )+
                    _ => None,
                }
            }
        }

        #[cfg(test)]
        const TEST_HEADERS: &'static [(StandardHeader, &'static [u8])] = &[
            $(
            (StandardHeader::$konst, $name_bytes),
            )+
        ];

        #[test]
        fn test_parse_standard_headers() {
            for &(std, name_bytes) in TEST_HEADERS {
                // Test lower case
                assert_eq!(HeaderName::from_bytes(name_bytes).unwrap(), HeaderName::from(std));

                // Test upper case
                let upper = std::str::from_utf8(name_bytes).expect("byte string constants are all utf-8").to_uppercase();
                assert_eq!(HeaderName::from_bytes(upper.as_bytes()).unwrap(), HeaderName::from(std));
            }
        }

        #[test]
        fn test_standard_headers_into_bytes() {
            for &(std, name_bytes) in TEST_HEADERS {
                let name = std::str::from_utf8(name_bytes).unwrap();
                let std = HeaderName::from(std);
                // Test lower case
                let bytes: Bytes =
                    HeaderName::from_bytes(name_bytes).unwrap().inner.into();
                assert_eq!(bytes, name);
                assert_eq!(HeaderName::from_bytes(name_bytes).unwrap(), std);

                // Test upper case
                let upper = name.to_uppercase();
                let bytes: Bytes =
                    HeaderName::from_bytes(upper.as_bytes()).unwrap().inner.into();
                assert_eq!(bytes, name_bytes);
                assert_eq!(HeaderName::from_bytes(upper.as_bytes()).unwrap(),
                           std);
            }

        }
    }
}

// Generate constants for all standard HTTP headers. This includes a static hash
// code for the "fast hash" path. The hash code for static headers *do not* have
// to match the text representation of those headers. This is because header
// strings are always converted to the static values (when they match) before
// being hashed. This means that it is impossible to compare the static hash
// code of CONTENT_LENGTH with "content-length".
standard_headers! {
    /// Advertises which content types the client is able to understand.
    ///
    /// The Accept request HTTP header advertises which content types, expressed
    /// as MIME types, the client is able to understand. Using content
    /// negotiation, the server then selects one of the proposals, uses it and
    /// informs the client of its choice with the Content-Type response header.
    /// Browsers set adequate values for this header depending of the context
    /// where the request is done: when fetching a CSS stylesheet a different
    /// value is set for the request than when fetching an image, video or a
    /// script.
    (Accept, ACCEPT, b"accept");

    /// Advertises which character set the client is able to understand.
    ///
    /// The Accept-Charset request HTTP header advertises which character set
    /// the client is able to understand. Using content negotiation, the server
    /// then selects one of the proposals, uses it and informs the client of its
    /// choice within the Content-Type response header. Browsers usually don't
    /// set this header as the default value for each content type is usually
    /// correct and transmitting it would allow easier fingerprinting.
    ///
    /// If the server cannot serve any matching character set, it can
    /// theoretically send back a 406 (Not Acceptable) error code. But, for a
    /// better user experience, this is rarely done and the more common way is
    /// to ignore the Accept-Charset header in this case.
    (AcceptCharset, ACCEPT_CHARSET, b"accept-charset");

    /// Advertises which content encoding the client is able to understand.
    ///
    /// The Accept-Encoding request HTTP header advertises which content
    /// encoding, usually a compression algorithm, the client is able to
    /// understand. Using content negotiation, the server selects one of the
    /// proposals, uses it and informs the client of its choice with the
    /// Content-Encoding response header.
    ///
    /// Even if both the client and the server supports the same compression
    /// algorithms, the server may choose not to compress the body of a
    /// response, if the identity value is also acceptable. Two common cases
    /// lead to this:
    ///
    /// * The data to be sent is already compressed and a second compression
    /// won't lead to smaller data to be transmitted. This may the case with
    /// some image formats;
    ///
    /// * The server is overloaded and cannot afford the computational overhead
    /// induced by the compression requirement. Typically, Microsoft recommends
    /// not to compress if a server use more than 80 % of its computational
    /// power.
    ///
    /// As long as the identity value, meaning no compression, is not explicitly
    /// forbidden, by an identity;q=0 or a *;q=0 without another explicitly set
    /// value for identity, the server must never send back a 406 Not Acceptable
    /// error.
    (AcceptEncoding, ACCEPT_ENCODING, b"accept-encoding");

    /// Advertises which languages the client is able to understand.
    ///
    /// The Accept-Language request HTTP header advertises which languages the
    /// client is able to understand, and which locale variant is preferred.
    /// Using content negotiation, the server then selects one of the proposals,
    /// uses it and informs the client of its choice with the Content-Language
    /// response header. Browsers set adequate values for this header according
    /// their user interface language and even if a user can change it, this
    /// happens rarely (and is frown upon as it leads to fingerprinting).
    ///
    /// This header is a hint to be used when the server has no way of
    /// determining the language via another way, like a specific URL, that is
    /// controlled by an explicit user decision. It is recommended that the
    /// server never overrides an explicit decision. The content of the
    /// Accept-Language is often out of the control of the user (like when
    /// traveling and using an Internet Cafe in a different country); the user
    /// may also want to visit a page in another language than the locale of
    /// their user interface.
    ///
    /// If the server cannot serve any matching language, it can theoretically
    /// send back a 406 (Not Acceptable) error code. But, for a better user
    /// experience, this is rarely done and more common way is to ignore the
    /// Accept-Language header in this case.
    (AcceptLanguage, ACCEPT_LANGUAGE, b"accept-language");

    /// Marker used by the server to advertise partial request support.
    ///
    /// The Accept-Ranges response HTTP header is a marker used by the server to
    /// advertise its support of partial requests. The value of this field
    /// indicates the unit that can be used to define a range.
    ///
    /// In presence of an Accept-Ranges header, the browser may try to resume an
    /// interrupted download, rather than to start it from the start again.
    (AcceptRanges, ACCEPT_RANGES, b"accept-ranges");

    /// Preflight response indicating if the response to the request can be
    /// exposed to the page.
    ///
    /// The Access-Control-Allow-Credentials response header indicates whether
    /// or not the response to the request can be exposed to the page. It can be
    /// exposed when the true value is returned; it can't in other cases.
    ///
    /// Credentials are cookies, authorization headers or TLS client
    /// certificates.
    ///
    /// When used as part of a response to a preflight request, this indicates
    /// whether or not the actual request can be made using credentials. Note
    /// that simple GET requests are not preflighted, and so if a request is
    /// made for a resource with credentials, if this header is not returned
    /// with the resource, the response is ignored by the browser and not
    /// returned to web content.
    ///
    /// The Access-Control-Allow-Credentials header works in conjunction with
    /// the XMLHttpRequest.withCredentials property or with the credentials
    /// option in the Request() constructor of the Fetch API. Credentials must
    /// be set on both sides (the Access-Control-Allow-Credentials header and in
    /// the XHR or Fetch request) in order for the CORS request with credentials
    /// to succeed.
    (AccessControlAllowCredentials, ACCESS_CONTROL_ALLOW_CREDENTIALS, b"access-control-allow-credentials");

    /// Preflight response indicating permitted HTTP headers.
    ///
    /// The Access-Control-Allow-Headers response header is used in response to
    /// a preflight request to indicate which HTTP headers will be available via
    /// Access-Control-Expose-Headers when making the actual request.
    ///
    /// The simple headers, Accept, Accept-Language, Content-Language,
    /// Content-Type (but only with a MIME type of its parsed value (ignoring
    /// parameters) of either application/x-www-form-urlencoded,
    /// multipart/form-data, or text/plain), are always available and don't need
    /// to be listed by this header.
    ///
    /// This header is required if the request has an
    /// Access-Control-Request-Headers header.
    (AccessControlAllowHeaders, ACCESS_CONTROL_ALLOW_HEADERS, b"access-control-allow-headers");

    /// Preflight header response indicating permitted access methods.
    ///
    /// The Access-Control-Allow-Methods response header specifies the method or
    /// methods allowed when accessing the resource in response to a preflight
    /// request.
    (AccessControlAllowMethods, ACCESS_CONTROL_ALLOW_METHODS, b"access-control-allow-methods");

    /// Indicates whether the response can be shared with resources with the
    /// given origin.
    (AccessControlAllowOrigin, ACCESS_CONTROL_ALLOW_ORIGIN, b"access-control-allow-origin");

    /// Indicates which headers can be exposed as part of the response by
    /// listing their names.
    (AccessControlExposeHeaders, ACCESS_CONTROL_EXPOSE_HEADERS, b"access-control-expose-headers");

    /// Indicates how long the results of a preflight request can be cached.
    (AccessControlMaxAge, ACCESS_CONTROL_MAX_AGE, b"access-control-max-age");

    /// Informs the server which HTTP headers will be used when an actual
    /// request is made.
    (AccessControlRequestHeaders, ACCESS_CONTROL_REQUEST_HEADERS, b"access-control-request-headers");

    /// Informs the server know which HTTP method will be used when the actual
    /// request is made.
    (AccessControlRequestMethod, ACCESS_CONTROL_REQUEST_METHOD, b"access-control-request-method");

    /// Indicates the time in seconds the object has been in a proxy cache.
    ///
    /// The Age header is usually close to zero. If it is Age: 0, it was
    /// probably just fetched from the origin server; otherwise It is usually
    /// calculated as a difference between the proxy's current date and the Date
    /// general header included in the HTTP response.
    (Age, AGE, b"age");

    /// Lists the set of methods support by a resource.
    ///
    /// This header must be sent if the server responds with a 405 Method Not
    /// Allowed status code to indicate which request methods can be used. An
    /// empty Allow header indicates that the resource allows no request
    /// methods, which might occur temporarily for a given resource, for
    /// example.
    (Allow, ALLOW, b"allow");

    /// Advertises the availability of alternate services to clients.
    (AltSvc, ALT_SVC, b"alt-svc");

    /// Contains the credentials to authenticate a user agent with a server.
    ///
    /// Usually this header is included after the server has responded with a
    /// 401 Unauthorized status and the WWW-Authenticate header.
    (Authorization, AUTHORIZATION, b"authorization");

    /// Specifies directives for caching mechanisms in both requests and
    /// responses.
    ///
    /// Caching directives are unidirectional, meaning that a given directive in
    /// a request is not implying that the same directive is to be given in the
    /// response.
    (CacheControl, CACHE_CONTROL, b"cache-control");

    /// Indicates how caches have handled a response and its corresponding request.
    ///
    /// See [RFC 9211](https://www.rfc-editor.org/rfc/rfc9211.html).
    (CacheStatus, CACHE_STATUS, b"cache-status");

    /// Specifies directives that allow origin servers to control the behavior of CDN caches
    /// interposed between them and clients separately from other caches that might handle the
    /// response.
    ///
    /// See [RFC 9213](https://www.rfc-editor.org/rfc/rfc9213.html).
    (CdnCacheControl, CDN_CACHE_CONTROL, b"cdn-cache-control");

    /// Controls whether or not the network connection stays open after the
    /// current transaction finishes.
    ///
    /// If the value sent is keep-alive, the connection is persistent and not
    /// closed, allowing for subsequent requests to the same server to be done.
    ///
    /// Except for the standard hop-by-hop headers (Keep-Alive,
    /// Transfer-Encoding, TE, Connection, Trailer, Upgrade, Proxy-Authorization
    /// and Proxy-Authenticate), any hop-by-hop headers used by the message must
    /// be listed in the Connection header, so that the first proxy knows he has
    /// to consume them and not to forward them further. Standard hop-by-hop
    /// headers can be listed too (it is often the case of Keep-Alive, but this
    /// is not mandatory.
    (Connection, CONNECTION, b"connection");

    /// Indicates if the content is expected to be displayed inline.
    ///
    /// In a regular HTTP response, the Content-Disposition response header is a
    /// header indicating if the content is expected to be displayed inline in
    /// the browser, that is, as a Web page or as part of a Web page, or as an
    /// attachment, that is downloaded and saved locally.
    ///
    /// In a multipart/form-data body, the HTTP Content-Disposition general
    /// header is a header that can be used on the subpart of a multipart body
    /// to give information about the field it applies to. The subpart is
    /// delimited by the boundary defined in the Content-Type header. Used on
    /// the body itself, Content-Disposition has no effect.
    ///
    /// The Content-Disposition header is defined in the larger context of MIME
    /// messages for e-mail, but only a subset of the possible parameters apply
    /// to HTTP forms and POST requests. Only the value form-data, as well as
    /// the optional directive name and filename, can be used in the HTTP
    /// context.
    (ContentDisposition, CONTENT_DISPOSITION, b"content-disposition");

    /// Used to compress the media-type.
    ///
    /// When present, its value indicates what additional content encoding has
    /// been applied to the entity-body. It lets the client know, how to decode
    /// in order to obtain the media-type referenced by the Content-Type header.
    ///
    /// It is recommended to compress data as much as possible and therefore to
    /// use this field, but some types of resources, like jpeg images, are
    /// already compressed.  Sometimes using additional compression doesn't
    /// reduce payload size and can even make the payload longer.
    (ContentEncoding, CONTENT_ENCODING, b"content-encoding");

    /// Used to describe the languages intended for the audience.
    ///
    /// This header allows a user to differentiate according to the users' own
    /// preferred language. For example, if "Content-Language: de-DE" is set, it
    /// says that the document is intended for German language speakers
    /// (however, it doesn't indicate the document is written in German. For
    /// example, it might be written in English as part of a language course for
    /// German speakers).
    ///
    /// If no Content-Language is specified, the default is that the content is
    /// intended for all language audiences. Multiple language tags are also
    /// possible, as well as applying the Content-Language header to various
    /// media types and not only to textual documents.
    (ContentLanguage, CONTENT_LANGUAGE, b"content-language");

    /// Indicates the size of the entity-body.
    ///
    /// The header value must be a decimal indicating the number of octets sent
    /// to the recipient.
    (ContentLength, CONTENT_LENGTH, b"content-length");

    /// Indicates an alternate location for the returned data.
    ///
    /// The principal use case is to indicate the URL of the resource
    /// transmitted as the result of content negotiation.
    ///
    /// Location and Content-Location are different: Location indicates the
    /// target of a redirection (or the URL of a newly created document), while
    /// Content-Location indicates the direct URL to use to access the resource,
    /// without the need of further content negotiation. Location is a header
    /// associated with the response, while Content-Location is associated with
    /// the entity returned.
    (ContentLocation, CONTENT_LOCATION, b"content-location");

    /// Indicates where in a full body message a partial message belongs.
    (ContentRange, CONTENT_RANGE, b"content-range");

    /// Allows controlling resources the user agent is allowed to load for a
    /// given page.
    ///
    /// With a few exceptions, policies mostly involve specifying server origins
    /// and script endpoints. This helps guard against cross-site scripting
    /// attacks (XSS).
    (ContentSecurityPolicy, CONTENT_SECURITY_POLICY, b"content-security-policy");

    /// Allows experimenting with policies by monitoring their effects.
    ///
    /// The HTTP Content-Security-Policy-Report-Only response header allows web
    /// developers to experiment with policies by monitoring (but not enforcing)
    /// their effects. These violation reports consist of JSON documents sent
    /// via an HTTP POST request to the specified URI.
    (ContentSecurityPolicyReportOnly, CONTENT_SECURITY_POLICY_REPORT_ONLY, b"content-security-policy-report-only");

    /// Used to indicate the media type of the resource.
    ///
    /// In responses, a Content-Type header tells the client what the content
    /// type of the returned content actually is. Browsers will do MIME sniffing
    /// in some cases and will not necessarily follow the value of this header;
    /// to prevent this behavior, the header X-Content-Type-Options can be set
    /// to nosniff.
    ///
    /// In requests, (such as POST or PUT), the client tells the server what
    /// type of data is actually sent.
    (ContentType, CONTENT_TYPE, b"content-type");

    /// Contains stored HTTP cookies previously sent by the server with the
    /// Set-Cookie header.
    ///
    /// The Cookie header might be omitted entirely, if the privacy setting of
    /// the browser are set to block them, for example.
    (Cookie, COOKIE, b"cookie");

    /// Indicates the client's tracking preference.
    ///
    /// This header lets users indicate whether they would prefer privacy rather
    /// than personalized content.
    (Dnt, DNT, b"dnt");

    /// Contains the date and time at which the message was originated.
    (Date, DATE, b"date");

    /// Identifier for a specific version of a resource.
    ///
    /// This header allows caches to be more efficient, and saves bandwidth, as
    /// a web server does not need to send a full response if the content has
    /// not changed. On the other side, if the content has changed, etags are
    /// useful to help prevent simultaneous updates of a resource from
    /// overwriting each other ("mid-air collisions").
    ///
    /// If the resource at a given URL changes, a new Etag value must be
    /// generated. Etags are therefore similar to fingerprints and might also be
    /// used for tracking purposes by some servers. A comparison of them allows
    /// to quickly determine whether two representations of a resource are the
    /// same, but they might also be set to persist indefinitely by a tracking
    /// server.
    (Etag, ETAG, b"etag");

    /// Indicates expectations that need to be fulfilled by the server in order
    /// to properly handle the request.
    ///
    /// The only expectation defined in the specification is Expect:
    /// 100-continue, to which the server shall respond with:
    ///
    /// * 100 if the information contained in the header is sufficient to cause
    /// an immediate success,
    ///
    /// * 417 (Expectation Failed) if it cannot meet the expectation; or any
    /// other 4xx status otherwise.
    ///
    /// For example, the server may reject a request if its Content-Length is
    /// too large.
    ///
    /// No common browsers send the Expect header, but some other clients such
    /// as cURL do so by default.
    (Expect, EXPECT, b"expect");

    /// Contains the date/time after which the response is considered stale.
    ///
    /// Invalid dates, like the value 0, represent a date in the past and mean
    /// that the resource is already expired.
    ///
    /// If there is a Cache-Control header with the "max-age" or "s-max-age"
    /// directive in the response, the Expires header is ignored.
    (Expires, EXPIRES, b"expires");

    /// Contains information from the client-facing side of proxy servers that
    /// is altered or lost when a proxy is involved in the path of the request.
    ///
    /// The alternative and de-facto standard versions of this header are the
    /// X-Forwarded-For, X-Forwarded-Host and X-Forwarded-Proto headers.
    ///
    /// This header is used for debugging, statistics, and generating
    /// location-dependent content and by design it exposes privacy sensitive
    /// information, such as the IP address of the client. Therefore the user's
    /// privacy must be kept in mind when deploying this header.
    (Forwarded, FORWARDED, b"forwarded");

    /// Contains an Internet email address for a human user who controls the
    /// requesting user agent.
    ///
    /// If you are running a robotic user agent (e.g. a crawler), the From
    /// header should be sent, so you can be contacted if problems occur on
    /// servers, such as if the robot is sending excessive, unwanted, or invalid
    /// requests.
    (From, FROM, b"from");

    /// Specifies the domain name of the server and (optionally) the TCP port
    /// number on which the server is listening.
    ///
    /// If no port is given, the default port for the service requested (e.g.,
    /// "80" for an HTTP URL) is implied.
    ///
    /// A Host header field must be sent in all HTTP/1.1 request messages. A 400
    /// (Bad Request) status code will be sent to any HTTP/1.1 request message
    /// that lacks a Host header field or contains more than one.
    (Host, HOST, b"host");

    /// Makes a request conditional based on the E-Tag.
    ///
    /// For GET and HEAD methods, the server will send back the requested
    /// resource only if it matches one of the listed ETags. For PUT and other
    /// non-safe methods, it will only upload the resource in this case.
    ///
    /// The comparison with the stored ETag uses the strong comparison
    /// algorithm, meaning two files are considered identical byte to byte only.
    /// This is weakened when the  W/ prefix is used in front of the ETag.
    ///
    /// There are two common use cases:
    ///
    /// * For GET and HEAD methods, used in combination with an Range header, it
    /// can guarantee that the new ranges requested comes from the same resource
    /// than the previous one. If it doesn't match, then a 416 (Range Not
    /// Satisfiable) response is returned.
    ///
    /// * For other methods, and in particular for PUT, If-Match can be used to
    /// prevent the lost update problem. It can check if the modification of a
    /// resource that the user wants to upload will not override another change
    /// that has been done since the original resource was fetched. If the
    /// request cannot be fulfilled, the 412 (Precondition Failed) response is
    /// returned.
    (IfMatch, IF_MATCH, b"if-match");

    /// Makes a request conditional based on the modification date.
    ///
    /// The If-Modified-Since request HTTP header makes the request conditional:
    /// the server will send back the requested resource, with a 200 status,
    /// only if it has been last modified after the given date. If the request
    /// has not been modified since, the response will be a 304 without any
    /// body; the Last-Modified header will contain the date of last
    /// modification. Unlike If-Unmodified-Since, If-Modified-Since can only be
    /// used with a GET or HEAD.
    ///
    /// When used in combination with If-None-Match, it is ignored, unless the
    /// server doesn't support If-None-Match.
    ///
    /// The most common use case is to update a cached entity that has no
    /// associated ETag.
    (IfModifiedSince, IF_MODIFIED_SINCE, b"if-modified-since");

    /// Makes a request conditional based on the E-Tag.
    ///
    /// The If-None-Match HTTP request header makes the request conditional. For
    /// GET and HEAD methods, the server will send back the requested resource,
    /// with a 200 status, only if it doesn't have an ETag matching the given
    /// ones. For other methods, the request will be processed only if the
    /// eventually existing resource's ETag doesn't match any of the values
    /// listed.
    ///
    /// When the condition fails for GET and HEAD methods, then the server must
    /// return HTTP status code 304 (Not Modified). For methods that apply
    /// server-side changes, the status code 412 (Precondition Failed) is used.
    /// Note that the server generating a 304 response MUST generate any of the
    /// following header fields that would have been sent in a 200 (OK) response
    /// to the same request: Cache-Control, Content-Location, Date, ETag,
    /// Expires, and Vary.
    ///
    /// The comparison with the stored ETag uses the weak comparison algorithm,
    /// meaning two files are considered identical not only if they are
    /// identical byte to byte, but if the content is equivalent. For example,
    /// two pages that would differ only by the date of generation in the footer
    /// would be considered as identical.
    ///
    /// When used in combination with If-Modified-Since, it has precedence (if
    /// the server supports it).
    ///
    /// There are two common use cases:
    ///
    /// * For `GET` and `HEAD` methods, to update a cached entity that has an associated ETag.
    /// * For other methods, and in particular for `PUT`, `If-None-Match` used with
    /// the `*` value can be used to save a file not known to exist,
    /// guaranteeing that another upload didn't happen before, losing the data
    /// of the previous put; this problems is the variation of the lost update
    /// problem.
    (IfNoneMatch, IF_NONE_MATCH, b"if-none-match");

    /// Makes a request conditional based on range.
    ///
    /// The If-Range HTTP request header makes a range request conditional: if
    /// the condition is fulfilled, the range request will be issued and the
    /// server sends back a 206 Partial Content answer with the appropriate
    /// body. If the condition is not fulfilled, the full resource is sent back,
    /// with a 200 OK status.
    ///
    /// This header can be used either with a Last-Modified validator, or with
    /// an ETag, but not with both.
    ///
    /// The most common use case is to resume a download, to guarantee that the
    /// stored resource has not been modified since the last fragment has been
    /// received.
    (IfRange, IF_RANGE, b"if-range");

    /// Makes the request conditional based on the last modification date.
    ///
    /// The If-Unmodified-Since request HTTP header makes the request
    /// conditional: the server will send back the requested resource, or accept
    /// it in the case of a POST or another non-safe method, only if it has not
    /// been last modified after the given date. If the request has been
    /// modified after the given date, the response will be a 412 (Precondition
    /// Failed) error.
    ///
    /// There are two common use cases:
    ///
    /// * In conjunction non-safe methods, like POST, it can be used to
    /// implement an optimistic concurrency control, like done by some wikis:
    /// editions are rejected if the stored document has been modified since the
    /// original has been retrieved.
    ///
    /// * In conjunction with a range request with a If-Range header, it can be
    /// used to ensure that the new fragment requested comes from an unmodified
    /// document.
    (IfUnmodifiedSince, IF_UNMODIFIED_SINCE, b"if-unmodified-since");

    /// The Last-Modified header contains the date and time when the origin believes
    /// the resource was last modified.
    ///
    /// The value is a valid Date/Time string defined in [RFC9910](https://datatracker.ietf.org/doc/html/rfc9110#section-5.6.7)
    (LastModified, LAST_MODIFIED, b"last-modified");

    /// Allows the server to point an interested client to another resource
    /// containing metadata about the requested resource.
    (Link, LINK, b"link");

    /// Indicates the URL to redirect a page to.
    ///
    /// The Location response header indicates the URL to redirect a page to. It
    /// only provides a meaning when served with a 3xx status response.
    ///
    /// The HTTP method used to make the new request to fetch the page pointed
    /// to by Location depends of the original method and of the kind of
    /// redirection:
    ///
    /// * If 303 (See Also) responses always lead to the use of a GET method,
    /// 307 (Temporary Redirect) and 308 (Permanent Redirect) don't change the
    /// method used in the original request;
    ///
    /// * 301 (Permanent Redirect) and 302 (Found) doesn't change the method
    /// most of the time, though older user-agents may (so you basically don't
    /// know).
    ///
    /// All responses with one of these status codes send a Location header.
    ///
    /// Beside redirect response, messages with 201 (Created) status also
    /// include the Location header. It indicates the URL to the newly created
    /// resource.
    ///
    /// Location and Content-Location are different: Location indicates the
    /// target of a redirection (or the URL of a newly created resource), while
    /// Content-Location indicates the direct URL to use to access the resource
    /// when content negotiation happened, without the need of further content
    /// negotiation. Location is a header associated with the response, while
    /// Content-Location is associated with the entity returned.
    (Location, LOCATION, b"location");

    /// Indicates the max number of intermediaries the request should be sent
    /// through.
    (MaxForwards, MAX_FORWARDS, b"max-forwards");

    /// Indicates where a fetch originates from.
    ///
    /// It doesn't include any path information, but only the server name. It is
    /// sent with CORS requests, as well as with POST requests. It is similar to
    /// the Referer header, but, unlike this header, it doesn't disclose the
    /// whole path.
    (Origin, ORIGIN, b"origin");

    /// HTTP/1.0 header usually used for backwards compatibility.
    ///
    /// The Pragma HTTP/1.0 general header is an implementation-specific header
    /// that may have various effects along the request-response chain. It is
    /// used for backwards compatibility with HTTP/1.0 caches where the
    /// Cache-Control HTTP/1.1 header is not yet present.
    (Pragma, PRAGMA, b"pragma");

    /// Defines the authentication method that should be used to gain access to
    /// a proxy.
    ///
    /// Unlike `www-authenticate`, the `proxy-authenticate` header field applies
    /// only to the next outbound client on the response chain. This is because
    /// only the client that chose a given proxy is likely to have the
    /// credentials necessary for authentication. However, when multiple proxies
    /// are used within the same administrative domain, such as office and
    /// regional caching proxies within a large corporate network, it is common
    /// for credentials to be generated by the user agent and passed through the
    /// hierarchy until consumed. Hence, in such a configuration, it will appear
    /// as if Proxy-Authenticate is being forwarded because each proxy will send
    /// the same challenge set.
    ///
    /// The `proxy-authenticate` header is sent along with a `407 Proxy
    /// Authentication Required`.
    (ProxyAuthenticate, PROXY_AUTHENTICATE, b"proxy-authenticate");

    /// Contains the credentials to authenticate a user agent to a proxy server.
    ///
    /// This header is usually included after the server has responded with a
    /// 407 Proxy Authentication Required status and the Proxy-Authenticate
    /// header.
    (ProxyAuthorization, PROXY_AUTHORIZATION, b"proxy-authorization");

    /// Associates a specific cryptographic public key with a certain server.
    ///
    /// This decreases the risk of MITM attacks with forged certificates. If one
    /// or several keys are pinned and none of them are used by the server, the
    /// browser will not accept the response as legitimate, and will not display
    /// it.
    (PublicKeyPins, PUBLIC_KEY_PINS, b"public-key-pins");

    /// Sends reports of pinning violation to the report-uri specified in the
    /// header.
    ///
    /// Unlike `Public-Key-Pins`, this header still allows browsers to connect
    /// to the server if the pinning is violated.
    (PublicKeyPinsReportOnly, PUBLIC_KEY_PINS_REPORT_ONLY, b"public-key-pins-report-only");

    /// Indicates the part of a document that the server should return.
    ///
    /// Several parts can be requested with one Range header at once, and the
    /// server may send back these ranges in a multipart document. If the server
    /// sends back ranges, it uses the 206 Partial Content for the response. If
    /// the ranges are invalid, the server returns the 416 Range Not Satisfiable
    /// error. The server can also ignore the Range header and return the whole
    /// document with a 200 status code.
    (Range, RANGE, b"range");

    /// Contains the address of the previous web page from which a link to the
    /// currently requested page was followed.
    ///
    /// The Referer header allows servers to identify where people are visiting
    /// them from and may use that data for analytics, logging, or optimized
    /// caching, for example.
    (Referer, REFERER, b"referer");

    /// Governs which referrer information should be included with requests
    /// made.
    (ReferrerPolicy, REFERRER_POLICY, b"referrer-policy");

    /// Informs the web browser that the current page or frame should be
    /// refreshed.
    (Refresh, REFRESH, b"refresh");

    /// The Retry-After response HTTP header indicates how long the user agent
    /// should wait before making a follow-up request. There are two main cases
    /// this header is used:
    ///
    /// * When sent with a 503 (Service Unavailable) response, it indicates how
    /// long the service is expected to be unavailable.
    ///
    /// * When sent with a redirect response, such as 301 (Moved Permanently),
    /// it indicates the minimum time that the user agent is asked to wait
    /// before issuing the redirected request.
    (RetryAfter, RETRY_AFTER, b"retry-after");

    /// The |Sec-WebSocket-Accept| header field is used in the WebSocket
    /// opening handshake. It is sent from the server to the client to
    /// confirm that the server is willing to initiate the WebSocket
    /// connection.
    (SecWebSocketAccept, SEC_WEBSOCKET_ACCEPT, b"sec-websocket-accept");

    /// The |Sec-WebSocket-Extensions| header field is used in the WebSocket
    /// opening handshake. It is initially sent from the client to the
    /// server, and then subsequently sent from the server to the client, to
    /// agree on a set of protocol-level extensions to use for the duration
    /// of the connection.
    (SecWebSocketExtensions, SEC_WEBSOCKET_EXTENSIONS, b"sec-websocket-extensions");

    /// The |Sec-WebSocket-Key| header field is used in the WebSocket opening
    /// handshake. It is sent from the client to the server to provide part
    /// of the information used by the server to prove that it received a
    /// valid WebSocket opening handshake. This helps ensure that the server
    /// does not accept connections from non-WebSocket clients (e.g., HTTP
    /// clients) that are being abused to send data to unsuspecting WebSocket
    /// servers.
    (SecWebSocketKey, SEC_WEBSOCKET_KEY, b"sec-websocket-key");

    /// The |Sec-WebSocket-Protocol| header field is used in the WebSocket
    /// opening handshake. It is sent from the client to the server and back
    /// from the server to the client to confirm the subprotocol of the
    /// connection.  This enables scripts to both select a subprotocol and be
    /// sure that the server agreed to serve that subprotocol.
    (SecWebSocketProtocol, SEC_WEBSOCKET_PROTOCOL, b"sec-websocket-protocol");

    /// The |Sec-WebSocket-Version| header field is used in the WebSocket
    /// opening handshake.  It is sent from the client to the server to
    /// indicate the protocol version of the connection.  This enables
    /// servers to correctly interpret the opening handshake and subsequent
    /// data being sent from the data, and close the connection if the server
    /// cannot interpret that data in a safe manner.
    (SecWebSocketVersion, SEC_WEBSOCKET_VERSION, b"sec-websocket-version");

    /// Contains information about the software used by the origin server to
    /// handle the request.
    ///
    /// Overly long and detailed Server values should be avoided as they
    /// potentially reveal internal implementation details that might make it
    /// (slightly) easier for attackers to find and exploit known security
    /// holes.
    (Server, SERVER, b"server");

    /// Used to send cookies from the server to the user agent.
    (SetCookie, SET_COOKIE, b"set-cookie");

    /// Tells the client to communicate with HTTPS instead of using HTTP.
    (StrictTransportSecurity, STRICT_TRANSPORT_SECURITY, b"strict-transport-security");

    /// Informs the server of transfer encodings willing to be accepted as part
    /// of the response.
    ///
    /// See also the Transfer-Encoding response header for more details on
    /// transfer encodings. Note that chunked is always acceptable for HTTP/1.1
    /// recipients and you that don't have to specify "chunked" using the TE
    /// header. However, it is useful for setting if the client is accepting
    /// trailer fields in a chunked transfer coding using the "trailers" value.
    (Te, TE, b"te");

    /// Allows the sender to include additional fields at the end of chunked
    /// messages.
    (Trailer, TRAILER, b"trailer");

    /// Specifies the form of encoding used to safely transfer the entity to the
    /// client.
    ///
    /// `transfer-encoding` is a hop-by-hop header, that is applying to a
    /// message between two nodes, not to a resource itself. Each segment of a
    /// multi-node connection can use different `transfer-encoding` values. If
    /// you want to compress data over the whole connection, use the end-to-end
    /// header `content-encoding` header instead.
    ///
    /// When present on a response to a `HEAD` request that has no body, it
    /// indicates the value that would have applied to the corresponding `GET`
    /// message.
    (TransferEncoding, TRANSFER_ENCODING, b"transfer-encoding");

    /// Contains a string that allows identifying the requesting client's
    /// software.
    (UserAgent, USER_AGENT, b"user-agent");

    /// Used as part of the exchange to upgrade the protocol.
    (Upgrade, UPGRADE, b"upgrade");

    /// Sends a signal to the server expressing the client’s preference for an
    /// encrypted and authenticated response.
    (UpgradeInsecureRequests, UPGRADE_INSECURE_REQUESTS, b"upgrade-insecure-requests");

    /// Determines how to match future requests with cached responses.
    ///
    /// The `vary` HTTP response header determines how to match future request
    /// headers to decide whether a cached response can be used rather than
    /// requesting a fresh one from the origin server. It is used by the server
    /// to indicate which headers it used when selecting a representation of a
    /// resource in a content negotiation algorithm.
    ///
    /// The `vary` header should be set on a 304 Not Modified response exactly
    /// like it would have been set on an equivalent 200 OK response.
    (Vary, VARY, b"vary");

    /// Added by proxies to track routing.
    ///
    /// The `via` general header is added by proxies, both forward and reverse
    /// proxies, and can appear in the request headers and the response headers.
    /// It is used for tracking message forwards, avoiding request loops, and
    /// identifying the protocol capabilities of senders along the
    /// request/response chain.
    (Via, VIA, b"via");

    /// General HTTP header contains information about possible problems with
    /// the status of the message.
    ///
    /// More than one `warning` header may appear in a response. Warning header
    /// fields can in general be applied to any message, however some warn-codes
    /// are specific to caches and can only be applied to response messages.
    (Warning, WARNING, b"warning");

    /// Defines the authentication method that should be used to gain access to
    /// a resource.
    (WwwAuthenticate, WWW_AUTHENTICATE, b"www-authenticate");

    /// Marker used by the server to indicate that the MIME types advertised in
    /// the `content-type` headers should not be changed and be followed.
    ///
    /// This allows to opt-out of MIME type sniffing, or, in other words, it is
    /// a way to say that the webmasters knew what they were doing.
    ///
    /// This header was introduced by Microsoft in IE 8 as a way for webmasters
    /// to block content sniffing that was happening and could transform
    /// non-executable MIME types into executable MIME types. Since then, other
    /// browsers have introduced it, even if their MIME sniffing algorithms were
    /// less aggressive.
    ///
    /// Site security testers usually expect this header to be set.
    (XContentTypeOptions, X_CONTENT_TYPE_OPTIONS, b"x-content-type-options");

    /// Controls DNS prefetching.
    ///
    /// The `x-dns-prefetch-control` HTTP response header controls DNS
    /// prefetching, a feature by which browsers proactively perform domain name
    /// resolution on both links that the user may choose to follow as well as
    /// URLs for items referenced by the document, including images, CSS,
    /// JavaScript, and so forth.
    ///
    /// This prefetching is performed in the background, so that the DNS is
    /// likely to have been resolved by the time the referenced items are
    /// needed. This reduces latency when the user clicks a link.
    (XDnsPrefetchControl, X_DNS_PREFETCH_CONTROL, b"x-dns-prefetch-control");

    /// Indicates whether or not a browser should be allowed to render a page in
    /// a frame.
    ///
    /// Sites can use this to avoid clickjacking attacks, by ensuring that their
    /// content is not embedded into other sites.
    ///
    /// The added security is only provided if the user accessing the document
    /// is using a browser supporting `x-frame-options`.
    (XFrameOptions, X_FRAME_OPTIONS, b"x-frame-options");

    /// Stop pages from loading when an XSS attack is detected.
    ///
    /// The HTTP X-XSS-Protection response header is a feature of Internet
    /// Explorer, Chrome and Safari that stops pages from loading when they
    /// detect reflected cross-site scripting (XSS) attacks. Although these
    /// protections are largely unnecessary in modern browsers when sites
    /// implement a strong Content-Security-Policy that disables the use of
    /// inline JavaScript ('unsafe-inline'), they can still provide protections
    /// for users of older web browsers that don't yet support CSP.
    (XXssProtection, X_XSS_PROTECTION, b"x-xss-protection");
}

/// Valid header name characters
///
/// ```not_rust
///       field-name     = token
///       separators     = "(" | ")" | "<" | ">" | "@"
///                      | "," | ";" | ":" | "\" | <">
///                      | "/" | "[" | "]" | "?" | "="
///                      | "{" | "}" | SP | HT
///       token          = 1*tchar
///       tchar          = "!" / "#" / "$" / "%" / "&" / "'" / "*"
///                      / "+" / "-" / "." / "^" / "_" / "`" / "|" / "~"
///                      / DIGIT / ALPHA
///                      ; any VCHAR, except delimiters
/// ```
// HEADER_CHARS maps every byte that is 128 or larger to 0 so everything that is
// mapped by HEADER_CHARS, maps to a valid single-byte UTF-8 codepoint.
#[rustfmt::skip]
const HEADER_CHARS: [u8; 256] = [
    //  0      1      2      3      4      5      6      7      8      9
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //   x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  1x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  2x
        0,     0,     0,  b'!',     0,  b'#',  b'$',  b'%',  b'&', b'\'', //  3x
        0,     0,  b'*',  b'+',     0,  b'-',  b'.',     0,  b'0',  b'1', //  4x
     b'2',  b'3',  b'4',  b'5',  b'6',  b'7',  b'8',  b'9',     0,     0, //  5x
        0,     0,     0,     0,     0,  b'a',  b'b',  b'c',  b'd',  b'e', //  6x
     b'f',  b'g',  b'h',  b'i',  b'j',  b'k',  b'l',  b'm',  b'n',  b'o', //  7x
     b'p',  b'q',  b'r',  b's',  b't',  b'u',  b'v',  b'w',  b'x',  b'y', //  8x
     b'z',     0,     0,     0,  b'^',  b'_',  b'`',  b'a',  b'b',  b'c', //  9x
     b'd',  b'e',  b'f',  b'g',  b'h',  b'i',  b'j',  b'k',  b'l',  b'm', // 10x
     b'n',  b'o',  b'p',  b'q',  b'r',  b's',  b't',  b'u',  b'v',  b'w', // 11x
     b'x',  b'y',  b'z',     0,  b'|',     0,  b'~',     0,     0,     0, // 12x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 13x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 14x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 15x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 16x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 17x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 18x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 19x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 20x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 21x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 22x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 23x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 24x
        0,     0,     0,     0,     0,     0                              // 25x
];

/// Valid header name characters for HTTP/2.0 and HTTP/3.0
// HEADER_CHARS_H2 maps every byte that is 128 or larger to 0 so everything that is
// mapped by HEADER_CHARS_H2, maps to a valid single-byte UTF-8 codepoint.
#[rustfmt::skip]
const HEADER_CHARS_H2: [u8; 256] = [
    //  0      1      2      3      4      5      6      7      8      9
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //   x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  1x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  2x
        0,     0,     0,  b'!',  b'"',  b'#',  b'$',  b'%',  b'&', b'\'', //  3x
        0,     0,  b'*',  b'+',     0,  b'-',  b'.',     0,  b'0',  b'1', //  4x
     b'2',  b'3',  b'4',  b'5',  b'6',  b'7',  b'8',  b'9',     0,     0, //  5x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  6x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  7x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  8x
        0,     0,     0,     0,  b'^',  b'_',  b'`',  b'a',  b'b',  b'c', //  9x
     b'd',  b'e',  b'f',  b'g',  b'h',  b'i',  b'j',  b'k',  b'l',  b'm', // 10x
     b'n',  b'o',  b'p',  b'q',  b'r',  b's',  b't',  b'u',  b'v',  b'w', // 11x
     b'x',  b'y',  b'z',     0,  b'|',     0,  b'~',     0,     0,     0, // 12x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 13x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 14x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 15x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 16x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 17x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 18x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 19x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 20x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 21x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 22x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 23x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 24x
        0,     0,     0,     0,     0,     0                              // 25x
];

fn parse_hdr<'a>(
    data: &'a [u8],
    b: &'a mut [MaybeUninit<u8>; SCRATCH_BUF_SIZE],
    table: &[u8; 256],
) -> Result<HdrName<'a>, InvalidHeaderName> {
    match data.len() {
        0 => Err(InvalidHeaderName::new()),
        len @ 1..=SCRATCH_BUF_SIZE => {
            // Read from data into the buffer - transforming using `table` as we go
            data.iter()
                .zip(b.iter_mut())
                .for_each(|(index, out)| *out = MaybeUninit::new(table[*index as usize]));
            // Safety: len bytes of b were just initialized.
            let name: &'a [u8] = unsafe { slice_assume_init(&b[0..len]) };
            match StandardHeader::from_bytes(name) {
                Some(sh) => Ok(sh.into()),
                None => {
                    if name.contains(&0) {
                        Err(InvalidHeaderName::new())
                    } else {
                        Ok(HdrName::custom(name, true))
                    }
                }
            }
        }
        SCRATCH_BUF_OVERFLOW..=super::MAX_HEADER_NAME_LEN => Ok(HdrName::custom(data, false)),
        _ => Err(InvalidHeaderName::new()),
    }
}

impl<'a> From<StandardHeader> for HdrName<'a> {
    fn from(hdr: StandardHeader) -> HdrName<'a> {
        HdrName {
            inner: Repr::Standard(hdr),
        }
    }
}

impl HeaderName {
    /// Converts a slice of bytes to an HTTP header name.
    ///
    /// This function normalizes the input.
    pub fn from_bytes(src: &[u8]) -> Result<HeaderName, InvalidHeaderName> {
        let mut buf = uninit_u8_array();
        // Precondition: HEADER_CHARS is a valid table for parse_hdr().
        match parse_hdr(src, &mut buf, &HEADER_CHARS)?.inner {
            Repr::Standard(std) => Ok(std.into()),
            Repr::Custom(MaybeLower { buf, lower: true }) => {
                let buf = Bytes::copy_from_slice(buf);
                // Safety: the invariant on MaybeLower ensures buf is valid UTF-8.
                let val = unsafe { ByteStr::from_utf8_unchecked(buf) };
                Ok(Custom(val).into())
            }
            Repr::Custom(MaybeLower { buf, lower: false }) => {
                use bytes::BufMut;
                let mut dst = BytesMut::with_capacity(buf.len());

                for b in buf.iter() {
                    // HEADER_CHARS maps all bytes to valid single-byte UTF-8
                    let b = HEADER_CHARS[*b as usize];

                    if b == 0 {
                        return Err(InvalidHeaderName::new());
                    }

                    dst.put_u8(b);
                }

                // Safety: the loop above maps all bytes in buf to valid single byte
                // UTF-8 before copying them into dst. This means that dst (and hence
                // dst.freeze()) is valid UTF-8.
                let val = unsafe { ByteStr::from_utf8_unchecked(dst.freeze()) };

                Ok(Custom(val).into())
            }
        }
    }

    /// Converts a slice of bytes to an HTTP header name.
    ///
    /// This function expects the input to only contain lowercase characters.
    /// This is useful when decoding HTTP/2.0 or HTTP/3.0 headers. Both
    /// require that all headers be represented in lower case.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::header::*;
    ///
    /// // Parsing a lower case header
    /// let hdr = HeaderName::from_lowercase(b"content-length").unwrap();
    /// assert_eq!(CONTENT_LENGTH, hdr);
    ///
    /// // Parsing a header that contains uppercase characters
    /// assert!(HeaderName::from_lowercase(b"Content-Length").is_err());
    /// ```
    pub fn from_lowercase(src: &[u8]) -> Result<HeaderName, InvalidHeaderName> {
        let mut buf = uninit_u8_array();
        // Precondition: HEADER_CHARS_H2 is a valid table for parse_hdr()
        match parse_hdr(src, &mut buf, &HEADER_CHARS_H2)?.inner {
            Repr::Standard(std) => Ok(std.into()),
            Repr::Custom(MaybeLower { buf, lower: true }) => {
                let buf = Bytes::copy_from_slice(buf);
                // Safety: the invariant on MaybeLower ensures buf is valid UTF-8.
                let val = unsafe { ByteStr::from_utf8_unchecked(buf) };
                Ok(Custom(val).into())
            }
            Repr::Custom(MaybeLower { buf, lower: false }) => {
                for &b in buf.iter() {
                    // HEADER_CHARS_H2 maps all bytes that are not valid single-byte
                    // UTF-8 to 0 so this check returns an error for invalid UTF-8.
                    if HEADER_CHARS_H2[b as usize] == 0 {
                        return Err(InvalidHeaderName::new());
                    }
                }

                let buf = Bytes::copy_from_slice(buf);
                // Safety: the loop above checks that each byte of buf (either
                // version) is valid UTF-8.
                let val = unsafe { ByteStr::from_utf8_unchecked(buf) };
                Ok(Custom(val).into())
            }
        }
    }

    /// Converts a static string to a HTTP header name.
    ///
    /// This function requires the static string to only contain lowercase
    /// characters, numerals and symbols, as per the HTTP/2.0 specification
    /// and header names internal representation within this library.
    ///
    /// # Panics
    ///
    /// This function panics when the static string is a invalid header.
    ///
    /// Until [Allow panicking in constants](https://github.com/rust-lang/rfcs/pull/2345)
    /// makes its way into stable, the panic message at compile-time is
    /// going to look cryptic, but should at least point at your header value:
    ///
    /// ```text
    /// error: any use of this value will cause an error
    ///     --> http/src/header/name.rs:1241:13
    ///      |
    /// 1241 |             ([] as [u8; 0])[0]; // Invalid header name
    ///      |             ^^^^^^^^^^^^^^^^^^
    ///      |             |
    ///      |             index out of bounds: the length is 0 but the index is 0
    ///      |             inside `http::HeaderName::from_static` at http/src/header/name.rs:1241:13
    ///      |             inside `INVALID_NAME` at src/main.rs:3:34
    ///      |
    ///     ::: src/main.rs:3:1
    ///      |
    /// 3    | const INVALID_NAME: HeaderName = HeaderName::from_static("Capitalized");
    ///      | ------------------------------------------------------------------------
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::header::*;
    /// // Parsing a standard header
    /// let hdr = HeaderName::from_static("content-length");
    /// assert_eq!(CONTENT_LENGTH, hdr);
    ///
    /// // Parsing a custom header
    /// let CUSTOM_HEADER: &'static str = "custom-header";
    ///
    /// let a = HeaderName::from_lowercase(b"custom-header").unwrap();
    /// let b = HeaderName::from_static(CUSTOM_HEADER);
    /// assert_eq!(a, b);
    /// ```
    ///
    /// ```should_panic
    /// # use http::header::*;
    /// #
    /// // Parsing a header that contains invalid symbols(s):
    /// HeaderName::from_static("content{}{}length"); // This line panics!
    ///
    /// // Parsing a header that contains invalid uppercase characters.
    /// let a = HeaderName::from_static("foobar");
    /// let b = HeaderName::from_static("FOOBAR"); // This line panics!
    /// ```
    #[allow(unconditional_panic)] // required for the panic circumvention
    pub const fn from_static(src: &'static str) -> HeaderName {
        let name_bytes = src.as_bytes();
        if let Some(standard) = StandardHeader::from_bytes(name_bytes) {
            return HeaderName {
                inner: Repr::Standard(standard),
            };
        }

        if name_bytes.is_empty() || name_bytes.len() > super::MAX_HEADER_NAME_LEN || {
            let mut i = 0;
            loop {
                if i >= name_bytes.len() {
                    break false;
                } else if HEADER_CHARS_H2[name_bytes[i] as usize] == 0 {
                    break true;
                }
                i += 1;
            }
        } {
            // TODO: When msrv is bumped to larger than 1.57, this should be
            // replaced with `panic!` macro.
            // https://blog.rust-lang.org/2021/12/02/Rust-1.57.0.html#panic-in-const-contexts
            //
            // See the panics section of this method's document for details.
            #[allow(clippy::no_effect, clippy::out_of_bounds_indexing)]
            ([] as [u8; 0])[0]; // Invalid header name
        }

        HeaderName {
            inner: Repr::Custom(Custom(ByteStr::from_static(src))),
        }
    }

    /// Returns a `str` representation of the header.
    ///
    /// The returned string will always be lower case.
    #[inline]
    pub fn as_str(&self) -> &str {
        match self.inner {
            Repr::Standard(v) => v.as_str(),
            Repr::Custom(ref v) => &v.0,
        }
    }

    pub(super) fn into_bytes(self) -> Bytes {
        self.inner.into()
    }
}

impl FromStr for HeaderName {
    type Err = InvalidHeaderName;

    fn from_str(s: &str) -> Result<HeaderName, InvalidHeaderName> {
        HeaderName::from_bytes(s.as_bytes()).map_err(|_| InvalidHeaderName { _priv: () })
    }
}

impl AsRef<str> for HeaderName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for HeaderName {
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}

impl Borrow<str> for HeaderName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Debug for HeaderName {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), fmt)
    }
}

impl fmt::Display for HeaderName {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), fmt)
    }
}

impl InvalidHeaderName {
    pub(super) fn new() -> InvalidHeaderName {
        InvalidHeaderName { _priv: () }
    }
}

impl<'a> From<&'a HeaderName> for HeaderName {
    fn from(src: &'a HeaderName) -> HeaderName {
        src.clone()
    }
}

#[doc(hidden)]
impl<T> From<Repr<T>> for Bytes
where
    T: Into<Bytes>,
{
    fn from(repr: Repr<T>) -> Bytes {
        match repr {
            Repr::Standard(header) => Bytes::from_static(header.as_str().as_bytes()),
            Repr::Custom(header) => header.into(),
        }
    }
}

impl From<Custom> for Bytes {
    #[inline]
    fn from(Custom(inner): Custom) -> Bytes {
        Bytes::from(inner)
    }
}

impl<'a> TryFrom<&'a str> for HeaderName {
    type Error = InvalidHeaderName;
    #[inline]
    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        Self::from_bytes(s.as_bytes())
    }
}

impl<'a> TryFrom<&'a String> for HeaderName {
    type Error = InvalidHeaderName;
    #[inline]
    fn try_from(s: &'a String) -> Result<Self, Self::Error> {
        Self::from_bytes(s.as_bytes())
    }
}

impl<'a> TryFrom<&'a [u8]> for HeaderName {
    type Error = InvalidHeaderName;
    #[inline]
    fn try_from(s: &'a [u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(s)
    }
}

impl TryFrom<String> for HeaderName {
    type Error = InvalidHeaderName;

    #[inline]
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_bytes(s.as_bytes())
    }
}

impl TryFrom<Vec<u8>> for HeaderName {
    type Error = InvalidHeaderName;

    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        Self::from_bytes(&vec)
    }
}

#[doc(hidden)]
impl From<StandardHeader> for HeaderName {
    fn from(src: StandardHeader) -> HeaderName {
        HeaderName {
            inner: Repr::Standard(src),
        }
    }
}

#[doc(hidden)]
impl From<Custom> for HeaderName {
    fn from(src: Custom) -> HeaderName {
        HeaderName {
            inner: Repr::Custom(src),
        }
    }
}

impl<'a> PartialEq<&'a HeaderName> for HeaderName {
    #[inline]
    fn eq(&self, other: &&'a HeaderName) -> bool {
        *self == **other
    }
}

impl<'a> PartialEq<HeaderName> for &'a HeaderName {
    #[inline]
    fn eq(&self, other: &HeaderName) -> bool {
        *other == *self
    }
}

impl PartialEq<str> for HeaderName {
    /// Performs a case-insensitive comparison of the string against the header
    /// name
    ///
    /// # Examples
    ///
    /// ```
    /// use http::header::CONTENT_LENGTH;
    ///
    /// assert_eq!(CONTENT_LENGTH, "content-length");
    /// assert_eq!(CONTENT_LENGTH, "Content-Length");
    /// assert_ne!(CONTENT_LENGTH, "content length");
    /// ```
    #[inline]
    fn eq(&self, other: &str) -> bool {
        eq_ignore_ascii_case(self.as_ref(), other.as_bytes())
    }
}

impl PartialEq<HeaderName> for str {
    /// Performs a case-insensitive comparison of the string against the header
    /// name
    ///
    /// # Examples
    ///
    /// ```
    /// use http::header::CONTENT_LENGTH;
    ///
    /// assert_eq!(CONTENT_LENGTH, "content-length");
    /// assert_eq!(CONTENT_LENGTH, "Content-Length");
    /// assert_ne!(CONTENT_LENGTH, "content length");
    /// ```
    #[inline]
    fn eq(&self, other: &HeaderName) -> bool {
        *other == *self
    }
}

impl<'a> PartialEq<&'a str> for HeaderName {
    /// Performs a case-insensitive comparison of the string against the header
    /// name
    #[inline]
    fn eq(&self, other: &&'a str) -> bool {
        *self == **other
    }
}

impl<'a> PartialEq<HeaderName> for &'a str {
    /// Performs a case-insensitive comparison of the string against the header
    /// name
    #[inline]
    fn eq(&self, other: &HeaderName) -> bool {
        *other == *self
    }
}

impl fmt::Debug for InvalidHeaderName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InvalidHeaderName")
            // skip _priv noise
            .finish()
    }
}

impl fmt::Display for InvalidHeaderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid HTTP header name")
    }
}

impl Error for InvalidHeaderName {}

// ===== HdrName =====

impl<'a> HdrName<'a> {
    // Precondition: if lower then buf is valid UTF-8
    fn custom(buf: &'a [u8], lower: bool) -> HdrName<'a> {
        HdrName {
            // Invariant (on MaybeLower): follows from the precondition
            inner: Repr::Custom(MaybeLower { buf, lower }),
        }
    }

    pub fn from_bytes<F, U>(hdr: &[u8], f: F) -> Result<U, InvalidHeaderName>
    where
        F: FnOnce(HdrName<'_>) -> U,
    {
        let mut buf = uninit_u8_array();
        // Precondition: HEADER_CHARS is a valid table for parse_hdr().
        let hdr = parse_hdr(hdr, &mut buf, &HEADER_CHARS)?;
        Ok(f(hdr))
    }

    pub fn from_static<F, U>(hdr: &'static str, f: F) -> U
    where
        F: FnOnce(HdrName<'_>) -> U,
    {
        let mut buf = uninit_u8_array();
        let hdr =
            // Precondition: HEADER_CHARS is a valid table for parse_hdr().
            parse_hdr(hdr.as_bytes(), &mut buf, &HEADER_CHARS).expect("static str is invalid name");
        f(hdr)
    }
}

#[doc(hidden)]
impl<'a> From<HdrName<'a>> for HeaderName {
    fn from(src: HdrName<'a>) -> HeaderName {
        match src.inner {
            Repr::Standard(s) => HeaderName {
                inner: Repr::Standard(s),
            },
            Repr::Custom(maybe_lower) => {
                if maybe_lower.lower {
                    let buf = Bytes::copy_from_slice(maybe_lower.buf);
                    // Safety: the invariant on MaybeLower ensures buf is valid UTF-8.
                    let byte_str = unsafe { ByteStr::from_utf8_unchecked(buf) };

                    HeaderName {
                        inner: Repr::Custom(Custom(byte_str)),
                    }
                } else {
                    use bytes::BufMut;
                    let mut dst = BytesMut::with_capacity(maybe_lower.buf.len());

                    for b in maybe_lower.buf.iter() {
                        // HEADER_CHARS maps each byte to a valid single-byte UTF-8
                        // codepoint.
                        dst.put_u8(HEADER_CHARS[*b as usize]);
                    }

                    // Safety: the loop above maps each byte of maybe_lower.buf to a
                    // valid single-byte UTF-8 codepoint before copying it into dst.
                    // dst (and hence dst.freeze()) is thus valid UTF-8.
                    let buf = unsafe { ByteStr::from_utf8_unchecked(dst.freeze()) };

                    HeaderName {
                        inner: Repr::Custom(Custom(buf)),
                    }
                }
            }
        }
    }
}

#[doc(hidden)]
impl<'a> PartialEq<HdrName<'a>> for HeaderName {
    #[inline]
    fn eq(&self, other: &HdrName<'a>) -> bool {
        match self.inner {
            Repr::Standard(a) => match other.inner {
                Repr::Standard(b) => a == b,
                _ => false,
            },
            Repr::Custom(Custom(ref a)) => match other.inner {
                Repr::Custom(ref b) => {
                    if b.lower {
                        a.as_bytes() == b.buf
                    } else {
                        eq_ignore_ascii_case(a.as_bytes(), b.buf)
                    }
                }
                _ => false,
            },
        }
    }
}

// ===== Custom =====

impl Hash for Custom {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        hasher.write(self.0.as_bytes())
    }
}

// ===== MaybeLower =====

impl<'a> Hash for MaybeLower<'a> {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        if self.lower {
            hasher.write(self.buf);
        } else {
            for &b in self.buf {
                hasher.write(&[HEADER_CHARS[b as usize]]);
            }
        }
    }
}

// Assumes that the left hand side is already lower case
#[inline]
fn eq_ignore_ascii_case(lower: &[u8], s: &[u8]) -> bool {
    if lower.len() != s.len() {
        return false;
    }

    lower
        .iter()
        .zip(s)
        .all(|(a, b)| *a == HEADER_CHARS[*b as usize])
}

// Utility functions for MaybeUninit<>. These are drawn from unstable API's on
// MaybeUninit<> itself.
const SCRATCH_BUF_SIZE: usize = 64;
const SCRATCH_BUF_OVERFLOW: usize = SCRATCH_BUF_SIZE + 1;

fn uninit_u8_array() -> [MaybeUninit<u8>; SCRATCH_BUF_SIZE] {
    let arr = MaybeUninit::<[MaybeUninit<u8>; SCRATCH_BUF_SIZE]>::uninit();
    // Safety: assume_init() is claiming that an array of MaybeUninit<>
    // has been initialized, but MaybeUninit<>'s do not require initialization.
    unsafe { arr.assume_init() }
}

// Assuming all the elements are initialized, get a slice of them.
//
// Safety: All elements of `slice` must be initialized to prevent
// undefined behavior.
unsafe fn slice_assume_init<T>(slice: &[MaybeUninit<T>]) -> &[T] {
    &*(slice as *const [MaybeUninit<T>] as *const [T])
}

#[cfg(test)]
mod tests {
    use self::StandardHeader::Vary;
    use super::*;

    #[test]
    fn test_bounds() {
        fn check_bounds<T: Sync + Send>() {}
        check_bounds::<HeaderName>();
    }

    #[test]
    fn test_parse_invalid_headers() {
        for i in 0..128 {
            let hdr = vec![1u8; i];
            assert!(
                HeaderName::from_bytes(&hdr).is_err(),
                "{} invalid header chars did not fail",
                i
            );
        }
    }

    const ONE_TOO_LONG: &[u8] = &[b'a'; super::super::MAX_HEADER_NAME_LEN + 1];

    #[test]
    fn test_invalid_name_lengths() {
        assert!(
            HeaderName::from_bytes(&[]).is_err(),
            "zero-length header name is an error",
        );

        let long = &ONE_TOO_LONG[0..super::super::MAX_HEADER_NAME_LEN];

        let long_str = std::str::from_utf8(long).unwrap();
        assert_eq!(HeaderName::from_static(long_str), long_str); // shouldn't panic!

        assert!(
            HeaderName::from_bytes(long).is_ok(),
            "max header name length is ok",
        );
        assert!(
            HeaderName::from_bytes(ONE_TOO_LONG).is_err(),
            "longer than max header name length is an error",
        );
    }

    #[test]
    #[should_panic]
    fn test_static_invalid_name_lengths() {
        // Safety: ONE_TOO_LONG contains only the UTF-8 safe, single-byte codepoint b'a'.
        let _ = HeaderName::from_static(unsafe { std::str::from_utf8_unchecked(ONE_TOO_LONG) });
    }

    #[test]
    fn test_from_hdr_name() {
        use self::StandardHeader::Vary;

        let name = HeaderName::from(HdrName {
            inner: Repr::Standard(Vary),
        });

        assert_eq!(name.inner, Repr::Standard(Vary));

        let name = HeaderName::from(HdrName {
            inner: Repr::Custom(MaybeLower {
                buf: b"hello-world",
                lower: true,
            }),
        });

        assert_eq!(
            name.inner,
            Repr::Custom(Custom(ByteStr::from_static("hello-world")))
        );

        let name = HeaderName::from(HdrName {
            inner: Repr::Custom(MaybeLower {
                buf: b"Hello-World",
                lower: false,
            }),
        });

        assert_eq!(
            name.inner,
            Repr::Custom(Custom(ByteStr::from_static("hello-world")))
        );
    }

    #[test]
    fn test_eq_hdr_name() {
        use self::StandardHeader::Vary;

        let a = HeaderName {
            inner: Repr::Standard(Vary),
        };
        let b = HdrName {
            inner: Repr::Standard(Vary),
        };

        assert_eq!(a, b);

        let a = HeaderName {
            inner: Repr::Custom(Custom(ByteStr::from_static("vaary"))),
        };
        assert_ne!(a, b);

        let b = HdrName {
            inner: Repr::Custom(MaybeLower {
                buf: b"vaary",
                lower: true,
            }),
        };

        assert_eq!(a, b);

        let b = HdrName {
            inner: Repr::Custom(MaybeLower {
                buf: b"vaary",
                lower: false,
            }),
        };

        assert_eq!(a, b);

        let b = HdrName {
            inner: Repr::Custom(MaybeLower {
                buf: b"VAARY",
                lower: false,
            }),
        };

        assert_eq!(a, b);

        let a = HeaderName {
            inner: Repr::Standard(Vary),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn test_from_static_std() {
        let a = HeaderName {
            inner: Repr::Standard(Vary),
        };

        let b = HeaderName::from_static("vary");
        assert_eq!(a, b);

        let b = HeaderName::from_static("vaary");
        assert_ne!(a, b);
    }

    #[test]
    #[should_panic]
    fn test_from_static_std_uppercase() {
        HeaderName::from_static("Vary");
    }

    #[test]
    #[should_panic]
    fn test_from_static_std_symbol() {
        HeaderName::from_static("vary{}");
    }

    // MaybeLower { lower: true }
    #[test]
    fn test_from_static_custom_short() {
        let a = HeaderName {
            inner: Repr::Custom(Custom(ByteStr::from_static("customheader"))),
        };
        let b = HeaderName::from_static("customheader");
        assert_eq!(a, b);
    }

    #[test]
    #[should_panic]
    fn test_from_static_custom_short_uppercase() {
        HeaderName::from_static("custom header");
    }

    #[test]
    #[should_panic]
    fn test_from_static_custom_short_symbol() {
        HeaderName::from_static("CustomHeader");
    }

    // MaybeLower { lower: false }
    #[test]
    fn test_from_static_custom_long() {
        let a = HeaderName {
            inner: Repr::Custom(Custom(ByteStr::from_static(
                "longer-than-63--thisheaderislongerthansixtythreecharactersandthushandleddifferent",
            ))),
        };
        let b = HeaderName::from_static(
            "longer-than-63--thisheaderislongerthansixtythreecharactersandthushandleddifferent",
        );
        assert_eq!(a, b);
    }

    #[test]
    #[should_panic]
    fn test_from_static_custom_long_uppercase() {
        HeaderName::from_static(
            "Longer-Than-63--ThisHeaderIsLongerThanSixtyThreeCharactersAndThusHandledDifferent",
        );
    }

    #[test]
    #[should_panic]
    fn test_from_static_custom_long_symbol() {
        HeaderName::from_static(
            "longer-than-63--thisheader{}{}{}{}islongerthansixtythreecharactersandthushandleddifferent"
        );
    }

    #[test]
    fn test_from_static_custom_single_char() {
        let a = HeaderName {
            inner: Repr::Custom(Custom(ByteStr::from_static("a"))),
        };
        let b = HeaderName::from_static("a");
        assert_eq!(a, b);
    }

    #[test]
    #[should_panic]
    fn test_from_static_empty() {
        HeaderName::from_static("");
    }

    #[test]
    fn test_all_tokens() {
        HeaderName::from_static("!#$%&'*+-.^_`|~0123456789abcdefghijklmnopqrstuvwxyz");
    }

    #[test]
    fn test_from_lowercase() {
        HeaderName::from_lowercase(&[0; 10]).unwrap_err();
        HeaderName::from_lowercase(&[b'A'; 10]).unwrap_err();
        HeaderName::from_lowercase(&[0x1; 10]).unwrap_err();
        HeaderName::from_lowercase(&[0xFF; 10]).unwrap_err();
        //HeaderName::from_lowercase(&[0; 100]).unwrap_err();
        HeaderName::from_lowercase(&[b'A'; 100]).unwrap_err();
        HeaderName::from_lowercase(&[0x1; 100]).unwrap_err();
        HeaderName::from_lowercase(&[0xFF; 100]).unwrap_err();
    }
}
