use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use zeroize::Zeroize;

/// Use a generous buffer size to avoid truncation and to allow for longer API
/// keys in the future.
const BUFFER_SIZE: usize = 1024;
const AUTH_HEADER_PREFIX: &[u8] = b"Bearer ";

/// Reads the auth token from stdin and returns a static `Authorization` header
/// value with the auth token used with `Bearer`. The header value is returned
/// as a `&'static str` whose bytes are locked in memory to avoid accidental
/// exposure.
#[cfg(unix)]
pub(crate) fn read_auth_header_from_stdin() -> Result<&'static str> {
    read_auth_header_with(read_from_unix_stdin)
}

#[cfg(windows)]
pub(crate) fn read_auth_header_from_stdin() -> Result<&'static str> {
    use std::io::Read;

    // Use of `stdio::io::stdin()` has the problem mentioned in the docstring on
    // the UNIX version of `read_from_unix_stdin()`, so this should ultimately
    // be replaced the low-level Windows equivalent. Because we do not have an
    // equivalent of mlock() on Windows right now, it is not pressing until we
    // address that issue.
    read_auth_header_with(|buffer| std::io::stdin().read(buffer))
}

/// We perform a low-level read with `read(2)` because `stdio::io::stdin()` has
/// an internal BufReader:
///
/// https://github.com/rust-lang/rust/blob/bcbbdcb8522fd3cb4a8dde62313b251ab107694d/library/std/src/io/stdio.rs#L250-L252
///
/// that can end up retaining a copy of stdin data in memory with no way to zero
/// it out, whereas we aim to guarantee there is exactly one copy of the API key
/// in memory, protected by mlock(2).
#[cfg(unix)]
fn read_from_unix_stdin(buffer: &mut [u8]) -> std::io::Result<usize> {
    use libc::c_void;
    use libc::read;

    // Perform a single read(2) call into the provided buffer slice.
    // Looping and newline/EOF handling are managed by the caller.
    loop {
        let result = unsafe {
            read(
                libc::STDIN_FILENO,
                buffer.as_mut_ptr().cast::<c_void>(),
                buffer.len(),
            )
        };

        if result == 0 {
            return Ok(0);
        }

        if result < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }

        return Ok(result as usize);
    }
}

fn read_auth_header_with<F>(mut read_fn: F) -> Result<&'static str>
where
    F: FnMut(&mut [u8]) -> std::io::Result<usize>,
{
    // TAKE CARE WHEN MODIFYING THIS CODE!!!
    //
    // This function goes to great lengths to avoid leaving the API key in
    // memory longer than necessary and to avoid copying it around. We read
    // directly into a stack buffer so the only heap allocation should be the
    // one to create the String (with the exact size) for the header value,
    // which we then immediately protect with mlock(2).
    let mut buf = [0u8; BUFFER_SIZE];
    buf[..AUTH_HEADER_PREFIX.len()].copy_from_slice(AUTH_HEADER_PREFIX);

    let prefix_len = AUTH_HEADER_PREFIX.len();
    let capacity = buf.len() - prefix_len;
    let mut total_read = 0usize; // number of bytes read into the token region
    let mut saw_newline = false;
    let mut saw_eof = false;

    while total_read < capacity {
        let slice = &mut buf[prefix_len + total_read..];
        let read = match read_fn(slice) {
            Ok(n) => n,
            Err(err) => {
                buf.zeroize();
                return Err(err.into());
            }
        };

        if read == 0 {
            saw_eof = true;
            break;
        }

        // Search only the newly written region for a newline.
        let newly_written = &slice[..read];
        if let Some(pos) = newly_written.iter().position(|&b| b == b'\n') {
            total_read += pos + 1; // include the newline for trimming below
            saw_newline = true;
            break;
        }

        total_read += read;

        // Continue loop; if buffer fills without newline/EOF we'll error below.
    }

    // If buffer filled and we did not see newline or EOF, error out.
    if total_read == capacity && !saw_newline && !saw_eof {
        buf.zeroize();
        return Err(anyhow!(
            "OPENAI_API_KEY is too large to fit in the 512-byte buffer"
        ));
    }

    let mut total = prefix_len + total_read;
    while total > prefix_len && (buf[total - 1] == b'\n' || buf[total - 1] == b'\r') {
        total -= 1;
    }

    if total == AUTH_HEADER_PREFIX.len() {
        buf.zeroize();
        return Err(anyhow!(
            "OPENAI_API_KEY must be provided via stdin (e.g. printenv OPENAI_API_KEY | codex responses-api-proxy)"
        ));
    }

    if let Err(err) = validate_auth_header_bytes(&buf[AUTH_HEADER_PREFIX.len()..total]) {
        buf.zeroize();
        return Err(err);
    }

    let header_str = match std::str::from_utf8(&buf[..total]) {
        Ok(value) => value,
        Err(err) => {
            // In theory, validate_auth_header_bytes() should have caught
            // any invalid UTF-8 sequences, but just in case...
            buf.zeroize();
            return Err(err).context("reading Authorization header from stdin as UTF-8");
        }
    };

    let header_value = String::from(header_str);
    buf.zeroize();

    let leaked: &'static mut str = header_value.leak();
    mlock_str(leaked);

    Ok(leaked)
}

