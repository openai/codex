use crate::{sentry_debug, ClientOptions, Transport};
use embedded_svc::http::client::Client as HttpClient;
use esp_idf_svc::{http::client::EspHttpConnection, io::Write};

/// Transport using the embedded-svc http client
pub struct EmbeddedSVCHttpTransport {
    options: ClientOptions,
}

impl EmbeddedSVCHttpTransport {
    /// Creates a new transport
    pub fn new(options: &ClientOptions) -> Self {
        Self {
            options: options.clone(),
        }
    }
}

impl EmbeddedSVCHttpTransport {
    fn send_envelope(
        &self,
        envelope: sentry_core::Envelope,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dsn = self
            .options
            .dsn
            .as_ref()
            .ok_or_else(|| "No DSN specified")?;
        let user_agent = &self.options.user_agent;
        let auth = dsn.to_auth(Some(user_agent)).to_string();
        let headers = [("X-Sentry-Auth", auth.as_str())];
        let url = dsn.envelope_api_url();

        let mut body = Vec::new();
        envelope.to_writer(&mut body)?;

        let config = esp_idf_svc::http::client::Configuration {
            use_global_ca_store: true,
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            ..Default::default()
        };

        let mut client = HttpClient::wrap(EspHttpConnection::new(&config)?);

        let mut request = client.post(url.as_str(), &headers)?;
        request.write_all(&body)?;
        request.flush()?;
        let mut response = request.submit()?;

        // read the whole response
        let mut buf = [0u8; 1024];
        while response.read(&mut buf)? > 0 {}

        Ok(())
    }
}

impl Transport for EmbeddedSVCHttpTransport {
    fn send_envelope(&self, envelope: sentry_core::Envelope) {
        if let Err(err) = self.send_envelope(envelope) {
            sentry_debug!("Failed to send envelope: {}", err);
        }
    }
}
