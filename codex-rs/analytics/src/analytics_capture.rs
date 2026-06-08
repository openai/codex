use crate::events::TrackEventsRequest;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::Path;

pub(crate) const ANALYTICS_EVENTS_CAPTURE_FILE_ENV_VAR: &str =
    "CODEX_ANALYTICS_EVENTS_CAPTURE_FILE";

pub(crate) fn append_payload(path: &Path, payload: &TrackEventsRequest) -> io::Result<()> {
    let mut line = serde_json::to_vec(payload)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    line.push(b'\n');

    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(path)?;
    file.write_all(&line)?;
    file.flush()
}
