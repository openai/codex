use std::sync::OnceLock;

#[cfg(unix)]
use std::mem::MaybeUninit;
#[cfg(unix)]
use std::os::fd::RawFd;

pub fn terminal_palette() -> Option<[(u8, u8, u8); 256]> {
    static CACHE: OnceLock<[(u8, u8, u8); 256]> = OnceLock::new();
    if let Some(cached) = CACHE.get() {
        return Some(*cached);
    }

    match query_terminal_palette() {
        Ok(Some(palette)) => {
            let _ = CACHE.set(palette);
            CACHE.get().copied()
        }
        _ => None,
    }
}

#[cfg(unix)]
fn query_terminal_palette() -> std::io::Result<Option<[(u8, u8, u8); 256]>> {
    use std::fs::OpenOptions;
    use std::io::ErrorKind;
    use std::io::IsTerminal;
    use std::io::Read;
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::time::Duration;
    use std::time::Instant;

    if !std::io::stdout().is_terminal() {
        return Ok(None);
    }

    let mut tty = match OpenOptions::new().read(true).write(true).open("/dev/tty") {
        Ok(file) => file,
        Err(_) => return Ok(None),
    };

    for index in 0..256 {
        write!(tty, "\x1b]4;{index};?\x07")?;
    }
    tty.flush()?;

    let fd = tty.as_raw_fd();
    let _termios_guard = unsafe { suppress_echo(fd) };
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags >= 0 {
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }

    let mut palette: [Option<(u8, u8, u8)>; 256] = [None; 256];
    let mut buffer = Vec::new();
    let mut remaining = palette.len();
    let read_deadline = Instant::now() + Duration::from_millis(1500);

    while remaining > 0 && Instant::now() < read_deadline {
        let mut chunk = [0u8; 512];
        match tty.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                buffer.extend_from_slice(&chunk[..read]);
                let newly = apply_palette_responses(&mut buffer, &mut palette);
                if newly > 0 {
                    remaining = remaining.saturating_sub(newly);
                }
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Ok(None),
        }
    }

    remaining = remaining.saturating_sub(apply_palette_responses(&mut buffer, &mut palette));
    remaining = remaining.saturating_sub(drain_remaining(&mut tty, &mut buffer, &mut palette));

    if remaining > 0 {
        return Ok(None);
    }

    let mut colors = [(0, 0, 0); 256];
    for (slot, value) in colors.iter_mut().zip(palette.into_iter()) {
        if let Some(rgb) = value {
            *slot = rgb;
        } else {
            return Ok(None);
        }
    }

    Ok(Some(colors))
}

#[cfg(not(unix))]
fn query_terminal_palette() -> std::io::Result<Option<[(u8, u8, u8); 256]>> {
    Ok(None)
}

#[cfg(unix)]
fn drain_remaining(
    tty: &mut std::fs::File,
    buffer: &mut Vec<u8>,
    palette: &mut [Option<(u8, u8, u8)>; 256],
) -> usize {
    use std::io::ErrorKind;
    use std::io::Read;
    use std::time::Duration;
    use std::time::Instant;

    let mut chunk = [0u8; 512];
    let mut idle_deadline = Instant::now() + Duration::from_millis(50);
    let mut newly_filled = 0usize;

    loop {
        match tty.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                buffer.extend_from_slice(&chunk[..read]);
                newly_filled += apply_palette_responses(buffer, palette);
                idle_deadline = Instant::now() + Duration::from_millis(50);
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                if Instant::now() >= idle_deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }

    buffer.clear();
    newly_filled
}

#[cfg(unix)]
struct TermiosGuard {
    fd: RawFd,
    original: libc::termios,
}

#[cfg(unix)]
impl Drop for TermiosGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSANOW, &self.original);
        }
    }
}

#[cfg(unix)]
unsafe fn suppress_echo(fd: RawFd) -> Option<TermiosGuard> {
    let mut termios = MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } != 0 {
        return None;
    }
    let termios = unsafe { termios.assume_init() };
    let mut modified = termios;
    modified.c_lflag &= !(libc::ECHO | libc::ECHONL);
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &modified) } != 0 {
        return None;
    }
    Some(TermiosGuard {
        fd,
        original: termios,
    })
}

#[cfg(unix)]
fn apply_palette_responses(
    buffer: &mut Vec<u8>,
    palette: &mut [Option<(u8, u8, u8)>; 256],
) -> usize {
    let mut newly_filled = 0;

    while let Some(start) = buffer.windows(2).position(|window| window == [0x1b, b']']) {
        if start > 0 {
            buffer.drain(..start);
            continue;
        }

        let mut index = 2; // skip ESC ]
        let mut terminator_len = None;
        while index < buffer.len() {
            match buffer[index] {
                0x07 => {
                    terminator_len = Some(1);
                    break;
                }
                0x1b if index + 1 < buffer.len() && buffer[index + 1] == b'\\' => {
                    terminator_len = Some(2);
                    break;
                }
                _ => index += 1,
            }
        }

        let Some(terminator_len) = terminator_len else {
            break;
        };

        let end = index;
        let parsed = std::str::from_utf8(&buffer[2..end])
            .ok()
            .and_then(parse_palette_message);
        let processed = end + terminator_len;
        buffer.drain(..processed);

        if let Some((slot, color)) = parsed
            && palette[slot].is_none()
        {
            palette[slot] = Some(color);
            newly_filled += 1;
        }
    }

    newly_filled
}

#[cfg(unix)]
fn parse_palette_message(message: &str) -> Option<(usize, (u8, u8, u8))> {
    let mut parts = message.splitn(3, ';');
    if parts.next()? != "4" {
        return None;
    }
    let index: usize = parts.next()?.trim().parse().ok()?;
    if index >= 256 {
        return None;
    }
    let payload = parts.next()?;
    let (model, values) = payload.split_once(':')?;
    if model != "rgb" && model != "rgba" {
        return None;
    }
    let mut components = values.split('/');
    let r = parse_component(components.next()?)?;
    let g = parse_component(components.next()?)?;
    let b = parse_component(components.next()?)?;
    Some((index, (r, g, b)))
}

#[cfg(unix)]
fn parse_component(component: &str) -> Option<u8> {
    let trimmed = component.trim();
    if trimmed.is_empty() {
        return None;
    }
    let bits = trimmed.len().checked_mul(4)?;
    if bits == 0 || bits > 64 {
        return None;
    }
    let max = if bits == 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    };
    let value = u64::from_str_radix(trimmed, 16).ok()?;
    Some(((value * 255 + max / 2) / max) as u8)
}
