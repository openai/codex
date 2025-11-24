use url::{ParseError, Url};

use std::error;
use std::fmt::{self, Display};
use std::io;

use crate::Response;

/// An error that may occur when processing a [Request](crate::Request).
///
/// This can represent connection-level errors (e.g. connection refused),
/// protocol-level errors (malformed response), or status code errors
/// (e.g. 404 Not Found). Status code errors are represented by the
/// [Status](Error::Status) enum variant, while connection-level and
/// protocol-level errors are represented by the [Transport](Error::Transport)
/// enum variant. You can use a match statement to extract a Response
/// from a `Status` error. For instance, you may want to read the full
/// body of a response because you expect it to contain a useful error
/// message. Or you may want to handle certain error code responses
/// differently.
///
/// # Examples
///
/// Example of matching out all unexpected server status codes.
///
/// ```no_run
/// use ureq::Error;
///
/// match ureq::get("http://mypage.example.com/").call() {
///     Ok(response) => { /* it worked */},
///     Err(Error::Status(code, response)) => {
///         /* the server returned an unexpected status
///            code (such as 400, 500 etc) */
///     }
///     Err(_) => { /* some kind of io/transport error */ }
/// }
/// ```
///
/// An example of a function that handles HTTP 429 and 500 errors differently
/// than other errors. They get retried after a suitable delay, up to 4 times.
///
/// ```
/// use std::{result::Result, time::Duration, thread};
/// use ureq::{Response, Error, Error::Status};
/// # fn main(){ ureq::is_test(true); get_response( "http://httpbin.org/status/500" ); }
///
/// fn get_response(url: &str) -> Result<Response, Error> {
///     for _ in 1..4 {
///         match ureq::get(url).call() {
///             Err(Status(503, r)) | Err(Status(429, r)) => {
///                 let retry: Option<u64> = r.header("retry-after")
///                     .and_then(|h| h.parse().ok());
///                 let retry = retry.unwrap_or(5);
///                 eprintln!("{} for {}, retry in {}", r.status(), r.get_url(), retry);
///                 thread::sleep(Duration::from_secs(retry));
///             }
///             result => return result,
///         };
///     }
///     // Ran out of retries; try one last time and return whatever result we get.
///     ureq::get(url).call()
/// }
/// ```
///
/// If you'd like to treat all status code errors as normal, successful responses,
/// you can use [OrAnyStatus::or_any_status] like this:
///
/// ```
/// use ureq::Error::Status;
/// # fn main() -> std::result::Result<(), ureq::Transport> {
/// # ureq::is_test(true);
/// use ureq::OrAnyStatus;
///
/// let resp = ureq::get("http://example.com/")
///   .call()
///   .or_any_status()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub enum Error {
    /// A response was successfully received but had status code >= 400.
    /// Values are (status_code, Response).
    Status(u16, Response),
    /// There was an error making the request or receiving the response.
    Transport(Transport),
}

impl Error {
    /// Optionally turn this error into an underlying `Transport`.
    ///
    /// `None` if the underlying error is `Error::Status`.
    pub fn into_transport(self) -> Option<Transport> {
        match self {
            Error::Status(_, _) => None,
            Error::Transport(t) => Some(t),
        }
    }

    /// Optionally turn this error into an underlying `Response`.
    ///
    /// `None` if the underlying error is `Error::Transport`.
    pub fn into_response(self) -> Option<Response> {
        match self {
            Error::Status(_, r) => Some(r),
            Error::Transport(_) => None,
        }
    }
}

/// Error that is not a status code error. For instance, DNS name not found,
/// connection refused, or malformed response.
///
/// * [`Transport::kind()`] provides a classification (same as for [`Error::kind`]).
/// * [`Transport::message()`] might vary for the same classification to give more context.
/// * [`Transport::source()`](std::error::Error::source) holds the underlying error with even more details.
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ureq::ErrorKind;
/// use std::error::Error;
/// use url::ParseError;
///
/// let result = ureq::get("broken/url").call();
/// let error = result.unwrap_err().into_transport().unwrap();
///
/// // the display trait is a combo of the underlying classifications
/// assert_eq!(error.to_string(),
///     "Bad URL: failed to parse URL: RelativeUrlWithoutBase: relative URL without a base");
///
/// // classification
/// assert_eq!(error.kind(), ErrorKind::InvalidUrl);
/// assert_eq!(error.kind().to_string(), "Bad URL");
///
/// // higher level message
/// assert_eq!(error.message(), Some("failed to parse URL: RelativeUrlWithoutBase"));
///
/// // boxed underlying error
/// let source = error.source().unwrap();
/// // downcast to original error
/// let downcast: &ParseError = source.downcast_ref().unwrap();
///
/// assert_eq!(downcast.to_string(), "relative URL without a base");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Transport {
    kind: ErrorKind,
    message: Option<String>,
    url: Option<Url>,
    source: Option<Box<dyn error::Error + Send + Sync + 'static>>,
}

