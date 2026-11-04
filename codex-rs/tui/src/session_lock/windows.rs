//! Snapshot the lock state of the current Windows session.

use windows_sys::Win32::System::RemoteDesktop::WTS_CURRENT_SERVER_HANDLE;
use windows_sys::Win32::System::RemoteDesktop::WTS_CURRENT_SESSION;
use windows_sys::Win32::System::RemoteDesktop::WTS_SESSIONSTATE_LOCK;
use windows_sys::Win32::System::RemoteDesktop::WTS_SESSIONSTATE_UNLOCK;
use windows_sys::Win32::System::RemoteDesktop::WTSFreeMemory;
use windows_sys::Win32::System::RemoteDesktop::WTSINFOEXW;
use windows_sys::Win32::System::RemoteDesktop::WTSQuerySessionInformationW;
use windows_sys::Win32::System::RemoteDesktop::WTSSessionInfoEx;

pub(super) fn is_locked() -> anyhow::Result<bool> {
    let mut buffer = std::ptr::null_mut();
    let mut bytes = 0;
    // SAFETY: The output pointers are valid, and the returned buffer is released below.
    let succeeded = unsafe {
        WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        )
    };
    if succeeded == 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let result = if buffer.is_null() || (bytes as usize) < std::mem::size_of::<WTSINFOEXW>() {
        Err(anyhow::anyhow!(
            "invalid Windows session information buffer"
        ))
    } else {
        // SAFETY: The API returned a buffer large enough for WTSINFOEXW. Reading
        // unaligned avoids relying on the allocator's alignment guarantees.
        let info = unsafe { buffer.cast::<WTSINFOEXW>().read_unaligned() };
        if info.Level != 1 {
            Err(anyhow::anyhow!(
                "unsupported Windows session information level: {}",
                info.Level
            ))
        } else {
            // SAFETY: Level 1 identifies this union member.
            let flags = unsafe { info.Data.WTSInfoExLevel1.SessionFlags } as u32;
            match flags {
                WTS_SESSIONSTATE_LOCK => Ok(true),
                WTS_SESSIONSTATE_UNLOCK => Ok(false),
                _ => Err(anyhow::anyhow!(
                    "unknown Windows session lock state: {flags}"
                )),
            }
        }
    };
    if !buffer.is_null() {
        // SAFETY: This buffer was allocated by WTSQuerySessionInformationW.
        unsafe { WTSFreeMemory(buffer.cast()) };
    }
    result
}
