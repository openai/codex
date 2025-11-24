#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn AdjustWindowRectExForDpi(lprect : *mut super::super::Foundation:: RECT, dwstyle : super::WindowsAndMessaging:: WINDOW_STYLE, bmenu : super::super::Foundation:: BOOL, dwexstyle : super::WindowsAndMessaging:: WINDOW_EX_STYLE, dpi : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AreDpiAwarenessContextsEqual(dpicontexta : DPI_AWARENESS_CONTEXT, dpicontextb : DPI_AWARENESS_CONTEXT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnableNonClientDpiScaling(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn GetAwarenessFromDpiAwarenessContext(value : DPI_AWARENESS_CONTEXT) -> DPI_AWARENESS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDialogControlDpiChangeBehavior(hwnd : super::super::Foundation:: HWND) -> DIALOG_CONTROL_DPI_CHANGE_BEHAVIORS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDialogDpiChangeBehavior(hdlg : super::super::Foundation:: HWND) -> DIALOG_DPI_CHANGE_BEHAVIORS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDpiAwarenessContextForProcess(hprocess : super::super::Foundation:: HANDLE) -> DPI_AWARENESS_CONTEXT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("api-ms-win-shcore-scaling-l1-1-1.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GetDpiForMonitor(hmonitor : super::super::Graphics::Gdi:: HMONITOR, dpitype : MONITOR_DPI_TYPE, dpix : *mut u32, dpiy : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("user32.dll" "system" fn GetDpiForSystem() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDpiForWindow(hwnd : super::super::Foundation:: HWND) -> u32);
::windows_targets::link!("user32.dll" "system" fn GetDpiFromDpiAwarenessContext(value : DPI_AWARENESS_CONTEXT) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("api-ms-win-shcore-scaling-l1-1-1.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetProcessDpiAwareness(hprocess : super::super::Foundation:: HANDLE, value : *mut PROCESS_DPI_AWARENESS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetSystemDpiForProcess(hprocess : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn GetSystemMetricsForDpi(nindex : super::WindowsAndMessaging:: SYSTEM_METRICS_INDEX, dpi : u32) -> i32);
::windows_targets::link!("user32.dll" "system" fn GetThreadDpiAwarenessContext() -> DPI_AWARENESS_CONTEXT);
::windows_targets::link!("user32.dll" "system" fn GetThreadDpiHostingBehavior() -> DPI_HOSTING_BEHAVIOR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowDpiAwarenessContext(hwnd : super::super::Foundation:: HWND) -> DPI_AWARENESS_CONTEXT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowDpiHostingBehavior(hwnd : super::super::Foundation:: HWND) -> DPI_HOSTING_BEHAVIOR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsValidDpiAwarenessContext(value : DPI_AWARENESS_CONTEXT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LogicalToPhysicalPointForPerMonitorDPI(hwnd : super::super::Foundation:: HWND, lppoint : *mut super::super::Foundation:: POINT) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("uxtheme.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn OpenThemeDataForDpi(hwnd : super::super::Foundation:: HWND, pszclasslist : ::windows_sys::core::PCWSTR, dpi : u32) -> super::Controls:: HTHEME);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PhysicalToLogicalPointForPerMonitorDPI(hwnd : super::super::Foundation:: HWND, lppoint : *mut super::super::Foundation:: POINT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetDialogControlDpiChangeBehavior(hwnd : super::super::Foundation:: HWND, mask : DIALOG_CONTROL_DPI_CHANGE_BEHAVIORS, values : DIALOG_CONTROL_DPI_CHANGE_BEHAVIORS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetDialogDpiChangeBehavior(hdlg : super::super::Foundation:: HWND, mask : DIALOG_DPI_CHANGE_BEHAVIORS, values : DIALOG_DPI_CHANGE_BEHAVIORS) -> super::super::Foundation:: BOOL);
::windows_targets::link!("api-ms-win-shcore-scaling-l1-1-1.dll" "system" fn SetProcessDpiAwareness(value : PROCESS_DPI_AWARENESS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetProcessDpiAwarenessContext(value : DPI_AWARENESS_CONTEXT) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn SetThreadDpiAwarenessContext(dpicontext : DPI_AWARENESS_CONTEXT) -> DPI_AWARENESS_CONTEXT);
::windows_targets::link!("user32.dll" "system" fn SetThreadDpiHostingBehavior(value : DPI_HOSTING_BEHAVIOR) -> DPI_HOSTING_BEHAVIOR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SystemParametersInfoForDpi(uiaction : u32, uiparam : u32, pvparam : *mut ::core::ffi::c_void, fwinini : u32, dpi : u32) -> super::super::Foundation:: BOOL);
pub const DCDC_DEFAULT: DIALOG_CONTROL_DPI_CHANGE_BEHAVIORS = 0i32;
pub const DCDC_DISABLE_FONT_UPDATE: DIALOG_CONTROL_DPI_CHANGE_BEHAVIORS = 1i32;
pub const DCDC_DISABLE_RELAYOUT: DIALOG_CONTROL_DPI_CHANGE_BEHAVIORS = 2i32;
pub const DDC_DEFAULT: DIALOG_DPI_CHANGE_BEHAVIORS = 0i32;
pub const DDC_DISABLE_ALL: DIALOG_DPI_CHANGE_BEHAVIORS = 1i32;
pub const DDC_DISABLE_CONTROL_RELAYOUT: DIALOG_DPI_CHANGE_BEHAVIORS = 4i32;
pub const DDC_DISABLE_RESIZE: DIALOG_DPI_CHANGE_BEHAVIORS = 2i32;
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE: DPI_AWARENESS_CONTEXT = -3i32 as _;
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: DPI_AWARENESS_CONTEXT = -4i32 as _;
pub const DPI_AWARENESS_CONTEXT_SYSTEM_AWARE: DPI_AWARENESS_CONTEXT = -2i32 as _;
pub const DPI_AWARENESS_CONTEXT_UNAWARE: DPI_AWARENESS_CONTEXT = -1i32 as _;
pub const DPI_AWARENESS_CONTEXT_UNAWARE_GDISCALED: DPI_AWARENESS_CONTEXT = -5i32 as _;
pub const DPI_AWARENESS_INVALID: DPI_AWARENESS = -1i32;
pub const DPI_AWARENESS_PER_MONITOR_AWARE: DPI_AWARENESS = 2i32;
pub const DPI_AWARENESS_SYSTEM_AWARE: DPI_AWARENESS = 1i32;
pub const DPI_AWARENESS_UNAWARE: DPI_AWARENESS = 0i32;
pub const DPI_HOSTING_BEHAVIOR_DEFAULT: DPI_HOSTING_BEHAVIOR = 0i32;
pub const DPI_HOSTING_BEHAVIOR_INVALID: DPI_HOSTING_BEHAVIOR = -1i32;
pub const DPI_HOSTING_BEHAVIOR_MIXED: DPI_HOSTING_BEHAVIOR = 1i32;
pub const MDT_ANGULAR_DPI: MONITOR_DPI_TYPE = 1i32;
pub const MDT_DEFAULT: MONITOR_DPI_TYPE = 0i32;
pub const MDT_EFFECTIVE_DPI: MONITOR_DPI_TYPE = 0i32;
pub const MDT_RAW_DPI: MONITOR_DPI_TYPE = 2i32;
pub const PROCESS_DPI_UNAWARE: PROCESS_DPI_AWARENESS = 0i32;
pub const PROCESS_PER_MONITOR_DPI_AWARE: PROCESS_DPI_AWARENESS = 2i32;
pub const PROCESS_SYSTEM_DPI_AWARE: PROCESS_DPI_AWARENESS = 1i32;
pub type DIALOG_CONTROL_DPI_CHANGE_BEHAVIORS = i32;
pub type DIALOG_DPI_CHANGE_BEHAVIORS = i32;
pub type DPI_AWARENESS = i32;
pub type DPI_HOSTING_BEHAVIOR = i32;
pub type MONITOR_DPI_TYPE = i32;
pub type PROCESS_DPI_AWARENESS = i32;
pub type DPI_AWARENESS_CONTEXT = isize;
