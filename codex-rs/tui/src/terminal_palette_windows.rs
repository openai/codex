use super::DefaultColors;
use super::terminal_palette_common::Cache;
use super::terminal_palette_common::apply_palette_responses;
use super::terminal_palette_common::parse_osc_color;
use std::env;
use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io;
use std::io::ErrorKind;
use std::io::IsTerminal;
use std::io::Write;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::ReadFile;
use windows_sys::Win32::System::Console::ENABLE_ECHO_INPUT;
use windows_sys::Win32::System::Console::ENABLE_LINE_INPUT;
use windows_sys::Win32::System::Console::ENABLE_PROCESSED_INPUT;
use windows_sys::Win32::System::Console::ENABLE_PROCESSED_OUTPUT;
use windows_sys::Win32::System::Console::ENABLE_VIRTUAL_TERMINAL_INPUT;
use windows_sys::Win32::System::Console::ENABLE_VIRTUAL_TERMINAL_PROCESSING;
use windows_sys::Win32::System::Console::GetConsoleMode;
use windows_sys::Win32::System::Console::GetStdHandle;
use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
use windows_sys::Win32::System::Console::SetConsoleMode;
use windows_sys::Win32::System::Threading::WAIT_FAILED;
use windows_sys::Win32::System::Threading::WAIT_OBJECT_0;
use windows_sys::Win32::System::Threading::WAIT_TIMEOUT;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

const RESPONSE_TIMEOUT: Duration = Duration::from_millis(1500);

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

fn default_colors_cache() -> &'static Mutex<Cache<DefaultColors>> {
    static CACHE: OnceLock<Mutex<Cache<DefaultColors>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}

