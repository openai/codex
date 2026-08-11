use tonic::transport::Channel;
use tonic::transport::Endpoint;

pub(super) struct SharedTransport {
    endpoint: TransportEndpoint,
    channel: tokio::sync::OnceCell<Channel>,
}

enum TransportEndpoint {
    Url(String),
    Connected(Channel),
}

impl SharedTransport {
    pub(super) fn new(endpoint: String) -> Self {
        Self {
            endpoint: TransportEndpoint::Url(endpoint),
            channel: tokio::sync::OnceCell::new(),
        }
    }

    pub(super) fn with_channel(channel: Channel) -> Self {
        Self {
            endpoint: TransportEndpoint::Connected(channel),
            channel: tokio::sync::OnceCell::new(),
        }
    }

    pub(super) async fn channel(&self) -> Result<Channel, String> {
        self.channel
            .get_or_try_init(|| async {
                match &self.endpoint {
                    TransportEndpoint::Url(endpoint) => Endpoint::from_shared(endpoint.clone())
                        .map_err(|error| format!("invalid gRPC code-mode host endpoint: {error}"))?
                        .connect()
                        .await
                        .map_err(|error| {
                            format!("failed to connect to gRPC code-mode host: {error}")
                        }),
                    TransportEndpoint::Connected(channel) => Ok(channel.clone()),
                }
            })
            .await
            .cloned()
    }
}
