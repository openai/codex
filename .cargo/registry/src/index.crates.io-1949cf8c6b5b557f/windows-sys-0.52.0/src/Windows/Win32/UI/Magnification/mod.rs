#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagGetColorEffect(hwnd : super::super::Foundation:: HWND, peffect : *mut MAGCOLOREFFECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagGetFullscreenColorEffect(peffect : *mut MAGCOLOREFFECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagGetFullscreenTransform(pmaglevel : *mut f32, pxoffset : *mut i32, pyoffset : *mut i32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn MagGetImageScalingCallback(hwnd : super::super::Foundation:: HWND) -> MagImageScalingCallback);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagGetInputTransform(pfenabled : *mut super::super::Foundation:: BOOL, prectsource : *mut super::super::Foundation:: RECT, prectdest : *mut super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagGetWindowFilterList(hwnd : super::super::Foundation:: HWND, pdwfiltermode : *mut MW_FILTERMODE, count : i32, phwnd : *mut super::super::Foundation:: HWND) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagGetWindowSource(hwnd : super::super::Foundation:: HWND, prect : *mut super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagGetWindowTransform(hwnd : super::super::Foundation:: HWND, ptransform : *mut MAGTRANSFORM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagInitialize() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagSetColorEffect(hwnd : super::super::Foundation:: HWND, peffect : *mut MAGCOLOREFFECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagSetFullscreenColorEffect(peffect : *const MAGCOLOREFFECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagSetFullscreenTransform(maglevel : f32, xoffset : i32, yoffset : i32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn MagSetImageScalingCallback(hwnd : super::super::Foundation:: HWND, callback : MagImageScalingCallback) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagSetInputTransform(fenabled : super::super::Foundation:: BOOL, prectsource : *const super::super::Foundation:: RECT, prectdest : *const super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagSetWindowFilterList(hwnd : super::super::Foundation:: HWND, dwfiltermode : MW_FILTERMODE, count : i32, phwnd : *mut super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagSetWindowSource(hwnd : super::super::Foundation:: HWND, rect : super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagSetWindowTransform(hwnd : super::super::Foundation:: HWND, ptransform : *mut MAGTRANSFORM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagShowSystemCursor(fshowcursor : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("magnification.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MagUninitialize() -> super::super::Foundation:: BOOL);
pub const MS_CLIPAROUNDCURSOR: i32 = 2i32;
pub const MS_INVERTCOLORS: i32 = 4i32;
pub const MS_SHOWMAGNIFIEDCURSOR: i32 = 1i32;
pub const MW_FILTERMODE_EXCLUDE: MW_FILTERMODE = 0u32;
pub const MW_FILTERMODE_INCLUDE: MW_FILTERMODE = 1u32;
pub const WC_MAGNIFIER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Magnifier");
pub const WC_MAGNIFIERA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Magnifier");
pub const WC_MAGNIFIERW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Magnifier");
pub type MW_FILTERMODE = u32;
#[repr(C)]
pub struct MAGCOLOREFFECT {
    pub transform: [f32; 25],
}
impl ::core::marker::Copy for MAGCOLOREFFECT {}
impl ::core::clone::Clone for MAGCOLOREFFECT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MAGIMAGEHEADER {
    pub width: u32,
    pub height: u32,
    pub format: ::windows_sys::core::GUID,
    pub stride: u32,
    pub offset: u32,
    pub cbSize: usize,
}
impl ::core::marker::Copy for MAGIMAGEHEADER {}
impl ::core::clone::Clone for MAGIMAGEHEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MAGTRANSFORM {
    pub v: [f32; 9],
}
impl ::core::marker::Copy for MAGTRANSFORM {}
impl ::core::clone::Clone for MAGTRANSFORM {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub type MagImageScalingCallback = ::core::option::Option<unsafe extern "system" fn(hwnd: super::super::Foundation::HWND, srcdata: *mut ::core::ffi::c_void, srcheader: MAGIMAGEHEADER, destdata: *mut ::core::ffi::c_void, destheader: MAGIMAGEHEADER, unclipped: super::super::Foundation::RECT, clipped: super::super::Foundation::RECT, dirty: super::super::Graphics::Gdi::HRGN) -> super::super::Foundation::BOOL>;
