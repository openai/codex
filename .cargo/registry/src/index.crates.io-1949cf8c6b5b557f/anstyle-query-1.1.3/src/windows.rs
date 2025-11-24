//! Windows-specific style queries

#[cfg(windows)]
mod windows_console {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::io::RawHandle;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Console::CONSOLE_MODE;
    use windows_sys::Win32::System::Console::ENABLE_VIRTUAL_TERMINAL_PROCESSING;

    fn enable_vt(handle: RawHandle) -> std::io::Result<()> {
        unsafe {
            let handle: HANDLE = handle as HANDLE;
            if handle.is_null() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "console is detached",
                ));
            }

            let mut dwmode: CONSOLE_MODE = 0;
            if windows_sys::Win32::System::Console::GetConsoleMode(handle, &mut dwmode) == 0 {
                return Err(std::io::Error::last_os_error());
            }

            dwmode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            if windows_sys::Win32::System::Console::SetConsoleMode(handle, dwmode) == 0 {
                return Err(std::io::Error::last_os_error());
            }

            Ok(())
        }
    }

    pub(crate) fn enable_virtual_terminal_processing() -> std::io::Result<()> {
        let stdout = std::io::stdout();
        let stdout_handle = stdout.as_raw_handle();
        let stderr = std::io::stderr();
        let stderr_handle = stderr.as_raw_handle();

        enable_vt(stdout_handle)?;
        if stdout_handle != stderr_handle {
            enable_vt(stderr_handle)?;
        }

        Ok(())
    }

    #[inline]
    pub(crate) fn enable_ansi_colors() -> Option<bool> {
        Some(
            enable_virtual_terminal_processing()
                .map(|_| true)
                .unwrap_or(false),
        )
    }
}

#[cfg(not(windows))]
mod windows_console {
    #[inline]
    pub(crate) fn enable_ansi_colors() -> Option<bool> {
        None
    }
}

/// Enable ANSI escape codes ([`ENABLE_VIRTUAL_TERMINAL_PROCESSING`](https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences#output-sequences))
///
/// For non-windows systems, returns `None`
pub fn enable_ansi_colors() -> Option<bool> {
    windows_console::enable_ansi_colors()
}

/// Raw `ENABLE_VIRTUAL_TERMINAL_PROCESSING` on stdout/stderr
#[cfg(windows)]
pub fn enable_virtual_terminal_processing() -> std::io::Result<()> {
    windows_console::enable_virtual_terminal_processing()
}
