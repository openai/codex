use std::fmt::{self, Display};
use std::io::{self, Write};
use std::ops::Range;
use std::time;

use base64::{prelude::BASE64_STANDARD, Engine};
use log::debug;
use url::Url;

#[cfg(feature = "cookies")]
use cookie::Cookie;

use crate::agent::RedirectAuthHeaders;
use crate::body::{self, BodySize, Payload, SizedReader};
use crate::error::{Error, ErrorKind};
use crate::header;
use crate::header::{get_header, Header};
use crate::proxy::Proto;
use crate::resolve::ArcResolver;
use crate::response::Response;
use crate::stream::{self, connect_test, Stream};
use crate::Agent;

/// A Unit is fully-built Request, ready to execute.
///
/// *Internal API*
#[derive(Clone)]
pub(crate) struct Unit {
    pub agent: Agent,
    pub method: String,
    pub url: Url,
    is_chunked: bool,
    headers: Vec<Header>,
    pub deadline: Option<time::Instant>,
}

impl Unit {
    //

    pub(crate) fn new(
        agent: &Agent,
        method: &str,
        url: &Url,
        mut headers: Vec<Header>,
        body: &SizedReader,
        deadline: Option<time::Instant>,
    ) -> Self {
        //

        let (is_transfer_encoding_set, mut is_chunked) = get_header(&headers, "transfer-encoding")
            // if the user has set an encoding header, obey that.
            .map(|enc| {
                let is_transfer_encoding_set = !enc.is_empty();
                let last_encoding = enc.split(',').last();
                let is_chunked = last_encoding
                    .map(|last_enc| last_enc.trim() == "chunked")
                    .unwrap_or(false);
                (is_transfer_encoding_set, is_chunked)
            })
            // otherwise, no chunking.
            .unwrap_or((false, false));

        let mut extra_headers = {
            let mut extra = vec![];

            // chunking and Content-Length headers are mutually exclusive
            // also don't write this if the user has set it themselves
            if !is_chunked && get_header(&headers, "content-length").is_none() {
                // if the payload is of known size (everything beside an unsized reader), set
                // Content-Length,
                // otherwise, use the chunked Transfer-Encoding (only if no other Transfer-Encoding
                // has been set
                match body.size {
                    BodySize::Known(size) => {
                        extra.push(Header::new("Content-Length", &format!("{}", size)))
                    }
                    BodySize::Unknown => {
                        if !is_transfer_encoding_set {
                            extra.push(Header::new("Transfer-Encoding", "chunked"));
                            is_chunked = true;
                        }
                    }
                    BodySize::Empty => {}
                }
            }

            let username = url.username();
            let password = url.password().unwrap_or("");
            if (!username.is_empty() || !password.is_empty())
                && get_header(&headers, "authorization").is_none()
            {
                let encoded = BASE64_STANDARD.encode(format!("{}:{}", username, password));
                extra.push(Header::new("Authorization", &format!("Basic {}", encoded)));
            }

            #[cfg(feature = "cookies")]
            extra.extend(extract_cookies(agent, url).into_iter());

            extra
        };

        headers.append(&mut extra_headers);

        Unit {
            agent: agent.clone(),
            method: method.to_string(),
            url: url.clone(),
            is_chunked,
            headers,
            deadline,
        }
    }

    pub fn resolver(&self) -> ArcResolver {
        self.agent.state.resolver.clone()
    }

    #[cfg(test)]
    pub fn header(&self, name: &str) -> Option<&str> {
        header::get_header(&self.headers, name)
    }
    #[cfg(test)]
    pub fn has(&self, name: &str) -> bool {
        header::has_header(&self.headers, name)
    }
    #[cfg(test)]
    pub fn all(&self, name: &str) -> Vec<&str> {
        header::get_all_headers(&self.headers, name)
    }

