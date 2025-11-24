// spell-checker:ignore dword, minwindef, ntdef, ntdll, ntstatus, osversioninfoex, osversioninfoexa
// spell-checker:ignore osversioninfoexw, serverr, sysinfoapi, winnt, winuser, pbool, libloaderapi
// spell-checker:ignore lpcstr, processthreadsapi, farproc, lstatus, wchar, lpbyte, hkey, winerror
// spell-checker:ignore osstr, winreg

#![allow(unsafe_code)]

use std::{
    ffi::{OsStr, OsString},
    mem::{self, MaybeUninit},
    os::windows::ffi::{OsStrExt, OsStringExt},
    ptr,
};

use windows_sys::Win32::{
    Foundation::{ERROR_SUCCESS, FARPROC, NTSTATUS, STATUS_SUCCESS},
    System::{
        LibraryLoader::{GetModuleHandleA, GetProcAddress},
        Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ},
        SystemInformation::{
            GetNativeSystemInfo, GetSystemInfo, PROCESSOR_ARCHITECTURE_AMD64,
            PROCESSOR_ARCHITECTURE_ARM, PROCESSOR_ARCHITECTURE_IA64, PROCESSOR_ARCHITECTURE_INTEL,
            SYSTEM_INFO,
        },
        SystemServices::{VER_NT_WORKSTATION, VER_SUITE_WH_SERVER},
    },
    UI::WindowsAndMessaging::{GetSystemMetrics, SM_SERVERR2},
};

use crate::{Bitness, Info, Type, Version};

#[cfg(target_arch = "x86")]
type OSVERSIONINFOEX = windows_sys::Win32::System::SystemInformation::OSVERSIONINFOEXA;

#[cfg(not(target_arch = "x86"))]
type OSVERSIONINFOEX = windows_sys::Win32::System::SystemInformation::OSVERSIONINFOEXW;

pub fn get() -> Info {
    let (version, edition) = version();
    let native_system_info = native_system_info();

    Info {
        os_type: Type::Windows,
        version,
        edition,
        bitness: bitness(),
        architecture: architecture(native_system_info),
        ..Default::default()
    }
}

fn version() -> (Version, Option<String>) {
    match version_info() {
        None => (Version::Unknown, None),
        Some(v) => (
            Version::Semantic(
                v.dwMajorVersion as u64,
                v.dwMinorVersion as u64,
                v.dwBuildNumber as u64,
            ),
            product_name(&v).or_else(|| edition(&v)),
        ),
    }
}

// According to https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/ns-sysinfoapi-system_info
// there is a variant for AMD64 CPUs, but it's not defined in generated bindings.
const PROCESSOR_ARCHITECTURE_ARM64: u16 = 12;

fn native_system_info() -> SYSTEM_INFO {
    let mut system_info: MaybeUninit<SYSTEM_INFO> = MaybeUninit::zeroed();
    unsafe {
        GetNativeSystemInfo(system_info.as_mut_ptr());
    };

    unsafe { system_info.assume_init() }
}

fn architecture(system_info: SYSTEM_INFO) -> Option<String> {
    let cpu_architecture = unsafe { system_info.Anonymous.Anonymous.wProcessorArchitecture };

    match cpu_architecture {
        PROCESSOR_ARCHITECTURE_AMD64 => Some("x86_64"),
        PROCESSOR_ARCHITECTURE_IA64 => Some("ia64"),
        PROCESSOR_ARCHITECTURE_ARM => Some("arm"),
        PROCESSOR_ARCHITECTURE_ARM64 => Some("aarch64"),
        PROCESSOR_ARCHITECTURE_INTEL => Some("i386"),
        _ => None,
    }
    .map(str::to_string)
}

#[cfg(target_pointer_width = "64")]
fn bitness() -> Bitness {
    // x64 program can only run on x64 Windows.
    Bitness::X64
}

#[cfg(target_pointer_width = "32")]
fn bitness() -> Bitness {
    use windows_sys::Win32::Foundation::{BOOL, FALSE, HANDLE};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // IsWow64Process is not available on all supported versions of Windows. Use GetModuleHandle to
    // get a handle to the DLL that contains the function and GetProcAddress to get a pointer to the
    // function if available.
    let is_wow_64 = match get_proc_address(b"kernel32\0", b"IsWow64Process\0") {
        None => return Bitness::Unknown,
        Some(val) => val,
    };

    type IsWow64 = unsafe extern "system" fn(HANDLE, *mut BOOL) -> BOOL;
    let is_wow_64: IsWow64 = unsafe { mem::transmute(is_wow_64) };

    let mut result = FALSE;
    if unsafe { is_wow_64(GetCurrentProcess(), &mut result) } == 0 {
        log::error!("IsWow64Process failed");
        return Bitness::Unknown;
    }

    if result == FALSE {
        Bitness::X32
    } else {
        Bitness::X64
    }
}

