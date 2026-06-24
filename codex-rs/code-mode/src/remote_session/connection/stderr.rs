use std::io;

use tokio::io::AsyncBufRead;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncRead;
use tokio::io::BufReader;
use tracing::debug;
use tracing::warn;

const MAX_STDERR_LINE_BYTES: usize = 4 * 1024 * 1024;

pub(super) async fn drain<R>(stderr: R)
where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(stderr);
    let mut line = Vec::new();
    loop {
        match read_capped_line(&mut reader, &mut line).await {
            Ok(Some(true)) => {
                let line = String::from_utf8_lossy(&line);
                debug!(
                    "code-mode host stderr: {line} [truncated after {MAX_STDERR_LINE_BYTES} bytes]"
                );
            }
            Ok(Some(false)) => {
                let line = String::from_utf8_lossy(&line);
                debug!("code-mode host stderr: {line}");
            }
            Ok(None) => break,
            Err(error) => {
                warn!("failed to read code-mode host stderr: {error}");
                break;
            }
        }
    }
}

async fn read_capped_line<R>(reader: &mut R, line: &mut Vec<u8>) -> io::Result<Option<bool>>
where
    R: AsyncBufRead + Unpin,
{
    line.clear();
    let mut truncated = false;
    loop {
        let buffer = reader.fill_buf().await?;
        if buffer.is_empty() {
            if line.is_empty() && !truncated {
                return Ok(None);
            }
            trim_carriage_return(line, truncated);
            return Ok(Some(truncated));
        }

        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let chunk_len = newline.unwrap_or(buffer.len());
        let remaining = MAX_STDERR_LINE_BYTES.saturating_sub(line.len());
        let copy_len = remaining.min(chunk_len);
        line.extend_from_slice(&buffer[..copy_len]);
        truncated |= copy_len < chunk_len;
        let consumed = newline.map_or(chunk_len, |index| index + 1);
        reader.consume(consumed);

        if newline.is_some() {
            trim_carriage_return(line, truncated);
            return Ok(Some(truncated));
        }
    }
}

fn trim_carriage_return(line: &mut Vec<u8>, truncated: bool) {
    if !truncated && line.last() == Some(&b'\r') {
        line.pop();
    }
}

#[cfg(test)]
#[path = "stderr_tests.rs"]
mod tests;
