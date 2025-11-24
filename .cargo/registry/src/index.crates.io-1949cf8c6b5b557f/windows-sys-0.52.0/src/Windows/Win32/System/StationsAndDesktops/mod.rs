#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BroadcastSystemMessageA(flags : u32, lpinfo : *mut u32, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BroadcastSystemMessageExA(flags : BROADCAST_SYSTEM_MESSAGE_FLAGS, lpinfo : *mut BROADCAST_SYSTEM_MESSAGE_INFO, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM, pbsminfo : *mut BSMINFO) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BroadcastSystemMessageExW(flags : BROADCAST_SYSTEM_MESSAGE_FLAGS, lpinfo : *mut BROADCAST_SYSTEM_MESSAGE_INFO, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM, pbsminfo : *mut BSMINFO) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BroadcastSystemMessageW(flags : BROADCAST_SYSTEM_MESSAGE_FLAGS, lpinfo : *mut BROADCAST_SYSTEM_MESSAGE_INFO, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseDesktop(hdesktop : HDESK) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseWindowStation(hwinsta : HWINSTA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_Security\"`"] fn CreateDesktopA(lpszdesktop : ::windows_sys::core::PCSTR, lpszdevice : ::windows_sys::core::PCSTR, pdevmode : *const super::super::Graphics::Gdi:: DEVMODEA, dwflags : DESKTOP_CONTROL_FLAGS, dwdesiredaccess : u32, lpsa : *const super::super::Security:: SECURITY_ATTRIBUTES) -> HDESK);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_Security\"`"] fn CreateDesktopExA(lpszdesktop : ::windows_sys::core::PCSTR, lpszdevice : ::windows_sys::core::PCSTR, pdevmode : *const super::super::Graphics::Gdi:: DEVMODEA, dwflags : DESKTOP_CONTROL_FLAGS, dwdesiredaccess : u32, lpsa : *const super::super::Security:: SECURITY_ATTRIBUTES, ulheapsize : u32, pvoid : *const ::core::ffi::c_void) -> HDESK);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_Security\"`"] fn CreateDesktopExW(lpszdesktop : ::windows_sys::core::PCWSTR, lpszdevice : ::windows_sys::core::PCWSTR, pdevmode : *const super::super::Graphics::Gdi:: DEVMODEW, dwflags : DESKTOP_CONTROL_FLAGS, dwdesiredaccess : u32, lpsa : *const super::super::Security:: SECURITY_ATTRIBUTES, ulheapsize : u32, pvoid : *const ::core::ffi::c_void) -> HDESK);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_Security\"`"] fn CreateDesktopW(lpszdesktop : ::windows_sys::core::PCWSTR, lpszdevice : ::windows_sys::core::PCWSTR, pdevmode : *const super::super::Graphics::Gdi:: DEVMODEW, dwflags : DESKTOP_CONTROL_FLAGS, dwdesiredaccess : u32, lpsa : *const super::super::Security:: SECURITY_ATTRIBUTES) -> HDESK);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn CreateWindowStationA(lpwinsta : ::windows_sys::core::PCSTR, dwflags : u32, dwdesiredaccess : u32, lpsa : *const super::super::Security:: SECURITY_ATTRIBUTES) -> HWINSTA);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn CreateWindowStationW(lpwinsta : ::windows_sys::core::PCWSTR, dwflags : u32, dwdesiredaccess : u32, lpsa : *const super::super::Security:: SECURITY_ATTRIBUTES) -> HWINSTA);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn EnumDesktopWindows(hdesktop : HDESK, lpfn : super::super::UI::WindowsAndMessaging:: WNDENUMPROC, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumDesktopsA(hwinsta : HWINSTA, lpenumfunc : DESKTOPENUMPROCA, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumDesktopsW(hwinsta : HWINSTA, lpenumfunc : DESKTOPENUMPROCW, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumWindowStationsA(lpenumfunc : WINSTAENUMPROCA, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumWindowStationsW(lpenumfunc : WINSTAENUMPROCW, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn GetProcessWindowStation() -> HWINSTA);
::windows_targets::link!("user32.dll" "system" fn GetThreadDesktop(dwthreadid : u32) -> HDESK);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetUserObjectInformationA(hobj : super::super::Foundation:: HANDLE, nindex : USER_OBJECT_INFORMATION_INDEX, pvinfo : *mut ::core::ffi::c_void, nlength : u32, lpnlengthneeded : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetUserObjectInformationW(hobj : super::super::Foundation:: HANDLE, nindex : USER_OBJECT_INFORMATION_INDEX, pvinfo : *mut ::core::ffi::c_void, nlength : u32, lpnlengthneeded : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenDesktopA(lpszdesktop : ::windows_sys::core::PCSTR, dwflags : DESKTOP_CONTROL_FLAGS, finherit : super::super::Foundation:: BOOL, dwdesiredaccess : u32) -> HDESK);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenDesktopW(lpszdesktop : ::windows_sys::core::PCWSTR, dwflags : DESKTOP_CONTROL_FLAGS, finherit : super::super::Foundation:: BOOL, dwdesiredaccess : u32) -> HDESK);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenInputDesktop(dwflags : DESKTOP_CONTROL_FLAGS, finherit : super::super::Foundation:: BOOL, dwdesiredaccess : DESKTOP_ACCESS_FLAGS) -> HDESK);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenWindowStationA(lpszwinsta : ::windows_sys::core::PCSTR, finherit : super::super::Foundation:: BOOL, dwdesiredaccess : u32) -> HWINSTA);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenWindowStationW(lpszwinsta : ::windows_sys::core::PCWSTR, finherit : super::super::Foundation:: BOOL, dwdesiredaccess : u32) -> HWINSTA);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetProcessWindowStation(hwinsta : HWINSTA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetThreadDesktop(hdesktop : HDESK) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetUserObjectInformationA(hobj : super::super::Foundation:: HANDLE, nindex : i32, pvinfo : *const ::core::ffi::c_void, nlength : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetUserObjectInformationW(hobj : super::super::Foundation:: HANDLE, nindex : i32, pvinfo : *const ::core::ffi::c_void, nlength : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SwitchDesktop(hdesktop : HDESK) -> super::super::Foundation:: BOOL);
pub const BSF_ALLOWSFW: BROADCAST_SYSTEM_MESSAGE_FLAGS = 128u32;
pub const BSF_FLUSHDISK: BROADCAST_SYSTEM_MESSAGE_FLAGS = 4u32;
pub const BSF_FORCEIFHUNG: BROADCAST_SYSTEM_MESSAGE_FLAGS = 32u32;
pub const BSF_IGNORECURRENTTASK: BROADCAST_SYSTEM_MESSAGE_FLAGS = 2u32;
pub const BSF_LUID: BROADCAST_SYSTEM_MESSAGE_FLAGS = 1024u32;
pub const BSF_NOHANG: BROADCAST_SYSTEM_MESSAGE_FLAGS = 8u32;
pub const BSF_NOTIMEOUTIFNOTHUNG: BROADCAST_SYSTEM_MESSAGE_FLAGS = 64u32;
pub const BSF_POSTMESSAGE: BROADCAST_SYSTEM_MESSAGE_FLAGS = 16u32;
pub const BSF_QUERY: BROADCAST_SYSTEM_MESSAGE_FLAGS = 1u32;
pub const BSF_RETURNHDESK: BROADCAST_SYSTEM_MESSAGE_FLAGS = 512u32;
pub const BSF_SENDNOTIFYMESSAGE: BROADCAST_SYSTEM_MESSAGE_FLAGS = 256u32;
pub const BSM_ALLCOMPONENTS: BROADCAST_SYSTEM_MESSAGE_INFO = 0u32;
pub const BSM_ALLDESKTOPS: BROADCAST_SYSTEM_MESSAGE_INFO = 16u32;
pub const BSM_APPLICATIONS: BROADCAST_SYSTEM_MESSAGE_INFO = 8u32;
pub const DESKTOP_CREATEMENU: DESKTOP_ACCESS_FLAGS = 4u32;
pub const DESKTOP_CREATEWINDOW: DESKTOP_ACCESS_FLAGS = 2u32;
pub const DESKTOP_DELETE: DESKTOP_ACCESS_FLAGS = 65536u32;
pub const DESKTOP_ENUMERATE: DESKTOP_ACCESS_FLAGS = 64u32;
pub const DESKTOP_HOOKCONTROL: DESKTOP_ACCESS_FLAGS = 8u32;
pub const DESKTOP_JOURNALPLAYBACK: DESKTOP_ACCESS_FLAGS = 32u32;
pub const DESKTOP_JOURNALRECORD: DESKTOP_ACCESS_FLAGS = 16u32;
pub const DESKTOP_READOBJECTS: DESKTOP_ACCESS_FLAGS = 1u32;
pub const DESKTOP_READ_CONTROL: DESKTOP_ACCESS_FLAGS = 131072u32;
pub const DESKTOP_SWITCHDESKTOP: DESKTOP_ACCESS_FLAGS = 256u32;
pub const DESKTOP_SYNCHRONIZE: DESKTOP_ACCESS_FLAGS = 1048576u32;
pub const DESKTOP_WRITEOBJECTS: DESKTOP_ACCESS_FLAGS = 128u32;
pub const DESKTOP_WRITE_DAC: DESKTOP_ACCESS_FLAGS = 262144u32;
pub const DESKTOP_WRITE_OWNER: DESKTOP_ACCESS_FLAGS = 524288u32;
pub const DF_ALLOWOTHERACCOUNTHOOK: DESKTOP_CONTROL_FLAGS = 1u32;
pub const UOI_FLAGS: USER_OBJECT_INFORMATION_INDEX = 1i32;
pub const UOI_HEAPSIZE: USER_OBJECT_INFORMATION_INDEX = 5i32;
pub const UOI_IO: USER_OBJECT_INFORMATION_INDEX = 6i32;
pub const UOI_NAME: USER_OBJECT_INFORMATION_INDEX = 2i32;
pub const UOI_TYPE: USER_OBJECT_INFORMATION_INDEX = 3i32;
pub const UOI_USER_SID: USER_OBJECT_INFORMATION_INDEX = 4i32;
pub type BROADCAST_SYSTEM_MESSAGE_FLAGS = u32;
pub type BROADCAST_SYSTEM_MESSAGE_INFO = u32;
pub type DESKTOP_ACCESS_FLAGS = u32;
pub type DESKTOP_CONTROL_FLAGS = u32;
pub type USER_OBJECT_INFORMATION_INDEX = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BSMINFO {
    pub cbSize: u32,
    pub hdesk: HDESK,
    pub hwnd: super::super::Foundation::HWND,
    pub luid: super::super::Foundation::LUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BSMINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BSMINFO {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HDESK = isize;
pub type HWINSTA = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct USEROBJECTFLAGS {
    pub fInherit: super::super::Foundation::BOOL,
    pub fReserved: super::super::Foundation::BOOL,
    pub dwFlags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for USEROBJECTFLAGS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for USEROBJECTFLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DESKTOPENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: super::super::Foundation::LPARAM) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DESKTOPENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: super::super::Foundation::LPARAM) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type WINSTAENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: super::super::Foundation::LPARAM) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type WINSTAENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: super::super::Foundation::LPARAM) -> super::super::Foundation::BOOL>;