// Calls the Win32 API function RtlGetVersion to get the OS version information:
// https://msdn.microsoft.com/en-us/library/mt723418(v=vs.85).aspx
fn version_info() -> Option<OSVERSIONINFOEX> {
    let rtl_get_version = match get_proc_address(b"ntdll\0", b"RtlGetVersion\0") {
        None => return None,
        Some(val) => val,
    };

    type RtlGetVersion = unsafe extern "system" fn(&mut OSVERSIONINFOEX) -> NTSTATUS;
    let rtl_get_version: RtlGetVersion = unsafe { mem::transmute(rtl_get_version) };

    let mut info: OSVERSIONINFOEX = unsafe { mem::zeroed() };
    info.dwOSVersionInfoSize = mem::size_of::<OSVERSIONINFOEX>() as u32;

    if unsafe { rtl_get_version(&mut info) } == STATUS_SUCCESS {
        Some(info)
    } else {
        None
    }
}

fn product_name(info: &OSVERSIONINFOEX) -> Option<String> {
    let sub_key = to_wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
    let mut key = Default::default();
    if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, sub_key.as_ptr(), 0, KEY_READ, &mut key) }
        != ERROR_SUCCESS
        || key == 0
    {
        log::error!("RegOpenKeyExW(HKEY_LOCAL_MACHINE, ...) failed");
        return None;
    }

    let is_win_11 = info.dwMajorVersion == 10 && info.dwBuildNumber >= 22000;

    // Get size of the data.
    let name = to_wide(if is_win_11 {
        "EditionID"
    } else {
        "ProductName"
    });
    let mut data_type = 0;
    let mut data_size = 0;
    if unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null_mut(),
            &mut data_type,
            ptr::null_mut(),
            &mut data_size,
        )
    } != ERROR_SUCCESS
        || data_type != REG_SZ
        || data_size == 0
        || data_size % 2 != 0
    {
        log::error!("RegQueryValueExW failed");
        return None;
    }

    // Get the data.
    let mut data = vec![0u16; data_size as usize / 2];
    if unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            data.as_mut_ptr().cast(),
            &mut data_size,
        )
    } != ERROR_SUCCESS
        || data_size as usize != data.len() * 2
    {
        return None;
    }

    // If the data has the REG_SZ, REG_MULTI_SZ or REG_EXPAND_SZ type, the string may not have been
    // stored with the proper terminating null characters.
    match data.last() {
        Some(0) => {
            data.pop();
        }
        _ => {}
    }

    let value = OsString::from_wide(data.as_slice())
        .to_string_lossy()
        .into_owned();

    if is_win_11 {
        Some(format!("Windows 11 {}", value))
    } else {
        Some(value)
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

// Examines data in the OSVERSIONINFOEX structure to determine the Windows edition:
// https://msdn.microsoft.com/en-us/library/windows/desktop/ms724833(v=vs.85).aspx
fn edition(version_info: &OSVERSIONINFOEX) -> Option<String> {
    match (
        version_info.dwMajorVersion,
        version_info.dwMinorVersion,
        version_info.wProductType as u32,
    ) {
        // Windows 10.
        (10, 0, VER_NT_WORKSTATION) => {
            if version_info.dwBuildNumber >= 22000 {
                Some("Windows 11")
            } else {
                Some("Windows 10")
            }
        }
        (10, 0, _) => Some("Windows Server 2016"),
        // Windows Vista, 7, 8 and 8.1.
        (6, 3, VER_NT_WORKSTATION) => Some("Windows 8.1"),
        (6, 3, _) => Some("Windows Server 2012 R2"),
        (6, 2, VER_NT_WORKSTATION) => Some("Windows 8"),
        (6, 2, _) => Some("Windows Server 2012"),
        (6, 1, VER_NT_WORKSTATION) => Some("Windows 7"),
        (6, 1, _) => Some("Windows Server 2008 R2"),
        (6, 0, VER_NT_WORKSTATION) => Some("Windows Vista"),
        (6, 0, _) => Some("Windows Server 2008"),
        // Windows 2000, Home Server, 2003 Server, 2003 R2 Server, XP and XP Professional x64.
        (5, 1, _) => Some("Windows XP"),
        (5, 0, _) => Some("Windows 2000"),
        (5, 2, _) if unsafe { GetSystemMetrics(SM_SERVERR2) } == 0 => {
            let mut info: SYSTEM_INFO = unsafe { mem::zeroed() };
            unsafe { GetSystemInfo(&mut info) };

            if Into::<u32>::into(version_info.wSuiteMask) & VER_SUITE_WH_SERVER
                == VER_SUITE_WH_SERVER
            {
                Some("Windows Home Server")
            } else if version_info.wProductType == VER_NT_WORKSTATION as u8
                && unsafe { info.Anonymous.Anonymous.wProcessorArchitecture }
                    == PROCESSOR_ARCHITECTURE_AMD64
            {
                Some("Windows XP Professional x64 Edition")
            } else {
                Some("Windows Server 2003")
            }
        }
        _ => None,
    }
    .map(str::to_string)
}

fn get_proc_address(module: &[u8], proc: &[u8]) -> Option<FARPROC> {
    assert!(
        *module.last().expect("Empty module name") == 0,
        "Module name should be zero-terminated"
    );
    assert!(
        *proc.last().expect("Empty procedure name") == 0,
        "Procedure name should be zero-terminated"
    );

    let handle = unsafe { GetModuleHandleA(module.as_ptr()) };
    if handle == 0 {
        log::error!(
            "GetModuleHandleA({}) failed",
            String::from_utf8_lossy(module)
        );
        return None;
    }

    unsafe { Some(GetProcAddress(handle, proc.as_ptr())) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn version() {
        let info = get();
        assert_eq!(Type::Windows, info.os_type());
    }

    #[test]
    fn get_version_info() {
        let version = version_info();
        assert!(version.is_some());
    }

    #[test]
    fn get_edition() {
        let test_data = [
            (10, 0, 0, "Windows Server 2016"),
            (6, 3, VER_NT_WORKSTATION, "Windows 8.1"),
            (6, 3, 0, "Windows Server 2012 R2"),
            (6, 2, VER_NT_WORKSTATION, "Windows 8"),
            (6, 2, 0, "Windows Server 2012"),
            (6, 1, VER_NT_WORKSTATION, "Windows 7"),
            (6, 1, 0, "Windows Server 2008 R2"),
            (6, 0, VER_NT_WORKSTATION, "Windows Vista"),
            (6, 0, 0, "Windows Server 2008"),
            (5, 1, 0, "Windows XP"),
            (5, 1, 1, "Windows XP"),
            (5, 1, 100, "Windows XP"),
            (5, 0, 0, "Windows 2000"),
            (5, 0, 1, "Windows 2000"),
            (5, 0, 100, "Windows 2000"),
        ];

        let mut info = version_info().unwrap();

        for &(major, minor, product_type, expected_edition) in &test_data {
            info.dwMajorVersion = major;
            info.dwMinorVersion = minor;
            info.wProductType = product_type as u8;

            let edition = edition(&info).unwrap();
            assert_eq!(edition, expected_edition);
        }
    }

    #[test]
    fn get_bitness() {
        let b = bitness();
        assert_ne!(b, Bitness::Unknown);
    }

    #[test]
    #[should_panic(expected = "Empty module name")]
    fn empty_module_name() {
        get_proc_address(b"", b"RtlGetVersion\0");
    }

    #[test]
    #[should_panic(expected = "Module name should be zero-terminated")]
    fn non_zero_terminated_module_name() {
        get_proc_address(b"ntdll", b"RtlGetVersion\0");
    }

    #[test]
    #[should_panic(expected = "Empty procedure name")]
    fn empty_proc_name() {
        get_proc_address(b"ntdll\0", b"");
    }

    #[test]
    #[should_panic(expected = "Procedure name should be zero-terminated")]
    fn non_zero_terminated_proc_name() {
        get_proc_address(b"ntdll\0", b"RtlGetVersion");
    }

    #[test]
    fn proc_address() {
        let address = get_proc_address(b"ntdll\0", b"RtlGetVersion\0");
        assert!(address.is_some());
    }

    #[test]
    fn get_architecture() {
        let cpu_types: [(u16, Option<String>); 6] = [
            (PROCESSOR_ARCHITECTURE_AMD64, Some("x86_64".to_owned())),
            (PROCESSOR_ARCHITECTURE_ARM, Some("arm".to_owned())),
            (PROCESSOR_ARCHITECTURE_ARM64, Some("aarch64".to_owned())),
            (PROCESSOR_ARCHITECTURE_IA64, Some("ia64".to_owned())),
            (PROCESSOR_ARCHITECTURE_INTEL, Some("i386".to_owned())),
            (0xffff, None),
        ];

        let mut native_info = native_system_info();

        for cpu_type in cpu_types {
            native_info.Anonymous.Anonymous.wProcessorArchitecture = cpu_type.0;
            assert_eq!(architecture(native_info), cpu_type.1);
        }
    }

    #[test]
    fn get_product_name() {
        let version = version_info().expect("version_info() failed");
        let edition = product_name(&version).expect("edition() failed");
        assert!(!edition.is_empty());
    }

    #[test]
    fn to_wide_str() {
        let data = [
            ("", [0x0000].as_ref()),
            ("U", &[0x0055, 0x0000]),
            ("你好", &[0x4F60, 0x597D, 0x0000]),
        ];

        for (s, expected) in &data {
            let wide = to_wide(s);
            assert_eq!(&wide, expected);
        }
    }
}
