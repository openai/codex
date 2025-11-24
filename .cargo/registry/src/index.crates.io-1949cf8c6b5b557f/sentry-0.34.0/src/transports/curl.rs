use std::io::{Cursor, Read};
use std::time::Duration;

use curl::easy::Easy as CurlClient;

use super::thread::TransportThread;

use crate::{sentry_debug, types::Scheme, ClientOptions, Envelope, Transport};

/// A [`Transport`] that sends events via the [`curl`] library.
///
/// This is enabled by the `curl` feature flag.
#[cfg_attr(doc_cfg, doc(cfg(feature = "curl")))]
pub struct CurlHttpTransport {
    thread: TransportThread,
}

impl CurlHttpTransport {
    /// Creates a new Transport.
    pub fn new(options: &ClientOptions) -> Self {
        Self::new_internal(options, None)
    }

    /// Creates a new Transport that uses the specified [`CurlClient`].
    pub fn with_client(options: &ClientOptions, client: CurlClient) -> Self {
        Self::new_internal(options, Some(client))
    }

    fn new_internal(options: &ClientOptions, client: Option<CurlClient>) -> Self {
        let client = client.unwrap_or_else(CurlClient::new);
        let http_proxy = options.http_proxy.as_ref().map(ToString::to_string);
        let https_proxy = options.https_proxy.as_ref().map(ToString::to_string);
        let dsn = options.dsn.as_ref().unwrap();
        let user_agent = options.user_agent.clone();
        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();
        let scheme = dsn.scheme();
        let accept_invalid_certs = options.accept_invalid_certs;

        let mut handle = client;
        let thread = TransportThread::new(move |envelope, rl| {
            handle.reset();
            handle.url(&url).unwrap();
            handle.custom_request("POST").unwrap();

            if accept_invalid_certs {
                handle.ssl_verify_host(false).unwrap();
                handle.ssl_verify_peer(false).unwrap();
            }

            match (scheme, &http_proxy, &https_proxy) {
                (Scheme::Https, _, Some(proxy)) => {
                    if let Err(err) = handle.proxy(proxy) {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                }
                (_, Some(proxy), _) => {
                    if let Err(err) = handle.proxy(proxy) {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                }
                _ => {}
            }

            let mut body = Vec::new();
            envelope.to_writer(&mut body).unwrap();
            let mut body = Cursor::new(body);

            let mut retry_after = None;
            let mut sentry_header = None;
            let mut headers = curl::easy::List::new();
            headers.append(&format!("X-Sentry-Auth: {auth}")).unwrap();
            headers.append("Expect:").unwrap();
            handle.http_headers(headers).unwrap();
            handle.upload(true).unwrap();
            handle.in_filesize(body.get_ref().len() as u64).unwrap();
            handle
                .read_function(move |buf| Ok(body.read(buf).unwrap_or(0)))
                .unwrap();
            handle.verbose(true).unwrap();
            handle
                .debug_function(move |info, data| {
                    let prefix = match info {
                        curl::easy::InfoType::HeaderIn => "< ",
                        curl::easy::InfoType::HeaderOut => "> ",
                        curl::easy::InfoType::DataOut => "",
                        _ => return,
                    };
                    sentry_debug!("curl: {}{}", prefix, String::from_utf8_lossy(data).trim());
                })
                .unwrap();

            {
                let mut handle = handle.transfer();
                let retry_after_setter = &mut retry_after;
                let sentry_header_setter = &mut sentry_header;
                handle
                    .header_function(move |data| {
                        if let Ok(data) = std::str::from_utf8(data) {
                            let mut iter = data.split(':');
                            if let Some(key) = iter.next().map(str::to_lowercase) {
                                if key == "retry-after" {
                                    *retry_after_setter = iter.next().map(|x| x.trim().to_string());
                                } else if key == "x-sentry-rate-limits" {
                                    *sentry_header_setter =
                                        iter.next().map(|x| x.trim().to_string());
                                }
                            }
                        }
                        true
                    })
                    .unwrap();
                handle.perform().ok();
            }

            match handle.response_code() {
                Ok(response_code) => {
                    if let Some(sentry_header) = sentry_header {
                        rl.update_from_sentry_header(&sentry_header);
                    } else if let Some(retry_after) = retry_after {
                        rl.update_from_retry_after(&retry_after);
                    } else if response_code == 429 {
                        rl.update_from_429();
                    }
                }
                Err(err) => {
                    sentry_debug!("Failed to send envelope: {}", err);
                }
            }
        });
        Self { thread }
    }
}

impl Transport for CurlHttpTransport {
    fn send_envelope(&self, envelope: Envelope) {
        self.thread.send(envelope)
    }
    fn flush(&self, timeout: Duration) -> bool {
        self.thread.flush(timeout)
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        self.flush(timeout)
    }
}