fn is_windows_terminal() -> bool {
    env::var("WT_SESSION")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn query_terminal_palette() -> io::Result<Option<[(u8, u8, u8); 256]>> {
    if !is_windows_terminal() {
        return Ok(None);
    }

    let mut stdout_handle = io::stdout();
    if !stdout_handle.is_terminal() {
        return Ok(None);
    }

    let _output_guard = ConsoleOutputGuard::acquire();
    for index in 0..256 {
        write!(stdout_handle, "\x1b]4;{index};?\x07")?;
    }
    stdout_handle.flush()?;

    let mut reader = match ConsoleInputReader::new() {
        Ok(reader) => reader,
        Err(_) => return Ok(None),
    };

    let mut palette: [Option<(u8, u8, u8)>; 256] = [None; 256];
    let mut buffer = Vec::new();
    let mut remaining = palette.len();
    let deadline = Instant::now() + RESPONSE_TIMEOUT;

    while remaining > 0 && Instant::now() < deadline {
        if !reader.read_available(&mut buffer, Duration::from_millis(25))? {
            continue;
        }
        let newly = apply_palette_responses(&mut buffer, &mut palette);
        if newly == 0 {
            continue;
        }
        remaining = remaining.saturating_sub(newly);
    }

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

fn query_default_colors() -> io::Result<Option<DefaultColors>> {
    if !is_windows_terminal() {
        return Ok(None);
    }

    let mut stdout_handle = io::stdout();
    if !stdout_handle.is_terminal() {
        return Ok(None);
    }

    let _output_guard = ConsoleOutputGuard::acquire();
    stdout_handle.write_all(b"\x1b]10;?\x07\x1b]11;?\x07")?;
    stdout_handle.flush()?;

    let mut reader = match ConsoleInputReader::new() {
        Ok(reader) => reader,
        Err(_) => return Ok(None),
    };

    let mut buffer = Vec::new();
    let mut fg = None;
    let mut bg = None;
    let deadline = Instant::now() + Duration::from_millis(250);

    while Instant::now() < deadline {
        reader.read_available(&mut buffer, Duration::from_millis(20))?;
        if fg.is_none() {
            fg = parse_osc_color(&buffer, 10);
        }
        if bg.is_none() {
            bg = parse_osc_color(&buffer, 11);
        }
        if fg.is_some() && bg.is_some() {
            break;
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

struct ConsoleOutputGuard {
    handle: HANDLE,
    original_mode: Option<u32>,
}

impl ConsoleOutputGuard {
    fn acquire() -> Option<Self> {
        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle == INVALID_HANDLE_VALUE || handle == 0 {
                return None;
            }
            let mut original = 0u32;
            if GetConsoleMode(handle, &mut original) == 0 {
                return None;
            }
            let desired = original | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            if desired == original {
                return Some(Self {
                    handle,
                    original_mode: Some(original),
                });
            }
            if SetConsoleMode(handle, desired) == 0 {
                return None;
            }
            Some(Self {
                handle,
                original_mode: Some(original),
            })
        }
    }
}

impl Drop for ConsoleOutputGuard {
    fn drop(&mut self) {
        if let Some(original) = self.original_mode {
            unsafe {
                let _ = SetConsoleMode(self.handle, original);
            }
        }
    }
}

struct ConsoleInputReader {
    _file: std::fs::File,
    handle: HANDLE,
    _guard: ConsoleInputModeGuard,
}

impl ConsoleInputReader {
    fn new() -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true);
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        let file = options.open("CONIN$")?;
        let handle = file.as_raw_handle() as HANDLE;
        if handle == INVALID_HANDLE_VALUE || handle == 0 {
            return Err(io::Error::other("invalid console handle"));
        }
        let guard = ConsoleInputModeGuard::acquire(handle)?;
        Ok(Self {
            _file: file,
            handle,
            _guard: guard,
        })
    }

    fn read_available(&mut self, buffer: &mut Vec<u8>, wait: Duration) -> io::Result<bool> {
        let mut any = false;
        let mut current_wait = duration_to_millis(wait);
        loop {
            let status = unsafe { WaitForSingleObject(self.handle, current_wait) };
            match status {
                WAIT_OBJECT_0 => {
                    if self.read_once(buffer)? {
                        any = true;
                    }
                    current_wait = 0;
                    continue;
                }
                WAIT_TIMEOUT => {
                    break;
                }
                WAIT_FAILED => {
                    return Err(io::Error::last_os_error());
                }
                _ => break,
            }
        }
        Ok(any)
    }

    fn read_once(&mut self, buffer: &mut Vec<u8>) -> io::Result<bool> {
        let mut chunk = [0u8; 512];
        let mut read = 0u32;
        let success = unsafe {
            ReadFile(
                self.handle,
                chunk.as_mut_ptr() as *mut c_void,
                chunk.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            )
        };
        if success == 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted || err.kind() == io::ErrorKind::WouldBlock {
                return Ok(false);
            }
            return Err(err);
        }
        if read == 0 {
            return Ok(false);
        }
        buffer.extend_from_slice(&chunk[..read as usize]);
        Ok(true)
    }
}

struct ConsoleInputModeGuard {
    handle: HANDLE,
    original_mode: u32,
}

impl ConsoleInputModeGuard {
    fn acquire(handle: HANDLE) -> io::Result<Self> {
        unsafe {
            let mut original = 0u32;
            if GetConsoleMode(handle, &mut original) == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut desired = original;
            desired |= ENABLE_VIRTUAL_TERMINAL_INPUT | ENABLE_PROCESSED_INPUT;
            desired &= !ENABLE_LINE_INPUT;
            desired &= !ENABLE_ECHO_INPUT;
            if desired != original && SetConsoleMode(handle, desired) == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self {
                handle,
                original_mode: original,
            })
        }
    }
}

impl Drop for ConsoleInputModeGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SetConsoleMode(self.handle, self.original_mode);
        }
    }
}

fn duration_to_millis(duration: Duration) -> u32 {
    duration.as_millis().try_into().unwrap_or(u32::MAX)
}
