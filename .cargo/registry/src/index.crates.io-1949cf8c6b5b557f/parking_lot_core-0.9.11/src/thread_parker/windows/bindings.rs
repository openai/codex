//! Manual bindings to the win32 API to avoid dependencies on windows-sys or winapi
//! as these bindings will **never** change and `parking_lot_core` is a foundational
//! dependency for the Rust ecosystem, so the dependencies used by it have an
//! outsize affect

pub const INFINITE: u32 = 4294967295;
pub const ERROR_TIMEOUT: u32 = 1460;
pub const GENERIC_READ: u32 = 2147483648;
pub const GENERIC_WRITE: u32 = 1073741824;
pub const STATUS_SUCCESS: i32 = 0;
pub const STATUS_TIMEOUT: i32 = 258;

pub type HANDLE = isize;
pub type HINSTANCE = isize;
pub type BOOL = i32;
pub type BOOLEAN = u8;
pub type NTSTATUS = i32;
pub type FARPROC = Option<unsafe extern "system" fn() -> isize>;
pub type WaitOnAddress = unsafe extern "system" fn(
    Address: *const std::ffi::c_void,
    CompareAddress: *const std::ffi::c_void,
    AddressSize: usize,
    dwMilliseconds: u32,
) -> BOOL;
pub type WakeByAddressSingle = unsafe extern "system" fn(Address: *const std::ffi::c_void);

windows_targets::link!("kernel32.dll" "system" fn GetLastError() -> u32);
windows_targets::link!("kernel32.dll" "system" fn CloseHandle(hObject: HANDLE) -> BOOL);
windows_targets::link!("kernel32.dll" "system" fn GetModuleHandleA(lpModuleName: *const u8) -> HINSTANCE);
windows_targets::link!("kernel32.dll" "system" fn GetProcAddress(hModule: HINSTANCE, lpProcName: *const u8) -> FARPROC);
windows_targets::link!("kernel32.dll" "system" fn Sleep(dwMilliseconds: u32) -> ());
