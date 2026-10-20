//! Checks failure details against real truncated and concatenated zstd streams.

use std::fs;
use std::io::Read;
use std::io::Write;

use pretty_assertions::assert_eq;

use super::*;

#[test]
fn incomplete_frames_report_decoded_bytes_and_changes_to_the_open_file() -> io::Result<()> {
    let contents = b"valid utf8\n";
    let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), /*level*/ 0)?;
    encoder.set_pledged_src_size(Some(contents.len() as u64))?;
    encoder.write_all(contents)?;
    let complete = encoder.finish()?;

    for (suffix, change_after_open, expected_contents) in [
        (None, false, &contents[..contents.len() - 1]),
        (None, true, &contents[..contents.len() - 1]),
        (Some(0x28), false, contents.as_slice()),
    ] {
        let home = tempfile::tempdir()?;
        let path = home.path().join("rollout.jsonl.zst");
        let mut damaged = complete.clone();
        if let Some(byte) = suffix {
            damaged.push(byte); // A complete payload followed by an unfinished frame header.
        } else {
            damaged.pop();
        }
        fs::write(
            &path,
            if change_after_open {
                &complete
            } else {
                &damaged
            },
        )?;
        let mut reader = DiagnosticDecoder::new(File::open(&path)?)?;
        if change_after_open {
            fs::OpenOptions::new()
                .write(true)
                .open(&path)?
                .set_len(damaged.len() as u64)?;
        }
        let mut decoded = Vec::new();
        let error = reader.read_to_end(&mut decoded).unwrap_err();
        assert_eq!(
            (error.kind(), decoded, reader.failure),
            (
                io::ErrorKind::UnexpectedEof,
                expected_contents.to_vec(),
                Some(FailureDetails {
                    compressed_bytes: Some(damaged.len() as u64),
                    first_frame_content_bytes: Some(contents.len() as u64),
                    decoded_bytes: expected_contents.len() as u64,
                    file_changed: Some(change_after_open),
                }),
            )
        );
    }
    Ok(())
}
