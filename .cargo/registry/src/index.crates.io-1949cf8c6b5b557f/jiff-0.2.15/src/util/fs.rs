use std::{fs::File, path::Path};

use crate::Timestamp;

/// Returns the last modified time for the given file path as a Jiff timestamp.
///
/// If there was a problem accessing the last modified time or if it could not
/// fit in a Jiff timestamp, then a warning message is logged and `None` is
/// returned.
pub(crate) fn last_modified_from_path(path: &Path) -> Option<Timestamp> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_err) => {
            warn!(
                "failed to open file to get last modified time {}: {_err}",
                path.display(),
            );
            return None;
        }
    };
    last_modified_from_file(path, &file)
}

/// Returns the last modified time for the given file as a Jiff timestamp.
///
/// If there was a problem accessing the last modified time or if it could not
/// fit in a Jiff timestamp, then a warning message is logged and `None` is
/// returned.
///
/// The path given should be the path to the given file. It is used for
/// diagnostic purposes.
pub(crate) fn last_modified_from_file(
    _path: &Path,
    file: &File,
) -> Option<Timestamp> {
    let md = match file.metadata() {
        Ok(md) => md,
        Err(_err) => {
            warn!(
                "failed to get metadata (for last modified time) \
                 for {}: {_err}",
                _path.display(),
            );
            return None;
        }
    };
    let systime = match md.modified() {
        Ok(systime) => systime,
        Err(_err) => {
            warn!(
                "failed to get last modified time for {}: {_err}",
                _path.display()
            );
            return None;
        }
    };
    let timestamp = match Timestamp::try_from(systime) {
        Ok(timestamp) => timestamp,
        Err(_err) => {
            warn!(
                "system time {systime:?} out of bounds \
                 for Jiff timestamp for last modified time \
                 from {}: {_err}",
                _path.display(),
            );
            return None;
        }
    };
    Some(timestamp)
}
