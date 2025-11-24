#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseGestureInfoHandle(hgestureinfo : HGESTUREINFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseTouchInputHandle(htouchinput : HTOUCHINPUT) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetGestureConfig(hwnd : super::super::super::Foundation:: HWND, dwreserved : u32, dwflags : u32, pcids : *const u32, pgestureconfig : *mut GESTURECONFIG, cbsize : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetGestureExtraArgs(hgestureinfo : HGESTUREINFO, cbextraargs : u32, pextraargs : *mut u8) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetGestureInfo(hgestureinfo : HGESTUREINFO, pgestureinfo : *mut GESTUREINFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetTouchInputInfo(htouchinput : HTOUCHINPUT, cinputs : u32, pinputs : *mut TOUCHINPUT, cbsize : i32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsTouchWindow(hwnd : super::super::super::Foundation:: HWND, pulflags : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterTouchWindow(hwnd : super::super::super::Foundation:: HWND, ulflags : REGISTER_TOUCH_WINDOW_FLAGS) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetGestureConfig(hwnd : super::super::super::Foundation:: HWND, dwreserved : u32, cids : u32, pgestureconfig : *const GESTURECONFIG, cbsize : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnregisterTouchWindow(hwnd : super::super::super::Foundation:: HWND) -> super::super::super::Foundation:: BOOL);
pub type IInertiaProcessor = *mut ::core::ffi::c_void;
pub type IManipulationProcessor = *mut ::core::ffi::c_void;
pub type _IManipulationEvents = *mut ::core::ffi::c_void;
pub const GID_BEGIN: GESTURECONFIG_ID = 1u32;
pub const GID_END: GESTURECONFIG_ID = 2u32;
pub const GID_PAN: GESTURECONFIG_ID = 4u32;
pub const GID_PRESSANDTAP: GESTURECONFIG_ID = 7u32;
pub const GID_ROLLOVER: GESTURECONFIG_ID = 7u32;
pub const GID_ROTATE: GESTURECONFIG_ID = 5u32;
pub const GID_TWOFINGERTAP: GESTURECONFIG_ID = 6u32;
pub const GID_ZOOM: GESTURECONFIG_ID = 3u32;
pub const InertiaProcessor: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xabb27087_4ce0_4e58_a0cb_e24df96814be);
pub const MANIPULATION_ALL: MANIPULATION_PROCESSOR_MANIPULATIONS = 15i32;
pub const MANIPULATION_NONE: MANIPULATION_PROCESSOR_MANIPULATIONS = 0i32;
pub const MANIPULATION_ROTATE: MANIPULATION_PROCESSOR_MANIPULATIONS = 8i32;
pub const MANIPULATION_SCALE: MANIPULATION_PROCESSOR_MANIPULATIONS = 4i32;
pub const MANIPULATION_TRANSLATE_X: MANIPULATION_PROCESSOR_MANIPULATIONS = 1i32;
pub const MANIPULATION_TRANSLATE_Y: MANIPULATION_PROCESSOR_MANIPULATIONS = 2i32;
pub const ManipulationProcessor: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x597d4fb0_47fd_4aff_89b9_c6cfae8cf08e);
pub const TOUCHEVENTF_DOWN: TOUCHEVENTF_FLAGS = 2u32;
pub const TOUCHEVENTF_INRANGE: TOUCHEVENTF_FLAGS = 8u32;
pub const TOUCHEVENTF_MOVE: TOUCHEVENTF_FLAGS = 1u32;
pub const TOUCHEVENTF_NOCOALESCE: TOUCHEVENTF_FLAGS = 32u32;
pub const TOUCHEVENTF_PALM: TOUCHEVENTF_FLAGS = 128u32;
pub const TOUCHEVENTF_PEN: TOUCHEVENTF_FLAGS = 64u32;
pub const TOUCHEVENTF_PRIMARY: TOUCHEVENTF_FLAGS = 16u32;
pub const TOUCHEVENTF_UP: TOUCHEVENTF_FLAGS = 4u32;
pub const TOUCHINPUTMASKF_CONTACTAREA: TOUCHINPUTMASKF_MASK = 4u32;
pub const TOUCHINPUTMASKF_EXTRAINFO: TOUCHINPUTMASKF_MASK = 2u32;
pub const TOUCHINPUTMASKF_TIMEFROMSYSTEM: TOUCHINPUTMASKF_MASK = 1u32;
pub const TWF_FINETOUCH: REGISTER_TOUCH_WINDOW_FLAGS = 1u32;
pub const TWF_WANTPALM: REGISTER_TOUCH_WINDOW_FLAGS = 2u32;
pub type GESTURECONFIG_ID = u32;
pub type MANIPULATION_PROCESSOR_MANIPULATIONS = i32;
pub type REGISTER_TOUCH_WINDOW_FLAGS = u32;
pub type TOUCHEVENTF_FLAGS = u32;
pub type TOUCHINPUTMASKF_MASK = u32;
#[repr(C)]
pub struct GESTURECONFIG {
    pub dwID: GESTURECONFIG_ID,
    pub dwWant: u32,
    pub dwBlock: u32,
}
impl ::core::marker::Copy for GESTURECONFIG {}
impl ::core::clone::Clone for GESTURECONFIG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct GESTUREINFO {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub dwID: u32,
    pub hwndTarget: super::super::super::Foundation::HWND,
    pub ptsLocation: super::super::super::Foundation::POINTS,
    pub dwInstanceID: u32,
    pub dwSequenceID: u32,
    pub ullArguments: u64,
    pub cbExtraArgs: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for GESTUREINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for GESTUREINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct GESTURENOTIFYSTRUCT {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub hwndTarget: super::super::super::Foundation::HWND,
    pub ptsLocation: super::super::super::Foundation::POINTS,
    pub dwInstanceID: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for GESTURENOTIFYSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for GESTURENOTIFYSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HGESTUREINFO = isize;
pub type HTOUCHINPUT = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TOUCHINPUT {
    pub x: i32,
    pub y: i32,
    pub hSource: super::super::super::Foundation::HANDLE,
    pub dwID: u32,
    pub dwFlags: TOUCHEVENTF_FLAGS,
    pub dwMask: TOUCHINPUTMASKF_MASK,
    pub dwTime: u32,
    pub dwExtraInfo: usize,
    pub cxContact: u32,
    pub cyContact: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TOUCHINPUT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TOUCHINPUT {
    fn clone(&self) -> Self {
        *self
    }
}