    // Returns true if this request, with the provided body, is retryable.
    pub(crate) fn is_retryable(&self, body: &SizedReader) -> bool {
        // Per https://tools.ietf.org/html/rfc7231#section-8.1.3
        // these methods are idempotent.
        let idempotent = match self.method.as_str() {
            "DELETE" | "GET" | "HEAD" | "OPTIONS" | "PUT" | "TRACE" => true,
            _ => false,
        };
        // Unsized bodies aren't retryable because we can't rewind the reader.
        // Sized bodies are retryable only if they are zero-length because of
        // coincidences of the current implementation - the function responsible
        // for retries doesn't have a way to replay a Payload.
        let retryable_body = match body.size {
            BodySize::Unknown => false,
            BodySize::Known(0) => true,
            BodySize::Known(_) => false,
            BodySize::Empty => true,
        };

        idempotent && retryable_body
    }
}

/// Perform a connection. Follows redirects.
pub(crate) fn connect(
    mut unit: Unit,
    use_pooled: bool,
    mut body: SizedReader,
) -> Result<Response, Error> {
    let mut history = vec![];
    let mut resp = loop {
        let resp = connect_inner(&unit, use_pooled, body, &history)?;

        // handle redirects
        if !(300..399).contains(&resp.status()) || unit.agent.config.redirects == 0 {
            break resp;
        }
        if history.len() + 1 >= unit.agent.config.redirects as usize {
            return Err(ErrorKind::TooManyRedirects.msg(format!(
                "reached max redirects ({})",
                unit.agent.config.redirects
            )));
        }
        // the location header
        let location = match resp.header("location") {
            Some(l) => l,
            None => break resp,
        };

        let url = &unit.url;
        let method = &unit.method;
        // join location header to current url in case it is relative
        let new_url = url.join(location).map_err(|e| {
            ErrorKind::InvalidUrl
                .msg(format!("Bad redirection: {}", location))
                .src(e)
        })?;

        // perform the redirect differently depending on 3xx code.
        let new_method = match resp.status() {
            // this is to follow how curl does it. POST, PUT etc change
            // to GET on a redirect.
            301 | 302 | 303 => match &method[..] {
                "GET" | "HEAD" => unit.method,
                _ => "GET".into(),
            },
            // never change the method for 307/308
            // only resend the request if it cannot have a body
            // NOTE: DELETE is intentionally excluded: https://stackoverflow.com/questions/299628
            307 | 308 if ["GET", "HEAD", "OPTIONS", "TRACE"].contains(&method.as_str()) => {
                unit.method
            }
            _ => break resp,
        };

        let keep_auth_header = can_propagate_authorization_on_redirect(
            &unit.agent.config.redirect_auth_headers,
            url,
            &new_url,
        );

        debug!("redirect {} {} -> {}", resp.status(), url, new_url);
        history.push(unit.url);
        body = Payload::Empty.into_read();

        // reuse the previous header vec on redirects.
        let mut headers = unit.headers;

        // on redirects we don't want to keep "content-length". we also might want to
        // strip away "authorization" and "cookie" to ensure credentials are not leaked.
        headers.retain(|h| {
            !h.is_name("content-length")
                && !h.is_name("cookie")
                && (!h.is_name("authorization") || keep_auth_header)
        });

        // recreate the unit to get a new hostname and cookies for the new host.
        unit = Unit::new(
            &unit.agent,
            &new_method,
            &new_url,
            headers,
            &body,
            unit.deadline,
        );
    };
    resp.history = history;
    Ok(resp)
}

