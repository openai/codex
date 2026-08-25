//! Probe visible Windows terminal colors without consuming startup typeahead.

use super::DefaultColors;
use super::windows_replay::terminal_color_response_ranges;
use super::windows_replay::terminal_default_colors;
use std::io;
use std::io::ErrorKind;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::Storage::FileSystem::WriteFile;
use windows_sys::Win32::System::Console::CONSOLE_SCREEN_BUFFER_INFOEX;
use windows_sys::Win32::System::Console::GetConsoleScreenBufferInfoEx;
use windows_sys::Win32::System::Console::GetNumberOfConsoleInputEvents;
use windows_sys::Win32::System::Console::GetStdHandle;
use windows_sys::Win32::System::Console::INPUT_RECORD;
use windows_sys::Win32::System::Console::KEY_EVENT;
use windows_sys::Win32::System::Console::ReadConsoleInputW;
use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
use windows_sys::Win32::System::Console::WriteConsoleInputW;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

const MAX_WINDOWS_PROBE_RECORDS: usize = 64 * 1_024;
const WINDOWS_PROBE_READ_RECORDS: usize = 64;

/// Prefer the terminal renderer's OSC colors while retaining the native console palette fallback.
///
/// Windows Terminal and xterm.js can render a theme unrelated to ConPTY's console color table.
/// Their OSC responses share the input queue with user input, so the probe preserves and restores
/// the original console records without changing Windows' input-record mode.
pub(crate) fn default_colors(timeout: Duration) -> io::Result<Option<DefaultColors>> {
    let Ok(output) = std_handle(STD_OUTPUT_HANDLE) else {
        return Ok(None);
    };

    if let Ok(input) = std_handle(STD_INPUT_HANDLE)
        && let Ok(Some(colors)) = query_osc_default_colors(input, output, timeout)
    {
        tracing::debug!(
            foreground = ?colors.fg,
            background = ?colors.bg,
            "using Windows terminal OSC default colors"
        );
        return Ok(Some(colors));
    }

    let colors = query_console_default_colors(output).ok().flatten();
    if let Some(colors) = colors {
        tracing::debug!(
            foreground = ?colors.fg,
            background = ?colors.bg,
            "using Windows console palette default colors"
        );
    }
    Ok(colors)
}

fn query_osc_default_colors(
    input: HANDLE,
    output: HANDLE,
    timeout: Duration,
) -> io::Result<Option<DefaultColors>> {
    let deadline = Instant::now() + timeout;
    write_all(output, b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\")?;

    let mut replay = ConsoleInputReplay::new(input);
    let result = replay.read_until(deadline);
    let replay_result = replay.replay();
    replay_result?;
    result
}

/// Preserve native console records and map response bytes back to their original records.
#[derive(Default)]
struct BufferedConsoleInput {
    records: Vec<INPUT_RECORD>,
    bytes: Vec<u8>,
    byte_record_indices: Vec<usize>,
}

impl BufferedConsoleInput {
    fn push(&mut self, record: INPUT_RECORD) {
        let index = self.records.len();
        self.records.push(record);

        if record.EventType != KEY_EVENT as u16 {
            return;
        }

        // SAFETY: EventType identifies the active INPUT_RECORD union member.
        let key = unsafe { record.Event.KeyEvent };
        if key.bKeyDown == 0 {
            return;
        }

        // SAFETY: ReadConsoleInputW populates the UnicodeChar member of KEY_EVENT_RECORD.
        let character = unsafe { key.uChar.UnicodeChar };
        if let Ok(byte) = u8::try_from(character)
            && byte.is_ascii()
            && byte != 0
        {
            self.bytes.push(byte);
            self.byte_record_indices.push(index);
        }
    }

    fn preserved_records(&self) -> Vec<INPUT_RECORD> {
        let mut omitted = vec![false; self.records.len()];
        for response in terminal_color_response_ranges(&self.bytes) {
            for byte_index in response {
                let record_index = self.byte_record_indices[byte_index];
                omitted[record_index] = true;
            }
        }

        self.records
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, record)| (!omitted[index]).then_some(record))
            .collect()
    }
}

/// Replay every non-response console event even when probing exits through an error path.
struct ConsoleInputReplay {
    handle: HANDLE,
    input: BufferedConsoleInput,
    replayed_records: usize,
}

impl ConsoleInputReplay {
    fn new(handle: HANDLE) -> Self {
        Self {
            handle,
            input: BufferedConsoleInput::default(),
            replayed_records: 0,
        }
    }

    fn read_until(&mut self, deadline: Instant) -> io::Result<Option<DefaultColors>> {
        loop {
            if let Some(colors) = terminal_default_colors(&self.input.bytes) {
                return Ok(Some(colors));
            }
            if self.input.records.len() >= MAX_WINDOWS_PROBE_RECORDS {
                return Ok(None);
            }

            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let timeout_ms = deadline
                .saturating_duration_since(now)
                .as_millis()
                .min(u32::MAX as u128) as u32;

            match unsafe {
                WaitForSingleObject(self.handle, timeout_ms.max(/*other*/ 1))
            } {
                WAIT_OBJECT_0 => {
                    self.read_available()?;
                }
                WAIT_TIMEOUT => return Ok(None),
                _ => return Err(io::Error::last_os_error()),
            }
        }
    }

