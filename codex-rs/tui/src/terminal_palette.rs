pub fn terminal_palette() -> Option<[(u8, u8, u8); 256]> {
    imp::terminal_palette()
}

pub fn requery_default_colors() {
    imp::requery_default_colors();
}

#[derive(Clone, Copy)]
pub struct DefaultColors {
    #[allow(dead_code)]
    fg: (u8, u8, u8),
    bg: (u8, u8, u8),
}

pub fn default_colors() -> Option<DefaultColors> {
    imp::default_colors()
}

#[allow(dead_code)]
pub fn default_fg() -> Option<(u8, u8, u8)> {
    default_colors().map(|c| c.fg)
}

pub fn default_bg() -> Option<(u8, u8, u8)> {
    default_colors().map(|c| c.bg)
}

#[cfg(all(unix, not(test)))]
mod imp {
    use super::DefaultColors;
    use std::mem::MaybeUninit;
    use std::os::fd::RawFd;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    struct Cache<T> {
        attempted: bool,
        value: Option<T>,
    }

    impl<T> Default for Cache<T> {
        fn default() -> Self {
            Self {
                attempted: false,
                value: None,
            }
        }
    }

    impl<T: Copy> Cache<T> {
        fn get_or_init_with(&mut self, mut init: impl FnMut() -> Option<T>) -> Option<T> {
            if !self.attempted {
                self.value = init();
                self.attempted = true;
            }
            self.value
        }

        fn refresh_with(&mut self, mut init: impl FnMut() -> Option<T>) -> Option<T> {
            self.value = init();
            self.attempted = true;
            self.value
        }
    }

    fn default_colors_cache() -> &'static Mutex<Cache<DefaultColors>> {
        static CACHE: OnceLock<Mutex<Cache<DefaultColors>>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(Cache::default()))
    }

    pub(super) fn terminal_palette() -> Option<[(u8, u8, u8); 256]> {
        static CACHE: OnceLock<Option<[(u8, u8, u8); 256]>> = OnceLock::new();
        *CACHE.get_or_init(|| match query_terminal_palette() {
            Ok(Some(palette)) => Some(palette),
            _ => None,
        })
    }

    pub(super) fn default_colors() -> Option<DefaultColors> {
        let cache = default_colors_cache();
        let mut cache = cache.lock().ok()?;
        cache.get_or_init_with(|| query_default_colors().unwrap_or_default())
    }

    pub(super) fn requery_default_colors() {
        if let Ok(mut cache) = default_colors_cache().lock() {
            cache.refresh_with(|| query_default_colors().unwrap_or_default());
        }
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn query_default_colors() -> std::io::Result<Option<DefaultColors>> {
        use std::fs::OpenOptions;
        use std::io::ErrorKind;
        use std::io::IsTerminal;
        use std::io::Read;
        use std::io::Write;
        use std::os::fd::AsRawFd;
        use std::time::Duration;
        use std::time::Instant;

        let mut stdout_handle = std::io::stdout();
        if !stdout_handle.is_terminal() {
            return Ok(None);
        }
        stdout_handle.write_all(b"\x1b]10;?\x07\x1b]11;?\x07")?;
        stdout_handle.flush()?;

        let mut tty = match OpenOptions::new().read(true).open("/dev/tty") {
            Ok(file) => file,
            Err(_) => return Ok(None),
        };

        let fd = tty.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags >= 0 {
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }

        let deadline = Instant::now() + Duration::from_millis(200);
        let mut buffer = Vec::new();
        let mut fg = None;
        let mut bg = None;

        while Instant::now() < deadline {
            let mut chunk = [0u8; 128];
            match tty.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buffer.extend_from_slice(&chunk[..n]);
                    if fg.is_none() {
                        fg = parse_osc_color(&buffer, 10);
                    }
                    if bg.is_none() {
                        bg = parse_osc_color(&buffer, 11);
                    }
                    if let (Some(fg), Some(bg)) = (fg, bg) {
                        return Ok(Some(DefaultColors { fg, bg }));
                    }
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        if fg.is_none() {
            fg = parse_osc_color(&buffer, 10);
        }
        if bg.is_none() {
            bg = parse_osc_color(&buffer, 11);
        }

        Ok(fg.zip(bg).map(|(fg, bg)| DefaultColors { fg, bg }))
    }

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

    struct TermiosGuard {
        fd: RawFd,
        original: libc::termios,
    }

    impl Drop for TermiosGuard {
        fn drop(&mut self) {
            unsafe {
                libc::tcsetattr(self.fd, libc::TCSANOW, &self.original);
            }
        }
    }

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

    fn parse_osc_color(buffer: &[u8], code: u8) -> Option<(u8, u8, u8)> {
        let text = std::str::from_utf8(buffer).ok()?;
        let prefix = match code {
            10 => "\u{1b}]10;",
            11 => "\u{1b}]11;",
            _ => return None,
        };
        let start = text.rfind(prefix)?;
        let after_prefix = &text[start + prefix.len()..];
        let end_bel = after_prefix.find('\u{7}');
        let end_st = after_prefix.find("\u{1b}\\");
        let end_idx = match (end_bel, end_st) {
            (Some(bel), Some(st)) => bel.min(st),
            (Some(bel), None) => bel,
            (None, Some(st)) => st,
            (None, None) => return None,
        };
        let payload = after_prefix[..end_idx].trim();
        parse_color_payload(payload)
    }

    fn parse_color_payload(payload: &str) -> Option<(u8, u8, u8)> {
        if payload.is_empty() || payload == "?" {
            return None;
        }
        let (model, values) = payload.split_once(':')?;
        if model != "rgb" && model != "rgba" {
            return None;
        }
        let mut parts = values.split('/');
        let r = parse_component(parts.next()?)?;
        let g = parse_component(parts.next()?)?;
        let b = parse_component(parts.next()?)?;
        Some((r, g, b))
    }
}

#[cfg(not(all(unix, not(test))))]
mod imp {
    use super::DefaultColors;

    pub(super) fn terminal_palette() -> Option<[(u8, u8, u8); 256]> {
        None
    }

    pub(super) fn default_colors() -> Option<DefaultColors> {
        None
    }

    pub(super) fn requery_default_colors() {}
}
