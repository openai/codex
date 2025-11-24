#[cfg(windows)]
#[allow(non_camel_case_types, non_snake_case)]
mod windows_console {
    use core::ffi::c_void;

    type c_ulong = u32;
    type c_int = i32;
    type wchar_t = u16;

    type DWORD = c_ulong;
    type LPDWORD = *mut DWORD;
    type HANDLE = *mut c_void;
    type BOOL = c_int;
    type LPCWSTR = *const WCHAR;
    type WCHAR = wchar_t;
    type LPSECURITY_ATTRIBUTES = *mut SECURITY_ATTRIBUTES;
    type LPVOID = *mut c_void;

    #[repr(C)]
    pub struct SECURITY_ATTRIBUTES {
        pub nLength: DWORD,
        pub lpSecurityDescriptor: LPVOID,
        pub bInheritHandle: BOOL,
    }

    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: DWORD = 0x0004;
    const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
    const FALSE: BOOL = 0;
    const TRUE: BOOL = 1;

    const GENERIC_READ: DWORD = 0x80000000;
    const GENERIC_WRITE: DWORD = 0x40000000;

    const FILE_SHARE_READ: DWORD = 0x00000001;
    const FILE_SHARE_WRITE: DWORD = 0x00000002;
    const OPEN_EXISTING: DWORD = 3;

    // This is the win32 console API, taken from the 'winapi' crate.
    extern "system" {
        fn CreateFileW(
            lpFileName: LPCWSTR,
            dwDesiredAccess: DWORD,
            dwShareMode: DWORD,
            lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
            dwCreationDisposition: DWORD,
            dwFlagsAndAttributes: DWORD,
            hTemplateFile: HANDLE
        ) -> HANDLE;

        fn GetLastError() -> DWORD;
        fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: LPDWORD) -> BOOL;
        fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: DWORD) -> BOOL;
    }

    fn get_output_handle() -> Result<HANDLE, DWORD> {
        // This is "CONOUT$\0" UTF-16 encoded.
        const CONOUT: &[u16] = &[0x43, 0x4F, 0x4E, 0x4F, 0x55, 0x54, 0x24, 0x00];

        let raw_handle = unsafe {
            CreateFileW(
                CONOUT.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                core::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                core::ptr::null_mut(),
            )
        };

        if raw_handle == INVALID_HANDLE_VALUE {
            return Err(6);
        }

        Ok(raw_handle)
    }

    unsafe fn enable_vt(handle: HANDLE) -> Result<(), DWORD> {
        let mut dw_mode: DWORD = 0;
        if GetConsoleMode(handle, &mut dw_mode) == FALSE {
            return Err(GetLastError());
        }

        dw_mode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING;
        match SetConsoleMode(handle, dw_mode) {
            result if result == TRUE => Ok(()),
            _ => Err(GetLastError())
        }
    }

    unsafe fn enable_ansi_colors_raw() -> Result<bool, DWORD> {
        enable_vt(get_output_handle()?)?;
        Ok(true)
    }

    #[inline(always)]
    pub fn enable() -> bool {
        unsafe { enable_ansi_colors_raw().unwrap_or(false) }
    }

    // Try to enable colors on Windows, and try to do it at most once.
    pub fn cache_enable() -> bool {
        use crate::condition::CachedBool;

        static ENABLED: CachedBool = CachedBool::new();
        ENABLED.get_or_init(enable)
    }
}

#[cfg(not(windows))]
mod windows_console {
    #[inline(always)]
    #[allow(dead_code)]
    pub fn enable() -> bool { true }

    #[inline(always)]
    pub fn cache_enable() -> bool { true }
}

// pub use self::windows_console::enable;
pub use self::windows_console::cache_enable;
