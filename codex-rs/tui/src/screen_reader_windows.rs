//! Detects Windows Narrator in the TUI's own login session.
//!
//! Narrator does not set the screen-reader flag used by other Windows accessibility aids. Do not
//! use a Narrator process in another session to change this terminal's behavior.

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::ERROR_NO_MORE_FILES;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Diagnostics::ToolHelp::CreateToolhelp32Snapshot;
use windows_sys::Win32::System::Diagnostics::ToolHelp::PROCESSENTRY32W;
use windows_sys::Win32::System::Diagnostics::ToolHelp::Process32FirstW;
use windows_sys::Win32::System::Diagnostics::ToolHelp::Process32NextW;
use windows_sys::Win32::System::Diagnostics::ToolHelp::TH32CS_SNAPPROCESS;
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;

pub(super) fn narrator_running() -> Option<bool> {
    let current_session = process_session(std::process::id())?;
    // SAFETY: This requests a system-wide process snapshot and ignores the process ID.
    let snapshot = unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, /*th32processid*/ 0)
    };
    if snapshot == INVALID_HANDLE_VALUE {
        return None;
    }

    let result = (|| {
        // SAFETY: All-zero is valid for this Win32 struct; dwSize must then be set for the API.
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut inaccessible_match = false;
        // SAFETY: The snapshot is valid and entry is an initialized, correctly sized output.
        let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) };
        while has_entry != 0 {
            let name_end = entry
                .szExeFile
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..name_end])
                .eq_ignore_ascii_case("Narrator.exe")
            {
                match process_session(entry.th32ProcessID) {
                    Some(session) if session == current_session => return Some(true),
                    Some(_) => {}
                    None => inaccessible_match = true,
                }
            }
            // SAFETY: The snapshot and output remain valid for the duration of the iteration.
            has_entry = unsafe { Process32NextW(snapshot, &mut entry) };
        }
        // SAFETY: This immediately follows the failed enumeration call on this thread.
        (unsafe { GetLastError() } == ERROR_NO_MORE_FILES && !inaccessible_match).then_some(false)
    })();
    // SAFETY: This closes the successful snapshot exactly once, after its last use.
    unsafe { CloseHandle(snapshot) };
    result
}

fn process_session(process_id: u32) -> Option<u32> {
    let mut session = 0;
    // SAFETY: The API writes a session ID to this valid output pointer.
    (unsafe { ProcessIdToSessionId(process_id, &mut session) } != 0).then_some(session)
}