/// Perform a connection. Does not follow redirects.
fn connect_inner(
    unit: &Unit,
    use_pooled: bool,
    body: SizedReader,
    history: &[Url],
) -> Result<Response, Error> {
    let host = unit
        .url
        .host_str()
        // This unwrap is ok because Request::parse_url() ensure there is always a host present.
        .unwrap();
    let url = &unit.url;
    let method = &unit.method;
    // open socket
    let (mut stream, is_recycled) = connect_socket(unit, host, use_pooled)?;

    if is_recycled {
        debug!("sending request (reused connection) {} {}", method, url);
    } else {
        debug!("sending request {} {}", method, url);
    }

    let send_result = send_prelude(unit, &mut stream);

    if let Err(err) = send_result {
        if is_recycled {
            debug!("retrying request early {} {}: {}", method, url, err);
            // we try open a new connection, this time there will be
            // no connection in the pool. don't use it.
            // NOTE: this recurses at most once because `use_pooled` is `false`.
            return connect_inner(unit, false, body, history);
        } else {
            // not a pooled connection, propagate the error.
            return Err(err.into());
        }
    }
    let retryable = unit.is_retryable(&body);

    // send the body (which can be empty now depending on redirects)
    body::send_body(body, unit.is_chunked, &mut stream)?;

    // start reading the response to process cookies and redirects.
    // TODO: this unit.clone() bothers me. At this stage, we're not
    // going to use the unit (much) anymore, and it should be possible
    // to have ownership of it and pass it into the Response.
    let result = Response::do_from_stream(stream, unit.clone());

    // https://tools.ietf.org/html/rfc7230#section-6.3.1
    // When an inbound connection is closed prematurely, a client MAY
    // open a new connection and automatically retransmit an aborted
    // sequence of requests if all of those requests have idempotent
    // methods.
    //
    // We choose to retry only requests that used a recycled connection
    // from the ConnectionPool, since those are most likely to have
    // reached a server-side timeout. Note that this means we may do
    // up to N+1 total tries, where N is max_idle_connections_per_host.
    let resp = match result {
        Err(err) if err.connection_closed() && retryable && is_recycled => {
            debug!("retrying request {} {}: {}", method, url, err);
            let empty = Payload::Empty.into_read();
            // NOTE: this recurses at most once because `use_pooled` is `false`.
            return connect_inner(unit, false, empty, history);
        }
        Err(e) => return Err(e),
        Ok(resp) => resp,
    };

    // squirrel away cookies
    #[cfg(feature = "cookies")]
    save_cookies(unit, &resp);

    debug!("response {} to {} {}", resp.status(), method, url);

    // release the response
    Ok(resp)
}

#[cfg(feature = "cookies")]
fn extract_cookies(agent: &Agent, url: &Url) -> Option<Header> {
    let header_value = agent
        .state
        .cookie_tin
        .get_request_cookies(url)
        .iter()
        // This guards against sending rfc non-compliant cookies, even if the user has
        // "prepped" their local cookie store with such cookies.
        .filter(|c| {
            let is_ok = is_cookie_rfc_compliant(c);
            if !is_ok {
                debug!("do not send non compliant cookie: {:?}", c);
            }
            is_ok
        })
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(";");
    match header_value.as_str() {
        "" => None,
        val => Some(Header::new("Cookie", val)),
    }
}

/// Connect the socket, either by using the pool or grab a new one.
fn connect_socket(unit: &Unit, hostname: &str, use_pooled: bool) -> Result<(Stream, bool), Error> {
    match unit.url.scheme() {
        "http" | "https" | "test" => (),
        scheme => return Err(ErrorKind::UnknownScheme.msg(format!("unknown scheme '{}'", scheme))),
    };
    if unit.url.scheme() != "https" && unit.agent.config.https_only {
        return Err(ErrorKind::InsecureRequestHttpsOnly
            .msg("can't perform non https request with https_only set"));
    }
    if use_pooled {
        let pool = &unit.agent.state.pool;
        let proxy = &unit.agent.config.proxy;
        // The connection may have been closed by the server
        // due to idle timeout while it was sitting in the pool.
        // Loop until we find one that is still good or run out of connections.
        while let Some(stream) = pool.try_get_connection(&unit.url, proxy.clone()) {
            let server_closed = stream.server_closed()?;
            if !server_closed {
                return Ok((stream, true));
            }
            debug!("dropping stream from pool; closed by server: {:?}", stream);
        }
    }
    let stream = match unit.url.scheme() {
        "http" => stream::connect_http(unit, hostname),
        "https" => stream::connect_https(unit, hostname),
        "test" => connect_test(unit),
        scheme => Err(ErrorKind::UnknownScheme.msg(format!("unknown scheme {}", scheme))),
    };
    Ok((stream?, false))
}

