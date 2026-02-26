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

    read_auth_header_with(|buffer| std::io::stdin().read(buffer))
}

/// We perform a low-level read with `read(2)` because `stdio::io::stdin()` has
/// an internal BufReader that can end up retaining a copy of stdin data in
/// memory with no way to zero it out, whereas we aim to guarantee there is
/// exactly one copy of the API key in memory, protected by mlock(2).
#[cfg(unix)]
fn read_from_unix_stdin(buffer: &mut [u8]) -> std::io::Result<usize> {
    use libc::c_void;
    use libc::read;

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
    let mut buf = [0u8; BUFFER_SIZE];
    buf[..AUTH_HEADER_PREFIX.len()].copy_from_slice(AUTH_HEADER_PREFIX);

    let prefix_len = AUTH_HEADER_PREFIX.len();
    let capacity = buf.len() - prefix_len;
    let mut total_read = 0usize;
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

        let newly_written = &slice[..read];
        if let Some(pos) = newly_written.iter().position(|&b| b == b'\n') {
            total_read += pos + 1;
            saw_newline = true;
            break;
        }

        total_read += read;
    }

    if total_read == capacity && !saw_newline && !saw_eof {
        buf.zeroize();
        return Err(anyhow!(
            "API key is too large to fit in the {BUFFER_SIZE}-byte buffer"
        ));
    }

    let mut total = prefix_len + total_read;
    while total > prefix_len && (buf[total - 1] == b'\n' || buf[total - 1] == b'\r') {
        total -= 1;
    }

    if total == AUTH_HEADER_PREFIX.len() {
        buf.zeroize();
        return Err(anyhow!(
            "API key must be provided via stdin (e.g. printenv AGENTEX_API_KEY | codex-sgp-proxy)"
        ));
    }

    if let Err(err) = validate_auth_header_bytes(&buf[AUTH_HEADER_PREFIX.len()..total]) {
        buf.zeroize();
        return Err(err);
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

fn validate_auth_header_bytes(key_bytes: &[u8]) -> Result<()> {
    if key_bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Ok(());
    }

    Err(anyhow!(
        "API key may only contain ASCII letters, numbers, '-' or '_'"
    ))
}
