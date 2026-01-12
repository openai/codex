//! TLS client support for Rama.

mod connector;
#[doc(inline)]
pub use connector::{
    AutoTlsStream, ConnectorKindAuto, ConnectorKindSecure, ConnectorKindTunnel, TlsConnector,
    TlsConnectorLayer,
};

mod connector_data;
#[doc(inline)]
pub use connector_data::{
    TlsConnectorData, TlsConnectorDataBuilder, client_root_certs, self_signed_client_auth,
};