impl Transport {
    /// The type of error that happened while processing the request.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Higher level error details, if there are any.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The url that failed. This can be interesting in cases of redirect where
    /// the original url worked, but a later redirected to url fails.
    pub fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }
}

/// Extension to [`Result<Response, Error>`] for handling all status codes as [`Response`].
pub trait OrAnyStatus {
    /// Ergonomic helper for handling all status codes as [`Response`].
    ///
    /// By default, ureq returns non-2xx responses as [`Error::Status`]. This
    /// helper is for handling all responses as [`Response`], regardless
    /// of status code.
    ///
    /// ```
    /// # ureq::is_test(true);
    /// # fn main() -> Result<(), ureq::Transport> {
    /// // Bring trait into context.
    /// use ureq::OrAnyStatus;
    ///
    /// let response = ureq::get("http://httpbin.org/status/500")
    ///     .call()
    ///     // Transport errors, such as DNS or connectivity problems
    ///     // must still be dealt with as `Err`.
    ///     .or_any_status()?;
    ///
    /// assert_eq!(response.status(), 500);
    /// # Ok(())
    /// # }
    /// ```
    fn or_any_status(self) -> Result<Response, Transport>;
}

impl OrAnyStatus for Result<Response, Error> {
    fn or_any_status(self) -> Result<Response, Transport> {
        match self {
            Ok(response) => Ok(response),
            Err(Error::Status(_, response)) => Ok(response),
            Err(Error::Transport(transport)) => Err(transport),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Status(status, response) => {
                write!(f, "{}: status code {}", response.get_url(), status)?;
                if let Some(original) = response.history.first() {
                    write!(f, " (redirected from {})", original)?;
                }
            }
            Error::Transport(err) => {
                write!(f, "{}", err)?;
            }
        }
        Ok(())
    }
}

impl Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(url) = &self.url {
            write!(f, "{}: ", url)?;
        }
        write!(f, "{}", self.kind)?;
        if let Some(message) = &self.message {
            write!(f, ": {}", message)?;
        }
        if let Some(source) = &self.source {
            write!(f, ": {}", source)?;
        }
        Ok(())
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self {
            Error::Transport(Transport {
                source: Some(s), ..
            }) => Some(s.as_ref()),
            _ => None,
        }
    }
}

impl error::Error for Transport {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|s| s.as_ref() as &(dyn error::Error + 'static))
    }
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: Option<String>) -> Self {
        Error::Transport(Transport {
            kind,
            message,
            url: None,
            source: None,
        })
    }

    pub(crate) fn url(self, url: Url) -> Self {
        if let Error::Transport(mut e) = self {
            e.url = Some(url);
            Error::Transport(e)
        } else {
            self
        }
    }

    pub(crate) fn src(self, e: impl error::Error + Send + Sync + 'static) -> Self {
        if let Error::Transport(mut oe) = self {
            oe.source = Some(Box::new(e));
            Error::Transport(oe)
        } else {
            self
        }
    }

    /// The type of this error.
    ///
    /// ```
    /// # ureq::is_test(true);
    /// let err = ureq::get("http://httpbin.org/status/500")
    ///     .call().unwrap_err();
    /// assert_eq!(err.kind(), ureq::ErrorKind::HTTP);
    /// ```
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Status(_, _) => ErrorKind::HTTP,
            Error::Transport(Transport { kind: k, .. }) => *k,
        }
    }

    /// Return true iff the error was due to a connection closing.
    pub(crate) fn connection_closed(&self) -> bool {
        if self.kind() != ErrorKind::Io {
            return false;
        }
        let other_err = match self {
            Error::Status(_, _) => return false,
            Error::Transport(e) => e,
        };
        let source = match other_err.source.as_ref() {
            Some(e) => e,
            None => return false,
        };
        let ioe: &io::Error = match source.downcast_ref() {
            Some(e) => e,
            None => return false,
        };
        match ioe.kind() {
            io::ErrorKind::ConnectionAborted => true,
            io::ErrorKind::ConnectionReset => true,
            _ => false,
        }
    }
}

/// One of the types of error the can occur when processing a Request.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ErrorKind {
    /// The url could not be understood.
    InvalidUrl,
    /// The url scheme could not be understood.
    UnknownScheme,
    /// DNS lookup failed.
    Dns,
    /// Insecure request attempted with https only set
    InsecureRequestHttpsOnly,
    /// Connection to server failed.
    ConnectionFailed,
    /// Too many redirects.
    TooManyRedirects,
    /// A status line we don't understand `HTTP/1.1 200 OK`.
    BadStatus,
    /// A header line that couldn't be parsed.
    BadHeader,
    /// Some unspecified `std::io::Error`.
    Io,
    /// Proxy information was not properly formatted
    InvalidProxyUrl,
    /// Proxy could not connect
    ProxyConnect,
    /// Incorrect credentials for proxy
    ProxyUnauthorized,
    /// HTTP status code indicating an error (e.g. 4xx, 5xx)
    /// Read the inner response body for details and to return
    /// the connection to the pool.
    HTTP,
}

