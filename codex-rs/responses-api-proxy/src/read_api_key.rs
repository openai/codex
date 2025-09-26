use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use std::io::Read;
use zeroize::Zeroize;

/// Use a generous buffer size to avoid truncation and to allow for longer API
/// keys in the future.
const BUFFER_SIZE: usize = 1024;
const AUTH_HEADER_PREFIX: &[u8] = b"Bearer ";

/// Reads the auth token from stdin and returns a static `Authorization` header
/// value with the auth token used with `Bearer`. The header value is returned
/// as a `&'static str` whose bytes are locked in memory to avoid accidental
/// exposure.
pub(crate) fn read_auth_header_from_stdin() -> Result<&'static str> {
    read_auth_header_with(|buffer| std::io::stdin().read(buffer))
}

fn read_auth_header_with<F>(read_fn: F) -> Result<&'static str>
where
    F: FnOnce(&mut [u8]) -> std::io::Result<usize>,
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

    let read = read_fn(&mut buf[AUTH_HEADER_PREFIX.len()..]).inspect_err(|_err| {
        buf.zeroize();
    })?;

    if read == buf.len() - AUTH_HEADER_PREFIX.len() {
        buf.zeroize();
        return Err(anyhow!(
            "OPENAI_API_KEY is too large to fit in the 512-byte buffer"
        ));
    }

    let mut total = AUTH_HEADER_PREFIX.len() + read;
    while total > AUTH_HEADER_PREFIX.len() && (buf[total - 1] == b'\n' || buf[total - 1] == b'\r') {
        total -= 1;
    }

    if total == AUTH_HEADER_PREFIX.len() {
        buf.zeroize();
        return Err(anyhow!(
            "OPENAI_API_KEY must be provided via stdin (e.g. printenv OPENAI_API_KEY | codex responses-api-proxy)"
        ));
    }

    let header_str = match std::str::from_utf8(&buf[..total]) {
        Ok(value) => value,
        Err(err) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn reads_key_with_no_newlines() {
        let result = read_auth_header_with(|buf| {
            let data = b"sk-abc123";
            buf[..data.len()].copy_from_slice(data);
            Ok(data.len())
        })
        .unwrap();

        assert_eq!(result, "Bearer sk-abc123");
    }

    #[test]
    fn reads_key_and_trims_newlines() {
        let result = read_auth_header_with(|buf| {
            let data = b"sk-abc123\r\n";
            buf[..data.len()].copy_from_slice(data);
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
        assert!(message.contains("too large"));
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
        let err = read_auth_header_with(|buf| {
            let data = b"sk-abc\xff";
            buf[..data.len()].copy_from_slice(data);
            Ok(data.len())
        })
        .unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("UTF-8"));
    }
}
