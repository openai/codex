::windows_targets::link!("oleaut32.dll" "system" fn BSTR_UserFree(param0 : *const u32, param1 : *const ::windows_sys::core::BSTR) -> ());
::windows_targets::link!("oleaut32.dll" "system" fn BSTR_UserFree64(param0 : *const u32, param1 : *const ::windows_sys::core::BSTR) -> ());
::windows_targets::link!("oleaut32.dll" "system" fn BSTR_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const ::windows_sys::core::BSTR) -> *mut u8);
::windows_targets::link!("oleaut32.dll" "system" fn BSTR_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const ::windows_sys::core::BSTR) -> *mut u8);
::windows_targets::link!("oleaut32.dll" "system" fn BSTR_UserSize(param0 : *const u32, param1 : u32, param2 : *const ::windows_sys::core::BSTR) -> u32);
::windows_targets::link!("oleaut32.dll" "system" fn BSTR_UserSize64(param0 : *const u32, param1 : u32, param2 : *const ::windows_sys::core::BSTR) -> u32);
::windows_targets::link!("oleaut32.dll" "system" fn BSTR_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut ::windows_sys::core::BSTR) -> *mut u8);
::windows_targets::link!("oleaut32.dll" "system" fn BSTR_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut ::windows_sys::core::BSTR) -> *mut u8);
::windows_targets::link!("ole32.dll" "system" fn CLIPFORMAT_UserFree(param0 : *const u32, param1 : *const u16) -> ());
::windows_targets::link!("ole32.dll" "system" fn CLIPFORMAT_UserFree64(param0 : *const u32, param1 : *const u16) -> ());
::windows_targets::link!("ole32.dll" "system" fn CLIPFORMAT_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const u16) -> *mut u8);
::windows_targets::link!("ole32.dll" "system" fn CLIPFORMAT_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const u16) -> *mut u8);
::windows_targets::link!("ole32.dll" "system" fn CLIPFORMAT_UserSize(param0 : *const u32, param1 : u32, param2 : *const u16) -> u32);
::windows_targets::link!("ole32.dll" "system" fn CLIPFORMAT_UserSize64(param0 : *const u32, param1 : u32, param2 : *const u16) -> u32);
::windows_targets::link!("ole32.dll" "system" fn CLIPFORMAT_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut u16) -> *mut u8);
::windows_targets::link!("ole32.dll" "system" fn CLIPFORMAT_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut u16) -> *mut u8);
::windows_targets::link!("ole32.dll" "system" fn CoGetMarshalSizeMax(pulsize : *mut u32, riid : *const ::windows_sys::core::GUID, punk : ::windows_sys::core::IUnknown, dwdestcontext : u32, pvdestcontext : *const ::core::ffi::c_void, mshlflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoGetStandardMarshal(riid : *const ::windows_sys::core::GUID, punk : ::windows_sys::core::IUnknown, dwdestcontext : u32, pvdestcontext : *const ::core::ffi::c_void, mshlflags : u32, ppmarshal : *mut IMarshal) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoGetStdMarshalEx(punkouter : ::windows_sys::core::IUnknown, smexflags : u32, ppunkinner : *mut ::windows_sys::core::IUnknown) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoMarshalHresult(pstm : super:: IStream, hresult : ::windows_sys::core::HRESULT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoMarshalInterThreadInterfaceInStream(riid : *const ::windows_sys::core::GUID, punk : ::windows_sys::core::IUnknown, ppstm : *mut super:: IStream) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoMarshalInterface(pstm : super:: IStream, riid : *const ::windows_sys::core::GUID, punk : ::windows_sys::core::IUnknown, dwdestcontext : u32, pvdestcontext : *const ::core::ffi::c_void, mshlflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoReleaseMarshalData(pstm : super:: IStream) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoUnmarshalHresult(pstm : super:: IStream, phresult : *mut ::windows_sys::core::HRESULT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoUnmarshalInterface(pstm : super:: IStream, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HACCEL_UserFree(param0 : *const u32, param1 : *const super::super::super::UI::WindowsAndMessaging:: HACCEL) -> ());
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HACCEL_UserFree64(param0 : *const u32, param1 : *const super::super::super::UI::WindowsAndMessaging:: HACCEL) -> ());
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HACCEL_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::UI::WindowsAndMessaging:: HACCEL) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HACCEL_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::UI::WindowsAndMessaging:: HACCEL) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HACCEL_UserSize(param0 : *const u32, param1 : u32, param2 : *const super::super::super::UI::WindowsAndMessaging:: HACCEL) -> u32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HACCEL_UserSize64(param0 : *const u32, param1 : u32, param2 : *const super::super::super::UI::WindowsAndMessaging:: HACCEL) -> u32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HACCEL_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::UI::WindowsAndMessaging:: HACCEL) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HACCEL_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::UI::WindowsAndMessaging:: HACCEL) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HBITMAP_UserFree(param0 : *const u32, param1 : *const super::super::super::Graphics::Gdi:: HBITMAP) -> ());
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HBITMAP_UserFree64(param0 : *const u32, param1 : *const super::super::super::Graphics::Gdi:: HBITMAP) -> ());
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HBITMAP_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Graphics::Gdi:: HBITMAP) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HBITMAP_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Graphics::Gdi:: HBITMAP) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HBITMAP_UserSize(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Graphics::Gdi:: HBITMAP) -> u32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HBITMAP_UserSize64(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Graphics::Gdi:: HBITMAP) -> u32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HBITMAP_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Graphics::Gdi:: HBITMAP) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HBITMAP_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Graphics::Gdi:: HBITMAP) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HDC_UserFree(param0 : *const u32, param1 : *const super::super::super::Graphics::Gdi:: HDC) -> ());
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HDC_UserFree64(param0 : *const u32, param1 : *const super::super::super::Graphics::Gdi:: HDC) -> ());
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HDC_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Graphics::Gdi:: HDC) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HDC_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Graphics::Gdi:: HDC) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HDC_UserSize(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Graphics::Gdi:: HDC) -> u32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HDC_UserSize64(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Graphics::Gdi:: HDC) -> u32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HDC_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Graphics::Gdi:: HDC) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HDC_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Graphics::Gdi:: HDC) -> *mut u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HGLOBAL_UserFree(param0 : *const u32, param1 : *const super::super::super::Foundation:: HGLOBAL) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HGLOBAL_UserFree64(param0 : *const u32, param1 : *const super::super::super::Foundation:: HGLOBAL) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HGLOBAL_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Foundation:: HGLOBAL) -> *mut u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HGLOBAL_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Foundation:: HGLOBAL) -> *mut u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HGLOBAL_UserSize(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Foundation:: HGLOBAL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HGLOBAL_UserSize64(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Foundation:: HGLOBAL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HGLOBAL_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Foundation:: HGLOBAL) -> *mut u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HGLOBAL_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Foundation:: HGLOBAL) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HICON_UserFree(param0 : *const u32, param1 : *const super::super::super::UI::WindowsAndMessaging:: HICON) -> ());
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HICON_UserFree64(param0 : *const u32, param1 : *const super::super::super::UI::WindowsAndMessaging:: HICON) -> ());
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HICON_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::UI::WindowsAndMessaging:: HICON) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HICON_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::UI::WindowsAndMessaging:: HICON) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HICON_UserSize(param0 : *const u32, param1 : u32, param2 : *const super::super::super::UI::WindowsAndMessaging:: HICON) -> u32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HICON_UserSize64(param0 : *const u32, param1 : u32, param2 : *const super::super::super::UI::WindowsAndMessaging:: HICON) -> u32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HICON_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::UI::WindowsAndMessaging:: HICON) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HICON_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::UI::WindowsAndMessaging:: HICON) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HMENU_UserFree(param0 : *const u32, param1 : *const super::super::super::UI::WindowsAndMessaging:: HMENU) -> ());
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HMENU_UserFree64(param0 : *const u32, param1 : *const super::super::super::UI::WindowsAndMessaging:: HMENU) -> ());
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HMENU_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::UI::WindowsAndMessaging:: HMENU) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HMENU_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::UI::WindowsAndMessaging:: HMENU) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HMENU_UserSize(param0 : *const u32, param1 : u32, param2 : *const super::super::super::UI::WindowsAndMessaging:: HMENU) -> u32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HMENU_UserSize64(param0 : *const u32, param1 : u32, param2 : *const super::super::super::UI::WindowsAndMessaging:: HMENU) -> u32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HMENU_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::UI::WindowsAndMessaging:: HMENU) -> *mut u8);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn HMENU_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::UI::WindowsAndMessaging:: HMENU) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HPALETTE_UserFree(param0 : *const u32, param1 : *const super::super::super::Graphics::Gdi:: HPALETTE) -> ());
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HPALETTE_UserFree64(param0 : *const u32, param1 : *const super::super::super::Graphics::Gdi:: HPALETTE) -> ());
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HPALETTE_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Graphics::Gdi:: HPALETTE) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HPALETTE_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Graphics::Gdi:: HPALETTE) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HPALETTE_UserSize(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Graphics::Gdi:: HPALETTE) -> u32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HPALETTE_UserSize64(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Graphics::Gdi:: HPALETTE) -> u32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HPALETTE_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Graphics::Gdi:: HPALETTE) -> *mut u8);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn HPALETTE_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Graphics::Gdi:: HPALETTE) -> *mut u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HWND_UserFree(param0 : *const u32, param1 : *const super::super::super::Foundation:: HWND) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HWND_UserFree64(param0 : *const u32, param1 : *const super::super::super::Foundation:: HWND) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HWND_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Foundation:: HWND) -> *mut u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HWND_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const super::super::super::Foundation:: HWND) -> *mut u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HWND_UserSize(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Foundation:: HWND) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HWND_UserSize64(param0 : *const u32, param1 : u32, param2 : *const super::super::super::Foundation:: HWND) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HWND_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Foundation:: HWND) -> *mut u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HWND_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut super::super::super::Foundation:: HWND) -> *mut u8);
::windows_targets::link!("oleaut32.dll" "system" fn LPSAFEARRAY_UserFree(param0 : *const u32, param1 : *const *const super:: SAFEARRAY) -> ());
::windows_targets::link!("oleaut32.dll" "system" fn LPSAFEARRAY_UserFree64(param0 : *const u32, param1 : *const *const super:: SAFEARRAY) -> ());
::windows_targets::link!("oleaut32.dll" "system" fn LPSAFEARRAY_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const *const super:: SAFEARRAY) -> *mut u8);
::windows_targets::link!("oleaut32.dll" "system" fn LPSAFEARRAY_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const *const super:: SAFEARRAY) -> *mut u8);
::windows_targets::link!("oleaut32.dll" "system" fn LPSAFEARRAY_UserSize(param0 : *const u32, param1 : u32, param2 : *const *const super:: SAFEARRAY) -> u32);
::windows_targets::link!("oleaut32.dll" "system" fn LPSAFEARRAY_UserSize64(param0 : *const u32, param1 : u32, param2 : *const *const super:: SAFEARRAY) -> u32);
::windows_targets::link!("oleaut32.dll" "system" fn LPSAFEARRAY_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut *mut super:: SAFEARRAY) -> *mut u8);
::windows_targets::link!("oleaut32.dll" "system" fn LPSAFEARRAY_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut *mut super:: SAFEARRAY) -> *mut u8);
::windows_targets::link!("ole32.dll" "system" fn SNB_UserFree(param0 : *const u32, param1 : *const *const *const u16) -> ());
::windows_targets::link!("ole32.dll" "system" fn SNB_UserFree64(param0 : *const u32, param1 : *const *const *const u16) -> ());
::windows_targets::link!("ole32.dll" "system" fn SNB_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const *const *const u16) -> *mut u8);
::windows_targets::link!("ole32.dll" "system" fn SNB_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const *const *const u16) -> *mut u8);
::windows_targets::link!("ole32.dll" "system" fn SNB_UserSize(param0 : *const u32, param1 : u32, param2 : *const *const *const u16) -> u32);
::windows_targets::link!("ole32.dll" "system" fn SNB_UserSize64(param0 : *const u32, param1 : u32, param2 : *const *const *const u16) -> u32);
::windows_targets::link!("ole32.dll" "system" fn SNB_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut *mut *mut u16) -> *mut u8);
::windows_targets::link!("ole32.dll" "system" fn SNB_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut *mut *mut u16) -> *mut u8);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn STGMEDIUM_UserFree(param0 : *const u32, param1 : *const super:: STGMEDIUM) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn STGMEDIUM_UserFree64(param0 : *const u32, param1 : *const super:: STGMEDIUM) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn STGMEDIUM_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const super:: STGMEDIUM) -> *mut u8);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn STGMEDIUM_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const super:: STGMEDIUM) -> *mut u8);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn STGMEDIUM_UserSize(param0 : *const u32, param1 : u32, param2 : *const super:: STGMEDIUM) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn STGMEDIUM_UserSize64(param0 : *const u32, param1 : u32, param2 : *const super:: STGMEDIUM) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn STGMEDIUM_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut super:: STGMEDIUM) -> *mut u8);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn STGMEDIUM_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut super:: STGMEDIUM) -> *mut u8);
pub type IMarshal = *mut ::core::ffi::c_void;
pub type IMarshal2 = *mut ::core::ffi::c_void;
pub type IMarshalingStream = *mut ::core::ffi::c_void;
pub const SMEXF_HANDLER: STDMSHLFLAGS = 2i32;
pub const SMEXF_SERVER: STDMSHLFLAGS = 1i32;
pub type STDMSHLFLAGS = i32;