    fn read_available(&mut self) -> io::Result<usize> {
        let mut pending = 0;
        if unsafe { GetNumberOfConsoleInputEvents(self.handle, &mut pending) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if pending == 0 {
            return Ok(0);
        }

        let mut records =
            [unsafe { std::mem::zeroed::<INPUT_RECORD>() }; WINDOWS_PROBE_READ_RECORDS];
        let max_records = records
            .len()
            .min(pending as usize)
            .min(MAX_WINDOWS_PROBE_RECORDS.saturating_sub(self.input.records.len()));
        if max_records == 0 {
            return Ok(0);
        }
        let mut read = 0;
        let result = unsafe {
            ReadConsoleInputW(
                self.handle,
                records.as_mut_ptr(),
                max_records as u32,
                &mut read,
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        for record in records.into_iter().take(read as usize) {
            self.input.push(record);
        }
        Ok(read as usize)
    }

    fn replay(&mut self) -> io::Result<()> {
        if self.input.records.is_empty() {
            return Ok(());
        }

        if self.replayed_records == 0 {
            // WriteConsoleInputW appends, so include already queued later records before restoring
            // the earlier records. Preserve one bounded queue snapshot without waiting on input.
            let mut pending = 0;
            if unsafe { GetNumberOfConsoleInputEvents(self.handle, &mut pending) } == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut remaining = (pending as usize)
                .min(MAX_WINDOWS_PROBE_RECORDS.saturating_sub(self.input.records.len()));
            while remaining != 0 {
                let read = self.read_available()?;
                if read == 0 {
                    break;
                }
                remaining = remaining.saturating_sub(read);
            }
        }

        let preserved = self.input.preserved_records();
        while self.replayed_records < preserved.len() {
            let remaining = &preserved[self.replayed_records..];
            let mut written = 0;
            let result = unsafe {
                WriteConsoleInputW(
                    self.handle,
                    remaining.as_ptr(),
                    remaining.len().min(u32::MAX as usize) as u32,
                    &mut written,
                )
            };
            if result == 0 {
                return Err(io::Error::last_os_error());
            }
            if written == 0 {
                return Err(io::Error::from(ErrorKind::WriteZero));
            }
            self.replayed_records += written as usize;
        }

        self.input.records.clear();
        self.input.bytes.clear();
        self.input.byte_record_indices.clear();
        self.replayed_records = 0;
        Ok(())
    }
}

impl Drop for ConsoleInputReplay {
    fn drop(&mut self) {
        if let Err(error) = self.replay() {
            tracing::warn!(%error, "failed to restore Windows terminal startup input");
        }
    }
}

fn write_all(handle: HANDLE, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        let mut written = 0;
        let result = unsafe {
            WriteFile(
                handle,
                bytes.as_ptr().cast(),
                bytes.len().min(u32::MAX as usize) as u32,
                &mut written,
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        if written == 0 {
            return Err(io::Error::from(ErrorKind::WriteZero));
        }
        bytes = &bytes[written as usize..];
    }
    Ok(())
}

fn query_console_default_colors(output: HANDLE) -> io::Result<Option<DefaultColors>> {
    let mut info = unsafe { std::mem::zeroed::<CONSOLE_SCREEN_BUFFER_INFOEX>() };
    info.cbSize = std::mem::size_of::<CONSOLE_SCREEN_BUFFER_INFOEX>() as u32;
    if unsafe { GetConsoleScreenBufferInfoEx(output, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Some(decode_console_default_colors(
        info.wAttributes,
        &info.ColorTable,
    )))
}

fn decode_console_default_colors(attributes: u16, color_table: &[u32; 16]) -> DefaultColors {
    let fg_index = (attributes & 0x0f) as usize;
    let bg_index = ((attributes >> 4) & 0x0f) as usize;
    // COMMON_LVB_REVERSE_VIDEO changes how cells render, but this probe is discovering the
    // configured default colors for palette blending. Keep the attribute fg/bg indices as-is.
    DefaultColors {
        fg: decode_color_ref(color_table[fg_index]),
        bg: decode_color_ref(color_table[bg_index]),
    }
}

fn decode_color_ref(color_ref: u32) -> (u8, u8, u8) {
    (
        (color_ref & 0xff) as u8,
        ((color_ref >> 8) & 0xff) as u8,
        ((color_ref >> 16) & 0xff) as u8,
    )
}

fn std_handle(kind: u32) -> io::Result<HANDLE> {
    let handle = unsafe { GetStdHandle(kind) };
    if handle == 0 || handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(handle)
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
