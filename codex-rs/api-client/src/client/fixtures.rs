use std::path::Path;

use codex_otel::otel_event_manager::OtelEventManager;
use futures::TryStreamExt;
use tokio_util::io::ReaderStream;

use crate::error::Error;
use crate::error::Result;
use codex_provider_config::ModelProviderInfo;

pub async fn stream_from_fixture_wire(
    path: impl AsRef<Path>,
    provider: ModelProviderInfo,
    otel_event_manager: OtelEventManager,
) -> Result<crate::stream::WireResponseStream> {
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
    let (_, rx_event) = crate::client::sse::spawn_wire_stream(
        stream,
        &provider,
        otel_event_manager,
        crate::decode_wire::responses::WireResponsesSseDecoder,
    );
    Ok(rx_event)
}
