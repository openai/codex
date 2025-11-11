use std::io::BufRead;
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
    let file = std::fs::File::open(path.as_ref())
        .map_err(|err| Error::Other(format!("failed to open fixture {display_path}: {err}")))?;
    let lines = std::io::BufReader::new(file).lines();

    let mut content = String::new();
    for line in lines {
        let line = line
            .map_err(|err| Error::Other(format!("failed to read fixture {display_path}: {err}")))?;
        content.push_str(&line);
        content.push('\n');
        content.push('\n');
    }

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