#[cfg(unix)]
fn mlock_str(value: &str) {
    use libc::_SC_PAGESIZE;
    use libc::c_void;
    use libc::mlock;
    use libc::sysconf;

    if value.is_empty() {
        return;
    }

    let page_size = unsafe { sysconf(_SC_PAGESIZE) };
    if page_size <= 0 {
        return;
    }
    let page_size = page_size as usize;
    if page_size == 0 {
        return;
    }

    let addr = value.as_ptr() as usize;
    let len = value.len();
    let start = addr & !(page_size - 1);
    let addr_end = match addr.checked_add(len) {
        Some(v) => match v.checked_add(page_size - 1) {
            Some(total) => total,
            None => return,
        },
        None => return,
    };
    let end = addr_end & !(page_size - 1);
    let size = end.saturating_sub(start);
    if size == 0 {
        return;
    }

    let _ = unsafe { mlock(start as *const c_void, size) };
}

#[cfg(not(unix))]
fn mlock_str(_value: &str) {}

/// The key should match /^[A-Za-z0-9\-_]+$/. Ensure there is no funny business
/// with NUL characters and whatnot.
fn validate_auth_header_bytes(key_bytes: &[u8]) -> Result<()> {
    if key_bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Ok(());
    }

    Err(anyhow!(
        "OPENAI_API_KEY may only contain ASCII letters, numbers, '-' or '_'"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io;

    #[test]
    fn reads_key_with_no_newlines() {
        let mut sent = false;
        let result = read_auth_header_with(|buf| {
            if sent {
                return Ok(0);
            }
            let data = b"sk-abc123";
            buf[..data.len()].copy_from_slice(data);
            sent = true;
            Ok(data.len())
        })
        .unwrap();

        assert_eq!(result, "Bearer sk-abc123");
    }

    #[test]
    fn reads_key_with_short_reads() {
        let mut chunks: VecDeque<&[u8]> =
            VecDeque::from(vec![b"sk-".as_ref(), b"abc".as_ref(), b"123\n".as_ref()]);
        let result = read_auth_header_with(|buf| match chunks.pop_front() {
            Some(chunk) if !chunk.is_empty() => {
                buf[..chunk.len()].copy_from_slice(chunk);
                Ok(chunk.len())
            }
            _ => Ok(0),
        })
        .unwrap();

        assert_eq!(result, "Bearer sk-abc123");
    }

    #[test]
    fn reads_key_and_trims_newlines() {
        let mut sent = false;
        let result = read_auth_header_with(|buf| {
            if sent {
                return Ok(0);
            }
            let data = b"sk-abc123\r\n";
            buf[..data.len()].copy_from_slice(data);
            sent = true;
            Ok(data.len())
        })
        .unwrap();

        assert_eq!(result, "Bearer sk-abc123");
    }

    #[test]
    fn errors_when_no_input_provided() {
        let err = read_auth_header_with(|_| Ok(0)).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("must be provided"));
    }

    #[test]
    fn errors_when_buffer_filled() {
        let err = read_auth_header_with(|buf| {
            let data = vec![b'a'; BUFFER_SIZE - AUTH_HEADER_PREFIX.len()];
            buf[..data.len()].copy_from_slice(&data);
            Ok(data.len())
        })
        .unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("OPENAI_API_KEY is too large to fit in the 512-byte buffer"));
    }

    #[test]
    fn propagates_io_error() {
        let err = read_auth_header_with(|_| Err(io::Error::other("boom"))).unwrap_err();

        let io_error = err.downcast_ref::<io::Error>().unwrap();
        assert_eq!(io_error.kind(), io::ErrorKind::Other);
        assert_eq!(io_error.to_string(), "boom");
    }

    #[test]
    fn errors_on_invalid_utf8() {
        let mut sent = false;
        let err = read_auth_header_with(|buf| {
            if sent {
                return Ok(0);
            }
            let data = b"sk-abc\xff";
            buf[..data.len()].copy_from_slice(data);
            sent = true;
            Ok(data.len())
        })
        .unwrap_err();

        let message = format!("{err:#}");
        assert!(
            message.contains("OPENAI_API_KEY may only contain ASCII letters, numbers, '-' or '_'")
        );
    }

    #[test]
    fn errors_on_invalid_characters() {
        let mut sent = false;
        let err = read_auth_header_with(|buf| {
            if sent {
                return Ok(0);
            }
            let data = b"sk-abc!23";
            buf[..data.len()].copy_from_slice(data);
            sent = true;
            Ok(data.len())
        })
        .unwrap_err();

        let message = format!("{err:#}");
        assert!(
            message.contains("OPENAI_API_KEY may only contain ASCII letters, numbers, '-' or '_'")
        );
    }
}
