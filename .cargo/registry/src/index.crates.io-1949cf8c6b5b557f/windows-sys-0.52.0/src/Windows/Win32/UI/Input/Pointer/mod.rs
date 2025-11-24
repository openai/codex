#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnableMouseInPointer(fenable : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetPointerCursorId(pointerid : u32, cursorid : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_UI_Controls"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_UI_Controls\"`"] fn GetPointerDevice(device : super::super::super::Foundation:: HANDLE, pointerdevice : *mut super::super::Controls:: POINTER_DEVICE_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn GetPointerDeviceCursors(device : super::super::super::Foundation:: HANDLE, cursorcount : *mut u32, devicecursors : *mut super::super::Controls:: POINTER_DEVICE_CURSOR_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn GetPointerDeviceProperties(device : super::super::super::Foundation:: HANDLE, propertycount : *mut u32, pointerproperties : *mut super::super::Controls:: POINTER_DEVICE_PROPERTY) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetPointerDeviceRects(device : super::super::super::Foundation:: HANDLE, pointerdevicerect : *mut super::super::super::Foundation:: RECT, displayrect : *mut super::super::super::Foundation:: RECT) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_UI_Controls"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_UI_Controls\"`"] fn GetPointerDevices(devicecount : *mut u32, pointerdevices : *mut super::super::Controls:: POINTER_DEVICE_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerFrameInfo(pointerid : u32, pointercount : *mut u32, pointerinfo : *mut POINTER_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerFrameInfoHistory(pointerid : u32, entriescount : *mut u32, pointercount : *mut u32, pointerinfo : *mut POINTER_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerFramePenInfo(pointerid : u32, pointercount : *mut u32, peninfo : *mut POINTER_PEN_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerFramePenInfoHistory(pointerid : u32, entriescount : *mut u32, pointercount : *mut u32, peninfo : *mut POINTER_PEN_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerFrameTouchInfo(pointerid : u32, pointercount : *mut u32, touchinfo : *mut POINTER_TOUCH_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerFrameTouchInfoHistory(pointerid : u32, entriescount : *mut u32, pointercount : *mut u32, touchinfo : *mut POINTER_TOUCH_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerInfo(pointerid : u32, pointerinfo : *mut POINTER_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerInfoHistory(pointerid : u32, entriescount : *mut u32, pointerinfo : *mut POINTER_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetPointerInputTransform(pointerid : u32, historycount : u32, inputtransform : *mut INPUT_TRANSFORM) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerPenInfo(pointerid : u32, peninfo : *mut POINTER_PEN_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerPenInfoHistory(pointerid : u32, entriescount : *mut u32, peninfo : *mut POINTER_PEN_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerTouchInfo(pointerid : u32, touchinfo : *mut POINTER_TOUCH_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerTouchInfoHistory(pointerid : u32, entriescount : *mut u32, touchinfo : *mut POINTER_TOUCH_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetPointerType(pointerid : u32, pointertype : *mut super::super::WindowsAndMessaging:: POINTER_INPUT_TYPE) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn GetRawPointerDeviceData(pointerid : u32, historycount : u32, propertiescount : u32, pproperties : *const super::super::Controls:: POINTER_DEVICE_PROPERTY, pvalues : *mut i32) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn GetUnpredictedMessagePos() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InitializeTouchInjection(maxcount : u32, dwmode : TOUCH_FEEDBACK_MODE) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn InjectSyntheticPointerInput(device : super::super::Controls:: HSYNTHETICPOINTERDEVICE, pointerinfo : *const super::super::Controls:: POINTER_TYPE_INFO, count : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn InjectTouchInput(count : u32, contacts : *const POINTER_TOUCH_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsMouseInPointerEnabled() -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SkipPointerFrameMessages(pointerid : u32) -> super::super::super::Foundation:: BOOL);
pub const POINTER_CHANGE_FIFTHBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 9i32;
pub const POINTER_CHANGE_FIFTHBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 10i32;
pub const POINTER_CHANGE_FIRSTBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 1i32;
pub const POINTER_CHANGE_FIRSTBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 2i32;
pub const POINTER_CHANGE_FOURTHBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 7i32;
pub const POINTER_CHANGE_FOURTHBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 8i32;
pub const POINTER_CHANGE_NONE: POINTER_BUTTON_CHANGE_TYPE = 0i32;
pub const POINTER_CHANGE_SECONDBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 3i32;
pub const POINTER_CHANGE_SECONDBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 4i32;
pub const POINTER_CHANGE_THIRDBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 5i32;
pub const POINTER_CHANGE_THIRDBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 6i32;
pub const POINTER_FLAG_CANCELED: POINTER_FLAGS = 32768u32;
pub const POINTER_FLAG_CAPTURECHANGED: POINTER_FLAGS = 2097152u32;
pub const POINTER_FLAG_CONFIDENCE: POINTER_FLAGS = 16384u32;
pub const POINTER_FLAG_DOWN: POINTER_FLAGS = 65536u32;
pub const POINTER_FLAG_FIFTHBUTTON: POINTER_FLAGS = 256u32;
pub const POINTER_FLAG_FIRSTBUTTON: POINTER_FLAGS = 16u32;
pub const POINTER_FLAG_FOURTHBUTTON: POINTER_FLAGS = 128u32;
pub const POINTER_FLAG_HASTRANSFORM: POINTER_FLAGS = 4194304u32;
pub const POINTER_FLAG_HWHEEL: POINTER_FLAGS = 1048576u32;
pub const POINTER_FLAG_INCONTACT: POINTER_FLAGS = 4u32;
pub const POINTER_FLAG_INRANGE: POINTER_FLAGS = 2u32;
pub const POINTER_FLAG_NEW: POINTER_FLAGS = 1u32;
pub const POINTER_FLAG_NONE: POINTER_FLAGS = 0u32;
pub const POINTER_FLAG_PRIMARY: POINTER_FLAGS = 8192u32;
pub const POINTER_FLAG_SECONDBUTTON: POINTER_FLAGS = 32u32;
pub const POINTER_FLAG_THIRDBUTTON: POINTER_FLAGS = 64u32;
pub const POINTER_FLAG_UP: POINTER_FLAGS = 262144u32;
pub const POINTER_FLAG_UPDATE: POINTER_FLAGS = 131072u32;
pub const POINTER_FLAG_WHEEL: POINTER_FLAGS = 524288u32;
pub const TOUCH_FEEDBACK_DEFAULT: TOUCH_FEEDBACK_MODE = 1u32;
pub const TOUCH_FEEDBACK_INDIRECT: TOUCH_FEEDBACK_MODE = 2u32;
pub const TOUCH_FEEDBACK_NONE: TOUCH_FEEDBACK_MODE = 3u32;
pub type POINTER_BUTTON_CHANGE_TYPE = i32;
pub type POINTER_FLAGS = u32;
pub type TOUCH_FEEDBACK_MODE = u32;
#[repr(C)]
pub struct INPUT_INJECTION_VALUE {
    pub page: u16,
    pub usage: u16,
    pub value: i32,
    pub index: u16,
}
impl ::core::marker::Copy for INPUT_INJECTION_VALUE {}
impl ::core::clone::Clone for INPUT_INJECTION_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct INPUT_TRANSFORM {
    pub Anonymous: INPUT_TRANSFORM_0,
}
impl ::core::marker::Copy for INPUT_TRANSFORM {}
impl ::core::clone::Clone for INPUT_TRANSFORM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union INPUT_TRANSFORM_0 {
    pub Anonymous: INPUT_TRANSFORM_0_0,
    pub m: [f32; 16],
}
impl ::core::marker::Copy for INPUT_TRANSFORM_0 {}
impl ::core::clone::Clone for INPUT_TRANSFORM_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct INPUT_TRANSFORM_0_0 {
    pub _11: f32,
    pub _12: f32,
    pub _13: f32,
    pub _14: f32,
    pub _21: f32,
    pub _22: f32,
    pub _23: f32,
    pub _24: f32,
    pub _31: f32,
    pub _32: f32,
    pub _33: f32,
    pub _34: f32,
    pub _41: f32,
    pub _42: f32,
    pub _43: f32,
    pub _44: f32,
}
impl ::core::marker::Copy for INPUT_TRANSFORM_0_0 {}
impl ::core::clone::Clone for INPUT_TRANSFORM_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
pub struct POINTER_INFO {
    pub pointerType: super::super::WindowsAndMessaging::POINTER_INPUT_TYPE,
    pub pointerId: u32,
    pub frameId: u32,
    pub pointerFlags: POINTER_FLAGS,
    pub sourceDevice: super::super::super::Foundation::HANDLE,
    pub hwndTarget: super::super::super::Foundation::HWND,
    pub ptPixelLocation: super::super::super::Foundation::POINT,
    pub ptHimetricLocation: super::super::super::Foundation::POINT,
    pub ptPixelLocationRaw: super::super::super::Foundation::POINT,
    pub ptHimetricLocationRaw: super::super::super::Foundation::POINT,
    pub dwTime: u32,
    pub historyCount: u32,
    pub InputData: i32,
    pub dwKeyStates: u32,
    pub PerformanceCount: u64,
    pub ButtonChangeType: POINTER_BUTTON_CHANGE_TYPE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::marker::Copy for POINTER_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::clone::Clone for POINTER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
pub struct POINTER_PEN_INFO {
    pub pointerInfo: POINTER_INFO,
    pub penFlags: u32,
    pub penMask: u32,
    pub pressure: u32,
    pub rotation: u32,
    pub tiltX: i32,
    pub tiltY: i32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::marker::Copy for POINTER_PEN_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::clone::Clone for POINTER_PEN_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
pub struct POINTER_TOUCH_INFO {
    pub pointerInfo: POINTER_INFO,
    pub touchFlags: u32,
    pub touchMask: u32,
    pub rcContact: super::super::super::Foundation::RECT,
    pub rcContactRaw: super::super::super::Foundation::RECT,
    pub orientation: u32,
    pub pressure: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::marker::Copy for POINTER_TOUCH_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::clone::Clone for POINTER_TOUCH_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
