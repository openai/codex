#[cfg(any(all(unix, not(test)), all(windows, not(test))))]
#[path = "terminal_palette/common.rs"]
mod terminal_palette_common;

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

#[allow(dead_code)]
pub fn default_bg() -> Option<(u8, u8, u8)> {
    default_colors().map(|c| c.bg)
}

#[cfg(all(unix, not(test)))]
mod imp {
    use super::DefaultColors;
    use super::terminal_palette_common::Cache;
    use super::terminal_palette_common::apply_palette_responses;
    use super::terminal_palette_common::parse_osc_color;
    use std::mem::MaybeUninit;
    use std::os::fd::RawFd;
    use std::sync::Mutex;
    use std::sync::OnceLock;

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
}

#[cfg(all(windows, not(test)))]
#[path = "terminal_palette_windows.rs"]
mod imp;

#[cfg(not(any(all(unix, not(test)), all(windows, not(test)))))]
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
