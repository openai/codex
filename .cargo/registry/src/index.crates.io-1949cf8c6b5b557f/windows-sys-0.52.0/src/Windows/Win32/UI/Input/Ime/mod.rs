#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmAssociateContext(param0 : super::super::super::Foundation:: HWND, param1 : super::super::super::Globalization:: HIMC) -> super::super::super::Globalization:: HIMC);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmAssociateContextEx(param0 : super::super::super::Foundation:: HWND, param1 : super::super::super::Globalization:: HIMC, param2 : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_TextServices\"`"] fn ImmConfigureIMEA(param0 : super::super::TextServices:: HKL, param1 : super::super::super::Foundation:: HWND, param2 : u32, param3 : *mut ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_TextServices\"`"] fn ImmConfigureIMEW(param0 : super::super::TextServices:: HKL, param1 : super::super::super::Foundation:: HWND, param2 : u32, param3 : *mut ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmCreateContext() -> super::super::super::Globalization:: HIMC);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmCreateIMCC(param0 : u32) -> super::super::super::Globalization:: HIMCC);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmCreateSoftKeyboard(param0 : u32, param1 : super::super::super::Foundation:: HWND, param2 : i32, param3 : i32) -> super::super::super::Foundation:: HWND);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmDestroyContext(param0 : super::super::super::Globalization:: HIMC) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmDestroyIMCC(param0 : super::super::super::Globalization:: HIMCC) -> super::super::super::Globalization:: HIMCC);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmDestroySoftKeyboard(param0 : super::super::super::Foundation:: HWND) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmDisableIME(param0 : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmDisableLegacyIME() -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmDisableTextFrameService(idthread : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmEnumInputContext(idthread : u32, lpfn : IMCENUMPROC, lparam : super::super::super::Foundation:: LPARAM) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmEnumRegisterWordA(param0 : super::super::TextServices:: HKL, param1 : REGISTERWORDENUMPROCA, lpszreading : ::windows_sys::core::PCSTR, param3 : u32, lpszregister : ::windows_sys::core::PCSTR, param5 : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmEnumRegisterWordW(param0 : super::super::TextServices:: HKL, param1 : REGISTERWORDENUMPROCW, lpszreading : ::windows_sys::core::PCWSTR, param3 : u32, lpszregister : ::windows_sys::core::PCWSTR, param5 : *mut ::core::ffi::c_void) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`, `\"Win32_UI_TextServices\"`"] fn ImmEscapeA(param0 : super::super::TextServices:: HKL, param1 : super::super::super::Globalization:: HIMC, param2 : IME_ESCAPE, param3 : *mut ::core::ffi::c_void) -> super::super::super::Foundation:: LRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`, `\"Win32_UI_TextServices\"`"] fn ImmEscapeW(param0 : super::super::TextServices:: HKL, param1 : super::super::super::Globalization:: HIMC, param2 : IME_ESCAPE, param3 : *mut ::core::ffi::c_void) -> super::super::super::Foundation:: LRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmGenerateMessage(param0 : super::super::super::Globalization:: HIMC) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetCandidateListA(param0 : super::super::super::Globalization:: HIMC, deindex : u32, lpcandlist : *mut CANDIDATELIST, dwbuflen : u32) -> u32);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetCandidateListCountA(param0 : super::super::super::Globalization:: HIMC, lpdwlistcount : *mut u32) -> u32);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetCandidateListCountW(param0 : super::super::super::Globalization:: HIMC, lpdwlistcount : *mut u32) -> u32);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetCandidateListW(param0 : super::super::super::Globalization:: HIMC, deindex : u32, lpcandlist : *mut CANDIDATELIST, dwbuflen : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmGetCandidateWindow(param0 : super::super::super::Globalization:: HIMC, param1 : u32, lpcandidate : *mut CANDIDATEFORM) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`, `\"Win32_Graphics_Gdi\"`"] fn ImmGetCompositionFontA(param0 : super::super::super::Globalization:: HIMC, lplf : *mut super::super::super::Graphics::Gdi:: LOGFONTA) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`, `\"Win32_Graphics_Gdi\"`"] fn ImmGetCompositionFontW(param0 : super::super::super::Globalization:: HIMC, lplf : *mut super::super::super::Graphics::Gdi:: LOGFONTW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetCompositionStringA(param0 : super::super::super::Globalization:: HIMC, param1 : IME_COMPOSITION_STRING, lpbuf : *mut ::core::ffi::c_void, dwbuflen : u32) -> i32);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetCompositionStringW(param0 : super::super::super::Globalization:: HIMC, param1 : IME_COMPOSITION_STRING, lpbuf : *mut ::core::ffi::c_void, dwbuflen : u32) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmGetCompositionWindow(param0 : super::super::super::Globalization:: HIMC, lpcompform : *mut COMPOSITIONFORM) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmGetContext(param0 : super::super::super::Foundation:: HWND) -> super::super::super::Globalization:: HIMC);
#[cfg(all(feature = "Win32_Globalization", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`, `\"Win32_UI_TextServices\"`"] fn ImmGetConversionListA(param0 : super::super::TextServices:: HKL, param1 : super::super::super::Globalization:: HIMC, lpsrc : ::windows_sys::core::PCSTR, lpdst : *mut CANDIDATELIST, dwbuflen : u32, uflag : GET_CONVERSION_LIST_FLAG) -> u32);
#[cfg(all(feature = "Win32_Globalization", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`, `\"Win32_UI_TextServices\"`"] fn ImmGetConversionListW(param0 : super::super::TextServices:: HKL, param1 : super::super::super::Globalization:: HIMC, lpsrc : ::windows_sys::core::PCWSTR, lpdst : *mut CANDIDATELIST, dwbuflen : u32, uflag : GET_CONVERSION_LIST_FLAG) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmGetConversionStatus(param0 : super::super::super::Globalization:: HIMC, lpfdwconversion : *mut IME_CONVERSION_MODE, lpfdwsentence : *mut IME_SENTENCE_MODE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmGetDefaultIMEWnd(param0 : super::super::super::Foundation:: HWND) -> super::super::super::Foundation:: HWND);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmGetDescriptionA(param0 : super::super::TextServices:: HKL, lpszdescription : ::windows_sys::core::PSTR, ubuflen : u32) -> u32);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmGetDescriptionW(param0 : super::super::TextServices:: HKL, lpszdescription : ::windows_sys::core::PWSTR, ubuflen : u32) -> u32);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetGuideLineA(param0 : super::super::super::Globalization:: HIMC, dwindex : GET_GUIDE_LINE_TYPE, lpbuf : ::windows_sys::core::PSTR, dwbuflen : u32) -> u32);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetGuideLineW(param0 : super::super::super::Globalization:: HIMC, dwindex : GET_GUIDE_LINE_TYPE, lpbuf : ::windows_sys::core::PWSTR, dwbuflen : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_TextServices\"`"] fn ImmGetHotKey(param0 : u32, lpumodifiers : *mut u32, lpuvkey : *mut u32, phkl : *mut super::super::TextServices:: HKL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetIMCCLockCount(param0 : super::super::super::Globalization:: HIMCC) -> u32);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetIMCCSize(param0 : super::super::super::Globalization:: HIMCC) -> u32);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmGetIMCLockCount(param0 : super::super::super::Globalization:: HIMC) -> u32);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmGetIMEFileNameA(param0 : super::super::TextServices:: HKL, lpszfilename : ::windows_sys::core::PSTR, ubuflen : u32) -> u32);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmGetIMEFileNameW(param0 : super::super::TextServices:: HKL, lpszfilename : ::windows_sys::core::PWSTR, ubuflen : u32) -> u32);
#[cfg(all(feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`, `\"Win32_Graphics_Gdi\"`"] fn ImmGetImeMenuItemsA(param0 : super::super::super::Globalization:: HIMC, param1 : u32, param2 : u32, lpimeparentmenu : *mut IMEMENUITEMINFOA, lpimemenu : *mut IMEMENUITEMINFOA, dwsize : u32) -> u32);
#[cfg(all(feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`, `\"Win32_Graphics_Gdi\"`"] fn ImmGetImeMenuItemsW(param0 : super::super::super::Globalization:: HIMC, param1 : u32, param2 : u32, lpimeparentmenu : *mut IMEMENUITEMINFOW, lpimemenu : *mut IMEMENUITEMINFOW, dwsize : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmGetOpenStatus(param0 : super::super::super::Globalization:: HIMC) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmGetProperty(param0 : super::super::TextServices:: HKL, param1 : u32) -> u32);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmGetRegisterWordStyleA(param0 : super::super::TextServices:: HKL, nitem : u32, lpstylebuf : *mut STYLEBUFA) -> u32);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmGetRegisterWordStyleW(param0 : super::super::TextServices:: HKL, nitem : u32, lpstylebuf : *mut STYLEBUFW) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmGetStatusWindowPos(param0 : super::super::super::Globalization:: HIMC, lpptpos : *mut super::super::super::Foundation:: POINT) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmGetVirtualKey(param0 : super::super::super::Foundation:: HWND) -> u32);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmInstallIMEA(lpszimefilename : ::windows_sys::core::PCSTR, lpszlayouttext : ::windows_sys::core::PCSTR) -> super::super::TextServices:: HKL);
#[cfg(feature = "Win32_UI_TextServices")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_UI_TextServices\"`"] fn ImmInstallIMEW(lpszimefilename : ::windows_sys::core::PCWSTR, lpszlayouttext : ::windows_sys::core::PCWSTR) -> super::super::TextServices:: HKL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_TextServices\"`"] fn ImmIsIME(param0 : super::super::TextServices:: HKL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmIsUIMessageA(param0 : super::super::super::Foundation:: HWND, param1 : u32, param2 : super::super::super::Foundation:: WPARAM, param3 : super::super::super::Foundation:: LPARAM) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmIsUIMessageW(param0 : super::super::super::Foundation:: HWND, param1 : u32, param2 : super::super::super::Foundation:: WPARAM, param3 : super::super::super::Foundation:: LPARAM) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`, `\"Win32_Graphics_Gdi\"`"] fn ImmLockIMC(param0 : super::super::super::Globalization:: HIMC) -> *mut INPUTCONTEXT);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmLockIMCC(param0 : super::super::super::Globalization:: HIMCC) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmNotifyIME(param0 : super::super::super::Globalization:: HIMC, dwaction : NOTIFY_IME_ACTION, dwindex : NOTIFY_IME_INDEX, dwvalue : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Globalization")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Globalization\"`"] fn ImmReSizeIMCC(param0 : super::super::super::Globalization:: HIMCC, param1 : u32) -> super::super::super::Globalization:: HIMCC);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_TextServices\"`"] fn ImmRegisterWordA(param0 : super::super::TextServices:: HKL, lpszreading : ::windows_sys::core::PCSTR, param2 : u32, lpszregister : ::windows_sys::core::PCSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_TextServices\"`"] fn ImmRegisterWordW(param0 : super::super::TextServices:: HKL, lpszreading : ::windows_sys::core::PCWSTR, param2 : u32, lpszregister : ::windows_sys::core::PCWSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmReleaseContext(param0 : super::super::super::Foundation:: HWND, param1 : super::super::super::Globalization:: HIMC) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmRequestMessageA(param0 : super::super::super::Globalization:: HIMC, param1 : super::super::super::Foundation:: WPARAM, param2 : super::super::super::Foundation:: LPARAM) -> super::super::super::Foundation:: LRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmRequestMessageW(param0 : super::super::super::Globalization:: HIMC, param1 : super::super::super::Foundation:: WPARAM, param2 : super::super::super::Foundation:: LPARAM) -> super::super::super::Foundation:: LRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmSetCandidateWindow(param0 : super::super::super::Globalization:: HIMC, lpcandidate : *const CANDIDATEFORM) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`, `\"Win32_Graphics_Gdi\"`"] fn ImmSetCompositionFontA(param0 : super::super::super::Globalization:: HIMC, lplf : *const super::super::super::Graphics::Gdi:: LOGFONTA) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`, `\"Win32_Graphics_Gdi\"`"] fn ImmSetCompositionFontW(param0 : super::super::super::Globalization:: HIMC, lplf : *const super::super::super::Graphics::Gdi:: LOGFONTW) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmSetCompositionStringA(param0 : super::super::super::Globalization:: HIMC, dwindex : SET_COMPOSITION_STRING_TYPE, lpcomp : *const ::core::ffi::c_void, dwcomplen : u32, lpread : *const ::core::ffi::c_void, dwreadlen : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmSetCompositionStringW(param0 : super::super::super::Globalization:: HIMC, dwindex : SET_COMPOSITION_STRING_TYPE, lpcomp : *const ::core::ffi::c_void, dwcomplen : u32, lpread : *const ::core::ffi::c_void, dwreadlen : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmSetCompositionWindow(param0 : super::super::super::Globalization:: HIMC, lpcompform : *const COMPOSITIONFORM) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmSetConversionStatus(param0 : super::super::super::Globalization:: HIMC, param1 : IME_CONVERSION_MODE, param2 : IME_SENTENCE_MODE) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_TextServices\"`"] fn ImmSetHotKey(param0 : u32, param1 : u32, param2 : u32, param3 : super::super::TextServices:: HKL) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmSetOpenStatus(param0 : super::super::super::Globalization:: HIMC, param1 : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmSetStatusWindowPos(param0 : super::super::super::Globalization:: HIMC, lpptpos : *const super::super::super::Foundation:: POINT) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmShowSoftKeyboard(param0 : super::super::super::Foundation:: HWND, param1 : i32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImmSimulateHotKey(param0 : super::super::super::Foundation:: HWND, param1 : IME_HOTKEY_IDENTIFIER) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmUnlockIMC(param0 : super::super::super::Globalization:: HIMC) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"] fn ImmUnlockIMCC(param0 : super::super::super::Globalization:: HIMCC) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_TextServices\"`"] fn ImmUnregisterWordA(param0 : super::super::TextServices:: HKL, lpszreading : ::windows_sys::core::PCSTR, param2 : u32, lpszunregister : ::windows_sys::core::PCSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_TextServices"))]
::windows_targets::link!("imm32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_TextServices\"`"] fn ImmUnregisterWordW(param0 : super::super::TextServices:: HKL, lpszreading : ::windows_sys::core::PCWSTR, param2 : u32, lpszunregister : ::windows_sys::core::PCWSTR) -> super::super::super::Foundation:: BOOL);
pub type IActiveIME = *mut ::core::ffi::c_void;
pub type IActiveIME2 = *mut ::core::ffi::c_void;
pub type IActiveIMMApp = *mut ::core::ffi::c_void;
pub type IActiveIMMIME = *mut ::core::ffi::c_void;
pub type IActiveIMMMessagePumpOwner = *mut ::core::ffi::c_void;
pub type IActiveIMMRegistrar = *mut ::core::ffi::c_void;
pub type IEnumInputContext = *mut ::core::ffi::c_void;
pub type IEnumRegisterWordA = *mut ::core::ffi::c_void;
pub type IEnumRegisterWordW = *mut ::core::ffi::c_void;
pub type IFEClassFactory = *mut ::core::ffi::c_void;
pub type IFECommon = *mut ::core::ffi::c_void;
pub type IFEDictionary = *mut ::core::ffi::c_void;
pub type IFELanguage = *mut ::core::ffi::c_void;
pub type IImePad = *mut ::core::ffi::c_void;
pub type IImePadApplet = *mut ::core::ffi::c_void;
pub type IImePlugInDictDictionaryList = *mut ::core::ffi::c_void;
pub type IImeSpecifyApplets = *mut ::core::ffi::c_void;
pub const ATTR_CONVERTED: u32 = 2u32;
pub const ATTR_FIXEDCONVERTED: u32 = 5u32;
pub const ATTR_INPUT: u32 = 0u32;
pub const ATTR_INPUT_ERROR: u32 = 4u32;
pub const ATTR_TARGET_CONVERTED: u32 = 1u32;
pub const ATTR_TARGET_NOTCONVERTED: u32 = 3u32;
pub const CATID_MSIME_IImePadApplet: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7566cad1_4ec9_4478_9fe9_8ed766619edf);
pub const CATID_MSIME_IImePadApplet1000: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe081e1d6_2389_43cb_b66f_609f823d9f9c);
pub const CATID_MSIME_IImePadApplet1200: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa47fb5fc_7d15_4223_a789_b781bf9ae667);
pub const CATID_MSIME_IImePadApplet900: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfaae51bf_5e5b_4a1d_8de1_17c1d9e1728d);
pub const CATID_MSIME_IImePadApplet_VER7: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4a0f8e31_c3ee_11d1_afef_00805f0c8b6d);
pub const CATID_MSIME_IImePadApplet_VER80: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x56f7a792_fef1_11d3_8463_00c04f7a06e5);
pub const CATID_MSIME_IImePadApplet_VER81: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x656520b0_bb88_11d4_84c0_00c04f7a06e5);
pub const CActiveIMM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4955dd33_b159_11d0_8fcf_00aa006bcc59);
pub const CFS_CANDIDATEPOS: u32 = 64u32;
pub const CFS_DEFAULT: u32 = 0u32;
pub const CFS_EXCLUDE: u32 = 128u32;
pub const CFS_FORCE_POSITION: u32 = 32u32;
pub const CFS_POINT: u32 = 2u32;
pub const CFS_RECT: u32 = 1u32;
pub const CHARINFO_APPLETID_MASK: u32 = 4278190080u32;
pub const CHARINFO_CHARID_MASK: u32 = 65535u32;
pub const CHARINFO_FEID_MASK: u32 = 15728640u32;
pub const CLSID_ImePlugInDictDictionaryList_CHS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7bf0129b_5bef_4de4_9b0b_5edb66ac2fa6);
pub const CLSID_ImePlugInDictDictionaryList_JPN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4fe2776b_b0f9_4396_b5fc_e9d4cf1ec195);
pub const CLSID_VERSION_DEPENDENT_MSIME_JAPANESE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6a91029e_aa49_471b_aee7_7d332785660d);
pub const CPS_CANCEL: NOTIFY_IME_INDEX = 4u32;
pub const CPS_COMPLETE: NOTIFY_IME_INDEX = 1u32;
pub const CPS_CONVERT: NOTIFY_IME_INDEX = 2u32;
pub const CPS_REVERT: NOTIFY_IME_INDEX = 3u32;
pub const CS_INSERTCHAR: u32 = 8192u32;
pub const CS_NOMOVECARET: u32 = 16384u32;
pub const E_LARGEINPUT: u32 = 51u32;
pub const E_NOCAND: u32 = 48u32;
pub const E_NOTENOUGH_BUFFER: u32 = 49u32;
pub const E_NOTENOUGH_WDD: u32 = 50u32;
pub const FEID_CHINESE_HONGKONG: u32 = 3u32;
pub const FEID_CHINESE_SIMPLIFIED: u32 = 2u32;
pub const FEID_CHINESE_SINGAPORE: u32 = 4u32;
pub const FEID_CHINESE_TRADITIONAL: u32 = 1u32;
pub const FEID_JAPANESE: u32 = 5u32;
pub const FEID_KOREAN: u32 = 6u32;
pub const FEID_KOREAN_JOHAB: u32 = 7u32;
pub const FEID_NONE: u32 = 0u32;
pub const FELANG_CLMN_FIXD: u32 = 32u32;
pub const FELANG_CLMN_FIXR: u32 = 16u32;
pub const FELANG_CLMN_NOPBREAK: u32 = 8u32;
pub const FELANG_CLMN_NOWBREAK: u32 = 2u32;
pub const FELANG_CLMN_PBREAK: u32 = 4u32;
pub const FELANG_CLMN_WBREAK: u32 = 1u32;
pub const FELANG_CMODE_AUTOMATIC: u32 = 134217728u32;
pub const FELANG_CMODE_BESTFIRST: u32 = 16384u32;
pub const FELANG_CMODE_BOPOMOFO: u32 = 64u32;
pub const FELANG_CMODE_CONVERSATION: u32 = 536870912u32;
pub const FELANG_CMODE_FULLWIDTHOUT: u32 = 32u32;
pub const FELANG_CMODE_HALFWIDTHOUT: u32 = 16u32;
pub const FELANG_CMODE_HANGUL: u32 = 128u32;
pub const FELANG_CMODE_HIRAGANAOUT: u32 = 0u32;
pub const FELANG_CMODE_KATAKANAOUT: u32 = 8u32;
pub const FELANG_CMODE_MERGECAND: u32 = 4096u32;
pub const FELANG_CMODE_MONORUBY: u32 = 2u32;
pub const FELANG_CMODE_NAME: u32 = 268435456u32;
pub const FELANG_CMODE_NOINVISIBLECHAR: u32 = 1073741824u32;
pub const FELANG_CMODE_NONE: u32 = 16777216u32;
pub const FELANG_CMODE_NOPRUNING: u32 = 4u32;
pub const FELANG_CMODE_PHRASEPREDICT: u32 = 268435456u32;
pub const FELANG_CMODE_PINYIN: u32 = 256u32;
pub const FELANG_CMODE_PLAURALCLAUSE: u32 = 33554432u32;
pub const FELANG_CMODE_PRECONV: u32 = 512u32;
pub const FELANG_CMODE_RADICAL: u32 = 1024u32;
pub const FELANG_CMODE_ROMAN: u32 = 8192u32;
pub const FELANG_CMODE_SINGLECONVERT: u32 = 67108864u32;
pub const FELANG_CMODE_UNKNOWNREADING: u32 = 2048u32;
pub const FELANG_CMODE_USENOREVWORDS: u32 = 32768u32;
pub const FELANG_INVALD_PO: u32 = 65535u32;
pub const FELANG_REQ_CONV: u32 = 65536u32;
pub const FELANG_REQ_RECONV: u32 = 131072u32;
pub const FELANG_REQ_REV: u32 = 196608u32;
pub const FID_MSIME_KMS_DEL_KEYLIST: u32 = 4u32;
pub const FID_MSIME_KMS_FUNCDESC: u32 = 9u32;
pub const FID_MSIME_KMS_GETMAP: u32 = 6u32;
pub const FID_MSIME_KMS_GETMAPFAST: u32 = 11u32;
pub const FID_MSIME_KMS_GETMAPSEAMLESS: u32 = 10u32;
pub const FID_MSIME_KMS_INIT: u32 = 2u32;
pub const FID_MSIME_KMS_INVOKE: u32 = 7u32;
pub const FID_MSIME_KMS_NOTIFY: u32 = 5u32;
pub const FID_MSIME_KMS_SETMAP: u32 = 8u32;
pub const FID_MSIME_KMS_TERM: u32 = 3u32;
pub const FID_MSIME_KMS_VERSION: u32 = 1u32;
pub const FID_MSIME_VERSION: u32 = 0u32;
pub const FID_RECONVERT_VERSION: u32 = 268435456u32;
pub const GCL_CONVERSION: GET_CONVERSION_LIST_FLAG = 1u32;
pub const GCL_REVERSECONVERSION: GET_CONVERSION_LIST_FLAG = 2u32;
pub const GCL_REVERSE_LENGTH: GET_CONVERSION_LIST_FLAG = 3u32;
pub const GCSEX_CANCELRECONVERT: u32 = 268435456u32;
pub const GCS_COMPATTR: IME_COMPOSITION_STRING = 16u32;
pub const GCS_COMPCLAUSE: IME_COMPOSITION_STRING = 32u32;
pub const GCS_COMPREADATTR: IME_COMPOSITION_STRING = 2u32;
pub const GCS_COMPREADCLAUSE: IME_COMPOSITION_STRING = 4u32;
pub const GCS_COMPREADSTR: IME_COMPOSITION_STRING = 1u32;
pub const GCS_COMPSTR: IME_COMPOSITION_STRING = 8u32;
pub const GCS_CURSORPOS: IME_COMPOSITION_STRING = 128u32;
pub const GCS_DELTASTART: IME_COMPOSITION_STRING = 256u32;
pub const GCS_RESULTCLAUSE: IME_COMPOSITION_STRING = 4096u32;
pub const GCS_RESULTREADCLAUSE: IME_COMPOSITION_STRING = 1024u32;
pub const GCS_RESULTREADSTR: IME_COMPOSITION_STRING = 512u32;
pub const GCS_RESULTSTR: IME_COMPOSITION_STRING = 2048u32;
pub const GGL_INDEX: GET_GUIDE_LINE_TYPE = 2u32;
pub const GGL_LEVEL: GET_GUIDE_LINE_TYPE = 1u32;
pub const GGL_PRIVATE: GET_GUIDE_LINE_TYPE = 4u32;
pub const GGL_STRING: GET_GUIDE_LINE_TYPE = 3u32;
pub const GL_ID_CANNOTSAVE: u32 = 17u32;
pub const GL_ID_CHOOSECANDIDATE: u32 = 40u32;
pub const GL_ID_INPUTCODE: u32 = 38u32;
pub const GL_ID_INPUTRADICAL: u32 = 37u32;
pub const GL_ID_INPUTREADING: u32 = 36u32;
pub const GL_ID_INPUTSYMBOL: u32 = 39u32;
pub const GL_ID_NOCONVERT: u32 = 32u32;
pub const GL_ID_NODICTIONARY: u32 = 16u32;
pub const GL_ID_NOMODULE: u32 = 1u32;
pub const GL_ID_PRIVATE_FIRST: u32 = 32768u32;
pub const GL_ID_PRIVATE_LAST: u32 = 65535u32;
pub const GL_ID_READINGCONFLICT: u32 = 35u32;
pub const GL_ID_REVERSECONVERSION: u32 = 41u32;
pub const GL_ID_TOOMANYSTROKE: u32 = 34u32;
pub const GL_ID_TYPINGERROR: u32 = 33u32;
pub const GL_ID_UNKNOWN: u32 = 0u32;
pub const GL_LEVEL_ERROR: u32 = 2u32;
pub const GL_LEVEL_FATAL: u32 = 1u32;
pub const GL_LEVEL_INFORMATION: u32 = 4u32;
pub const GL_LEVEL_NOGUIDELINE: u32 = 0u32;
pub const GL_LEVEL_WARNING: u32 = 3u32;
pub const IACE_CHILDREN: u32 = 1u32;
pub const IACE_DEFAULT: u32 = 16u32;
pub const IACE_IGNORENOCONTEXT: u32 = 32u32;
pub const IFEC_S_ALREADY_DEFAULT: ::windows_sys::core::HRESULT = 291840i32;
pub const IFED_ACTIVE_DICT: IMEFMT = 13i32;
pub const IFED_ATOK10: IMEFMT = 15i32;
pub const IFED_ATOK9: IMEFMT = 14i32;
pub const IFED_E_INVALID_FORMAT: ::windows_sys::core::HRESULT = -2147192063i32;
pub const IFED_E_NOT_FOUND: ::windows_sys::core::HRESULT = -2147192064i32;
pub const IFED_E_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -2147192057i32;
pub const IFED_E_NOT_USER_DIC: ::windows_sys::core::HRESULT = -2147192058i32;
pub const IFED_E_NO_ENTRY: ::windows_sys::core::HRESULT = -2147192060i32;
pub const IFED_E_OPEN_FAILED: ::windows_sys::core::HRESULT = -2147192062i32;
pub const IFED_E_REGISTER_DISCONNECTED: ::windows_sys::core::HRESULT = -2147192053i32;
pub const IFED_E_REGISTER_FAILED: ::windows_sys::core::HRESULT = -2147192059i32;
pub const IFED_E_REGISTER_ILLEGAL_POS: ::windows_sys::core::HRESULT = -2147192055i32;
pub const IFED_E_REGISTER_IMPROPER_WORD: ::windows_sys::core::HRESULT = -2147192054i32;
pub const IFED_E_USER_COMMENT: ::windows_sys::core::HRESULT = -2147192056i32;
pub const IFED_E_WRITE_FAILED: ::windows_sys::core::HRESULT = -2147192061i32;
pub const IFED_MSIME2_BIN_SYSTEM: IMEFMT = 1i32;
pub const IFED_MSIME2_BIN_USER: IMEFMT = 2i32;
pub const IFED_MSIME2_TEXT_USER: IMEFMT = 3i32;
pub const IFED_MSIME95_BIN_SYSTEM: IMEFMT = 4i32;
pub const IFED_MSIME95_BIN_USER: IMEFMT = 5i32;
pub const IFED_MSIME95_TEXT_USER: IMEFMT = 6i32;
pub const IFED_MSIME97_BIN_SYSTEM: IMEFMT = 7i32;
pub const IFED_MSIME97_BIN_USER: IMEFMT = 8i32;
pub const IFED_MSIME97_TEXT_USER: IMEFMT = 9i32;
pub const IFED_MSIME98_BIN_SYSTEM: IMEFMT = 10i32;
pub const IFED_MSIME98_BIN_USER: IMEFMT = 11i32;
pub const IFED_MSIME98_SYSTEM_CE: IMEFMT = 20i32;
pub const IFED_MSIME98_TEXT_USER: IMEFMT = 12i32;
pub const IFED_MSIME_BIN_SYSTEM: IMEFMT = 21i32;
pub const IFED_MSIME_BIN_USER: IMEFMT = 22i32;
pub const IFED_MSIME_TEXT_USER: IMEFMT = 23i32;
pub const IFED_NEC_AI_: IMEFMT = 16i32;
pub const IFED_PIME2_BIN_STANDARD_SYSTEM: IMEFMT = 26i32;
pub const IFED_PIME2_BIN_SYSTEM: IMEFMT = 25i32;
pub const IFED_PIME2_BIN_USER: IMEFMT = 24i32;
pub const IFED_POS_ADJECTIVE: u32 = 4u32;
pub const IFED_POS_ADJECTIVE_VERB: u32 = 8u32;
pub const IFED_POS_ADNOUN: u32 = 32u32;
pub const IFED_POS_ADVERB: u32 = 16u32;
pub const IFED_POS_AFFIX: u32 = 1536u32;
pub const IFED_POS_ALL: u32 = 131071u32;
pub const IFED_POS_AUXILIARY_VERB: u32 = 32768u32;
pub const IFED_POS_CONJUNCTION: u32 = 64u32;
pub const IFED_POS_DEPENDENT: u32 = 114688u32;
pub const IFED_POS_IDIOMS: u32 = 4096u32;
pub const IFED_POS_INDEPENDENT: u32 = 255u32;
pub const IFED_POS_INFLECTIONALSUFFIX: u32 = 256u32;
pub const IFED_POS_INTERJECTION: u32 = 128u32;
pub const IFED_POS_NONE: u32 = 0u32;
pub const IFED_POS_NOUN: u32 = 1u32;
pub const IFED_POS_PARTICLE: u32 = 16384u32;
pub const IFED_POS_PREFIX: u32 = 512u32;
pub const IFED_POS_SUB_VERB: u32 = 65536u32;
pub const IFED_POS_SUFFIX: u32 = 1024u32;
pub const IFED_POS_SYMBOLS: u32 = 8192u32;
pub const IFED_POS_TANKANJI: u32 = 2048u32;
pub const IFED_POS_VERB: u32 = 2u32;
pub const IFED_REG_ALL: u32 = 7u32;
pub const IFED_REG_AUTO: u32 = 2u32;
pub const IFED_REG_DEL: IMEREG = 2i32;
pub const IFED_REG_GRAMMAR: u32 = 4u32;
pub const IFED_REG_HEAD: IMEREG = 0i32;
pub const IFED_REG_NONE: u32 = 0u32;
pub const IFED_REG_TAIL: IMEREG = 1i32;
pub const IFED_REG_USER: u32 = 1u32;
pub const IFED_REL_ALL: IMEREL = 24i32;
pub const IFED_REL_DE: IMEREL = 5i32;
pub const IFED_REL_FUKU_YOUGEN: IMEREL = 12i32;
pub const IFED_REL_GA: IMEREL = 2i32;
pub const IFED_REL_HE: IMEREL = 9i32;
pub const IFED_REL_IDEOM: IMEREL = 11i32;
pub const IFED_REL_KARA: IMEREL = 7i32;
pub const IFED_REL_KEIDOU1_YOUGEN: IMEREL = 14i32;
pub const IFED_REL_KEIDOU2_YOUGEN: IMEREL = 15i32;
pub const IFED_REL_KEIYOU_TARU_YOUGEN: IMEREL = 21i32;
pub const IFED_REL_KEIYOU_TO_YOUGEN: IMEREL = 20i32;
pub const IFED_REL_KEIYOU_YOUGEN: IMEREL = 13i32;
pub const IFED_REL_MADE: IMEREL = 8i32;
pub const IFED_REL_NI: IMEREL = 4i32;
pub const IFED_REL_NO: IMEREL = 1i32;
pub const IFED_REL_NONE: IMEREL = 0i32;
pub const IFED_REL_RENSOU: IMEREL = 19i32;
pub const IFED_REL_RENTAI_MEI: IMEREL = 18i32;
pub const IFED_REL_TAIGEN: IMEREL = 16i32;
pub const IFED_REL_TO: IMEREL = 10i32;
pub const IFED_REL_UNKNOWN1: IMEREL = 22i32;
pub const IFED_REL_UNKNOWN2: IMEREL = 23i32;
pub const IFED_REL_WO: IMEREL = 3i32;
pub const IFED_REL_YORI: IMEREL = 6i32;
pub const IFED_REL_YOUGEN: IMEREL = 17i32;
pub const IFED_SELECT_ALL: u32 = 15u32;
pub const IFED_SELECT_COMMENT: u32 = 8u32;
pub const IFED_SELECT_DISPLAY: u32 = 2u32;
pub const IFED_SELECT_NONE: u32 = 0u32;
pub const IFED_SELECT_POS: u32 = 4u32;
pub const IFED_SELECT_READING: u32 = 1u32;
pub const IFED_S_COMMENT_CHANGED: ::windows_sys::core::HRESULT = 291331i32;
pub const IFED_S_EMPTY_DICTIONARY: ::windows_sys::core::HRESULT = 291329i32;
pub const IFED_S_MORE_ENTRIES: ::windows_sys::core::HRESULT = 291328i32;
pub const IFED_S_WORD_EXISTS: ::windows_sys::core::HRESULT = 291330i32;
pub const IFED_TYPE_ALL: u32 = 31u32;
pub const IFED_TYPE_ENGLISH: u32 = 16u32;
pub const IFED_TYPE_GENERAL: u32 = 1u32;
pub const IFED_TYPE_NAMEPLACE: u32 = 2u32;
pub const IFED_TYPE_NONE: u32 = 0u32;
pub const IFED_TYPE_REVERSE: u32 = 8u32;
pub const IFED_TYPE_SPEECH: u32 = 4u32;
pub const IFED_UCT_MAX: IMEUCT = 4i32;
pub const IFED_UCT_NONE: IMEUCT = 0i32;
pub const IFED_UCT_STRING_SJIS: IMEUCT = 1i32;
pub const IFED_UCT_STRING_UNICODE: IMEUCT = 2i32;
pub const IFED_UCT_USER_DEFINED: IMEUCT = 3i32;
pub const IFED_UNKNOWN: IMEFMT = 0i32;
pub const IFED_VJE_20: IMEFMT = 19i32;
pub const IFED_WX_II: IMEFMT = 17i32;
pub const IFED_WX_III: IMEFMT = 18i32;
pub const IGIMIF_RIGHTMENU: u32 = 1u32;
pub const IGIMII_CMODE: u32 = 1u32;
pub const IGIMII_CONFIGURE: u32 = 4u32;
pub const IGIMII_HELP: u32 = 16u32;
pub const IGIMII_INPUTTOOLS: u32 = 64u32;
pub const IGIMII_OTHER: u32 = 32u32;
pub const IGIMII_SMODE: u32 = 2u32;
pub const IGIMII_TOOLS: u32 = 8u32;
pub const IMC_CLOSESTATUSWINDOW: u32 = 33u32;
pub const IMC_GETCANDIDATEPOS: u32 = 7u32;
pub const IMC_GETCOMPOSITIONFONT: u32 = 9u32;
pub const IMC_GETCOMPOSITIONWINDOW: u32 = 11u32;
pub const IMC_GETSOFTKBDFONT: u32 = 17u32;
pub const IMC_GETSOFTKBDPOS: u32 = 19u32;
pub const IMC_GETSOFTKBDSUBTYPE: u32 = 21u32;
pub const IMC_GETSTATUSWINDOWPOS: u32 = 15u32;
pub const IMC_OPENSTATUSWINDOW: u32 = 34u32;
pub const IMC_SETCANDIDATEPOS: u32 = 8u32;
pub const IMC_SETCOMPOSITIONFONT: u32 = 10u32;
pub const IMC_SETCOMPOSITIONWINDOW: u32 = 12u32;
pub const IMC_SETCONVERSIONMODE: u32 = 2u32;
pub const IMC_SETOPENSTATUS: u32 = 6u32;
pub const IMC_SETSENTENCEMODE: u32 = 4u32;
pub const IMC_SETSOFTKBDDATA: u32 = 24u32;
pub const IMC_SETSOFTKBDFONT: u32 = 18u32;
pub const IMC_SETSOFTKBDPOS: u32 = 20u32;
pub const IMC_SETSOFTKBDSUBTYPE: u32 = 22u32;
pub const IMC_SETSTATUSWINDOWPOS: u32 = 16u32;
pub const IMEFAREASTINFO_TYPE_COMMENT: u32 = 2u32;
pub const IMEFAREASTINFO_TYPE_COSTTIME: u32 = 3u32;
pub const IMEFAREASTINFO_TYPE_DEFAULT: u32 = 0u32;
pub const IMEFAREASTINFO_TYPE_READING: u32 = 1u32;
pub const IMEKEYCTRLMASK_ALT: u32 = 1u32;
pub const IMEKEYCTRLMASK_CTRL: u32 = 2u32;
pub const IMEKEYCTRLMASK_SHIFT: u32 = 4u32;
pub const IMEKEYCTRL_DOWN: u32 = 0u32;
pub const IMEKEYCTRL_UP: u32 = 1u32;
pub const IMEKMS_2NDLEVEL: u32 = 4u32;
pub const IMEKMS_CANDIDATE: u32 = 6u32;
pub const IMEKMS_COMPOSITION: u32 = 1u32;
pub const IMEKMS_IMEOFF: u32 = 3u32;
pub const IMEKMS_INPTGL: u32 = 5u32;
pub const IMEKMS_NOCOMPOSITION: u32 = 0u32;
pub const IMEKMS_SELECTION: u32 = 2u32;
pub const IMEKMS_TYPECAND: u32 = 7u32;
pub const IMEMENUITEM_STRING_SIZE: u32 = 80u32;
pub const IMEMOUSERET_NOTHANDLED: i32 = -1i32;
pub const IMEMOUSE_LDOWN: u32 = 1u32;
pub const IMEMOUSE_MDOWN: u32 = 4u32;
pub const IMEMOUSE_NONE: u32 = 0u32;
pub const IMEMOUSE_RDOWN: u32 = 2u32;
pub const IMEMOUSE_VERSION: u32 = 255u32;
pub const IMEMOUSE_WDOWN: u32 = 32u32;
pub const IMEMOUSE_WUP: u32 = 16u32;
pub const IMEPADCTRL_CARETBACKSPACE: u32 = 10u32;
pub const IMEPADCTRL_CARETBOTTOM: u32 = 9u32;
pub const IMEPADCTRL_CARETDELETE: u32 = 11u32;
pub const IMEPADCTRL_CARETLEFT: u32 = 6u32;
pub const IMEPADCTRL_CARETRIGHT: u32 = 7u32;
pub const IMEPADCTRL_CARETSET: u32 = 5u32;
pub const IMEPADCTRL_CARETTOP: u32 = 8u32;
pub const IMEPADCTRL_CLEARALL: u32 = 4u32;
pub const IMEPADCTRL_CONVERTALL: u32 = 1u32;
pub const IMEPADCTRL_DETERMINALL: u32 = 2u32;
pub const IMEPADCTRL_DETERMINCHAR: u32 = 3u32;
pub const IMEPADCTRL_INSERTFULLSPACE: u32 = 14u32;
pub const IMEPADCTRL_INSERTHALFSPACE: u32 = 15u32;
pub const IMEPADCTRL_INSERTSPACE: u32 = 13u32;
pub const IMEPADCTRL_OFFIME: u32 = 17u32;
pub const IMEPADCTRL_OFFPRECONVERSION: u32 = 19u32;
pub const IMEPADCTRL_ONIME: u32 = 16u32;
pub const IMEPADCTRL_ONPRECONVERSION: u32 = 18u32;
pub const IMEPADCTRL_PHONETICCANDIDATE: u32 = 20u32;
pub const IMEPADCTRL_PHRASEDELETE: u32 = 12u32;
pub const IMEPADREQ_CHANGESTRING: IME_PAD_REQUEST_FLAGS = 4113u32;
pub const IMEPADREQ_CHANGESTRINGCANDIDATEINFO: u32 = 4111u32;
pub const IMEPADREQ_CHANGESTRINGINFO: u32 = 4115u32;
pub const IMEPADREQ_DELETESTRING: IME_PAD_REQUEST_FLAGS = 4112u32;
pub const IMEPADREQ_FIRST: u32 = 4096u32;
pub const IMEPADREQ_FORCEIMEPADWINDOWSHOW: IME_PAD_REQUEST_FLAGS = 4117u32;
pub const IMEPADREQ_GETAPPLETDATA: u32 = 4106u32;
pub const IMEPADREQ_GETAPPLETUISTYLE: IME_PAD_REQUEST_FLAGS = 4121u32;
pub const IMEPADREQ_GETAPPLHWND: IME_PAD_REQUEST_FLAGS = 4116u32;
pub const IMEPADREQ_GETCOMPOSITIONSTRING: IME_PAD_REQUEST_FLAGS = 4102u32;
pub const IMEPADREQ_GETCOMPOSITIONSTRINGID: u32 = 4109u32;
pub const IMEPADREQ_GETCOMPOSITIONSTRINGINFO: IME_PAD_REQUEST_FLAGS = 4108u32;
pub const IMEPADREQ_GETCONVERSIONSTATUS: IME_PAD_REQUEST_FLAGS = 4126u32;
pub const IMEPADREQ_GETCURRENTIMEINFO: IME_PAD_REQUEST_FLAGS = 4128u32;
pub const IMEPADREQ_GETCURRENTUILANGID: u32 = 4120u32;
pub const IMEPADREQ_GETDEFAULTUILANGID: IME_PAD_REQUEST_FLAGS = 4119u32;
pub const IMEPADREQ_GETSELECTEDSTRING: u32 = 4103u32;
pub const IMEPADREQ_GETVERSION: IME_PAD_REQUEST_FLAGS = 4127u32;
pub const IMEPADREQ_INSERTITEMCANDIDATE: u32 = 4099u32;
pub const IMEPADREQ_INSERTSTRING: IME_PAD_REQUEST_FLAGS = 4097u32;
pub const IMEPADREQ_INSERTSTRINGCANDIDATE: u32 = 4098u32;
pub const IMEPADREQ_INSERTSTRINGCANDIDATEINFO: u32 = 4110u32;
pub const IMEPADREQ_INSERTSTRINGINFO: u32 = 4114u32;
pub const IMEPADREQ_ISAPPLETACTIVE: IME_PAD_REQUEST_FLAGS = 4123u32;
pub const IMEPADREQ_ISIMEPADWINDOWVISIBLE: IME_PAD_REQUEST_FLAGS = 4124u32;
pub const IMEPADREQ_POSTMODALNOTIFY: IME_PAD_REQUEST_FLAGS = 4118u32;
pub const IMEPADREQ_SENDCONTROL: IME_PAD_REQUEST_FLAGS = 4100u32;
pub const IMEPADREQ_SENDKEYCONTROL: u32 = 4101u32;
pub const IMEPADREQ_SETAPPLETDATA: u32 = 4105u32;
pub const IMEPADREQ_SETAPPLETMINMAXSIZE: IME_PAD_REQUEST_FLAGS = 4125u32;
pub const IMEPADREQ_SETAPPLETSIZE: IME_PAD_REQUEST_FLAGS = 4104u32;
pub const IMEPADREQ_SETAPPLETUISTYLE: IME_PAD_REQUEST_FLAGS = 4122u32;
pub const IMEPADREQ_SETTITLEFONT: u32 = 4107u32;
pub const IMEPN_ACTIVATE: u32 = 257u32;
pub const IMEPN_APPLYCAND: u32 = 267u32;
pub const IMEPN_APPLYCANDEX: u32 = 268u32;
pub const IMEPN_CONFIG: u32 = 264u32;
pub const IMEPN_FIRST: u32 = 256u32;
pub const IMEPN_HELP: u32 = 265u32;
pub const IMEPN_HIDE: u32 = 261u32;
pub const IMEPN_INACTIVATE: u32 = 258u32;
pub const IMEPN_QUERYCAND: u32 = 266u32;
pub const IMEPN_SETTINGCHANGED: u32 = 269u32;
pub const IMEPN_SHOW: u32 = 260u32;
pub const IMEPN_SIZECHANGED: u32 = 263u32;
pub const IMEPN_SIZECHANGING: u32 = 262u32;
pub const IMEPN_USER: u32 = 356u32;
pub const IMEVER_0310: u32 = 196618u32;
pub const IMEVER_0400: u32 = 262144u32;
pub const IME_CAND_CODE: u32 = 2u32;
pub const IME_CAND_MEANING: u32 = 3u32;
pub const IME_CAND_RADICAL: u32 = 4u32;
pub const IME_CAND_READ: u32 = 1u32;
pub const IME_CAND_STROKE: u32 = 5u32;
pub const IME_CAND_UNKNOWN: u32 = 0u32;
pub const IME_CHOTKEY_IME_NONIME_TOGGLE: IME_HOTKEY_IDENTIFIER = 16u32;
pub const IME_CHOTKEY_SHAPE_TOGGLE: IME_HOTKEY_IDENTIFIER = 17u32;
pub const IME_CHOTKEY_SYMBOL_TOGGLE: IME_HOTKEY_IDENTIFIER = 18u32;
pub const IME_CMODE_ALPHANUMERIC: IME_CONVERSION_MODE = 0u32;
pub const IME_CMODE_CHARCODE: IME_CONVERSION_MODE = 32u32;
pub const IME_CMODE_CHINESE: IME_CONVERSION_MODE = 1u32;
pub const IME_CMODE_EUDC: IME_CONVERSION_MODE = 512u32;
pub const IME_CMODE_FIXED: IME_CONVERSION_MODE = 2048u32;
pub const IME_CMODE_FULLSHAPE: IME_CONVERSION_MODE = 8u32;
pub const IME_CMODE_HANGEUL: IME_CONVERSION_MODE = 1u32;
pub const IME_CMODE_HANGUL: IME_CONVERSION_MODE = 1u32;
pub const IME_CMODE_HANJACONVERT: IME_CONVERSION_MODE = 64u32;
pub const IME_CMODE_JAPANESE: IME_CONVERSION_MODE = 1u32;
pub const IME_CMODE_KATAKANA: IME_CONVERSION_MODE = 2u32;
pub const IME_CMODE_LANGUAGE: IME_CONVERSION_MODE = 3u32;
pub const IME_CMODE_NATIVE: IME_CONVERSION_MODE = 1u32;
pub const IME_CMODE_NATIVESYMBOL: IME_CONVERSION_MODE = 128u32;
pub const IME_CMODE_NOCONVERSION: IME_CONVERSION_MODE = 256u32;
pub const IME_CMODE_RESERVED: IME_CONVERSION_MODE = 4026531840u32;
pub const IME_CMODE_ROMAN: IME_CONVERSION_MODE = 16u32;
pub const IME_CMODE_SOFTKBD: IME_CONVERSION_MODE = 128u32;
pub const IME_CMODE_SYMBOL: IME_CONVERSION_MODE = 1024u32;
pub const IME_CONFIG_GENERAL: u32 = 1u32;
pub const IME_CONFIG_REGISTERWORD: u32 = 2u32;
pub const IME_CONFIG_SELECTDICTIONARY: u32 = 3u32;
pub const IME_ESC_AUTOMATA: IME_ESCAPE = 4105u32;
pub const IME_ESC_GETHELPFILENAME: IME_ESCAPE = 4107u32;
pub const IME_ESC_GET_EUDC_DICTIONARY: IME_ESCAPE = 4099u32;
pub const IME_ESC_HANJA_MODE: IME_ESCAPE = 4104u32;
pub const IME_ESC_IME_NAME: IME_ESCAPE = 4102u32;
pub const IME_ESC_MAX_KEY: IME_ESCAPE = 4101u32;
pub const IME_ESC_PRIVATE_FIRST: IME_ESCAPE = 2048u32;
pub const IME_ESC_PRIVATE_HOTKEY: IME_ESCAPE = 4106u32;
pub const IME_ESC_PRIVATE_LAST: IME_ESCAPE = 4095u32;
pub const IME_ESC_QUERY_SUPPORT: IME_ESCAPE = 3u32;
pub const IME_ESC_RESERVED_FIRST: IME_ESCAPE = 4u32;
pub const IME_ESC_RESERVED_LAST: IME_ESCAPE = 2047u32;
pub const IME_ESC_SEQUENCE_TO_INTERNAL: IME_ESCAPE = 4097u32;
pub const IME_ESC_SET_EUDC_DICTIONARY: IME_ESCAPE = 4100u32;
pub const IME_ESC_STRING_BUFFER_SIZE: u32 = 80u32;
pub const IME_ESC_SYNC_HOTKEY: IME_ESCAPE = 4103u32;
pub const IME_HOTKEY_DSWITCH_FIRST: u32 = 256u32;
pub const IME_HOTKEY_DSWITCH_LAST: u32 = 287u32;
pub const IME_HOTKEY_PRIVATE_FIRST: u32 = 512u32;
pub const IME_HOTKEY_PRIVATE_LAST: u32 = 543u32;
pub const IME_ITHOTKEY_PREVIOUS_COMPOSITION: IME_HOTKEY_IDENTIFIER = 513u32;
pub const IME_ITHOTKEY_RECONVERTSTRING: IME_HOTKEY_IDENTIFIER = 515u32;
pub const IME_ITHOTKEY_RESEND_RESULTSTR: IME_HOTKEY_IDENTIFIER = 512u32;
pub const IME_ITHOTKEY_UISTYLE_TOGGLE: IME_HOTKEY_IDENTIFIER = 514u32;
pub const IME_JHOTKEY_CLOSE_OPEN: IME_HOTKEY_IDENTIFIER = 48u32;
pub const IME_KHOTKEY_ENGLISH: IME_HOTKEY_IDENTIFIER = 82u32;
pub const IME_KHOTKEY_HANJACONVERT: IME_HOTKEY_IDENTIFIER = 81u32;
pub const IME_KHOTKEY_SHAPE_TOGGLE: IME_HOTKEY_IDENTIFIER = 80u32;
pub const IME_PROP_ACCEPT_WIDE_VKEY: u32 = 32u32;
pub const IME_PROP_AT_CARET: u32 = 65536u32;
pub const IME_PROP_CANDLIST_START_FROM_1: u32 = 262144u32;
pub const IME_PROP_COMPLETE_ON_UNSELECT: u32 = 1048576u32;
pub const IME_PROP_END_UNLOAD: u32 = 1u32;
pub const IME_PROP_IGNORE_UPKEYS: u32 = 4u32;
pub const IME_PROP_KBD_CHAR_FIRST: u32 = 2u32;
pub const IME_PROP_NEED_ALTKEY: u32 = 8u32;
pub const IME_PROP_NO_KEYS_ON_CLOSE: u32 = 16u32;
pub const IME_PROP_SPECIAL_UI: u32 = 131072u32;
pub const IME_PROP_UNICODE: u32 = 524288u32;
pub const IME_REGWORD_STYLE_EUDC: u32 = 1u32;
pub const IME_REGWORD_STYLE_USER_FIRST: u32 = 2147483648u32;
pub const IME_REGWORD_STYLE_USER_LAST: u32 = 4294967295u32;
pub const IME_SMODE_AUTOMATIC: IME_SENTENCE_MODE = 4u32;
pub const IME_SMODE_CONVERSATION: IME_SENTENCE_MODE = 16u32;
pub const IME_SMODE_NONE: IME_SENTENCE_MODE = 0u32;
pub const IME_SMODE_PHRASEPREDICT: IME_SENTENCE_MODE = 8u32;
pub const IME_SMODE_PLAURALCLAUSE: IME_SENTENCE_MODE = 1u32;
pub const IME_SMODE_RESERVED: IME_SENTENCE_MODE = 61440u32;
pub const IME_SMODE_SINGLECONVERT: IME_SENTENCE_MODE = 2u32;
pub const IME_SYSINFO_WINLOGON: u32 = 1u32;
pub const IME_THOTKEY_IME_NONIME_TOGGLE: IME_HOTKEY_IDENTIFIER = 112u32;
pub const IME_THOTKEY_SHAPE_TOGGLE: IME_HOTKEY_IDENTIFIER = 113u32;
pub const IME_THOTKEY_SYMBOL_TOGGLE: IME_HOTKEY_IDENTIFIER = 114u32;
pub const IME_UI_CLASS_NAME_SIZE: u32 = 16u32;
pub const IMFT_RADIOCHECK: u32 = 1u32;
pub const IMFT_SEPARATOR: u32 = 2u32;
pub const IMFT_SUBMENU: u32 = 4u32;
pub const IMMGWLP_IMC: u32 = 0u32;
pub const IMMGWL_IMC: u32 = 0u32;
pub const IMM_ERROR_GENERAL: i32 = -2i32;
pub const IMM_ERROR_NODATA: i32 = -1i32;
pub const IMN_CHANGECANDIDATE: u32 = 3u32;
pub const IMN_CLOSECANDIDATE: u32 = 4u32;
pub const IMN_CLOSESTATUSWINDOW: u32 = 1u32;
pub const IMN_GUIDELINE: u32 = 13u32;
pub const IMN_OPENCANDIDATE: u32 = 5u32;
pub const IMN_OPENSTATUSWINDOW: u32 = 2u32;
pub const IMN_PRIVATE: u32 = 14u32;
pub const IMN_SETCANDIDATEPOS: u32 = 9u32;
pub const IMN_SETCOMPOSITIONFONT: u32 = 10u32;
pub const IMN_SETCOMPOSITIONWINDOW: u32 = 11u32;
pub const IMN_SETCONVERSIONMODE: u32 = 6u32;
pub const IMN_SETOPENSTATUS: u32 = 8u32;
pub const IMN_SETSENTENCEMODE: u32 = 7u32;
pub const IMN_SETSTATUSWINDOWPOS: u32 = 12u32;
pub const IMN_SOFTKBDDESTROYED: u32 = 17u32;
pub const IMR_CANDIDATEWINDOW: u32 = 2u32;
pub const IMR_COMPOSITIONFONT: u32 = 3u32;
pub const IMR_COMPOSITIONWINDOW: u32 = 1u32;
pub const IMR_CONFIRMRECONVERTSTRING: u32 = 5u32;
pub const IMR_DOCUMENTFEED: u32 = 7u32;
pub const IMR_QUERYCHARPOSITION: u32 = 6u32;
pub const IMR_RECONVERTSTRING: u32 = 4u32;
pub const INFOMASK_APPLY_CAND: u32 = 2u32;
pub const INFOMASK_APPLY_CAND_EX: u32 = 4u32;
pub const INFOMASK_BLOCK_CAND: u32 = 262144u32;
pub const INFOMASK_HIDE_CAND: u32 = 131072u32;
pub const INFOMASK_NONE: u32 = 0u32;
pub const INFOMASK_QUERY_CAND: u32 = 1u32;
pub const INFOMASK_STRING_FIX: u32 = 65536u32;
pub const INIT_COMPFORM: u32 = 16u32;
pub const INIT_CONVERSION: u32 = 2u32;
pub const INIT_LOGFONT: u32 = 8u32;
pub const INIT_SENTENCE: u32 = 4u32;
pub const INIT_SOFTKBDPOS: u32 = 32u32;
pub const INIT_STATUSWNDPOS: u32 = 1u32;
pub const IPACFG_CATEGORY: i32 = 262144i32;
pub const IPACFG_HELP: i32 = 2i32;
pub const IPACFG_LANG: i32 = 16i32;
pub const IPACFG_NONE: i32 = 0i32;
pub const IPACFG_PROPERTY: i32 = 1i32;
pub const IPACFG_TITLE: i32 = 65536i32;
pub const IPACFG_TITLEFONTFACE: i32 = 131072i32;
pub const IPACID_CHARLIST: u32 = 9u32;
pub const IPACID_EPWING: u32 = 7u32;
pub const IPACID_HANDWRITING: u32 = 2u32;
pub const IPACID_NONE: u32 = 0u32;
pub const IPACID_OCR: u32 = 8u32;
pub const IPACID_RADICALSEARCH: u32 = 4u32;
pub const IPACID_SOFTKEY: u32 = 1u32;
pub const IPACID_STROKESEARCH: u32 = 3u32;
pub const IPACID_SYMBOLSEARCH: u32 = 5u32;
pub const IPACID_USER: u32 = 256u32;
pub const IPACID_VOICE: u32 = 6u32;
pub const IPAWS_ENABLED: i32 = 1i32;
pub const IPAWS_HORIZONTALFIXED: i32 = 512i32;
pub const IPAWS_MAXHEIGHTFIXED: i32 = 8192i32;
pub const IPAWS_MAXSIZEFIXED: i32 = 12288i32;
pub const IPAWS_MAXWIDTHFIXED: i32 = 4096i32;
pub const IPAWS_MINHEIGHTFIXED: i32 = 131072i32;
pub const IPAWS_MINSIZEFIXED: i32 = 196608i32;
pub const IPAWS_MINWIDTHFIXED: i32 = 65536i32;
pub const IPAWS_SIZEFIXED: i32 = 768i32;
pub const IPAWS_SIZINGNOTIFY: i32 = 4i32;
pub const IPAWS_VERTICALFIXED: i32 = 256i32;
pub const ISC_SHOWUIALL: u32 = 3221225487u32;
pub const ISC_SHOWUIALLCANDIDATEWINDOW: u32 = 15u32;
pub const ISC_SHOWUICANDIDATEWINDOW: u32 = 1u32;
pub const ISC_SHOWUICOMPOSITIONWINDOW: u32 = 2147483648u32;
pub const ISC_SHOWUIGUIDELINE: u32 = 1073741824u32;
pub const JPOS_1DAN: u32 = 213u32;
pub const JPOS_4DAN_HA: u32 = 212u32;
pub const JPOS_5DAN_AWA: u32 = 200u32;
pub const JPOS_5DAN_AWAUON: u32 = 209u32;
pub const JPOS_5DAN_BA: u32 = 206u32;
pub const JPOS_5DAN_GA: u32 = 202u32;
pub const JPOS_5DAN_KA: u32 = 201u32;
pub const JPOS_5DAN_KASOKUON: u32 = 210u32;
pub const JPOS_5DAN_MA: u32 = 207u32;
pub const JPOS_5DAN_NA: u32 = 205u32;
pub const JPOS_5DAN_RA: u32 = 208u32;
pub const JPOS_5DAN_RAHEN: u32 = 211u32;
pub const JPOS_5DAN_SA: u32 = 203u32;
pub const JPOS_5DAN_TA: u32 = 204u32;
pub const JPOS_BUPPIN: u32 = 122u32;
pub const JPOS_CHIMEI: u32 = 109u32;
pub const JPOS_CHIMEI_EKI: u32 = 117u32;
pub const JPOS_CHIMEI_GUN: u32 = 112u32;
pub const JPOS_CHIMEI_KEN: u32 = 111u32;
pub const JPOS_CHIMEI_KU: u32 = 113u32;
pub const JPOS_CHIMEI_KUNI: u32 = 110u32;
pub const JPOS_CHIMEI_MACHI: u32 = 115u32;
pub const JPOS_CHIMEI_MURA: u32 = 116u32;
pub const JPOS_CHIMEI_SHI: u32 = 114u32;
pub const JPOS_CLOSEBRACE: u32 = 911u32;
pub const JPOS_DAIMEISHI: u32 = 123u32;
pub const JPOS_DAIMEISHI_NINSHOU: u32 = 124u32;
pub const JPOS_DAIMEISHI_SHIJI: u32 = 125u32;
pub const JPOS_DOKURITSUGO: u32 = 903u32;
pub const JPOS_EIJI: u32 = 906u32;
pub const JPOS_FUKUSHI: u32 = 500u32;
pub const JPOS_FUKUSHI_DA: u32 = 504u32;
pub const JPOS_FUKUSHI_NANO: u32 = 503u32;
pub const JPOS_FUKUSHI_NI: u32 = 502u32;
pub const JPOS_FUKUSHI_SAHEN: u32 = 501u32;
pub const JPOS_FUKUSHI_TO: u32 = 505u32;
pub const JPOS_FUKUSHI_TOSURU: u32 = 506u32;
pub const JPOS_FUTEIGO: u32 = 904u32;
pub const JPOS_HUKUSIMEISHI: u32 = 104u32;
pub const JPOS_JINMEI: u32 = 106u32;
pub const JPOS_JINMEI_MEI: u32 = 108u32;
pub const JPOS_JINMEI_SEI: u32 = 107u32;
pub const JPOS_KANDOUSHI: u32 = 670u32;
pub const JPOS_KANJI: u32 = 909u32;
pub const JPOS_KANYOUKU: u32 = 902u32;
pub const JPOS_KAZU: u32 = 126u32;
pub const JPOS_KAZU_SURYOU: u32 = 127u32;
pub const JPOS_KAZU_SUSHI: u32 = 128u32;
pub const JPOS_KEIDOU: u32 = 400u32;
pub const JPOS_KEIDOU_GARU: u32 = 403u32;
pub const JPOS_KEIDOU_NO: u32 = 401u32;
pub const JPOS_KEIDOU_TARU: u32 = 402u32;
pub const JPOS_KEIYOU: u32 = 300u32;
pub const JPOS_KEIYOU_GARU: u32 = 301u32;
pub const JPOS_KEIYOU_GE: u32 = 302u32;
pub const JPOS_KEIYOU_ME: u32 = 303u32;
pub const JPOS_KEIYOU_U: u32 = 305u32;
pub const JPOS_KEIYOU_YUU: u32 = 304u32;
pub const JPOS_KENCHIKU: u32 = 121u32;
pub const JPOS_KIGOU: u32 = 905u32;
pub const JPOS_KURU_KI: u32 = 219u32;
pub const JPOS_KURU_KITA: u32 = 220u32;
pub const JPOS_KURU_KITARA: u32 = 221u32;
pub const JPOS_KURU_KITARI: u32 = 222u32;
pub const JPOS_KURU_KITAROU: u32 = 223u32;
pub const JPOS_KURU_KITE: u32 = 224u32;
pub const JPOS_KURU_KO: u32 = 226u32;
pub const JPOS_KURU_KOI: u32 = 227u32;
pub const JPOS_KURU_KOYOU: u32 = 228u32;
pub const JPOS_KURU_KUREBA: u32 = 225u32;
pub const JPOS_KUTEN: u32 = 907u32;
pub const JPOS_MEISA_KEIDOU: u32 = 105u32;
pub const JPOS_MEISHI_FUTSU: u32 = 100u32;
pub const JPOS_MEISHI_KEIYOUDOUSHI: u32 = 103u32;
pub const JPOS_MEISHI_SAHEN: u32 = 101u32;
pub const JPOS_MEISHI_ZAHEN: u32 = 102u32;
pub const JPOS_OPENBRACE: u32 = 910u32;
pub const JPOS_RENTAISHI: u32 = 600u32;
pub const JPOS_RENTAISHI_SHIJI: u32 = 601u32;
pub const JPOS_RENYOU_SETSUBI: u32 = 826u32;
pub const JPOS_SETSUBI: u32 = 800u32;
pub const JPOS_SETSUBI_CHIMEI: u32 = 811u32;
pub const JPOS_SETSUBI_CHOU: u32 = 818u32;
pub const JPOS_SETSUBI_CHU: u32 = 804u32;
pub const JPOS_SETSUBI_DONO: u32 = 835u32;
pub const JPOS_SETSUBI_EKI: u32 = 821u32;
pub const JPOS_SETSUBI_FU: u32 = 805u32;
pub const JPOS_SETSUBI_FUKUSU: u32 = 836u32;
pub const JPOS_SETSUBI_GUN: u32 = 814u32;
pub const JPOS_SETSUBI_JIKAN: u32 = 829u32;
pub const JPOS_SETSUBI_JIKANPLUS: u32 = 830u32;
pub const JPOS_SETSUBI_JINMEI: u32 = 810u32;
pub const JPOS_SETSUBI_JOSUSHI: u32 = 827u32;
pub const JPOS_SETSUBI_JOSUSHIPLUS: u32 = 828u32;
pub const JPOS_SETSUBI_KA: u32 = 803u32;
pub const JPOS_SETSUBI_KATA: u32 = 808u32;
pub const JPOS_SETSUBI_KEN: u32 = 813u32;
pub const JPOS_SETSUBI_KENCHIKU: u32 = 825u32;
pub const JPOS_SETSUBI_KU: u32 = 815u32;
pub const JPOS_SETSUBI_KUN: u32 = 833u32;
pub const JPOS_SETSUBI_KUNI: u32 = 812u32;
pub const JPOS_SETSUBI_MACHI: u32 = 817u32;
pub const JPOS_SETSUBI_MEISHIRENDAKU: u32 = 809u32;
pub const JPOS_SETSUBI_MURA: u32 = 819u32;
pub const JPOS_SETSUBI_RA: u32 = 838u32;
pub const JPOS_SETSUBI_RYU: u32 = 806u32;
pub const JPOS_SETSUBI_SAMA: u32 = 834u32;
pub const JPOS_SETSUBI_SAN: u32 = 832u32;
pub const JPOS_SETSUBI_SEI: u32 = 802u32;
pub const JPOS_SETSUBI_SHAMEI: u32 = 823u32;
pub const JPOS_SETSUBI_SHI: u32 = 816u32;
pub const JPOS_SETSUBI_SON: u32 = 820u32;
pub const JPOS_SETSUBI_SONOTA: u32 = 822u32;
pub const JPOS_SETSUBI_SOSHIKI: u32 = 824u32;
pub const JPOS_SETSUBI_TACHI: u32 = 837u32;
pub const JPOS_SETSUBI_TEINEI: u32 = 831u32;
pub const JPOS_SETSUBI_TEKI: u32 = 801u32;
pub const JPOS_SETSUBI_YOU: u32 = 807u32;
pub const JPOS_SETSUZOKUSHI: u32 = 650u32;
pub const JPOS_SETTOU: u32 = 700u32;
pub const JPOS_SETTOU_CHIMEI: u32 = 710u32;
pub const JPOS_SETTOU_CHOUTAN: u32 = 707u32;
pub const JPOS_SETTOU_DAISHOU: u32 = 705u32;
pub const JPOS_SETTOU_FUKU: u32 = 703u32;
pub const JPOS_SETTOU_JINMEI: u32 = 709u32;
pub const JPOS_SETTOU_JOSUSHI: u32 = 712u32;
pub const JPOS_SETTOU_KAKU: u32 = 701u32;
pub const JPOS_SETTOU_KOUTEI: u32 = 706u32;
pub const JPOS_SETTOU_MI: u32 = 704u32;
pub const JPOS_SETTOU_SAI: u32 = 702u32;
pub const JPOS_SETTOU_SHINKYU: u32 = 708u32;
pub const JPOS_SETTOU_SONOTA: u32 = 711u32;
pub const JPOS_SETTOU_TEINEI_GO: u32 = 714u32;
pub const JPOS_SETTOU_TEINEI_O: u32 = 713u32;
pub const JPOS_SETTOU_TEINEI_ON: u32 = 715u32;
pub const JPOS_SHAMEI: u32 = 119u32;
pub const JPOS_SONOTA: u32 = 118u32;
pub const JPOS_SOSHIKI: u32 = 120u32;
pub const JPOS_SURU_SA: u32 = 229u32;
pub const JPOS_SURU_SE: u32 = 238u32;
pub const JPOS_SURU_SEYO: u32 = 239u32;
pub const JPOS_SURU_SI: u32 = 230u32;
pub const JPOS_SURU_SIATRI: u32 = 233u32;
pub const JPOS_SURU_SITA: u32 = 231u32;
pub const JPOS_SURU_SITARA: u32 = 232u32;
pub const JPOS_SURU_SITAROU: u32 = 234u32;
pub const JPOS_SURU_SITE: u32 = 235u32;
pub const JPOS_SURU_SIYOU: u32 = 236u32;
pub const JPOS_SURU_SUREBA: u32 = 237u32;
pub const JPOS_TANKANJI: u32 = 900u32;
pub const JPOS_TANKANJI_KAO: u32 = 901u32;
pub const JPOS_TANSHUKU: u32 = 913u32;
pub const JPOS_TOKUSHU_KAHEN: u32 = 214u32;
pub const JPOS_TOKUSHU_NAHEN: u32 = 218u32;
pub const JPOS_TOKUSHU_SAHEN: u32 = 216u32;
pub const JPOS_TOKUSHU_SAHENSURU: u32 = 215u32;
pub const JPOS_TOKUSHU_ZAHEN: u32 = 217u32;
pub const JPOS_TOUTEN: u32 = 908u32;
pub const JPOS_UNDEFINED: u32 = 0u32;
pub const JPOS_YOKUSEI: u32 = 912u32;
pub const MAX_APPLETTITLE: u32 = 64u32;
pub const MAX_FONTFACE: u32 = 32u32;
pub const MODEBIASMODE_DEFAULT: u32 = 0u32;
pub const MODEBIASMODE_DIGIT: u32 = 4u32;
pub const MODEBIASMODE_FILENAME: u32 = 1u32;
pub const MODEBIASMODE_READING: u32 = 2u32;
pub const MODEBIAS_GETVALUE: u32 = 2u32;
pub const MODEBIAS_GETVERSION: u32 = 0u32;
pub const MODEBIAS_SETVALUE: u32 = 1u32;
pub const MOD_IGNORE_ALL_MODIFIER: u32 = 1024u32;
pub const MOD_LEFT: u32 = 32768u32;
pub const MOD_ON_KEYUP: u32 = 2048u32;
pub const MOD_RIGHT: u32 = 16384u32;
pub const NI_CHANGECANDIDATELIST: NOTIFY_IME_ACTION = 19u32;
pub const NI_CLOSECANDIDATE: NOTIFY_IME_ACTION = 17u32;
pub const NI_COMPOSITIONSTR: NOTIFY_IME_ACTION = 21u32;
pub const NI_CONTEXTUPDATED: u32 = 3u32;
pub const NI_FINALIZECONVERSIONRESULT: u32 = 20u32;
pub const NI_IMEMENUSELECTED: NOTIFY_IME_ACTION = 24u32;
pub const NI_OPENCANDIDATE: NOTIFY_IME_ACTION = 16u32;
pub const NI_SELECTCANDIDATESTR: NOTIFY_IME_ACTION = 18u32;
pub const NI_SETCANDIDATE_PAGESIZE: NOTIFY_IME_ACTION = 23u32;
pub const NI_SETCANDIDATE_PAGESTART: NOTIFY_IME_ACTION = 22u32;
pub const POS_UNDEFINED: u32 = 0u32;
pub const RECONVOPT_NONE: u32 = 0u32;
pub const RECONVOPT_USECANCELNOTIFY: u32 = 1u32;
pub const RWM_CHGKEYMAP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEChangeKeyMap");
pub const RWM_DOCUMENTFEED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEDocumentFeed");
pub const RWM_KEYMAP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEKeyMap");
pub const RWM_MODEBIAS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEModeBias");
pub const RWM_MOUSE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEMouseOperation");
pub const RWM_NTFYKEYMAP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMENotifyKeyMap");
pub const RWM_QUERYPOSITION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEQueryPosition");
pub const RWM_RECONVERT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEReconvert");
pub const RWM_RECONVERTOPTIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEReconvertOptions");
pub const RWM_RECONVERTREQUEST: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEReconvertRequest");
pub const RWM_SERVICE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEService");
pub const RWM_SHOWIMEPAD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEShowImePad");
pub const RWM_UIREADY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIMEUIReady");
pub const SCS_CAP_COMPSTR: u32 = 1u32;
pub const SCS_CAP_MAKEREAD: u32 = 2u32;
pub const SCS_CAP_SETRECONVERTSTRING: u32 = 4u32;
pub const SCS_CHANGEATTR: SET_COMPOSITION_STRING_TYPE = 18u32;
pub const SCS_CHANGECLAUSE: SET_COMPOSITION_STRING_TYPE = 36u32;
pub const SCS_QUERYRECONVERTSTRING: SET_COMPOSITION_STRING_TYPE = 131072u32;
pub const SCS_SETRECONVERTSTRING: SET_COMPOSITION_STRING_TYPE = 65536u32;
pub const SCS_SETSTR: SET_COMPOSITION_STRING_TYPE = 9u32;
pub const SELECT_CAP_CONVERSION: u32 = 1u32;
pub const SELECT_CAP_SENTENCE: u32 = 2u32;
pub const SHOWIMEPAD_CATEGORY: u32 = 1u32;
pub const SHOWIMEPAD_DEFAULT: u32 = 0u32;
pub const SHOWIMEPAD_GUID: u32 = 2u32;
pub const SOFTKEYBOARD_TYPE_C1: u32 = 2u32;
pub const SOFTKEYBOARD_TYPE_T1: u32 = 1u32;
pub const STYLE_DESCRIPTION_SIZE: u32 = 32u32;
pub const UI_CAP_2700: u32 = 1u32;
pub const UI_CAP_ROT90: u32 = 2u32;
pub const UI_CAP_ROTANY: u32 = 4u32;
pub const UI_CAP_SOFTKBD: u32 = 65536u32;
pub const VERSION_DOCUMENTFEED: u32 = 1u32;
pub const VERSION_ID_CHINESE_SIMPLIFIED: u32 = 134217728u32;
pub const VERSION_ID_CHINESE_TRADITIONAL: u32 = 67108864u32;
pub const VERSION_ID_JAPANESE: u32 = 16777216u32;
pub const VERSION_ID_KOREAN: u32 = 33554432u32;
pub const VERSION_MODEBIAS: u32 = 1u32;
pub const VERSION_MOUSE_OPERATION: u32 = 1u32;
pub const VERSION_QUERYPOSITION: u32 = 1u32;
pub const VERSION_RECONVERSION: u32 = 1u32;
pub const cbCommentMax: u32 = 256u32;
pub const szImeChina: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIME.China");
pub const szImeJapan: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIME.Japan");
pub const szImeKorea: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIME.Korea");
pub const szImeTaiwan: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSIME.Taiwan");
pub const wchPrivate1: u32 = 57344u32;
pub type GET_CONVERSION_LIST_FLAG = u32;
pub type GET_GUIDE_LINE_TYPE = u32;
pub type IMEFMT = i32;
pub type IMEREG = i32;
pub type IMEREL = i32;
pub type IMEUCT = i32;
pub type IME_COMPOSITION_STRING = u32;
pub type IME_CONVERSION_MODE = u32;
pub type IME_ESCAPE = u32;
pub type IME_HOTKEY_IDENTIFIER = u32;
pub type IME_PAD_REQUEST_FLAGS = u32;
pub type IME_SENTENCE_MODE = u32;
pub type NOTIFY_IME_ACTION = u32;
pub type NOTIFY_IME_INDEX = u32;
pub type SET_COMPOSITION_STRING_TYPE = u32;
#[repr(C)]
pub struct APPLETIDLIST {
    pub count: i32,
    pub pIIDList: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for APPLETIDLIST {}
impl ::core::clone::Clone for APPLETIDLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct APPLYCANDEXPARAM {
    pub dwSize: u32,
    pub lpwstrDisplay: ::windows_sys::core::PWSTR,
    pub lpwstrReading: ::windows_sys::core::PWSTR,
    pub dwReserved: u32,
}
impl ::core::marker::Copy for APPLYCANDEXPARAM {}
impl ::core::clone::Clone for APPLYCANDEXPARAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CANDIDATEFORM {
    pub dwIndex: u32,
    pub dwStyle: u32,
    pub ptCurrentPos: super::super::super::Foundation::POINT,
    pub rcArea: super::super::super::Foundation::RECT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CANDIDATEFORM {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CANDIDATEFORM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CANDIDATEINFO {
    pub dwSize: u32,
    pub dwCount: u32,
    pub dwOffset: [u32; 32],
    pub dwPrivateSize: u32,
    pub dwPrivateOffset: u32,
}
impl ::core::marker::Copy for CANDIDATEINFO {}
impl ::core::clone::Clone for CANDIDATEINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CANDIDATELIST {
    pub dwSize: u32,
    pub dwStyle: u32,
    pub dwCount: u32,
    pub dwSelection: u32,
    pub dwPageStart: u32,
    pub dwPageSize: u32,
    pub dwOffset: [u32; 1],
}
impl ::core::marker::Copy for CANDIDATELIST {}
impl ::core::clone::Clone for CANDIDATELIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct COMPOSITIONFORM {
    pub dwStyle: u32,
    pub ptCurrentPos: super::super::super::Foundation::POINT,
    pub rcArea: super::super::super::Foundation::RECT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for COMPOSITIONFORM {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for COMPOSITIONFORM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COMPOSITIONSTRING {
    pub dwSize: u32,
    pub dwCompReadAttrLen: u32,
    pub dwCompReadAttrOffset: u32,
    pub dwCompReadClauseLen: u32,
    pub dwCompReadClauseOffset: u32,
    pub dwCompReadStrLen: u32,
    pub dwCompReadStrOffset: u32,
    pub dwCompAttrLen: u32,
    pub dwCompAttrOffset: u32,
    pub dwCompClauseLen: u32,
    pub dwCompClauseOffset: u32,
    pub dwCompStrLen: u32,
    pub dwCompStrOffset: u32,
    pub dwCursorPos: u32,
    pub dwDeltaStart: u32,
    pub dwResultReadClauseLen: u32,
    pub dwResultReadClauseOffset: u32,
    pub dwResultReadStrLen: u32,
    pub dwResultReadStrOffset: u32,
    pub dwResultClauseLen: u32,
    pub dwResultClauseOffset: u32,
    pub dwResultStrLen: u32,
    pub dwResultStrOffset: u32,
    pub dwPrivateSize: u32,
    pub dwPrivateOffset: u32,
}
impl ::core::marker::Copy for COMPOSITIONSTRING {}
impl ::core::clone::Clone for COMPOSITIONSTRING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GUIDELINE {
    pub dwSize: u32,
    pub dwLevel: u32,
    pub dwIndex: u32,
    pub dwStrLen: u32,
    pub dwStrOffset: u32,
    pub dwPrivateSize: u32,
    pub dwPrivateOffset: u32,
}
impl ::core::marker::Copy for GUIDELINE {}
impl ::core::clone::Clone for GUIDELINE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
pub struct IMEAPPLETCFG {
    pub dwConfig: u32,
    pub wchTitle: [u16; 64],
    pub wchTitleFontFace: [u16; 32],
    pub dwCharSet: u32,
    pub iCategory: i32,
    pub hIcon: super::super::WindowsAndMessaging::HICON,
    pub langID: u16,
    pub dummy: u16,
    pub lReserved1: super::super::super::Foundation::LPARAM,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::marker::Copy for IMEAPPLETCFG {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::clone::Clone for IMEAPPLETCFG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMEAPPLETUI {
    pub hwnd: super::super::super::Foundation::HWND,
    pub dwStyle: u32,
    pub width: i32,
    pub height: i32,
    pub minWidth: i32,
    pub minHeight: i32,
    pub maxWidth: i32,
    pub maxHeight: i32,
    pub lReserved1: super::super::super::Foundation::LPARAM,
    pub lReserved2: super::super::super::Foundation::LPARAM,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMEAPPLETUI {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMEAPPLETUI {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMECHARINFO {
    pub wch: u16,
    pub dwCharInfo: u32,
}
impl ::core::marker::Copy for IMECHARINFO {}
impl ::core::clone::Clone for IMECHARINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMECHARPOSITION {
    pub dwSize: u32,
    pub dwCharPos: u32,
    pub pt: super::super::super::Foundation::POINT,
    pub cLineHeight: u32,
    pub rcDocument: super::super::super::Foundation::RECT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMECHARPOSITION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMECHARPOSITION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMECOMPOSITIONSTRINGINFO {
    pub iCompStrLen: i32,
    pub iCaretPos: i32,
    pub iEditStart: i32,
    pub iEditLen: i32,
    pub iTargetStart: i32,
    pub iTargetLen: i32,
}
impl ::core::marker::Copy for IMECOMPOSITIONSTRINGINFO {}
impl ::core::clone::Clone for IMECOMPOSITIONSTRINGINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMEDLG {
    pub cbIMEDLG: i32,
    pub hwnd: super::super::super::Foundation::HWND,
    pub lpwstrWord: ::windows_sys::core::PWSTR,
    pub nTabId: i32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMEDLG {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMEDLG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IMEDP {
    pub wrdModifier: IMEWRD,
    pub wrdModifiee: IMEWRD,
    pub relID: IMEREL,
}
impl ::core::marker::Copy for IMEDP {}
impl ::core::clone::Clone for IMEDP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMEFAREASTINFO {
    pub dwSize: u32,
    pub dwType: u32,
    pub dwData: [u32; 1],
}
impl ::core::marker::Copy for IMEFAREASTINFO {}
impl ::core::clone::Clone for IMEFAREASTINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMEINFO {
    pub dwPrivateDataSize: u32,
    pub fdwProperty: u32,
    pub fdwConversionCaps: u32,
    pub fdwSentenceCaps: u32,
    pub fdwUICaps: u32,
    pub fdwSCSCaps: u32,
    pub fdwSelectCaps: u32,
}
impl ::core::marker::Copy for IMEINFO {}
impl ::core::clone::Clone for IMEINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMEITEM {
    pub cbSize: i32,
    pub iType: i32,
    pub lpItemData: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for IMEITEM {}
impl ::core::clone::Clone for IMEITEM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMEITEMCANDIDATE {
    pub uCount: u32,
    pub imeItem: [IMEITEM; 1],
}
impl ::core::marker::Copy for IMEITEMCANDIDATE {}
impl ::core::clone::Clone for IMEITEMCANDIDATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Globalization\"`"]
#[cfg(feature = "Win32_Globalization")]
pub struct IMEKMS {
    pub cbSize: i32,
    pub hIMC: super::super::super::Globalization::HIMC,
    pub cKeyList: u32,
    pub pKeyList: *mut IMEKMSKEY,
}
#[cfg(feature = "Win32_Globalization")]
impl ::core::marker::Copy for IMEKMS {}
#[cfg(feature = "Win32_Globalization")]
impl ::core::clone::Clone for IMEKMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IMEKMSFUNCDESC {
    pub cbSize: i32,
    pub idLang: u16,
    pub dwControl: u32,
    pub pwszDescription: [u16; 128],
}
impl ::core::marker::Copy for IMEKMSFUNCDESC {}
impl ::core::clone::Clone for IMEKMSFUNCDESC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMEKMSINIT {
    pub cbSize: i32,
    pub hWnd: super::super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMEKMSINIT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMEKMSINIT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Globalization\"`"]
#[cfg(feature = "Win32_Globalization")]
pub struct IMEKMSINVK {
    pub cbSize: i32,
    pub hIMC: super::super::super::Globalization::HIMC,
    pub dwControl: u32,
}
#[cfg(feature = "Win32_Globalization")]
impl ::core::marker::Copy for IMEKMSINVK {}
#[cfg(feature = "Win32_Globalization")]
impl ::core::clone::Clone for IMEKMSINVK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IMEKMSKEY {
    pub dwStatus: u32,
    pub dwCompStatus: u32,
    pub dwVKEY: u32,
    pub Anonymous1: IMEKMSKEY_0,
    pub Anonymous2: IMEKMSKEY_1,
}
impl ::core::marker::Copy for IMEKMSKEY {}
impl ::core::clone::Clone for IMEKMSKEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union IMEKMSKEY_0 {
    pub dwControl: u32,
    pub dwNotUsed: u32,
}
impl ::core::marker::Copy for IMEKMSKEY_0 {}
impl ::core::clone::Clone for IMEKMSKEY_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union IMEKMSKEY_1 {
    pub pwszDscr: [u16; 31],
    pub pwszNoUse: [u16; 31],
}
impl ::core::marker::Copy for IMEKMSKEY_1 {}
impl ::core::clone::Clone for IMEKMSKEY_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Globalization\"`"]
#[cfg(feature = "Win32_Globalization")]
pub struct IMEKMSKMP {
    pub cbSize: i32,
    pub hIMC: super::super::super::Globalization::HIMC,
    pub idLang: u16,
    pub wVKStart: u16,
    pub wVKEnd: u16,
    pub cKeyList: i32,
    pub pKeyList: *mut IMEKMSKEY,
}
#[cfg(feature = "Win32_Globalization")]
impl ::core::marker::Copy for IMEKMSKMP {}
#[cfg(feature = "Win32_Globalization")]
impl ::core::clone::Clone for IMEKMSKMP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
pub struct IMEKMSNTFY {
    pub cbSize: i32,
    pub hIMC: super::super::super::Globalization::HIMC,
    pub fSelect: super::super::super::Foundation::BOOL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
impl ::core::marker::Copy for IMEKMSNTFY {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
impl ::core::clone::Clone for IMEKMSNTFY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct IMEMENUITEMINFOA {
    pub cbSize: u32,
    pub fType: u32,
    pub fState: u32,
    pub wID: u32,
    pub hbmpChecked: super::super::super::Graphics::Gdi::HBITMAP,
    pub hbmpUnchecked: super::super::super::Graphics::Gdi::HBITMAP,
    pub dwItemData: u32,
    pub szString: [u8; 80],
    pub hbmpItem: super::super::super::Graphics::Gdi::HBITMAP,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for IMEMENUITEMINFOA {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for IMEMENUITEMINFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct IMEMENUITEMINFOW {
    pub cbSize: u32,
    pub fType: u32,
    pub fState: u32,
    pub wID: u32,
    pub hbmpChecked: super::super::super::Graphics::Gdi::HBITMAP,
    pub hbmpUnchecked: super::super::super::Graphics::Gdi::HBITMAP,
    pub dwItemData: u32,
    pub szString: [u16; 80],
    pub hbmpItem: super::super::super::Graphics::Gdi::HBITMAP,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for IMEMENUITEMINFOW {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for IMEMENUITEMINFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IMESHF {
    pub cbShf: u16,
    pub verDic: u16,
    pub szTitle: [u8; 48],
    pub szDescription: [u8; 256],
    pub szCopyright: [u8; 128],
}
impl ::core::marker::Copy for IMESHF {}
impl ::core::clone::Clone for IMESHF {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMESTRINGCANDIDATE {
    pub uCount: u32,
    pub lpwstr: [::windows_sys::core::PWSTR; 1],
}
impl ::core::marker::Copy for IMESTRINGCANDIDATE {}
impl ::core::clone::Clone for IMESTRINGCANDIDATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMESTRINGCANDIDATEINFO {
    pub dwFarEastId: u32,
    pub lpFarEastInfo: *mut IMEFAREASTINFO,
    pub fInfoMask: u32,
    pub iSelIndex: i32,
    pub uCount: u32,
    pub lpwstr: [::windows_sys::core::PWSTR; 1],
}
impl ::core::marker::Copy for IMESTRINGCANDIDATEINFO {}
impl ::core::clone::Clone for IMESTRINGCANDIDATEINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMESTRINGINFO {
    pub dwFarEastId: u32,
    pub lpwstr: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for IMESTRINGINFO {}
impl ::core::clone::Clone for IMESTRINGINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IMEWRD {
    pub pwchReading: ::windows_sys::core::PWSTR,
    pub pwchDisplay: ::windows_sys::core::PWSTR,
    pub Anonymous: IMEWRD_0,
    pub rgulAttrs: [u32; 2],
    pub cbComment: i32,
    pub uct: IMEUCT,
    pub pvComment: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for IMEWRD {}
impl ::core::clone::Clone for IMEWRD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union IMEWRD_0 {
    pub ulPos: u32,
    pub Anonymous: IMEWRD_0_0,
}
impl ::core::marker::Copy for IMEWRD_0 {}
impl ::core::clone::Clone for IMEWRD_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IMEWRD_0_0 {
    pub nPos1: u16,
    pub nPos2: u16,
}
impl ::core::marker::Copy for IMEWRD_0_0 {}
impl ::core::clone::Clone for IMEWRD_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
pub struct INPUTCONTEXT {
    pub hWnd: super::super::super::Foundation::HWND,
    pub fOpen: super::super::super::Foundation::BOOL,
    pub ptStatusWndPos: super::super::super::Foundation::POINT,
    pub ptSoftKbdPos: super::super::super::Foundation::POINT,
    pub fdwConversion: u32,
    pub fdwSentence: u32,
    pub lfFont: INPUTCONTEXT_0,
    pub cfCompForm: COMPOSITIONFORM,
    pub cfCandForm: [CANDIDATEFORM; 4],
    pub hCompStr: super::super::super::Globalization::HIMCC,
    pub hCandInfo: super::super::super::Globalization::HIMCC,
    pub hGuideLine: super::super::super::Globalization::HIMCC,
    pub hPrivate: super::super::super::Globalization::HIMCC,
    pub dwNumMsgBuf: u32,
    pub hMsgBuf: super::super::super::Globalization::HIMCC,
    pub fdwInit: u32,
    pub dwReserve: [u32; 3],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for INPUTCONTEXT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for INPUTCONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
pub union INPUTCONTEXT_0 {
    pub A: super::super::super::Graphics::Gdi::LOGFONTA,
    pub W: super::super::super::Graphics::Gdi::LOGFONTW,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for INPUTCONTEXT_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for INPUTCONTEXT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MORRSLT {
    pub dwSize: u32,
    pub pwchOutput: ::windows_sys::core::PWSTR,
    pub cchOutput: u16,
    pub Anonymous1: MORRSLT_0,
    pub Anonymous2: MORRSLT_1,
    pub pchInputPos: *mut u16,
    pub pchOutputIdxWDD: *mut u16,
    pub Anonymous3: MORRSLT_2,
    pub paMonoRubyPos: *mut u16,
    pub pWDD: *mut WDD,
    pub cWDD: i32,
    pub pPrivate: *mut ::core::ffi::c_void,
    pub BLKBuff: [u16; 1],
}
impl ::core::marker::Copy for MORRSLT {}
impl ::core::clone::Clone for MORRSLT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union MORRSLT_0 {
    pub pwchRead: ::windows_sys::core::PWSTR,
    pub pwchComp: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for MORRSLT_0 {}
impl ::core::clone::Clone for MORRSLT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union MORRSLT_1 {
    pub cchRead: u16,
    pub cchComp: u16,
}
impl ::core::marker::Copy for MORRSLT_1 {}
impl ::core::clone::Clone for MORRSLT_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union MORRSLT_2 {
    pub pchReadIdxWDD: *mut u16,
    pub pchCompIdxWDD: *mut u16,
}
impl ::core::marker::Copy for MORRSLT_2 {}
impl ::core::clone::Clone for MORRSLT_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct POSTBL {
    pub nPos: u16,
    pub szName: *mut u8,
}
impl ::core::marker::Copy for POSTBL {}
impl ::core::clone::Clone for POSTBL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RECONVERTSTRING {
    pub dwSize: u32,
    pub dwVersion: u32,
    pub dwStrLen: u32,
    pub dwStrOffset: u32,
    pub dwCompStrLen: u32,
    pub dwCompStrOffset: u32,
    pub dwTargetStrLen: u32,
    pub dwTargetStrOffset: u32,
}
impl ::core::marker::Copy for RECONVERTSTRING {}
impl ::core::clone::Clone for RECONVERTSTRING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REGISTERWORDA {
    pub lpReading: ::windows_sys::core::PSTR,
    pub lpWord: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for REGISTERWORDA {}
impl ::core::clone::Clone for REGISTERWORDA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REGISTERWORDW {
    pub lpReading: ::windows_sys::core::PWSTR,
    pub lpWord: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for REGISTERWORDW {}
impl ::core::clone::Clone for REGISTERWORDW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SOFTKBDDATA {
    pub uCount: u32,
    pub wCode: [u16; 256],
}
impl ::core::marker::Copy for SOFTKBDDATA {}
impl ::core::clone::Clone for SOFTKBDDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct STYLEBUFA {
    pub dwStyle: u32,
    pub szDescription: [u8; 32],
}
impl ::core::marker::Copy for STYLEBUFA {}
impl ::core::clone::Clone for STYLEBUFA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct STYLEBUFW {
    pub dwStyle: u32,
    pub szDescription: [u16; 32],
}
impl ::core::marker::Copy for STYLEBUFW {}
impl ::core::clone::Clone for STYLEBUFW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TRANSMSG {
    pub message: u32,
    pub wParam: super::super::super::Foundation::WPARAM,
    pub lParam: super::super::super::Foundation::LPARAM,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TRANSMSG {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TRANSMSG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TRANSMSGLIST {
    pub uMsgCount: u32,
    pub TransMsg: [TRANSMSG; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TRANSMSGLIST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TRANSMSGLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WDD {
    pub wDispPos: u16,
    pub Anonymous1: WDD_0,
    pub cchDisp: u16,
    pub Anonymous2: WDD_1,
    pub WDD_nReserve1: u32,
    pub nPos: u16,
    pub _bitfield: u16,
    pub pReserved: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for WDD {}
impl ::core::clone::Clone for WDD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union WDD_0 {
    pub wReadPos: u16,
    pub wCompPos: u16,
}
impl ::core::marker::Copy for WDD_0 {}
impl ::core::clone::Clone for WDD_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union WDD_1 {
    pub cchRead: u16,
    pub cchComp: u16,
}
impl ::core::marker::Copy for WDD_1 {}
impl ::core::clone::Clone for WDD_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Globalization\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Globalization"))]
pub type IMCENUMPROC = ::core::option::Option<unsafe extern "system" fn(param0: super::super::super::Globalization::HIMC, param1: super::super::super::Foundation::LPARAM) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNLOG = ::core::option::Option<unsafe extern "system" fn(param0: *mut IMEDP, param1: ::windows_sys::core::HRESULT) -> super::super::super::Foundation::BOOL>;
pub type REGISTERWORDENUMPROCA = ::core::option::Option<unsafe extern "system" fn(lpszreading: ::windows_sys::core::PCSTR, param1: u32, lpszstring: ::windows_sys::core::PCSTR, param3: *mut ::core::ffi::c_void) -> i32>;
pub type REGISTERWORDENUMPROCW = ::core::option::Option<unsafe extern "system" fn(lpszreading: ::windows_sys::core::PCWSTR, param1: u32, lpszstring: ::windows_sys::core::PCWSTR, param3: *mut ::core::ffi::c_void) -> i32>;
pub type fpCreateIFECommonInstanceType = ::core::option::Option<unsafe extern "system" fn(ppvobj: *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
pub type fpCreateIFEDictionaryInstanceType = ::core::option::Option<unsafe extern "system" fn(ppvobj: *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
pub type fpCreateIFELanguageInstanceType = ::core::option::Option<unsafe extern "system" fn(clsid: *const ::windows_sys::core::GUID, ppvobj: *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