fn can_propagate_authorization_on_redirect(
    redirect_auth_headers: &RedirectAuthHeaders,
    prev_url: &Url,
    url: &Url,
) -> bool {
    fn scheme_is_https(url: &Url) -> bool {
        url.scheme() == "https" || (cfg!(test) && url.scheme() == "test")
    }

    match redirect_auth_headers {
        RedirectAuthHeaders::Never => false,
        RedirectAuthHeaders::SameHost => {
            let host = url.host_str();
            let is_https = scheme_is_https(url);

            let prev_host = prev_url.host_str();
            let prev_is_https = scheme_is_https(prev_url);

            let same_scheme_or_more_secure =
                is_https == prev_is_https || (!prev_is_https && is_https);

            host == prev_host && same_scheme_or_more_secure
        }
    }
}

/// Send request line + headers (all up until the body).
#[allow(clippy::write_with_newline)]
fn send_prelude(unit: &Unit, stream: &mut Stream) -> io::Result<()> {
    // build into a buffer and send in one go.
    let mut prelude = PreludeBuilder::new();

    let path = if let Some(proxy) = &unit.agent.config.proxy {
        // HTTP proxies require the path to be in absolute URI form
        // https://www.rfc-editor.org/rfc/rfc7230#section-5.3.2
        match proxy.proto {
            Proto::HTTP => match unit.url.port() {
                Some(port) => format!(
                    "{}://{}:{}{}",
                    unit.url.scheme(),
                    unit.url.host().unwrap(),
                    port,
                    unit.url.path()
                ),
                None => format!(
                    "{}://{}{}",
                    unit.url.scheme(),
                    unit.url.host().unwrap(),
                    unit.url.path()
                ),
            },
            _ => unit.url.path().into(),
        }
    } else {
        unit.url.path().into()
    };

    // request line
    prelude.write_request_line(&unit.method, &path, unit.url.query().unwrap_or_default())?;

    // host header if not set by user.
    if !header::has_header(&unit.headers, "host") {
        let host = unit.url.host().unwrap();
        match unit.url.port() {
            Some(port) => {
                let scheme_default: u16 = match unit.url.scheme() {
                    "http" => 80,
                    "https" => 443,
                    _ => 0,
                };
                if scheme_default != 0 && scheme_default == port {
                    prelude.write_header("Host", host)?;
                } else {
                    prelude.write_header("Host", format_args!("{}:{}", host, port))?;
                }
            }
            None => {
                prelude.write_header("Host", host)?;
            }
        }
    }
    if !header::has_header(&unit.headers, "user-agent") {
        prelude.write_header("User-Agent", &unit.agent.config.user_agent)?;
    }
    if !header::has_header(&unit.headers, "accept") {
        prelude.write_header("Accept", "*/*")?;
    }

    // other headers
    for header in &unit.headers {
        if let Some(v) = header.value() {
            if is_header_sensitive(header) {
                prelude.write_sensitive_header(header.name(), v)?;
            } else {
                prelude.write_header(header.name(), v)?;
            }
        }
    }

    // finish
    prelude.finish()?;

    debug!("writing prelude: {}", prelude);
    // write all to the wire
    stream.write_all(prelude.as_slice())?;

    Ok(())
}

fn is_header_sensitive(header: &Header) -> bool {
    header.is_name("Authorization") || header.is_name("Cookie")
}

struct PreludeBuilder {
    prelude: Vec<u8>,
    // Sensitive information to be omitted in debug logging
    sensitive_spans: Vec<Range<usize>>,
}

