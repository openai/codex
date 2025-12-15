use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::System::Diagnostics::Debug::FormatMessageW;
use windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_ALLOCATE_BUFFER;
use windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_FROM_SYSTEM;
use windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_IGNORE_INSERTS;
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;

pub fn to_wide<S: AsRef<OsStr>>(s: S) -> Vec<u16> {
    let mut v: Vec<u16> = s.as_ref().encode_wide().collect();
    v.push(0);
    v
}

/// Quote a single Windows command-line argument following the rules used by
/// CommandLineToArgvW/CRT so that spaces, quotes, and backslashes are preserved.
/// Reference behavior matches Rust std::process::Command on Windows.
#[cfg(target_os = "windows")]
pub fn quote_windows_arg(arg: &str) -> String {
    let needs_quotes = arg.is_empty()
        || arg
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '"'));
    if !needs_quotes {
        return arg.to_string();
    }

    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    let mut backslashes = 0;
    for ch in arg.chars() {
        match ch {
            '\\' => {
                backslashes += 1;
            }
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                if backslashes > 0 {
                    quoted.push_str(&"\\".repeat(backslashes));
                    backslashes = 0;
                }
                quoted.push(ch);
            }
        }
    }
    if backslashes > 0 {
        quoted.push_str(&"\\".repeat(backslashes * 2));
    }
    quoted.push('"');
    quoted
}

// Produce a readable description for a Win32 error code.
pub fn format_last_error(err: i32) -> String {
    unsafe {
        let mut buf_ptr: *mut u16 = std::ptr::null_mut();
        let flags = FORMAT_MESSAGE_ALLOCATE_BUFFER
            | FORMAT_MESSAGE_FROM_SYSTEM
            | FORMAT_MESSAGE_IGNORE_INSERTS;
        let len = FormatMessageW(
            flags,
            std::ptr::null(),
            err as u32,
            0,
            // FORMAT_MESSAGE_ALLOCATE_BUFFER expects a pointer to receive the allocated buffer.
            // Cast &mut *mut u16 to *mut u16 as required by windows-sys.
            (&mut buf_ptr as *mut *mut u16) as *mut u16,
            0,
            std::ptr::null_mut(),
        );
        if len == 0 || buf_ptr.is_null() {
            return format!("Win32 error {}", err);
        }
        let slice = std::slice::from_raw_parts(buf_ptr, len as usize);
        let mut s = String::from_utf16_lossy(slice);
        s = s.trim().to_string();
        let _ = LocalFree(buf_ptr as HLOCAL);
        s
    }
}

#[allow(dead_code)]
pub fn string_from_sid_bytes(sid: &[u8]) -> Result<String, String> {
    unsafe {
        let mut str_ptr: *mut u16 = std::ptr::null_mut();
        let ok = ConvertSidToStringSidW(sid.as_ptr() as *mut std::ffi::c_void, &mut str_ptr);
        if ok == 0 || str_ptr.is_null() {
            return Err(format!("ConvertSidToStringSidW failed: {}", std::io::Error::last_os_error()));
        }
        let mut len = 0;
        while *str_ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(str_ptr, len);
        let out = String::from_utf16_lossy(slice);
        let _ = LocalFree(str_ptr as HLOCAL);
        Ok(out)
    }
}
