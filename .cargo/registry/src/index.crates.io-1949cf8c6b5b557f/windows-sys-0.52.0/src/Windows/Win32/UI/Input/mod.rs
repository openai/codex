#[cfg(feature = "Win32_UI_Input_Ime")]
#[doc = "Required features: `\"Win32_UI_Input_Ime\"`"]
pub mod Ime;
#[cfg(feature = "Win32_UI_Input_KeyboardAndMouse")]
#[doc = "Required features: `\"Win32_UI_Input_KeyboardAndMouse\"`"]
pub mod KeyboardAndMouse;
#[cfg(feature = "Win32_UI_Input_Pointer")]
#[doc = "Required features: `\"Win32_UI_Input_Pointer\"`"]
pub mod Pointer;
#[cfg(feature = "Win32_UI_Input_Touch")]
#[doc = "Required features: `\"Win32_UI_Input_Touch\"`"]
pub mod Touch;
#[cfg(feature = "Win32_UI_Input_XboxController")]
#[doc = "Required features: `\"Win32_UI_Input_XboxController\"`"]
pub mod XboxController;
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DefRawInputProc(parawinput : *const *const RAWINPUT, ninput : i32, cbsizeheader : u32) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCIMSSM(inputmessagesource : *mut INPUT_MESSAGE_SOURCE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCurrentInputMessageSource(inputmessagesource : *mut INPUT_MESSAGE_SOURCE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetRawInputBuffer(pdata : *mut RAWINPUT, pcbsize : *mut u32, cbsizeheader : u32) -> u32);
::windows_targets::link!("user32.dll" "system" fn GetRawInputData(hrawinput : HRAWINPUT, uicommand : RAW_INPUT_DATA_COMMAND_FLAGS, pdata : *mut ::core::ffi::c_void, pcbsize : *mut u32, cbsizeheader : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetRawInputDeviceInfoA(hdevice : super::super::Foundation:: HANDLE, uicommand : RAW_INPUT_DEVICE_INFO_COMMAND, pdata : *mut ::core::ffi::c_void, pcbsize : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetRawInputDeviceInfoW(hdevice : super::super::Foundation:: HANDLE, uicommand : RAW_INPUT_DEVICE_INFO_COMMAND, pdata : *mut ::core::ffi::c_void, pcbsize : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetRawInputDeviceList(prawinputdevicelist : *mut RAWINPUTDEVICELIST, puinumdevices : *mut u32, cbsize : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetRegisteredRawInputDevices(prawinputdevices : *mut RAWINPUTDEVICE, puinumdevices : *mut u32, cbsize : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterRawInputDevices(prawinputdevices : *const RAWINPUTDEVICE, uinumdevices : u32, cbsize : u32) -> super::super::Foundation:: BOOL);
pub const IMDT_KEYBOARD: INPUT_MESSAGE_DEVICE_TYPE = 1i32;
pub const IMDT_MOUSE: INPUT_MESSAGE_DEVICE_TYPE = 2i32;
pub const IMDT_PEN: INPUT_MESSAGE_DEVICE_TYPE = 8i32;
pub const IMDT_TOUCH: INPUT_MESSAGE_DEVICE_TYPE = 4i32;
pub const IMDT_TOUCHPAD: INPUT_MESSAGE_DEVICE_TYPE = 16i32;
pub const IMDT_UNAVAILABLE: INPUT_MESSAGE_DEVICE_TYPE = 0i32;
pub const IMO_HARDWARE: INPUT_MESSAGE_ORIGIN_ID = 1i32;
pub const IMO_INJECTED: INPUT_MESSAGE_ORIGIN_ID = 2i32;
pub const IMO_SYSTEM: INPUT_MESSAGE_ORIGIN_ID = 4i32;
pub const IMO_UNAVAILABLE: INPUT_MESSAGE_ORIGIN_ID = 0i32;
pub const RIDEV_APPKEYS: RAWINPUTDEVICE_FLAGS = 1024u32;
pub const RIDEV_CAPTUREMOUSE: RAWINPUTDEVICE_FLAGS = 512u32;
pub const RIDEV_DEVNOTIFY: RAWINPUTDEVICE_FLAGS = 8192u32;
pub const RIDEV_EXCLUDE: RAWINPUTDEVICE_FLAGS = 16u32;
pub const RIDEV_EXINPUTSINK: RAWINPUTDEVICE_FLAGS = 4096u32;
pub const RIDEV_INPUTSINK: RAWINPUTDEVICE_FLAGS = 256u32;
pub const RIDEV_NOHOTKEYS: RAWINPUTDEVICE_FLAGS = 512u32;
pub const RIDEV_NOLEGACY: RAWINPUTDEVICE_FLAGS = 48u32;
pub const RIDEV_PAGEONLY: RAWINPUTDEVICE_FLAGS = 32u32;
pub const RIDEV_REMOVE: RAWINPUTDEVICE_FLAGS = 1u32;
pub const RIDI_DEVICEINFO: RAW_INPUT_DEVICE_INFO_COMMAND = 536870923u32;
pub const RIDI_DEVICENAME: RAW_INPUT_DEVICE_INFO_COMMAND = 536870919u32;
pub const RIDI_PREPARSEDDATA: RAW_INPUT_DEVICE_INFO_COMMAND = 536870917u32;
pub const RID_HEADER: RAW_INPUT_DATA_COMMAND_FLAGS = 268435461u32;
pub const RID_INPUT: RAW_INPUT_DATA_COMMAND_FLAGS = 268435459u32;
pub const RIM_TYPEHID: RID_DEVICE_INFO_TYPE = 2u32;
pub const RIM_TYPEKEYBOARD: RID_DEVICE_INFO_TYPE = 1u32;
pub const RIM_TYPEMOUSE: RID_DEVICE_INFO_TYPE = 0u32;
pub type INPUT_MESSAGE_DEVICE_TYPE = i32;
pub type INPUT_MESSAGE_ORIGIN_ID = i32;
pub type RAWINPUTDEVICE_FLAGS = u32;
pub type RAW_INPUT_DATA_COMMAND_FLAGS = u32;
pub type RAW_INPUT_DEVICE_INFO_COMMAND = u32;
pub type RID_DEVICE_INFO_TYPE = u32;
pub type HRAWINPUT = isize;
#[repr(C)]
pub struct INPUT_MESSAGE_SOURCE {
    pub deviceType: INPUT_MESSAGE_DEVICE_TYPE,
    pub originId: INPUT_MESSAGE_ORIGIN_ID,
}
impl ::core::marker::Copy for INPUT_MESSAGE_SOURCE {}
impl ::core::clone::Clone for INPUT_MESSAGE_SOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAWHID {
    pub dwSizeHid: u32,
    pub dwCount: u32,
    pub bRawData: [u8; 1],
}
impl ::core::marker::Copy for RAWHID {}
impl ::core::clone::Clone for RAWHID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAWINPUT {
    pub header: RAWINPUTHEADER,
    pub data: RAWINPUT_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAWINPUT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAWINPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union RAWINPUT_0 {
    pub mouse: RAWMOUSE,
    pub keyboard: RAWKEYBOARD,
    pub hid: RAWHID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAWINPUT_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAWINPUT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAWINPUTDEVICE {
    pub usUsagePage: u16,
    pub usUsage: u16,
    pub dwFlags: RAWINPUTDEVICE_FLAGS,
    pub hwndTarget: super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAWINPUTDEVICE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAWINPUTDEVICE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAWINPUTDEVICELIST {
    pub hDevice: super::super::Foundation::HANDLE,
    pub dwType: RID_DEVICE_INFO_TYPE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAWINPUTDEVICELIST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAWINPUTDEVICELIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAWINPUTHEADER {
    pub dwType: u32,
    pub dwSize: u32,
    pub hDevice: super::super::Foundation::HANDLE,
    pub wParam: super::super::Foundation::WPARAM,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAWINPUTHEADER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAWINPUTHEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAWKEYBOARD {
    pub MakeCode: u16,
    pub Flags: u16,
    pub Reserved: u16,
    pub VKey: u16,
    pub Message: u32,
    pub ExtraInformation: u32,
}
impl ::core::marker::Copy for RAWKEYBOARD {}
impl ::core::clone::Clone for RAWKEYBOARD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAWMOUSE {
    pub usFlags: u16,
    pub Anonymous: RAWMOUSE_0,
    pub ulRawButtons: u32,
    pub lLastX: i32,
    pub lLastY: i32,
    pub ulExtraInformation: u32,
}
impl ::core::marker::Copy for RAWMOUSE {}
impl ::core::clone::Clone for RAWMOUSE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union RAWMOUSE_0 {
    pub ulButtons: u32,
    pub Anonymous: RAWMOUSE_0_0,
}
impl ::core::marker::Copy for RAWMOUSE_0 {}
impl ::core::clone::Clone for RAWMOUSE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAWMOUSE_0_0 {
    pub usButtonFlags: u16,
    pub usButtonData: u16,
}
impl ::core::marker::Copy for RAWMOUSE_0_0 {}
impl ::core::clone::Clone for RAWMOUSE_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RID_DEVICE_INFO {
    pub cbSize: u32,
    pub dwType: RID_DEVICE_INFO_TYPE,
    pub Anonymous: RID_DEVICE_INFO_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RID_DEVICE_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RID_DEVICE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union RID_DEVICE_INFO_0 {
    pub mouse: RID_DEVICE_INFO_MOUSE,
    pub keyboard: RID_DEVICE_INFO_KEYBOARD,
    pub hid: RID_DEVICE_INFO_HID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RID_DEVICE_INFO_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RID_DEVICE_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RID_DEVICE_INFO_HID {
    pub dwVendorId: u32,
    pub dwProductId: u32,
    pub dwVersionNumber: u32,
    pub usUsagePage: u16,
    pub usUsage: u16,
}
impl ::core::marker::Copy for RID_DEVICE_INFO_HID {}
impl ::core::clone::Clone for RID_DEVICE_INFO_HID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RID_DEVICE_INFO_KEYBOARD {
    pub dwType: u32,
    pub dwSubType: u32,
    pub dwKeyboardMode: u32,
    pub dwNumberOfFunctionKeys: u32,
    pub dwNumberOfIndicators: u32,
    pub dwNumberOfKeysTotal: u32,
}
impl ::core::marker::Copy for RID_DEVICE_INFO_KEYBOARD {}
impl ::core::clone::Clone for RID_DEVICE_INFO_KEYBOARD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RID_DEVICE_INFO_MOUSE {
    pub dwId: u32,
    pub dwNumberOfButtons: u32,
    pub dwSampleRate: u32,
    pub fHasHorizontalWheel: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RID_DEVICE_INFO_MOUSE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RID_DEVICE_INFO_MOUSE {
    fn clone(&self) -> Self {
        *self
    }
}
