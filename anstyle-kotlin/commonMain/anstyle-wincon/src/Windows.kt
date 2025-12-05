//! Low-level wincon-styling

use std::os::windows::io::AsHandle;
use std::os::windows::io::AsRawHandle;

type StdioColorResult = std::io::Result<(anstyle::AnsiColor, anstyle::AnsiColor)>;
type StdioColorInnerResult = Result<(anstyle::AnsiColor, anstyle::AnsiColor), inner::IoError>;

/// Cached [`get_colors`] call for [`std::io::stdout`]
pub fn stdout_initial_colors() -> StdioColorResult {
    static INITIAL: once_cell_polyfill::sync::OnceLock<StdioColorInnerResult> =
        once_cell_polyfill::sync::OnceLock::new();
    (*INITIAL.get_or_init(|| get_colors_(&std::io::stdout()))).map_err(Into::into)
}

/// Cached [`get_colors`] call for [`std::io::stderr`]
pub fn stderr_initial_colors() -> StdioColorResult {
    static INITIAL: once_cell_polyfill::sync::OnceLock<StdioColorInnerResult> =
        once_cell_polyfill::sync::OnceLock::new();
    (*INITIAL.get_or_init(|| get_colors_(&std::io::stderr()))).map_err(Into::into)
}

/// Apply colors to future writes
///
/// **Note:** Make sure any buffers are first flushed or else these colors will apply
pub fn set_colors<S: AsHandle>(
    stream: &mut S,
    fg: anstyle::AnsiColor,
    bg: anstyle::AnsiColor,
) -> std::io::Result<()> {
    set_colors_(stream, fg, bg).map_err(Into::into)
}

fn set_colors_<S: AsHandle>(
    stream: &mut S,
    fg: anstyle::AnsiColor,
    bg: anstyle::AnsiColor,
) -> Result<(), inner::IoError> {
    let handle = stream.as_handle();
    let handle = handle.as_raw_handle();
    let attributes = inner::set_colors(fg, bg);
    inner::set_console_text_attributes(handle, attributes)
}

/// Get the colors currently active on the console
pub fn get_colors<S: AsHandle>(stream: &S) -> StdioColorResult {
    get_colors_(stream).map_err(Into::into)
}

fn get_colors_<S: AsHandle>(stream: &S) -> StdioColorInnerResult {
    let handle = stream.as_handle();
    let handle = handle.as_raw_handle();
    let info = inner::get_screen_buffer_info(handle)?;
    let (fg, bg) = inner::get_colors(&info);
    Ok((fg, bg))
}

pub(crate) fn write_colored<S: AsHandle + std::io::Write>(
    stream: &mut S,
    fg: Option<anstyle::AnsiColor>,
    bg: Option<anstyle::AnsiColor>,
    data: &[u8],
    initial: StdioColorResult,
) -> std::io::Result<usize> {
    let (initial_fg, initial_bg) = initial?;
    let non_default = fg.is_some() || bg.is_some();

    if non_default {
        let fg = fg.unwrap_or(initial_fg);
        let bg = bg.unwrap_or(initial_bg);
        // Ensure everything is written with the last set of colors before applying the next set
        stream.flush()?;
        set_colors(stream, fg, bg)?;
    }
    let written = stream.write(data)?;
    if non_default {
        // Ensure everything is written with the last set of colors before applying the next set
        stream.flush()?;
        set_colors(stream, initial_fg, initial_bg)?;
    }
    Ok(written)
}

mod inner {
    use std::os::windows::io::RawHandle;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Console::CONSOLE_CHARACTER_ATTRIBUTES;
    use windows_sys::Win32::System::Console::CONSOLE_SCREEN_BUFFER_INFO;
    use windows_sys::Win32::System::Console::FOREGROUND_BLUE;
    use windows_sys::Win32::System::Console::FOREGROUND_GREEN;
    use windows_sys::Win32::System::Console::FOREGROUND_INTENSITY;
    use windows_sys::Win32::System::Console::FOREGROUND_RED;

    const FOREGROUND_CYAN: CONSOLE_CHARACTER_ATTRIBUTES = FOREGROUND_BLUE | FOREGROUND_GREEN;
    const FOREGROUND_MAGENTA: CONSOLE_CHARACTER_ATTRIBUTES = FOREGROUND_BLUE | FOREGROUND_RED;
    const FOREGROUND_YELLOW: CONSOLE_CHARACTER_ATTRIBUTES = FOREGROUND_GREEN | FOREGROUND_RED;
    const FOREGROUND_WHITE: CONSOLE_CHARACTER_ATTRIBUTES =
        FOREGROUND_BLUE | FOREGROUND_GREEN | FOREGROUND_RED;

    #[derive(Copy, Clone, Debug)]
    pub(crate) enum IoError {
        BrokenPipe,
        RawOs(i32),
    }

    impl From<IoError> for std::io::Error {
        fn from(io: IoError) -> Self {
            match io {
                IoError::BrokenPipe => {
                    std::io::Error::new(std::io::ErrorKind::BrokenPipe, "console is detached")
                }
                IoError::RawOs(code) => std::io::Error::from_raw_os_error(code),
            }
        }
    }

    impl IoError {
        fn last_os_error() -> Self {
            Self::RawOs(std::io::Error::last_os_error().raw_os_error().unwrap())
        }
    }