impl PreludeBuilder {
    fn new() -> Self {
        PreludeBuilder {
            prelude: Vec::with_capacity(256),
            sensitive_spans: Vec::new(),
        }
    }

    fn write_request_line(&mut self, method: &str, path: &str, query: &str) -> io::Result<()> {
        write!(self.prelude, "{} {}", method, path,)?;
        if !query.is_empty() {
            write!(self.prelude, "?{}", query)?;
        }
        write!(self.prelude, " HTTP/1.1\r\n")?;
        Ok(())
    }

    fn write_header(&mut self, name: &str, value: impl Display) -> io::Result<()> {
        write!(self.prelude, "{}: {}\r\n", name, value)
    }

    fn write_sensitive_header(&mut self, name: &str, value: impl Display) -> io::Result<()> {
        write!(self.prelude, "{}: ", name)?;
        let start = self.prelude.len();
        write!(self.prelude, "{}", value)?;
        let end = self.prelude.len();
        self.sensitive_spans.push(start..end);
        write!(self.prelude, "\r\n")?;
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        write!(self.prelude, "\r\n")
    }

    fn as_slice(&self) -> &[u8] {
        &self.prelude
    }
}

impl fmt::Display for PreludeBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pos = 0;
        for span in &self.sensitive_spans {
            write!(
                f,
                "{}",
                String::from_utf8_lossy(&self.prelude[pos..span.start])
            )?;
            write!(f, "***")?;
            pos = span.end;
        }
        write!(
            f,
            "{}",
            String::from_utf8_lossy(&self.prelude[pos..]).trim_end()
        )?;
        Ok(())
    }
}

/// Investigate a response for "Set-Cookie" headers.
#[cfg(feature = "cookies")]
fn save_cookies(unit: &Unit, resp: &Response) {
    //

    let headers = resp.all("set-cookie");
    // Avoid locking if there are no cookie headers
    if headers.is_empty() {
        return;
    }
    let cookies = headers.into_iter().flat_map(|header_value| {
        debug!(
            "received 'set-cookie: {}' from {} {}",
            header_value, unit.method, unit.url
        );
        match Cookie::parse(header_value.to_string()) {
            Err(_) => None,
            Ok(c) => {
                // This guards against accepting rfc non-compliant cookies from a host.
                if is_cookie_rfc_compliant(&c) {
                    Some(c)
                } else {
                    debug!("ignore incoming non compliant cookie: {:?}", c);
                    None
                }
            }
        }
    });
    unit.agent
        .state
        .cookie_tin
        .store_response_cookies(cookies, &unit.url.clone());
}

#[cfg(feature = "cookies")]
fn is_cookie_rfc_compliant(cookie: &Cookie) -> bool {
    // https://tools.ietf.org/html/rfc6265#page-9
    // set-cookie-header = "Set-Cookie:" SP set-cookie-string
    // set-cookie-string = cookie-pair *( ";" SP cookie-av )
    // cookie-pair       = cookie-name "=" cookie-value
    // cookie-name       = token
    // cookie-value      = *cookie-octet / ( DQUOTE *cookie-octet DQUOTE )
    // cookie-octet      = %x21 / %x23-2B / %x2D-3A / %x3C-5B / %x5D-7E
    //                       ; US-ASCII characters excluding CTLs,
    //                       ; whitespace DQUOTE, comma, semicolon,
    //                       ; and backslash
    // token             = <token, defined in [RFC2616], Section 2.2>

    // https://tools.ietf.org/html/rfc2616#page-17
    // CHAR           = <any US-ASCII character (octets 0 - 127)>
    // ...
    //        CTL            = <any US-ASCII control character
    //                         (octets 0 - 31) and DEL (127)>
    // ...
    //        token          = 1*<any CHAR except CTLs or separators>
    //        separators     = "(" | ")" | "<" | ">" | "@"
    //                       | "," | ";" | ":" | "\" | <">
    //                       | "/" | "[" | "]" | "?" | "="
    //                       | "{" | "}" | SP | HT

    fn is_valid_name(b: &u8) -> bool {
        header::is_tchar(b)
    }

    fn is_valid_value(b: &u8) -> bool {
        b.is_ascii()
            && !b.is_ascii_control()
            && !b.is_ascii_whitespace()
            && *b != b'"'
            && *b != b','
            && *b != b';'
            && *b != b'\\'
    }

    let name = cookie.name().as_bytes();

    let valid_name = name.iter().all(is_valid_name);

    if !valid_name {
        log::trace!("cookie name is not valid: {:?}", cookie.name());
        return false;
    }

    let value = cookie.value().as_bytes();

    let valid_value = value.iter().all(is_valid_value);

    if !valid_value {
        log::trace!("cookie value is not valid: {:?}", cookie.value());
        return false;
    }

    true
}