impl ErrorKind {
    #[allow(clippy::wrong_self_convention)]
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new(self) -> Error {
        Error::new(self, None)
    }

    pub(crate) fn msg(self, s: impl Into<String>) -> Error {
        Error::new(self, Some(s.into()))
    }
}

impl From<Response> for Error {
    fn from(resp: Response) -> Error {
        Error::Status(resp.status(), resp)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        ErrorKind::Io.new().src(err)
    }
}

impl From<Transport> for Error {
    fn from(err: Transport) -> Error {
        Error::Transport(err)
    }
}

impl From<ParseError> for Error {
    fn from(err: ParseError) -> Self {
        ErrorKind::InvalidUrl
            .msg(format!("failed to parse URL: {:?}", err))
            .src(err)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorKind::InvalidUrl => write!(f, "Bad URL"),
            ErrorKind::UnknownScheme => write!(f, "Unknown Scheme"),
            ErrorKind::Dns => write!(f, "Dns Failed"),
            ErrorKind::InsecureRequestHttpsOnly => {
                write!(f, "Insecure request attempted with https_only set")
            }
            ErrorKind::ConnectionFailed => write!(f, "Connection Failed"),
            ErrorKind::TooManyRedirects => write!(f, "Too Many Redirects"),
            ErrorKind::BadStatus => write!(f, "Bad Status"),
            ErrorKind::BadHeader => write!(f, "Bad Header"),
            ErrorKind::Io => write!(f, "Network Error"),
            ErrorKind::InvalidProxyUrl => write!(f, "Malformed proxy"),
            ErrorKind::ProxyConnect => write!(f, "Proxy failed to connect"),
            ErrorKind::ProxyUnauthorized => write!(f, "Provided proxy credentials are incorrect"),
            ErrorKind::HTTP => write!(f, "HTTP status error"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_code_error() {
        let mut response = Response::new(404, "NotFound", "").unwrap();
        response.set_url("http://example.org/".parse().unwrap());
        let err = Error::Status(response.status(), response);

        assert_eq!(err.to_string(), "http://example.org/: status code 404");
    }

    #[test]
    fn status_code_error_redirect() {
        use crate::{get, test};

        test::set_handler("/redirect_a", |unit| {
            assert_eq!(unit.method, "GET");
            test::make_response(
                302,
                "Go here",
                vec!["Location: test://example.edu/redirect_b"],
                vec![],
            )
        });
        test::set_handler("/redirect_b", |unit| {
            assert_eq!(unit.method, "GET");
            test::make_response(
                302,
                "Go here",
                vec!["Location: http://example.com/status/500"],
                vec![],
            )
        });

        let err = get("test://example.org/redirect_a").call().unwrap_err();
        assert_eq!(err.kind(), ErrorKind::HTTP, "{:?}", err);
        assert_eq!(
        err.to_string(),
        "http://example.com/status/500: status code 500 (redirected from test://example.org/redirect_a)"
    );
    }

    #[test]
    fn io_error() {
        let ioe = io::Error::new(io::ErrorKind::TimedOut, "too slow");
        let mut err = Error::new(ErrorKind::Io, Some("oops".to_string())).src(ioe);

        err = err.url("http://example.com/".parse().unwrap());
        assert_eq!(
            err.to_string(),
            "http://example.com/: Network Error: oops: too slow"
        );
    }

    #[test]
    fn connection_closed() {
        let ioe = io::Error::new(io::ErrorKind::ConnectionReset, "connection reset");
        let err = ErrorKind::Io.new().src(ioe);
        assert!(err.connection_closed());

        let ioe = io::Error::new(io::ErrorKind::ConnectionAborted, "connection aborted");
        let err = ErrorKind::Io.new().src(ioe);
        assert!(err.connection_closed());
    }

    #[test]
    fn error_implements_send_and_sync() {
        let _error: Box<dyn Send> = Box::new(Error::new(ErrorKind::Io, None));
        let _error: Box<dyn Sync> = Box::new(Error::new(ErrorKind::Io, None));
    }

    #[test]
    fn ensure_error_size() {
        // This is platform dependent, so we can't be too strict or precise.
        let size = std::mem::size_of::<Error>();
        println!("Error size: {}", size);
        assert!(size < 500); // 344 on Macbook M1
    }
}
