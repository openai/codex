use std::path::Path;

use codex_otel::otel_event_manager::OtelEventManager;
use futures::TryStreamExt;
use tokio::sync::mpsc;
use tokio_util::io::ReaderStream;

use crate::error::Error;
use crate::error::Result;
use codex_provider_config::ModelProviderInfo;

pub async fn stream_from_fixture_wire(
    path: impl AsRef<Path>,
    provider: ModelProviderInfo,
    otel_event_manager: OtelEventManager,
) -> Result<crate::stream::WireResponseStream> {
    let (tx_event, rx_event) = mpsc::channel::<Result<crate::stream::WireEvent>>(1600);
    let display_path = path.as_ref().display().to_string();
    let content = std::fs::read_to_string(path.as_ref()).map_err(|err| {
        Error::Other(format!(
            "failed to read fixture text from {display_path}: {err}"
        ))
    })?;
    let content = content
        .lines()
        .map(|line| {
            let mut line_with_spacing = line.to_string();
            line_with_spacing.push('\n');
            line_with_spacing.push('\n');
            line_with_spacing
        })
        .collect::<String>();

    let rdr = std::io::Cursor::new(content);
    let stream = ReaderStream::new(rdr).map_err(|err| Error::Other(err.to_string()));
    tokio::spawn(crate::client::sse::process_sse_wire(
        stream,
        tx_event,
        provider.stream_idle_timeout(),
        otel_event_manager,
        crate::decode_wire::responses::WireResponsesSseDecoder,
    ));
    Ok(crate::stream::EventStream::from_receiver(rx_event))
}
