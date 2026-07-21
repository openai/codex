const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;

#[derive(Clone, Copy)]
pub(super) enum VirtualTerminalInput {
    Enabled,
    Disabled,
}

pub(super) fn input_record_mode(mode: u32) -> u32 {
    mode & !ENABLE_VIRTUAL_TERMINAL_INPUT
}

pub(super) fn restored_input_mode(mode: u32, original: VirtualTerminalInput) -> u32 {
    match original {
        VirtualTerminalInput::Enabled => mode | ENABLE_VIRTUAL_TERMINAL_INPUT,
        VirtualTerminalInput::Disabled => input_record_mode(mode),
    }
}

#[cfg(windows)]
static ORIGINAL_VT_INPUT: std::sync::Mutex<Vec<VirtualTerminalInput>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(windows)]
fn current_input_mode() -> Option<(windows_sys::Win32::Foundation::HANDLE, u32)> {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::GetConsoleMode;
    use windows_sys::Win32::System::Console::GetStdHandle;
    use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;

    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if handle == INVALID_HANDLE_VALUE || handle == 0 {
        return None;
    }

    let mut mode = 0;
    if unsafe { GetConsoleMode(handle, &mut mode) } == 0 {
        return None;
    }

    Some((handle, mode))
}

#[cfg(windows)]
pub(super) fn set_input_record_mode() -> std::io::Result<()> {
    use windows_sys::Win32::System::Console::SetConsoleMode;

    let Some((handle, mode)) = current_input_mode() else {
        return Ok(());
    };
    let requested_mode = input_record_mode(mode);
    if requested_mode != mode && unsafe { SetConsoleMode(handle, requested_mode) } == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let original = if mode & ENABLE_VIRTUAL_TERMINAL_INPUT != 0 {
        VirtualTerminalInput::Enabled
    } else {
        VirtualTerminalInput::Disabled
    };
    ORIGINAL_VT_INPUT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(original);
    Ok(())
}

#[cfg(windows)]
pub(super) fn ensure_input_record_mode() -> std::io::Result<()> {
    use windows_sys::Win32::System::Console::SetConsoleMode;

    let Some((handle, mode)) = current_input_mode() else {
        return Ok(());
    };
    let requested_mode = input_record_mode(mode);
    if requested_mode != mode && unsafe { SetConsoleMode(handle, requested_mode) } == 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(windows)]
pub(super) fn restore_input_mode() -> std::io::Result<()> {
    use windows_sys::Win32::System::Console::SetConsoleMode;

    let mut original_modes = ORIGINAL_VT_INPUT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(original) = original_modes.last().copied() else {
        return Ok(());
    };
    let Some((handle, mode)) = current_input_mode() else {
        original_modes.pop();
        return Ok(());
    };
    let requested_mode = restored_input_mode(mode, original);
    if requested_mode != mode && unsafe { SetConsoleMode(handle, requested_mode) } == 0 {
        return Err(std::io::Error::last_os_error());
    }

    original_modes.pop();
    Ok(())
}
