use crate::events::TrackEventsRequest;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::Path;

pub(crate) const ANALYTICS_EVENTS_CAPTURE_FILE_ENV_VAR: &str =
    "CODEX_ANALYTICS_EVENTS_CAPTURE_FILE";

pub(crate) fn initialize(path: &Path) -> io::Result<()> {
    open_capture_file(path).map(drop)
}

pub(crate) fn append_payload(path: &Path, payload: &TrackEventsRequest) -> io::Result<()> {
    let mut line = serde_json::to_vec(payload)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    line.push(b'\n');

    let mut file = open_capture_file(path)?;
    file.write_all(&line)?;
    file.flush()
}

fn open_capture_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
