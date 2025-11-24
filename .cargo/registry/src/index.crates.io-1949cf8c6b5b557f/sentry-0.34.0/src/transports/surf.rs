use std::time::Duration;

use isahc::{
    config::{Configurable, SslOption},
    HttpClient,
};
use surf::{http::headers as SurfHeaders, Client as SurfClient, StatusCode};

use super::tokio_thread::TransportThread;

use crate::{sentry_debug, ClientOptions, Envelope, Transport};

/// A [`Transport`] that sends events via the [`surf`] library.
///
/// This is enabled by the `surf` feature flag.
///
/// [`surf`]: https://crates.io/crates/surf
#[cfg_attr(doc_cfg, doc(cfg(feature = "surf")))]
pub struct SurfHttpTransport {
    thread: TransportThread,
}

impl SurfHttpTransport {
    /// Creates a new Transport.
    pub fn new(options: &ClientOptions) -> Self {
        Self::new_internal(options, None)
    }

    /// Creates a new Transport that uses the specified [`SurfClient`].
    pub fn with_client(options: &ClientOptions, client: SurfClient) -> Self {
        Self::new_internal(options, Some(client))
    }

    fn new_internal(options: &ClientOptions, client: Option<SurfClient>) -> Self {
        let mut client = client.unwrap_or_default();
        if options.accept_invalid_certs {
            let hc = HttpClient::builder()
                .ssl_options(SslOption::DANGER_ACCEPT_INVALID_CERTS)
                .build()
                .unwrap();
            let http_client = http_client::isahc::IsahcClient::from_client(hc);
            client = SurfClient::with_http_client(http_client)
        }

        let dsn = options.dsn.as_ref().unwrap();
        let user_agent = options.user_agent.clone();
        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();

        let thread = TransportThread::new(move |envelope, mut rl| {
            let mut body = Vec::new();
            envelope.to_writer(&mut body).unwrap();
            let request = client.post(&url).header("X-Sentry-Auth", &auth).body(body);

            async move {
                match request.await {
                    Ok(mut response) => {
                        if let Some(sentry_header) =
                            response.header("x-sentry-rate-limits").map(|x| x.as_str())
                        {
                            rl.update_from_retry_after(sentry_header);
                        } else if let Some(retry_after) = response
                            .header(SurfHeaders::RETRY_AFTER)
                            .map(|x| x.as_str())
                        {
                            rl.update_from_retry_after(retry_after);
                        } else if response.status() == StatusCode::TooManyRequests {
                            rl.update_from_429();
                        }

                        match response.body_string().await {
                            Err(err) => {
                                sentry_debug!("Failed to read sentry response: {}", err);
                            }
                            Ok(text) => {
                                sentry_debug!("Get response: `{}`", text);
                            }
                        }
                    }
                    Err(err) => {
                        sentry_debug!("Failed to send envelope: {}", err);
                    }
                }
                rl
            }
        });
        Self { thread }
    }
}

impl Transport for SurfHttpTransport {
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