    pub(crate) fn get_screen_buffer_info(
        handle: RawHandle,
    ) -> Result<CONSOLE_SCREEN_BUFFER_INFO, IoError> {
        unsafe {
            let handle: HANDLE = handle as HANDLE;
            if handle.is_null() {
                return Err(IoError::BrokenPipe);
            }

            let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if windows_sys::Win32::System::Console::GetConsoleScreenBufferInfo(handle, &mut info)
                != 0
            {
                Ok(info)
            } else {
                Err(IoError::last_os_error())
            }
        }
    }

    pub(crate) fn set_console_text_attributes(
        handle: RawHandle,
        attributes: CONSOLE_CHARACTER_ATTRIBUTES,
    ) -> Result<(), IoError> {
        unsafe {
            let handle: HANDLE = handle as HANDLE;
            if handle.is_null() {
                return Err(IoError::BrokenPipe);
            }

            if windows_sys::Win32::System::Console::SetConsoleTextAttribute(handle, attributes) != 0
            {
                Ok(())
            } else {
                Err(IoError::last_os_error())
            }
        }
    }

    pub(crate) fn get_colors(
        info: &CONSOLE_SCREEN_BUFFER_INFO,
    ) -> (anstyle::AnsiColor, anstyle::AnsiColor) {
        let attributes = info.wAttributes;
        let bg = from_nibble(attributes >> 4);
        let fg = from_nibble(attributes);
        (fg, bg)
    }

    pub(crate) fn set_colors(
        fg: anstyle::AnsiColor,
        bg: anstyle::AnsiColor,
    ) -> CONSOLE_CHARACTER_ATTRIBUTES {
        to_nibble(bg) << 4 | to_nibble(fg)
    }

    fn from_nibble(color: CONSOLE_CHARACTER_ATTRIBUTES) -> anstyle::AnsiColor {
        if color & FOREGROUND_WHITE == FOREGROUND_WHITE {
            // 3 bits high
            anstyle::AnsiColor::White
        } else if color & FOREGROUND_CYAN == FOREGROUND_CYAN {
            // 2 bits high
            anstyle::AnsiColor::Cyan
        } else if color & FOREGROUND_YELLOW == FOREGROUND_YELLOW {
            // 2 bits high
            anstyle::AnsiColor::Yellow
        } else if color & FOREGROUND_MAGENTA == FOREGROUND_MAGENTA {
            // 2 bits high
            anstyle::AnsiColor::Magenta
        } else if color & FOREGROUND_RED == FOREGROUND_RED {
            // 1 bit high
            anstyle::AnsiColor::Red
        } else if color & FOREGROUND_GREEN == FOREGROUND_GREEN {
            // 1 bit high
            anstyle::AnsiColor::Green
        } else if color & FOREGROUND_BLUE == FOREGROUND_BLUE {
            // 1 bit high
            anstyle::AnsiColor::Blue
        } else {
            // 0 bits high
            anstyle::AnsiColor::Black
        }
        .bright(color & FOREGROUND_INTENSITY == FOREGROUND_INTENSITY)
    }

    fn to_nibble(color: anstyle::AnsiColor) -> CONSOLE_CHARACTER_ATTRIBUTES {
        let mut attributes = 0;
        attributes |= match color.bright(false) {
            anstyle::AnsiColor::Black => 0,
            anstyle::AnsiColor::Red => FOREGROUND_RED,
            anstyle::AnsiColor::Green => FOREGROUND_GREEN,
            anstyle::AnsiColor::Yellow => FOREGROUND_YELLOW,
            anstyle::AnsiColor::Blue => FOREGROUND_BLUE,
            anstyle::AnsiColor::Magenta => FOREGROUND_MAGENTA,
            anstyle::AnsiColor::Cyan => FOREGROUND_CYAN,
            anstyle::AnsiColor::White => FOREGROUND_WHITE,
            anstyle::AnsiColor::BrightBlack
            | anstyle::AnsiColor::BrightRed
            | anstyle::AnsiColor::BrightGreen
            | anstyle::AnsiColor::BrightYellow
            | anstyle::AnsiColor::BrightBlue
            | anstyle::AnsiColor::BrightMagenta
            | anstyle::AnsiColor::BrightCyan
            | anstyle::AnsiColor::BrightWhite => unreachable!("brights were toggled off"),
        };
        if color.is_bright() {
            attributes |= FOREGROUND_INTENSITY;
        }
        attributes
    }

    #[test]
    fn to_from_nibble() {
        const COLORS: [anstyle::AnsiColor; 16] = [
            anstyle::AnsiColor::Black,
            anstyle::AnsiColor::Red,
            anstyle::AnsiColor::Green,
            anstyle::AnsiColor::Yellow,
            anstyle::AnsiColor::Blue,
            anstyle::AnsiColor::Magenta,
            anstyle::AnsiColor::Cyan,
            anstyle::AnsiColor::White,
            anstyle::AnsiColor::BrightBlack,
            anstyle::AnsiColor::BrightRed,
            anstyle::AnsiColor::BrightGreen,
            anstyle::AnsiColor::BrightYellow,
            anstyle::AnsiColor::BrightBlue,
            anstyle::AnsiColor::BrightMagenta,
            anstyle::AnsiColor::BrightCyan,
            anstyle::AnsiColor::BrightWhite,
        ];
        for expected in COLORS {
            let nibble = to_nibble(expected);
            let actual = from_nibble(nibble);
            assert_eq!(expected, actual, "Intermediate: {nibble}");
        }
    }
}