#[cfg(test)]
#[cfg(feature = "cookies")]
mod tests {
    use cookie::Cookie;
    use cookie_store::CookieStore;

    use super::*;

    use crate::Agent;
    ///////////////////// COOKIE TESTS //////////////////////////////

    #[test]
    fn match_cookies_returns_one_header() {
        let agent = Agent::new();
        let url: Url = "https://crates.io/".parse().unwrap();
        let cookie1: Cookie = "cookie1=value1; Domain=crates.io; Path=/".parse().unwrap();
        let cookie2: Cookie = "cookie2=value2; Domain=crates.io; Path=/".parse().unwrap();
        agent
            .state
            .cookie_tin
            .store_response_cookies(vec![cookie1, cookie2].into_iter(), &url);

        // There's no guarantee to the order in which cookies are defined.
        // Ensure that they're either in one order or the other.
        let result = extract_cookies(&agent, &url);
        let order1 = "cookie1=value1;cookie2=value2";
        let order2 = "cookie2=value2;cookie1=value1";

        assert!(
            result == Some(Header::new("Cookie", order1))
                || result == Some(Header::new("Cookie", order2))
        );
    }

    #[test]
    fn not_send_illegal_cookies() {
        // This prepares a cookie store with a cookie that isn't legal
        // according to the relevant rfcs. ureq should not send this.
        let empty = b"";
        #[allow(deprecated)]
        let mut store = CookieStore::load_json(&empty[..]).unwrap();
        let url = Url::parse("https://mydomain.com").unwrap();
        let cookie = Cookie::new("borked///", "illegal<>//");
        store.insert_raw(&cookie, &url).unwrap();

        let agent = crate::builder().cookie_store(store).build();
        let cookies = extract_cookies(&agent, &url);
        assert_eq!(cookies, None);
    }

    #[test]
    fn check_cookie_crate_allows_illegal() {
        // This test is there to see whether the cookie crate enforces
        // https://tools.ietf.org/html/rfc6265#page-9
        // https://tools.ietf.org/html/rfc2616#page-17
        // for cookie name or cookie value.
        // As long as it doesn't, we do additional filtering in ureq
        // to not let non-compliant cookies through.
        let cookie = Cookie::parse("borked///=illegal\\,").unwrap();
        // these should not be allowed according to the RFCs.
        assert_eq!(cookie.name(), "borked///");
        assert_eq!(cookie.value(), "illegal\\,");
    }

    #[test]
    fn illegal_cookie_name() {
        let cookie = Cookie::parse("borked/=value").unwrap();
        assert!(!is_cookie_rfc_compliant(&cookie));
    }

    #[test]
    fn illegal_cookie_value() {
        let cookie = Cookie::parse("name=borked,").unwrap();
        assert!(!is_cookie_rfc_compliant(&cookie));
    }

    #[test]
    fn legal_cookie_name_value() {
        let cookie = Cookie::parse("name=value").unwrap();
        assert!(is_cookie_rfc_compliant(&cookie));
    }
}
