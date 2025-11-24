#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AdjustWindowRect(lprect : *mut super::super::Foundation:: RECT, dwstyle : WINDOW_STYLE, bmenu : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AdjustWindowRectEx(lprect : *mut super::super::Foundation:: RECT, dwstyle : WINDOW_STYLE, bmenu : super::super::Foundation:: BOOL, dwexstyle : WINDOW_EX_STYLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AllowSetForegroundWindow(dwprocessid : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AnimateWindow(hwnd : super::super::Foundation:: HWND, dwtime : u32, dwflags : ANIMATE_WINDOW_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AnyPopup() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AppendMenuA(hmenu : HMENU, uflags : MENU_ITEM_FLAGS, uidnewitem : usize, lpnewitem : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AppendMenuW(hmenu : HMENU, uflags : MENU_ITEM_FLAGS, uidnewitem : usize, lpnewitem : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ArrangeIconicWindows(hwnd : super::super::Foundation:: HWND) -> u32);
::windows_targets::link!("user32.dll" "system" fn BeginDeferWindowPos(nnumwindows : i32) -> HDWP);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BringWindowToTop(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CalculatePopupWindowPosition(anchorpoint : *const super::super::Foundation:: POINT, windowsize : *const super::super::Foundation:: SIZE, flags : u32, excluderect : *const super::super::Foundation:: RECT, popupwindowposition : *mut super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CallMsgFilterA(lpmsg : *const MSG, ncode : i32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CallMsgFilterW(lpmsg : *const MSG, ncode : i32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CallNextHookEx(hhk : HHOOK, ncode : i32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CallWindowProcA(lpprevwndfunc : WNDPROC, hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CallWindowProcW(lpprevwndfunc : WNDPROC, hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CancelShutdown() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CascadeWindows(hwndparent : super::super::Foundation:: HWND, whow : CASCADE_WINDOWS_HOW, lprect : *const super::super::Foundation:: RECT, ckids : u32, lpkids : *const super::super::Foundation:: HWND) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ChangeMenuA(hmenu : HMENU, cmd : u32, lpsznewitem : ::windows_sys::core::PCSTR, cmdinsert : u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ChangeMenuW(hmenu : HMENU, cmd : u32, lpsznewitem : ::windows_sys::core::PCWSTR, cmdinsert : u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ChangeWindowMessageFilter(message : u32, dwflag : CHANGE_WINDOW_MESSAGE_FILTER_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ChangeWindowMessageFilterEx(hwnd : super::super::Foundation:: HWND, message : u32, action : WINDOW_MESSAGE_FILTER_ACTION, pchangefilterstruct : *mut CHANGEFILTERSTRUCT) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn CharLowerA(lpsz : ::windows_sys::core::PSTR) -> ::windows_sys::core::PSTR);
::windows_targets::link!("user32.dll" "system" fn CharLowerBuffA(lpsz : ::windows_sys::core::PSTR, cchlength : u32) -> u32);
::windows_targets::link!("user32.dll" "system" fn CharLowerBuffW(lpsz : ::windows_sys::core::PWSTR, cchlength : u32) -> u32);
::windows_targets::link!("user32.dll" "system" fn CharLowerW(lpsz : ::windows_sys::core::PWSTR) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("user32.dll" "system" fn CharNextA(lpsz : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PSTR);
::windows_targets::link!("user32.dll" "system" fn CharNextExA(codepage : u16, lpcurrentchar : ::windows_sys::core::PCSTR, dwflags : u32) -> ::windows_sys::core::PSTR);
::windows_targets::link!("user32.dll" "system" fn CharNextW(lpsz : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("user32.dll" "system" fn CharPrevA(lpszstart : ::windows_sys::core::PCSTR, lpszcurrent : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PSTR);
::windows_targets::link!("user32.dll" "system" fn CharPrevExA(codepage : u16, lpstart : ::windows_sys::core::PCSTR, lpcurrentchar : ::windows_sys::core::PCSTR, dwflags : u32) -> ::windows_sys::core::PSTR);
::windows_targets::link!("user32.dll" "system" fn CharPrevW(lpszstart : ::windows_sys::core::PCWSTR, lpszcurrent : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CharToOemA(psrc : ::windows_sys::core::PCSTR, pdst : ::windows_sys::core::PSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CharToOemBuffA(lpszsrc : ::windows_sys::core::PCSTR, lpszdst : ::windows_sys::core::PSTR, cchdstlength : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CharToOemBuffW(lpszsrc : ::windows_sys::core::PCWSTR, lpszdst : ::windows_sys::core::PSTR, cchdstlength : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CharToOemW(psrc : ::windows_sys::core::PCWSTR, pdst : ::windows_sys::core::PSTR) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn CharUpperA(lpsz : ::windows_sys::core::PSTR) -> ::windows_sys::core::PSTR);
::windows_targets::link!("user32.dll" "system" fn CharUpperBuffA(lpsz : ::windows_sys::core::PSTR, cchlength : u32) -> u32);
::windows_targets::link!("user32.dll" "system" fn CharUpperBuffW(lpsz : ::windows_sys::core::PWSTR, cchlength : u32) -> u32);
::windows_targets::link!("user32.dll" "system" fn CharUpperW(lpsz : ::windows_sys::core::PWSTR) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("user32.dll" "system" fn CheckMenuItem(hmenu : HMENU, uidcheckitem : u32, ucheck : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CheckMenuRadioItem(hmenu : HMENU, first : u32, last : u32, check : u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ChildWindowFromPoint(hwndparent : super::super::Foundation:: HWND, point : super::super::Foundation:: POINT) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ChildWindowFromPointEx(hwnd : super::super::Foundation:: HWND, pt : super::super::Foundation:: POINT, flags : CWP_FLAGS) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClipCursor(lprect : *const super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseWindow(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn CopyAcceleratorTableA(haccelsrc : HACCEL, lpacceldst : *mut ACCEL, caccelentries : i32) -> i32);
::windows_targets::link!("user32.dll" "system" fn CopyAcceleratorTableW(haccelsrc : HACCEL, lpacceldst : *mut ACCEL, caccelentries : i32) -> i32);
::windows_targets::link!("user32.dll" "system" fn CopyIcon(hicon : HICON) -> HICON);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CopyImage(h : super::super::Foundation:: HANDLE, r#type : GDI_IMAGE_TYPE, cx : i32, cy : i32, flags : IMAGE_FLAGS) -> super::super::Foundation:: HANDLE);
::windows_targets::link!("user32.dll" "system" fn CreateAcceleratorTableA(paccel : *const ACCEL, caccel : i32) -> HACCEL);
::windows_targets::link!("user32.dll" "system" fn CreateAcceleratorTableW(paccel : *const ACCEL, caccel : i32) -> HACCEL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn CreateCaret(hwnd : super::super::Foundation:: HWND, hbitmap : super::super::Graphics::Gdi:: HBITMAP, nwidth : i32, nheight : i32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateCursor(hinst : super::super::Foundation:: HINSTANCE, xhotspot : i32, yhotspot : i32, nwidth : i32, nheight : i32, pvandplane : *const ::core::ffi::c_void, pvxorplane : *const ::core::ffi::c_void) -> HCURSOR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateDialogIndirectParamA(hinstance : super::super::Foundation:: HINSTANCE, lptemplate : *const DLGTEMPLATE, hwndparent : super::super::Foundation:: HWND, lpdialogfunc : DLGPROC, dwinitparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateDialogIndirectParamW(hinstance : super::super::Foundation:: HINSTANCE, lptemplate : *const DLGTEMPLATE, hwndparent : super::super::Foundation:: HWND, lpdialogfunc : DLGPROC, dwinitparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateDialogParamA(hinstance : super::super::Foundation:: HINSTANCE, lptemplatename : ::windows_sys::core::PCSTR, hwndparent : super::super::Foundation:: HWND, lpdialogfunc : DLGPROC, dwinitparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateDialogParamW(hinstance : super::super::Foundation:: HINSTANCE, lptemplatename : ::windows_sys::core::PCWSTR, hwndparent : super::super::Foundation:: HWND, lpdialogfunc : DLGPROC, dwinitparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateIcon(hinstance : super::super::Foundation:: HINSTANCE, nwidth : i32, nheight : i32, cplanes : u8, cbitspixel : u8, lpbandbits : *const u8, lpbxorbits : *const u8) -> HICON);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateIconFromResource(presbits : *const u8, dwressize : u32, ficon : super::super::Foundation:: BOOL, dwver : u32) -> HICON);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateIconFromResourceEx(presbits : *const u8, dwressize : u32, ficon : super::super::Foundation:: BOOL, dwver : u32, cxdesired : i32, cydesired : i32, flags : IMAGE_FLAGS) -> HICON);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn CreateIconIndirect(piconinfo : *const ICONINFO) -> HICON);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateMDIWindowA(lpclassname : ::windows_sys::core::PCSTR, lpwindowname : ::windows_sys::core::PCSTR, dwstyle : WINDOW_STYLE, x : i32, y : i32, nwidth : i32, nheight : i32, hwndparent : super::super::Foundation:: HWND, hinstance : super::super::Foundation:: HINSTANCE, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateMDIWindowW(lpclassname : ::windows_sys::core::PCWSTR, lpwindowname : ::windows_sys::core::PCWSTR, dwstyle : WINDOW_STYLE, x : i32, y : i32, nwidth : i32, nheight : i32, hwndparent : super::super::Foundation:: HWND, hinstance : super::super::Foundation:: HINSTANCE, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: HWND);
::windows_targets::link!("user32.dll" "system" fn CreateMenu() -> HMENU);
::windows_targets::link!("user32.dll" "system" fn CreatePopupMenu() -> HMENU);
::windows_targets::link!("mrmsupport.dll" "system" fn CreateResourceIndexer(projectroot : ::windows_sys::core::PCWSTR, extensiondllpath : ::windows_sys::core::PCWSTR, ppresourceindexer : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateWindowExA(dwexstyle : WINDOW_EX_STYLE, lpclassname : ::windows_sys::core::PCSTR, lpwindowname : ::windows_sys::core::PCSTR, dwstyle : WINDOW_STYLE, x : i32, y : i32, nwidth : i32, nheight : i32, hwndparent : super::super::Foundation:: HWND, hmenu : HMENU, hinstance : super::super::Foundation:: HINSTANCE, lpparam : *const ::core::ffi::c_void) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateWindowExW(dwexstyle : WINDOW_EX_STYLE, lpclassname : ::windows_sys::core::PCWSTR, lpwindowname : ::windows_sys::core::PCWSTR, dwstyle : WINDOW_STYLE, x : i32, y : i32, nwidth : i32, nheight : i32, hwndparent : super::super::Foundation:: HWND, hmenu : HMENU, hinstance : super::super::Foundation:: HINSTANCE, lpparam : *const ::core::ffi::c_void) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DefDlgProcA(hdlg : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DefDlgProcW(hdlg : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DefFrameProcA(hwnd : super::super::Foundation:: HWND, hwndmdiclient : super::super::Foundation:: HWND, umsg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DefFrameProcW(hwnd : super::super::Foundation:: HWND, hwndmdiclient : super::super::Foundation:: HWND, umsg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DefMDIChildProcA(hwnd : super::super::Foundation:: HWND, umsg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DefMDIChildProcW(hwnd : super::super::Foundation:: HWND, umsg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DefWindowProcA(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DefWindowProcW(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DeferWindowPos(hwinposinfo : HDWP, hwnd : super::super::Foundation:: HWND, hwndinsertafter : super::super::Foundation:: HWND, x : i32, y : i32, cx : i32, cy : i32, uflags : SET_WINDOW_POS_FLAGS) -> HDWP);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DeleteMenu(hmenu : HMENU, uposition : u32, uflags : MENU_ITEM_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DeregisterShellHookWindow(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DestroyAcceleratorTable(haccel : HACCEL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DestroyCaret() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DestroyCursor(hcursor : HCURSOR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DestroyIcon(hicon : HICON) -> super::super::Foundation:: BOOL);
::windows_targets::link!("mrmsupport.dll" "system" fn DestroyIndexedResults(resourceuri : ::windows_sys::core::PCWSTR, qualifiercount : u32, qualifiers : *const IndexedResourceQualifier) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DestroyMenu(hmenu : HMENU) -> super::super::Foundation:: BOOL);
::windows_targets::link!("mrmsupport.dll" "system" fn DestroyResourceIndexer(resourceindexer : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DestroyWindow(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DialogBoxIndirectParamA(hinstance : super::super::Foundation:: HINSTANCE, hdialogtemplate : *const DLGTEMPLATE, hwndparent : super::super::Foundation:: HWND, lpdialogfunc : DLGPROC, dwinitparam : super::super::Foundation:: LPARAM) -> isize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DialogBoxIndirectParamW(hinstance : super::super::Foundation:: HINSTANCE, hdialogtemplate : *const DLGTEMPLATE, hwndparent : super::super::Foundation:: HWND, lpdialogfunc : DLGPROC, dwinitparam : super::super::Foundation:: LPARAM) -> isize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DialogBoxParamA(hinstance : super::super::Foundation:: HINSTANCE, lptemplatename : ::windows_sys::core::PCSTR, hwndparent : super::super::Foundation:: HWND, lpdialogfunc : DLGPROC, dwinitparam : super::super::Foundation:: LPARAM) -> isize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DialogBoxParamW(hinstance : super::super::Foundation:: HINSTANCE, lptemplatename : ::windows_sys::core::PCWSTR, hwndparent : super::super::Foundation:: HWND, lpdialogfunc : DLGPROC, dwinitparam : super::super::Foundation:: LPARAM) -> isize);
::windows_targets::link!("user32.dll" "system" fn DisableProcessWindowsGhosting() -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DispatchMessageA(lpmsg : *const MSG) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DispatchMessageW(lpmsg : *const MSG) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DragObject(hwndparent : super::super::Foundation:: HWND, hwndfrom : super::super::Foundation:: HWND, fmt : u32, data : usize, hcur : HCURSOR) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn DrawIcon(hdc : super::super::Graphics::Gdi:: HDC, x : i32, y : i32, hicon : HICON) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn DrawIconEx(hdc : super::super::Graphics::Gdi:: HDC, xleft : i32, ytop : i32, hicon : HICON, cxwidth : i32, cywidth : i32, istepifanicur : u32, hbrflickerfreedraw : super::super::Graphics::Gdi:: HBRUSH, diflags : DI_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DrawMenuBar(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnableMenuItem(hmenu : HMENU, uidenableitem : u32, uenable : MENU_ITEM_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EndDeferWindowPos(hwinposinfo : HDWP) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EndDialog(hdlg : super::super::Foundation:: HWND, nresult : isize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EndMenu() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumChildWindows(hwndparent : super::super::Foundation:: HWND, lpenumfunc : WNDENUMPROC, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumPropsA(hwnd : super::super::Foundation:: HWND, lpenumfunc : PROPENUMPROCA) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumPropsExA(hwnd : super::super::Foundation:: HWND, lpenumfunc : PROPENUMPROCEXA, lparam : super::super::Foundation:: LPARAM) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumPropsExW(hwnd : super::super::Foundation:: HWND, lpenumfunc : PROPENUMPROCEXW, lparam : super::super::Foundation:: LPARAM) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumPropsW(hwnd : super::super::Foundation:: HWND, lpenumfunc : PROPENUMPROCW) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumThreadWindows(dwthreadid : u32, lpfn : WNDENUMPROC, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumWindows(lpenumfunc : WNDENUMPROC, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindWindowA(lpclassname : ::windows_sys::core::PCSTR, lpwindowname : ::windows_sys::core::PCSTR) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindWindowExA(hwndparent : super::super::Foundation:: HWND, hwndchildafter : super::super::Foundation:: HWND, lpszclass : ::windows_sys::core::PCSTR, lpszwindow : ::windows_sys::core::PCSTR) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindWindowExW(hwndparent : super::super::Foundation:: HWND, hwndchildafter : super::super::Foundation:: HWND, lpszclass : ::windows_sys::core::PCWSTR, lpszwindow : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindWindowW(lpclassname : ::windows_sys::core::PCWSTR, lpwindowname : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FlashWindow(hwnd : super::super::Foundation:: HWND, binvert : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FlashWindowEx(pfwi : *const FLASHWINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetAltTabInfoA(hwnd : super::super::Foundation:: HWND, iitem : i32, pati : *mut ALTTABINFO, pszitemtext : ::windows_sys::core::PSTR, cchitemtext : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetAltTabInfoW(hwnd : super::super::Foundation:: HWND, iitem : i32, pati : *mut ALTTABINFO, pszitemtext : ::windows_sys::core::PWSTR, cchitemtext : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetAncestor(hwnd : super::super::Foundation:: HWND, gaflags : GET_ANCESTOR_FLAGS) -> super::super::Foundation:: HWND);
::windows_targets::link!("user32.dll" "system" fn GetCaretBlinkTime() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCaretPos(lppoint : *mut super::super::Foundation:: POINT) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetClassInfoA(hinstance : super::super::Foundation:: HINSTANCE, lpclassname : ::windows_sys::core::PCSTR, lpwndclass : *mut WNDCLASSA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetClassInfoExA(hinstance : super::super::Foundation:: HINSTANCE, lpszclass : ::windows_sys::core::PCSTR, lpwcx : *mut WNDCLASSEXA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetClassInfoExW(hinstance : super::super::Foundation:: HINSTANCE, lpszclass : ::windows_sys::core::PCWSTR, lpwcx : *mut WNDCLASSEXW) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetClassInfoW(hinstance : super::super::Foundation:: HINSTANCE, lpclassname : ::windows_sys::core::PCWSTR, lpwndclass : *mut WNDCLASSW) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClassLongA(hwnd : super::super::Foundation:: HWND, nindex : GET_CLASS_LONG_INDEX) -> u32);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClassLongPtrA(hwnd : super::super::Foundation:: HWND, nindex : GET_CLASS_LONG_INDEX) -> usize);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClassLongPtrW(hwnd : super::super::Foundation:: HWND, nindex : GET_CLASS_LONG_INDEX) -> usize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClassLongW(hwnd : super::super::Foundation:: HWND, nindex : GET_CLASS_LONG_INDEX) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClassNameA(hwnd : super::super::Foundation:: HWND, lpclassname : ::windows_sys::core::PSTR, nmaxcount : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClassNameW(hwnd : super::super::Foundation:: HWND, lpclassname : ::windows_sys::core::PWSTR, nmaxcount : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClassWord(hwnd : super::super::Foundation:: HWND, nindex : i32) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClientRect(hwnd : super::super::Foundation:: HWND, lprect : *mut super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClipCursor(lprect : *mut super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn GetCursor() -> HCURSOR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCursorInfo(pci : *mut CURSORINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCursorPos(lppoint : *mut super::super::Foundation:: POINT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDesktopWindow() -> super::super::Foundation:: HWND);
::windows_targets::link!("user32.dll" "system" fn GetDialogBaseUnits() -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDlgCtrlID(hwnd : super::super::Foundation:: HWND) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDlgItem(hdlg : super::super::Foundation:: HWND, niddlgitem : i32) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDlgItemInt(hdlg : super::super::Foundation:: HWND, niddlgitem : i32, lptranslated : *mut super::super::Foundation:: BOOL, bsigned : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDlgItemTextA(hdlg : super::super::Foundation:: HWND, niddlgitem : i32, lpstring : ::windows_sys::core::PSTR, cchmax : i32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDlgItemTextW(hdlg : super::super::Foundation:: HWND, niddlgitem : i32, lpstring : ::windows_sys::core::PWSTR, cchmax : i32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetForegroundWindow() -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetGUIThreadInfo(idthread : u32, pgui : *mut GUITHREADINFO) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetIconInfo(hicon : HICON, piconinfo : *mut ICONINFO) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetIconInfoExA(hicon : HICON, piconinfo : *mut ICONINFOEXA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetIconInfoExW(hicon : HICON, piconinfo : *mut ICONINFOEXW) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetInputState() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetLastActivePopup(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetLayeredWindowAttributes(hwnd : super::super::Foundation:: HWND, pcrkey : *mut super::super::Foundation:: COLORREF, pbalpha : *mut u8, pdwflags : *mut LAYERED_WINDOW_ATTRIBUTES_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetMenu(hwnd : super::super::Foundation:: HWND) -> HMENU);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetMenuBarInfo(hwnd : super::super::Foundation:: HWND, idobject : OBJECT_IDENTIFIER, iditem : i32, pmbi : *mut MENUBARINFO) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn GetMenuCheckMarkDimensions() -> i32);
::windows_targets::link!("user32.dll" "system" fn GetMenuDefaultItem(hmenu : HMENU, fbypos : u32, gmdiflags : GET_MENU_DEFAULT_ITEM_FLAGS) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetMenuInfo(param0 : HMENU, param1 : *mut MENUINFO) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn GetMenuItemCount(hmenu : HMENU) -> i32);
::windows_targets::link!("user32.dll" "system" fn GetMenuItemID(hmenu : HMENU, npos : i32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetMenuItemInfoA(hmenu : HMENU, item : u32, fbyposition : super::super::Foundation:: BOOL, lpmii : *mut MENUITEMINFOA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GetMenuItemInfoW(hmenu : HMENU, item : u32, fbyposition : super::super::Foundation:: BOOL, lpmii : *mut MENUITEMINFOW) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetMenuItemRect(hwnd : super::super::Foundation:: HWND, hmenu : HMENU, uitem : u32, lprcitem : *mut super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn GetMenuState(hmenu : HMENU, uid : u32, uflags : MENU_ITEM_FLAGS) -> u32);
::windows_targets::link!("user32.dll" "system" fn GetMenuStringA(hmenu : HMENU, uiditem : u32, lpstring : ::windows_sys::core::PSTR, cchmax : i32, flags : MENU_ITEM_FLAGS) -> i32);
::windows_targets::link!("user32.dll" "system" fn GetMenuStringW(hmenu : HMENU, uiditem : u32, lpstring : ::windows_sys::core::PWSTR, cchmax : i32, flags : MENU_ITEM_FLAGS) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetMessageA(lpmsg : *mut MSG, hwnd : super::super::Foundation:: HWND, wmsgfiltermin : u32, wmsgfiltermax : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetMessageExtraInfo() -> super::super::Foundation:: LPARAM);
::windows_targets::link!("user32.dll" "system" fn GetMessagePos() -> u32);
::windows_targets::link!("user32.dll" "system" fn GetMessageTime() -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetMessageW(lpmsg : *mut MSG, hwnd : super::super::Foundation:: HWND, wmsgfiltermin : u32, wmsgfiltermax : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetNextDlgGroupItem(hdlg : super::super::Foundation:: HWND, hctl : super::super::Foundation:: HWND, bprevious : super::super::Foundation:: BOOL) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetNextDlgTabItem(hdlg : super::super::Foundation:: HWND, hctl : super::super::Foundation:: HWND, bprevious : super::super::Foundation:: BOOL) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetParent(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetPhysicalCursorPos(lppoint : *mut super::super::Foundation:: POINT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetProcessDefaultLayout(pdwdefaultlayout : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetPropA(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PCSTR) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetPropW(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: HANDLE);
::windows_targets::link!("user32.dll" "system" fn GetQueueStatus(flags : QUEUE_STATUS_FLAGS) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetScrollBarInfo(hwnd : super::super::Foundation:: HWND, idobject : OBJECT_IDENTIFIER, psbi : *mut SCROLLBARINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetScrollInfo(hwnd : super::super::Foundation:: HWND, nbar : SCROLLBAR_CONSTANTS, lpsi : *mut SCROLLINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetScrollPos(hwnd : super::super::Foundation:: HWND, nbar : SCROLLBAR_CONSTANTS) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetScrollRange(hwnd : super::super::Foundation:: HWND, nbar : SCROLLBAR_CONSTANTS, lpminpos : *mut i32, lpmaxpos : *mut i32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetShellWindow() -> super::super::Foundation:: HWND);
::windows_targets::link!("user32.dll" "system" fn GetSubMenu(hmenu : HMENU, npos : i32) -> HMENU);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetSystemMenu(hwnd : super::super::Foundation:: HWND, brevert : super::super::Foundation:: BOOL) -> HMENU);
::windows_targets::link!("user32.dll" "system" fn GetSystemMetrics(nindex : SYSTEM_METRICS_INDEX) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetTitleBarInfo(hwnd : super::super::Foundation:: HWND, pti : *mut TITLEBARINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetTopWindow(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindow(hwnd : super::super::Foundation:: HWND, ucmd : GET_WINDOW_CMD) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowDisplayAffinity(hwnd : super::super::Foundation:: HWND, pdwaffinity : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowInfo(hwnd : super::super::Foundation:: HWND, pwi : *mut WINDOWINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowLongA(hwnd : super::super::Foundation:: HWND, nindex : WINDOW_LONG_PTR_INDEX) -> i32);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowLongPtrA(hwnd : super::super::Foundation:: HWND, nindex : WINDOW_LONG_PTR_INDEX) -> isize);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowLongPtrW(hwnd : super::super::Foundation:: HWND, nindex : WINDOW_LONG_PTR_INDEX) -> isize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowLongW(hwnd : super::super::Foundation:: HWND, nindex : WINDOW_LONG_PTR_INDEX) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowModuleFileNameA(hwnd : super::super::Foundation:: HWND, pszfilename : ::windows_sys::core::PSTR, cchfilenamemax : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowModuleFileNameW(hwnd : super::super::Foundation:: HWND, pszfilename : ::windows_sys::core::PWSTR, cchfilenamemax : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowPlacement(hwnd : super::super::Foundation:: HWND, lpwndpl : *mut WINDOWPLACEMENT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowRect(hwnd : super::super::Foundation:: HWND, lprect : *mut super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowTextA(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PSTR, nmaxcount : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowTextLengthA(hwnd : super::super::Foundation:: HWND) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowTextLengthW(hwnd : super::super::Foundation:: HWND) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowTextW(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PWSTR, nmaxcount : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowThreadProcessId(hwnd : super::super::Foundation:: HWND, lpdwprocessid : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetWindowWord(hwnd : super::super::Foundation:: HWND, nindex : i32) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HideCaret(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HiliteMenuItem(hwnd : super::super::Foundation:: HWND, hmenu : HMENU, uidhiliteitem : u32, uhilite : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InSendMessage() -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn InSendMessageEx(lpreserved : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("mrmsupport.dll" "system" fn IndexFilePath(resourceindexer : *const ::core::ffi::c_void, filepath : ::windows_sys::core::PCWSTR, ppresourceuri : *mut ::windows_sys::core::PWSTR, pqualifiercount : *mut u32, ppqualifiers : *mut *mut IndexedResourceQualifier) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InheritWindowMonitor(hwnd : super::super::Foundation:: HWND, hwndinherit : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InsertMenuA(hmenu : HMENU, uposition : u32, uflags : MENU_ITEM_FLAGS, uidnewitem : usize, lpnewitem : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn InsertMenuItemA(hmenu : HMENU, item : u32, fbyposition : super::super::Foundation:: BOOL, lpmi : *const MENUITEMINFOA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn InsertMenuItemW(hmenu : HMENU, item : u32, fbyposition : super::super::Foundation:: BOOL, lpmi : *const MENUITEMINFOW) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InsertMenuW(hmenu : HMENU, uposition : u32, uflags : MENU_ITEM_FLAGS, uidnewitem : usize, lpnewitem : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InternalGetWindowText(hwnd : super::super::Foundation:: HWND, pstring : ::windows_sys::core::PWSTR, cchmaxcount : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsCharAlphaA(ch : u8) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsCharAlphaNumericA(ch : u8) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsCharAlphaNumericW(ch : u16) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsCharAlphaW(ch : u16) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsCharLowerA(ch : u8) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsCharUpperA(ch : u8) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsCharUpperW(ch : u16) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsChild(hwndparent : super::super::Foundation:: HWND, hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsDialogMessageA(hdlg : super::super::Foundation:: HWND, lpmsg : *const MSG) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsDialogMessageW(hdlg : super::super::Foundation:: HWND, lpmsg : *const MSG) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsGUIThread(bconvert : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsHungAppWindow(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsIconic(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsMenu(hmenu : HMENU) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsProcessDPIAware() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsWindow(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsWindowUnicode(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsWindowVisible(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsWow64Message() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsZoomed(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn KillTimer(hwnd : super::super::Foundation:: HWND, uidevent : usize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadAcceleratorsA(hinstance : super::super::Foundation:: HINSTANCE, lptablename : ::windows_sys::core::PCSTR) -> HACCEL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadAcceleratorsW(hinstance : super::super::Foundation:: HINSTANCE, lptablename : ::windows_sys::core::PCWSTR) -> HACCEL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadCursorA(hinstance : super::super::Foundation:: HINSTANCE, lpcursorname : ::windows_sys::core::PCSTR) -> HCURSOR);
::windows_targets::link!("user32.dll" "system" fn LoadCursorFromFileA(lpfilename : ::windows_sys::core::PCSTR) -> HCURSOR);
::windows_targets::link!("user32.dll" "system" fn LoadCursorFromFileW(lpfilename : ::windows_sys::core::PCWSTR) -> HCURSOR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadCursorW(hinstance : super::super::Foundation:: HINSTANCE, lpcursorname : ::windows_sys::core::PCWSTR) -> HCURSOR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadIconA(hinstance : super::super::Foundation:: HINSTANCE, lpiconname : ::windows_sys::core::PCSTR) -> HICON);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadIconW(hinstance : super::super::Foundation:: HINSTANCE, lpiconname : ::windows_sys::core::PCWSTR) -> HICON);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadImageA(hinst : super::super::Foundation:: HINSTANCE, name : ::windows_sys::core::PCSTR, r#type : GDI_IMAGE_TYPE, cx : i32, cy : i32, fuload : IMAGE_FLAGS) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadImageW(hinst : super::super::Foundation:: HINSTANCE, name : ::windows_sys::core::PCWSTR, r#type : GDI_IMAGE_TYPE, cx : i32, cy : i32, fuload : IMAGE_FLAGS) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadMenuA(hinstance : super::super::Foundation:: HINSTANCE, lpmenuname : ::windows_sys::core::PCSTR) -> HMENU);
::windows_targets::link!("user32.dll" "system" fn LoadMenuIndirectA(lpmenutemplate : *const ::core::ffi::c_void) -> HMENU);
::windows_targets::link!("user32.dll" "system" fn LoadMenuIndirectW(lpmenutemplate : *const ::core::ffi::c_void) -> HMENU);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadMenuW(hinstance : super::super::Foundation:: HINSTANCE, lpmenuname : ::windows_sys::core::PCWSTR) -> HMENU);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadStringA(hinstance : super::super::Foundation:: HINSTANCE, uid : u32, lpbuffer : ::windows_sys::core::PSTR, cchbuffermax : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadStringW(hinstance : super::super::Foundation:: HINSTANCE, uid : u32, lpbuffer : ::windows_sys::core::PWSTR, cchbuffermax : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LockSetForegroundWindow(ulockcode : FOREGROUND_WINDOW_LOCK_CODE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LogicalToPhysicalPoint(hwnd : super::super::Foundation:: HWND, lppoint : *mut super::super::Foundation:: POINT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LookupIconIdFromDirectory(presbits : *const u8, ficon : super::super::Foundation:: BOOL) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LookupIconIdFromDirectoryEx(presbits : *const u8, ficon : super::super::Foundation:: BOOL, cxdesired : i32, cydesired : i32, flags : IMAGE_FLAGS) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MapDialogRect(hdlg : super::super::Foundation:: HWND, lprect : *mut super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MenuItemFromPoint(hwnd : super::super::Foundation:: HWND, hmenu : HMENU, ptscreen : super::super::Foundation:: POINT) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MessageBoxA(hwnd : super::super::Foundation:: HWND, lptext : ::windows_sys::core::PCSTR, lpcaption : ::windows_sys::core::PCSTR, utype : MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MessageBoxExA(hwnd : super::super::Foundation:: HWND, lptext : ::windows_sys::core::PCSTR, lpcaption : ::windows_sys::core::PCSTR, utype : MESSAGEBOX_STYLE, wlanguageid : u16) -> MESSAGEBOX_RESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MessageBoxExW(hwnd : super::super::Foundation:: HWND, lptext : ::windows_sys::core::PCWSTR, lpcaption : ::windows_sys::core::PCWSTR, utype : MESSAGEBOX_STYLE, wlanguageid : u16) -> MESSAGEBOX_RESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell\"`"] fn MessageBoxIndirectA(lpmbp : *const MSGBOXPARAMSA) -> MESSAGEBOX_RESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell\"`"] fn MessageBoxIndirectW(lpmbp : *const MSGBOXPARAMSW) -> MESSAGEBOX_RESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MessageBoxW(hwnd : super::super::Foundation:: HWND, lptext : ::windows_sys::core::PCWSTR, lpcaption : ::windows_sys::core::PCWSTR, utype : MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ModifyMenuA(hmnu : HMENU, uposition : u32, uflags : MENU_ITEM_FLAGS, uidnewitem : usize, lpnewitem : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ModifyMenuW(hmnu : HMENU, uposition : u32, uflags : MENU_ITEM_FLAGS, uidnewitem : usize, lpnewitem : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MoveWindow(hwnd : super::super::Foundation:: HWND, x : i32, y : i32, nwidth : i32, nheight : i32, brepaint : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateConfig(platformversion : MrmPlatformVersion, defaultqualifiers : ::windows_sys::core::PCWSTR, outputxmlfile : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateConfigInMemory(platformversion : MrmPlatformVersion, defaultqualifiers : ::windows_sys::core::PCWSTR, outputxmldata : *mut *mut u8, outputxmlsize : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateResourceFile(indexer : MrmResourceIndexerHandle, packagingmode : MrmPackagingMode, packagingoptions : MrmPackagingOptions, outputdirectory : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateResourceFileInMemory(indexer : MrmResourceIndexerHandle, packagingmode : MrmPackagingMode, packagingoptions : MrmPackagingOptions, outputpridata : *mut *mut u8, outputprisize : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateResourceFileWithChecksum(indexer : MrmResourceIndexerHandle, packagingmode : MrmPackagingMode, packagingoptions : MrmPackagingOptions, checksum : u32, outputdirectory : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateResourceIndexer(packagefamilyname : ::windows_sys::core::PCWSTR, projectroot : ::windows_sys::core::PCWSTR, platformversion : MrmPlatformVersion, defaultqualifiers : ::windows_sys::core::PCWSTR, indexer : *mut MrmResourceIndexerHandle) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateResourceIndexerFromPreviousPriData(projectroot : ::windows_sys::core::PCWSTR, platformversion : MrmPlatformVersion, defaultqualifiers : ::windows_sys::core::PCWSTR, pridata : *const u8, prisize : u32, indexer : *mut MrmResourceIndexerHandle) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateResourceIndexerFromPreviousPriFile(projectroot : ::windows_sys::core::PCWSTR, platformversion : MrmPlatformVersion, defaultqualifiers : ::windows_sys::core::PCWSTR, prifile : ::windows_sys::core::PCWSTR, indexer : *mut MrmResourceIndexerHandle) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateResourceIndexerFromPreviousSchemaData(projectroot : ::windows_sys::core::PCWSTR, platformversion : MrmPlatformVersion, defaultqualifiers : ::windows_sys::core::PCWSTR, schemaxmldata : *const u8, schemaxmlsize : u32, indexer : *mut MrmResourceIndexerHandle) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateResourceIndexerFromPreviousSchemaFile(projectroot : ::windows_sys::core::PCWSTR, platformversion : MrmPlatformVersion, defaultqualifiers : ::windows_sys::core::PCWSTR, schemafile : ::windows_sys::core::PCWSTR, indexer : *mut MrmResourceIndexerHandle) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmCreateResourceIndexerWithFlags(packagefamilyname : ::windows_sys::core::PCWSTR, projectroot : ::windows_sys::core::PCWSTR, platformversion : MrmPlatformVersion, defaultqualifiers : ::windows_sys::core::PCWSTR, flags : MrmIndexerFlags, indexer : *mut MrmResourceIndexerHandle) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmDestroyIndexerAndMessages(indexer : MrmResourceIndexerHandle) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmDumpPriDataInMemory(inputpridata : *const u8, inputprisize : u32, schemapridata : *const u8, schemaprisize : u32, dumptype : MrmDumpType, outputxmldata : *mut *mut u8, outputxmlsize : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmDumpPriFile(indexfilename : ::windows_sys::core::PCWSTR, schemaprifile : ::windows_sys::core::PCWSTR, dumptype : MrmDumpType, outputxmlfile : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmDumpPriFileInMemory(indexfilename : ::windows_sys::core::PCWSTR, schemaprifile : ::windows_sys::core::PCWSTR, dumptype : MrmDumpType, outputxmldata : *mut *mut u8, outputxmlsize : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmFreeMemory(data : *const u8) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmGetPriFileContentChecksum(prifile : ::windows_sys::core::PCWSTR, checksum : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmIndexEmbeddedData(indexer : MrmResourceIndexerHandle, resourceuri : ::windows_sys::core::PCWSTR, embeddeddata : *const u8, embeddeddatasize : u32, qualifiers : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmIndexFile(indexer : MrmResourceIndexerHandle, resourceuri : ::windows_sys::core::PCWSTR, filepath : ::windows_sys::core::PCWSTR, qualifiers : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmIndexFileAutoQualifiers(indexer : MrmResourceIndexerHandle, filepath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmIndexResourceContainerAutoQualifiers(indexer : MrmResourceIndexerHandle, containerpath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmIndexString(indexer : MrmResourceIndexerHandle, resourceuri : ::windows_sys::core::PCWSTR, resourcestring : ::windows_sys::core::PCWSTR, qualifiers : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mrmsupport.dll" "system" fn MrmPeekResourceIndexerMessages(handle : MrmResourceIndexerHandle, messages : *mut *mut MrmResourceIndexerMessage, nummsgs : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MsgWaitForMultipleObjects(ncount : u32, phandles : *const super::super::Foundation:: HANDLE, fwaitall : super::super::Foundation:: BOOL, dwmilliseconds : u32, dwwakemask : QUEUE_STATUS_FLAGS) -> super::super::Foundation:: WAIT_EVENT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MsgWaitForMultipleObjectsEx(ncount : u32, phandles : *const super::super::Foundation:: HANDLE, dwmilliseconds : u32, dwwakemask : QUEUE_STATUS_FLAGS, dwflags : MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS) -> super::super::Foundation:: WAIT_EVENT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OemToCharA(psrc : ::windows_sys::core::PCSTR, pdst : ::windows_sys::core::PSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OemToCharBuffA(lpszsrc : ::windows_sys::core::PCSTR, lpszdst : ::windows_sys::core::PSTR, cchdstlength : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OemToCharBuffW(lpszsrc : ::windows_sys::core::PCSTR, lpszdst : ::windows_sys::core::PWSTR, cchdstlength : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OemToCharW(psrc : ::windows_sys::core::PCSTR, pdst : ::windows_sys::core::PWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenIcon(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeekMessageA(lpmsg : *mut MSG, hwnd : super::super::Foundation:: HWND, wmsgfiltermin : u32, wmsgfiltermax : u32, wremovemsg : PEEK_MESSAGE_REMOVE_TYPE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeekMessageW(lpmsg : *mut MSG, hwnd : super::super::Foundation:: HWND, wmsgfiltermin : u32, wmsgfiltermax : u32, wremovemsg : PEEK_MESSAGE_REMOVE_TYPE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PhysicalToLogicalPoint(hwnd : super::super::Foundation:: HWND, lppoint : *mut super::super::Foundation:: POINT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PostMessageA(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PostMessageW(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn PostQuitMessage(nexitcode : i32) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PostThreadMessageA(idthread : u32, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PostThreadMessageW(idthread : u32, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn PrivateExtractIconsA(szfilename : ::windows_sys::core::PCSTR, niconindex : i32, cxicon : i32, cyicon : i32, phicon : *mut HICON, piconid : *mut u32, nicons : u32, flags : u32) -> u32);
::windows_targets::link!("user32.dll" "system" fn PrivateExtractIconsW(szfilename : ::windows_sys::core::PCWSTR, niconindex : i32, cxicon : i32, cyicon : i32, phicon : *mut HICON, piconid : *mut u32, nicons : u32, flags : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RealChildWindowFromPoint(hwndparent : super::super::Foundation:: HWND, ptparentclientcoords : super::super::Foundation:: POINT) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RealGetWindowClassA(hwnd : super::super::Foundation:: HWND, ptszclassname : ::windows_sys::core::PSTR, cchclassnamemax : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RealGetWindowClassW(hwnd : super::super::Foundation:: HWND, ptszclassname : ::windows_sys::core::PWSTR, cchclassnamemax : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn RegisterClassA(lpwndclass : *const WNDCLASSA) -> u16);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn RegisterClassExA(param0 : *const WNDCLASSEXA) -> u16);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn RegisterClassExW(param0 : *const WNDCLASSEXW) -> u16);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn RegisterClassW(lpwndclass : *const WNDCLASSW) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterDeviceNotificationA(hrecipient : super::super::Foundation:: HANDLE, notificationfilter : *const ::core::ffi::c_void, flags : REGISTER_NOTIFICATION_FLAGS) -> HDEVNOTIFY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterDeviceNotificationW(hrecipient : super::super::Foundation:: HANDLE, notificationfilter : *const ::core::ffi::c_void, flags : REGISTER_NOTIFICATION_FLAGS) -> HDEVNOTIFY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterForTooltipDismissNotification(hwnd : super::super::Foundation:: HWND, tdflags : TOOLTIP_DISMISS_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterShellHookWindow(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn RegisterWindowMessageA(lpstring : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("user32.dll" "system" fn RegisterWindowMessageW(lpstring : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RemoveMenu(hmenu : HMENU, uposition : u32, uflags : MENU_ITEM_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RemovePropA(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PCSTR) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RemovePropW(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReplyMessage(lresult : super::super::Foundation:: LRESULT) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn ScrollDC(hdc : super::super::Graphics::Gdi:: HDC, dx : i32, dy : i32, lprcscroll : *const super::super::Foundation:: RECT, lprcclip : *const super::super::Foundation:: RECT, hrgnupdate : super::super::Graphics::Gdi:: HRGN, lprcupdate : *mut super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ScrollWindow(hwnd : super::super::Foundation:: HWND, xamount : i32, yamount : i32, lprect : *const super::super::Foundation:: RECT, lpcliprect : *const super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn ScrollWindowEx(hwnd : super::super::Foundation:: HWND, dx : i32, dy : i32, prcscroll : *const super::super::Foundation:: RECT, prcclip : *const super::super::Foundation:: RECT, hrgnupdate : super::super::Graphics::Gdi:: HRGN, prcupdate : *mut super::super::Foundation:: RECT, flags : SCROLL_WINDOW_FLAGS) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendDlgItemMessageA(hdlg : super::super::Foundation:: HWND, niddlgitem : i32, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendDlgItemMessageW(hdlg : super::super::Foundation:: HWND, niddlgitem : i32, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendMessageA(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendMessageCallbackA(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM, lpresultcallback : SENDASYNCPROC, dwdata : usize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendMessageCallbackW(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM, lpresultcallback : SENDASYNCPROC, dwdata : usize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendMessageTimeoutA(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM, fuflags : SEND_MESSAGE_TIMEOUT_FLAGS, utimeout : u32, lpdwresult : *mut usize) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendMessageTimeoutW(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM, fuflags : SEND_MESSAGE_TIMEOUT_FLAGS, utimeout : u32, lpdwresult : *mut usize) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendMessageW(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendNotifyMessageA(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SendNotifyMessageW(hwnd : super::super::Foundation:: HWND, msg : u32, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetAdditionalForegroundBoostProcesses(toplevelwindow : super::super::Foundation:: HWND, processhandlecount : u32, processhandlearray : *const super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCaretBlinkTime(umseconds : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCaretPos(x : i32, y : i32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetClassLongA(hwnd : super::super::Foundation:: HWND, nindex : GET_CLASS_LONG_INDEX, dwnewlong : i32) -> u32);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetClassLongPtrA(hwnd : super::super::Foundation:: HWND, nindex : GET_CLASS_LONG_INDEX, dwnewlong : isize) -> usize);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetClassLongPtrW(hwnd : super::super::Foundation:: HWND, nindex : GET_CLASS_LONG_INDEX, dwnewlong : isize) -> usize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetClassLongW(hwnd : super::super::Foundation:: HWND, nindex : GET_CLASS_LONG_INDEX, dwnewlong : i32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetClassWord(hwnd : super::super::Foundation:: HWND, nindex : i32, wnewword : u16) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCoalescableTimer(hwnd : super::super::Foundation:: HWND, nidevent : usize, uelapse : u32, lptimerfunc : TIMERPROC, utolerancedelay : u32) -> usize);
::windows_targets::link!("user32.dll" "system" fn SetCursor(hcursor : HCURSOR) -> HCURSOR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCursorPos(x : i32, y : i32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn SetDebugErrorLevel(dwlevel : u32) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetDlgItemInt(hdlg : super::super::Foundation:: HWND, niddlgitem : i32, uvalue : u32, bsigned : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetDlgItemTextA(hdlg : super::super::Foundation:: HWND, niddlgitem : i32, lpstring : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetDlgItemTextW(hdlg : super::super::Foundation:: HWND, niddlgitem : i32, lpstring : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetForegroundWindow(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetLayeredWindowAttributes(hwnd : super::super::Foundation:: HWND, crkey : super::super::Foundation:: COLORREF, balpha : u8, dwflags : LAYERED_WINDOW_ATTRIBUTES_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetMenu(hwnd : super::super::Foundation:: HWND, hmenu : HMENU) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetMenuDefaultItem(hmenu : HMENU, uitem : u32, fbypos : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn SetMenuInfo(param0 : HMENU, param1 : *const MENUINFO) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn SetMenuItemBitmaps(hmenu : HMENU, uposition : u32, uflags : MENU_ITEM_FLAGS, hbitmapunchecked : super::super::Graphics::Gdi:: HBITMAP, hbitmapchecked : super::super::Graphics::Gdi:: HBITMAP) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn SetMenuItemInfoA(hmenu : HMENU, item : u32, fbypositon : super::super::Foundation:: BOOL, lpmii : *const MENUITEMINFOA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn SetMenuItemInfoW(hmenu : HMENU, item : u32, fbypositon : super::super::Foundation:: BOOL, lpmii : *const MENUITEMINFOW) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetMessageExtraInfo(lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: LPARAM);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetMessageQueue(cmessagesmax : i32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetParent(hwndchild : super::super::Foundation:: HWND, hwndnewparent : super::super::Foundation:: HWND) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetPhysicalCursorPos(x : i32, y : i32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetProcessDPIAware() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetProcessDefaultLayout(dwdefaultlayout : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetPropA(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PCSTR, hdata : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetPropW(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PCWSTR, hdata : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetSystemCursor(hcur : HCURSOR, id : SYSTEM_CURSOR_ID) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetTimer(hwnd : super::super::Foundation:: HWND, nidevent : usize, uelapse : u32, lptimerfunc : TIMERPROC) -> usize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowDisplayAffinity(hwnd : super::super::Foundation:: HWND, dwaffinity : WINDOW_DISPLAY_AFFINITY) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowLongA(hwnd : super::super::Foundation:: HWND, nindex : WINDOW_LONG_PTR_INDEX, dwnewlong : i32) -> i32);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowLongPtrA(hwnd : super::super::Foundation:: HWND, nindex : WINDOW_LONG_PTR_INDEX, dwnewlong : isize) -> isize);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowLongPtrW(hwnd : super::super::Foundation:: HWND, nindex : WINDOW_LONG_PTR_INDEX, dwnewlong : isize) -> isize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowLongW(hwnd : super::super::Foundation:: HWND, nindex : WINDOW_LONG_PTR_INDEX, dwnewlong : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowPlacement(hwnd : super::super::Foundation:: HWND, lpwndpl : *const WINDOWPLACEMENT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowPos(hwnd : super::super::Foundation:: HWND, hwndinsertafter : super::super::Foundation:: HWND, x : i32, y : i32, cx : i32, cy : i32, uflags : SET_WINDOW_POS_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowTextA(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowTextW(hwnd : super::super::Foundation:: HWND, lpstring : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowWord(hwnd : super::super::Foundation:: HWND, nindex : i32, wnewword : u16) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowsHookA(nfiltertype : i32, pfnfilterproc : HOOKPROC) -> HHOOK);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowsHookExA(idhook : WINDOWS_HOOK_ID, lpfn : HOOKPROC, hmod : super::super::Foundation:: HINSTANCE, dwthreadid : u32) -> HHOOK);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowsHookExW(idhook : WINDOWS_HOOK_ID, lpfn : HOOKPROC, hmod : super::super::Foundation:: HINSTANCE, dwthreadid : u32) -> HHOOK);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWindowsHookW(nfiltertype : i32, pfnfilterproc : HOOKPROC) -> HHOOK);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ShowCaret(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ShowCursor(bshow : super::super::Foundation:: BOOL) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ShowOwnedPopups(hwnd : super::super::Foundation:: HWND, fshow : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ShowWindow(hwnd : super::super::Foundation:: HWND, ncmdshow : SHOW_WINDOW_CMD) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ShowWindowAsync(hwnd : super::super::Foundation:: HWND, ncmdshow : SHOW_WINDOW_CMD) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SoundSentry() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SwitchToThisWindow(hwnd : super::super::Foundation:: HWND, funknown : super::super::Foundation:: BOOL) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SystemParametersInfoA(uiaction : SYSTEM_PARAMETERS_INFO_ACTION, uiparam : u32, pvparam : *mut ::core::ffi::c_void, fwinini : SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SystemParametersInfoW(uiaction : SYSTEM_PARAMETERS_INFO_ACTION, uiparam : u32, pvparam : *mut ::core::ffi::c_void, fwinini : SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TileWindows(hwndparent : super::super::Foundation:: HWND, whow : TILE_WINDOWS_HOW, lprect : *const super::super::Foundation:: RECT, ckids : u32, lpkids : *const super::super::Foundation:: HWND) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TrackPopupMenu(hmenu : HMENU, uflags : TRACK_POPUP_MENU_FLAGS, x : i32, y : i32, nreserved : i32, hwnd : super::super::Foundation:: HWND, prcrect : *const super::super::Foundation:: RECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TrackPopupMenuEx(hmenu : HMENU, uflags : u32, x : i32, y : i32, hwnd : super::super::Foundation:: HWND, lptpm : *const TPMPARAMS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TranslateAcceleratorA(hwnd : super::super::Foundation:: HWND, hacctable : HACCEL, lpmsg : *const MSG) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TranslateAcceleratorW(hwnd : super::super::Foundation:: HWND, hacctable : HACCEL, lpmsg : *const MSG) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TranslateMDISysAccel(hwndclient : super::super::Foundation:: HWND, lpmsg : *const MSG) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TranslateMessage(lpmsg : *const MSG) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnhookWindowsHook(ncode : i32, pfnfilterproc : HOOKPROC) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnhookWindowsHookEx(hhk : HHOOK) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnregisterClassA(lpclassname : ::windows_sys::core::PCSTR, hinstance : super::super::Foundation:: HINSTANCE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnregisterClassW(lpclassname : ::windows_sys::core::PCWSTR, hinstance : super::super::Foundation:: HINSTANCE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnregisterDeviceNotification(handle : HDEVNOTIFY) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn UpdateLayeredWindow(hwnd : super::super::Foundation:: HWND, hdcdst : super::super::Graphics::Gdi:: HDC, pptdst : *const super::super::Foundation:: POINT, psize : *const super::super::Foundation:: SIZE, hdcsrc : super::super::Graphics::Gdi:: HDC, pptsrc : *const super::super::Foundation:: POINT, crkey : super::super::Foundation:: COLORREF, pblend : *const super::super::Graphics::Gdi:: BLENDFUNCTION, dwflags : UPDATE_LAYERED_WINDOW_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn UpdateLayeredWindowIndirect(hwnd : super::super::Foundation:: HWND, pulwinfo : *const UPDATELAYEREDWINDOWINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WaitMessage() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WindowFromPhysicalPoint(point : super::super::Foundation:: POINT) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WindowFromPoint(point : super::super::Foundation:: POINT) -> super::super::Foundation:: HWND);
::windows_targets::link!("user32.dll" "cdecl" fn wsprintfA(param0 : ::windows_sys::core::PSTR, param1 : ::windows_sys::core::PCSTR, ...) -> i32);
::windows_targets::link!("user32.dll" "cdecl" fn wsprintfW(param0 : ::windows_sys::core::PWSTR, param1 : ::windows_sys::core::PCWSTR, ...) -> i32);
::windows_targets::link!("user32.dll" "system" fn wvsprintfA(param0 : ::windows_sys::core::PSTR, param1 : ::windows_sys::core::PCSTR, arglist : *const i8) -> i32);
::windows_targets::link!("user32.dll" "system" fn wvsprintfW(param0 : ::windows_sys::core::PWSTR, param1 : ::windows_sys::core::PCWSTR, arglist : *const i8) -> i32);
pub const ARW_BOTTOMLEFT: MINIMIZEDMETRICS_ARRANGE = 0i32;
pub const ARW_BOTTOMRIGHT: MINIMIZEDMETRICS_ARRANGE = 1i32;
pub const ARW_DOWN: i32 = 4i32;
pub const ARW_HIDE: i32 = 8i32;
pub const ARW_LEFT: i32 = 0i32;
pub const ARW_RIGHT: i32 = 0i32;
pub const ARW_STARTMASK: i32 = 3i32;
pub const ARW_STARTRIGHT: i32 = 1i32;
pub const ARW_STARTTOP: i32 = 2i32;
pub const ARW_TOPLEFT: MINIMIZEDMETRICS_ARRANGE = 2i32;
pub const ARW_TOPRIGHT: MINIMIZEDMETRICS_ARRANGE = 3i32;
pub const ARW_UP: i32 = 4i32;
pub const ASFW_ANY: u32 = 4294967295u32;
pub const AW_ACTIVATE: ANIMATE_WINDOW_FLAGS = 131072u32;
pub const AW_BLEND: ANIMATE_WINDOW_FLAGS = 524288u32;
pub const AW_CENTER: ANIMATE_WINDOW_FLAGS = 16u32;
pub const AW_HIDE: ANIMATE_WINDOW_FLAGS = 65536u32;
pub const AW_HOR_NEGATIVE: ANIMATE_WINDOW_FLAGS = 2u32;
pub const AW_HOR_POSITIVE: ANIMATE_WINDOW_FLAGS = 1u32;
pub const AW_SLIDE: ANIMATE_WINDOW_FLAGS = 262144u32;
pub const AW_VER_NEGATIVE: ANIMATE_WINDOW_FLAGS = 8u32;
pub const AW_VER_POSITIVE: ANIMATE_WINDOW_FLAGS = 4u32;
pub const BM_CLICK: u32 = 245u32;
pub const BM_GETCHECK: u32 = 240u32;
pub const BM_GETIMAGE: u32 = 246u32;
pub const BM_GETSTATE: u32 = 242u32;
pub const BM_SETCHECK: u32 = 241u32;
pub const BM_SETDONTCLICK: u32 = 248u32;
pub const BM_SETIMAGE: u32 = 247u32;
pub const BM_SETSTATE: u32 = 243u32;
pub const BM_SETSTYLE: u32 = 244u32;
pub const BN_CLICKED: u32 = 0u32;
pub const BN_DBLCLK: u32 = 5u32;
pub const BN_DISABLE: u32 = 4u32;
pub const BN_DOUBLECLICKED: u32 = 5u32;
pub const BN_HILITE: u32 = 2u32;
pub const BN_KILLFOCUS: u32 = 7u32;
pub const BN_PAINT: u32 = 1u32;
pub const BN_PUSHED: u32 = 2u32;
pub const BN_SETFOCUS: u32 = 6u32;
pub const BN_UNHILITE: u32 = 3u32;
pub const BN_UNPUSHED: u32 = 3u32;
pub const BROADCAST_QUERY_DENY: u32 = 1112363332u32;
pub const BSF_MSGSRV32ISOK: u32 = 2147483648u32;
pub const BSF_MSGSRV32ISOK_BIT: u32 = 31u32;
pub const BSM_INSTALLABLEDRIVERS: u32 = 4u32;
pub const BSM_NETDRIVER: u32 = 2u32;
pub const BSM_VXDS: u32 = 1u32;
pub const BST_FOCUS: u32 = 8u32;
pub const BST_PUSHED: u32 = 4u32;
pub const BS_3STATE: i32 = 5i32;
pub const BS_AUTO3STATE: i32 = 6i32;
pub const BS_AUTOCHECKBOX: i32 = 3i32;
pub const BS_AUTORADIOBUTTON: i32 = 9i32;
pub const BS_BITMAP: i32 = 128i32;
pub const BS_BOTTOM: i32 = 2048i32;
pub const BS_CENTER: i32 = 768i32;
pub const BS_CHECKBOX: i32 = 2i32;
pub const BS_DEFPUSHBUTTON: i32 = 1i32;
pub const BS_FLAT: i32 = 32768i32;
pub const BS_GROUPBOX: i32 = 7i32;
pub const BS_ICON: i32 = 64i32;
pub const BS_LEFT: i32 = 256i32;
pub const BS_LEFTTEXT: i32 = 32i32;
pub const BS_MULTILINE: i32 = 8192i32;
pub const BS_NOTIFY: i32 = 16384i32;
pub const BS_OWNERDRAW: i32 = 11i32;
pub const BS_PUSHBOX: i32 = 10i32;
pub const BS_PUSHBUTTON: i32 = 0i32;
pub const BS_PUSHLIKE: i32 = 4096i32;
pub const BS_RADIOBUTTON: i32 = 4i32;
pub const BS_RIGHT: i32 = 512i32;
pub const BS_RIGHTBUTTON: i32 = 32i32;
pub const BS_TEXT: i32 = 0i32;
pub const BS_TOP: i32 = 1024i32;
pub const BS_TYPEMASK: i32 = 15i32;
pub const BS_USERBUTTON: i32 = 8i32;
pub const BS_VCENTER: i32 = 3072i32;
pub const CALERT_SYSTEM: u32 = 6u32;
pub const CBN_CLOSEUP: u32 = 8u32;
pub const CBN_DBLCLK: u32 = 2u32;
pub const CBN_DROPDOWN: u32 = 7u32;
pub const CBN_EDITCHANGE: u32 = 5u32;
pub const CBN_EDITUPDATE: u32 = 6u32;
pub const CBN_ERRSPACE: i32 = -1i32;
pub const CBN_KILLFOCUS: u32 = 4u32;
pub const CBN_SELCHANGE: u32 = 1u32;
pub const CBN_SELENDCANCEL: u32 = 10u32;
pub const CBN_SELENDOK: u32 = 9u32;
pub const CBN_SETFOCUS: u32 = 3u32;
pub const CBS_AUTOHSCROLL: i32 = 64i32;
pub const CBS_DISABLENOSCROLL: i32 = 2048i32;
pub const CBS_DROPDOWN: i32 = 2i32;
pub const CBS_DROPDOWNLIST: i32 = 3i32;
pub const CBS_HASSTRINGS: i32 = 512i32;
pub const CBS_LOWERCASE: i32 = 16384i32;
pub const CBS_NOINTEGRALHEIGHT: i32 = 1024i32;
pub const CBS_OEMCONVERT: i32 = 128i32;
pub const CBS_OWNERDRAWFIXED: i32 = 16i32;
pub const CBS_OWNERDRAWVARIABLE: i32 = 32i32;
pub const CBS_SIMPLE: i32 = 1i32;
pub const CBS_SORT: i32 = 256i32;
pub const CBS_UPPERCASE: i32 = 8192i32;
pub const CB_ADDSTRING: u32 = 323u32;
pub const CB_DELETESTRING: u32 = 324u32;
pub const CB_DIR: u32 = 325u32;
pub const CB_ERR: i32 = -1i32;
pub const CB_ERRSPACE: i32 = -2i32;
pub const CB_FINDSTRING: u32 = 332u32;
pub const CB_FINDSTRINGEXACT: u32 = 344u32;
pub const CB_GETCOMBOBOXINFO: u32 = 356u32;
pub const CB_GETCOUNT: u32 = 326u32;
pub const CB_GETCURSEL: u32 = 327u32;
pub const CB_GETDROPPEDCONTROLRECT: u32 = 338u32;
pub const CB_GETDROPPEDSTATE: u32 = 343u32;
pub const CB_GETDROPPEDWIDTH: u32 = 351u32;
pub const CB_GETEDITSEL: u32 = 320u32;
pub const CB_GETEXTENDEDUI: u32 = 342u32;
pub const CB_GETHORIZONTALEXTENT: u32 = 349u32;
pub const CB_GETITEMDATA: u32 = 336u32;
pub const CB_GETITEMHEIGHT: u32 = 340u32;
pub const CB_GETLBTEXT: u32 = 328u32;
pub const CB_GETLBTEXTLEN: u32 = 329u32;
pub const CB_GETLOCALE: u32 = 346u32;
pub const CB_GETTOPINDEX: u32 = 347u32;
pub const CB_INITSTORAGE: u32 = 353u32;
pub const CB_INSERTSTRING: u32 = 330u32;
pub const CB_LIMITTEXT: u32 = 321u32;
pub const CB_MSGMAX: u32 = 357u32;
pub const CB_MULTIPLEADDSTRING: u32 = 355u32;
pub const CB_OKAY: u32 = 0u32;
pub const CB_RESETCONTENT: u32 = 331u32;
pub const CB_SELECTSTRING: u32 = 333u32;
pub const CB_SETCURSEL: u32 = 334u32;
pub const CB_SETDROPPEDWIDTH: u32 = 352u32;
pub const CB_SETEDITSEL: u32 = 322u32;
pub const CB_SETEXTENDEDUI: u32 = 341u32;
pub const CB_SETHORIZONTALEXTENT: u32 = 350u32;
pub const CB_SETITEMDATA: u32 = 337u32;
pub const CB_SETITEMHEIGHT: u32 = 339u32;
pub const CB_SETLOCALE: u32 = 345u32;
pub const CB_SETTOPINDEX: u32 = 348u32;
pub const CB_SHOWDROPDOWN: u32 = 335u32;
pub const CCHILDREN_SCROLLBAR: u32 = 5u32;
pub const CCHILDREN_TITLEBAR: u32 = 5u32;
pub const CHILDID_SELF: u32 = 0u32;
pub const CONSOLE_APPLICATION_16BIT: u32 = 0u32;
pub const CONSOLE_CARET_SELECTION: u32 = 1u32;
pub const CONSOLE_CARET_VISIBLE: u32 = 2u32;
pub const CONTACTVISUALIZATION_OFF: u32 = 0u32;
pub const CONTACTVISUALIZATION_ON: u32 = 1u32;
pub const CONTACTVISUALIZATION_PRESENTATIONMODE: u32 = 2u32;
pub const CREATEPROCESS_MANIFEST_RESOURCE_ID: u32 = 1u32;
pub const CSOUND_SYSTEM: u32 = 16u32;
pub const CS_BYTEALIGNCLIENT: WNDCLASS_STYLES = 4096u32;
pub const CS_BYTEALIGNWINDOW: WNDCLASS_STYLES = 8192u32;
pub const CS_CLASSDC: WNDCLASS_STYLES = 64u32;
pub const CS_DBLCLKS: WNDCLASS_STYLES = 8u32;
pub const CS_DROPSHADOW: WNDCLASS_STYLES = 131072u32;
pub const CS_GLOBALCLASS: WNDCLASS_STYLES = 16384u32;
pub const CS_HREDRAW: WNDCLASS_STYLES = 2u32;
pub const CS_IME: WNDCLASS_STYLES = 65536u32;
pub const CS_NOCLOSE: WNDCLASS_STYLES = 512u32;
pub const CS_OWNDC: WNDCLASS_STYLES = 32u32;
pub const CS_PARENTDC: WNDCLASS_STYLES = 128u32;
pub const CS_SAVEBITS: WNDCLASS_STYLES = 2048u32;
pub const CS_VREDRAW: WNDCLASS_STYLES = 1u32;
pub const CTLCOLOR_BTN: u32 = 3u32;
pub const CTLCOLOR_DLG: u32 = 4u32;
pub const CTLCOLOR_EDIT: u32 = 1u32;
pub const CTLCOLOR_LISTBOX: u32 = 2u32;
pub const CTLCOLOR_MAX: u32 = 7u32;
pub const CTLCOLOR_MSGBOX: u32 = 0u32;
pub const CTLCOLOR_SCROLLBAR: u32 = 5u32;
pub const CTLCOLOR_STATIC: u32 = 6u32;
pub const CURSOR_CREATION_SCALING_DEFAULT: u32 = 2u32;
pub const CURSOR_CREATION_SCALING_NONE: u32 = 1u32;
pub const CURSOR_SHOWING: CURSORINFO_FLAGS = 1u32;
pub const CURSOR_SUPPRESSED: CURSORINFO_FLAGS = 2u32;
pub const CWF_CREATE_ONLY: u32 = 1u32;
pub const CWP_ALL: CWP_FLAGS = 0u32;
pub const CWP_SKIPDISABLED: CWP_FLAGS = 2u32;
pub const CWP_SKIPINVISIBLE: CWP_FLAGS = 1u32;
pub const CWP_SKIPTRANSPARENT: CWP_FLAGS = 4u32;
pub const CW_USEDEFAULT: i32 = -2147483648i32;
pub const DBTF_MEDIA: DEV_BROADCAST_VOLUME_FLAGS = 1u16;
pub const DBTF_NET: DEV_BROADCAST_VOLUME_FLAGS = 2u16;
pub const DBTF_RESOURCE: u32 = 1u32;
pub const DBTF_SLOWNET: u32 = 4u32;
pub const DBTF_XPORT: u32 = 2u32;
pub const DBT_APPYBEGIN: u32 = 0u32;
pub const DBT_APPYEND: u32 = 1u32;
pub const DBT_CONFIGCHANGECANCELED: u32 = 25u32;
pub const DBT_CONFIGCHANGED: u32 = 24u32;
pub const DBT_CONFIGMGAPI32: u32 = 34u32;
pub const DBT_CONFIGMGPRIVATE: u32 = 32767u32;
pub const DBT_CUSTOMEVENT: u32 = 32774u32;
pub const DBT_DEVICEARRIVAL: u32 = 32768u32;
pub const DBT_DEVICEQUERYREMOVE: u32 = 32769u32;
pub const DBT_DEVICEQUERYREMOVEFAILED: u32 = 32770u32;
pub const DBT_DEVICEREMOVECOMPLETE: u32 = 32772u32;
pub const DBT_DEVICEREMOVEPENDING: u32 = 32771u32;
pub const DBT_DEVICETYPESPECIFIC: u32 = 32773u32;
pub const DBT_DEVNODES_CHANGED: u32 = 7u32;
pub const DBT_DEVTYP_DEVICEINTERFACE: DEV_BROADCAST_HDR_DEVICE_TYPE = 5u32;
pub const DBT_DEVTYP_DEVNODE: u32 = 1u32;
pub const DBT_DEVTYP_HANDLE: DEV_BROADCAST_HDR_DEVICE_TYPE = 6u32;
pub const DBT_DEVTYP_NET: u32 = 4u32;
pub const DBT_DEVTYP_OEM: DEV_BROADCAST_HDR_DEVICE_TYPE = 0u32;
pub const DBT_DEVTYP_PORT: DEV_BROADCAST_HDR_DEVICE_TYPE = 3u32;
pub const DBT_DEVTYP_VOLUME: DEV_BROADCAST_HDR_DEVICE_TYPE = 2u32;
pub const DBT_LOW_DISK_SPACE: u32 = 72u32;
pub const DBT_MONITORCHANGE: u32 = 27u32;
pub const DBT_NO_DISK_SPACE: u32 = 71u32;
pub const DBT_QUERYCHANGECONFIG: u32 = 23u32;
pub const DBT_SHELLLOGGEDON: u32 = 32u32;
pub const DBT_USERDEFINED: u32 = 65535u32;
pub const DBT_VOLLOCKLOCKFAILED: u32 = 32835u32;
pub const DBT_VOLLOCKLOCKRELEASED: u32 = 32837u32;
pub const DBT_VOLLOCKLOCKTAKEN: u32 = 32834u32;
pub const DBT_VOLLOCKQUERYLOCK: u32 = 32833u32;
pub const DBT_VOLLOCKQUERYUNLOCK: u32 = 32836u32;
pub const DBT_VOLLOCKUNLOCKFAILED: u32 = 32838u32;
pub const DBT_VPOWERDAPI: u32 = 33024u32;
pub const DBT_VXDINITCOMPLETE: u32 = 35u32;
pub const DCX_EXCLUDEUPDATE: i32 = 256i32;
pub const DC_HASDEFID: u32 = 21323u32;
pub const DEVICE_NOTIFY_ALL_INTERFACE_CLASSES: REGISTER_NOTIFICATION_FLAGS = 4u32;
pub const DEVICE_NOTIFY_CALLBACK: REGISTER_NOTIFICATION_FLAGS = 2u32;
pub const DEVICE_NOTIFY_SERVICE_HANDLE: REGISTER_NOTIFICATION_FLAGS = 1u32;
pub const DEVICE_NOTIFY_WINDOW_HANDLE: REGISTER_NOTIFICATION_FLAGS = 0u32;
pub const DIFFERENCE: u32 = 11u32;
pub const DI_COMPAT: DI_FLAGS = 4u32;
pub const DI_DEFAULTSIZE: DI_FLAGS = 8u32;
pub const DI_IMAGE: DI_FLAGS = 2u32;
pub const DI_MASK: DI_FLAGS = 1u32;
pub const DI_NOMIRROR: DI_FLAGS = 16u32;
pub const DI_NORMAL: DI_FLAGS = 3u32;
pub const DLGC_BUTTON: u32 = 8192u32;
pub const DLGC_DEFPUSHBUTTON: u32 = 16u32;
pub const DLGC_HASSETSEL: u32 = 8u32;
pub const DLGC_RADIOBUTTON: u32 = 64u32;
pub const DLGC_STATIC: u32 = 256u32;
pub const DLGC_UNDEFPUSHBUTTON: u32 = 32u32;
pub const DLGC_WANTALLKEYS: u32 = 4u32;
pub const DLGC_WANTARROWS: u32 = 1u32;
pub const DLGC_WANTCHARS: u32 = 128u32;
pub const DLGC_WANTMESSAGE: u32 = 4u32;
pub const DLGC_WANTTAB: u32 = 2u32;
pub const DLGWINDOWEXTRA: u32 = 30u32;
pub const DM_GETDEFID: u32 = 1024u32;
pub const DM_POINTERHITTEST: u32 = 592u32;
pub const DM_REPOSITION: u32 = 1026u32;
pub const DM_SETDEFID: u32 = 1025u32;
pub const DOF_DIRECTORY: u32 = 32771u32;
pub const DOF_DOCUMENT: u32 = 32770u32;
pub const DOF_EXECUTABLE: u32 = 32769u32;
pub const DOF_MULTIPLE: u32 = 32772u32;
pub const DOF_PROGMAN: u32 = 1u32;
pub const DOF_SHELLDATA: u32 = 2u32;
pub const DO_DROPFILE: i32 = 1162627398i32;
pub const DO_PRINTFILE: i32 = 1414419024i32;
pub const DS_3DLOOK: i32 = 4i32;
pub const DS_ABSALIGN: i32 = 1i32;
pub const DS_CENTER: i32 = 2048i32;
pub const DS_CENTERMOUSE: i32 = 4096i32;
pub const DS_CONTEXTHELP: i32 = 8192i32;
pub const DS_CONTROL: i32 = 1024i32;
pub const DS_FIXEDSYS: i32 = 8i32;
pub const DS_LOCALEDIT: i32 = 32i32;
pub const DS_MODALFRAME: i32 = 128i32;
pub const DS_NOFAILCREATE: i32 = 16i32;
pub const DS_NOIDLEMSG: i32 = 256i32;
pub const DS_SETFONT: i32 = 64i32;
pub const DS_SETFOREGROUND: i32 = 512i32;
pub const DS_SYSMODAL: i32 = 2i32;
pub const DS_USEPIXELS: i32 = 32768i32;
pub const DWLP_MSGRESULT: u32 = 0u32;
pub const DWL_DLGPROC: u32 = 4u32;
pub const DWL_MSGRESULT: u32 = 0u32;
pub const DWL_USER: u32 = 8u32;
pub const EC_LEFTMARGIN: u32 = 1u32;
pub const EC_RIGHTMARGIN: u32 = 2u32;
pub const EC_USEFONTINFO: u32 = 65535u32;
pub const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 1u32;
pub const EDIT_CONTROL_FEATURE_ENTERPRISE_DATA_PROTECTION_PASTE_SUPPORT: EDIT_CONTROL_FEATURE = 0i32;
pub const EDIT_CONTROL_FEATURE_PASTE_NOTIFICATIONS: EDIT_CONTROL_FEATURE = 1i32;
pub const EIMES_CANCELCOMPSTRINFOCUS: u32 = 2u32;
pub const EIMES_COMPLETECOMPSTRKILLFOCUS: u32 = 4u32;
pub const EIMES_GETCOMPSTRATONCE: u32 = 1u32;
pub const EMSIS_COMPOSITIONSTRING: u32 = 1u32;
pub const ENDSESSION_CLOSEAPP: u32 = 1u32;
pub const ENDSESSION_CRITICAL: u32 = 1073741824u32;
pub const ENDSESSION_LOGOFF: u32 = 2147483648u32;
pub const EN_AFTER_PASTE: u32 = 2049u32;
pub const EN_ALIGN_LTR_EC: u32 = 1792u32;
pub const EN_ALIGN_RTL_EC: u32 = 1793u32;
pub const EN_BEFORE_PASTE: u32 = 2048u32;
pub const EN_CHANGE: u32 = 768u32;
pub const EN_ERRSPACE: u32 = 1280u32;
pub const EN_HSCROLL: u32 = 1537u32;
pub const EN_KILLFOCUS: u32 = 512u32;
pub const EN_MAXTEXT: u32 = 1281u32;
pub const EN_SETFOCUS: u32 = 256u32;
pub const EN_UPDATE: u32 = 1024u32;
pub const EN_VSCROLL: u32 = 1538u32;
pub const ES_AUTOHSCROLL: i32 = 128i32;
pub const ES_AUTOVSCROLL: i32 = 64i32;
pub const ES_CENTER: i32 = 1i32;
pub const ES_LEFT: i32 = 0i32;
pub const ES_LOWERCASE: i32 = 16i32;
pub const ES_MULTILINE: i32 = 4i32;
pub const ES_NOHIDESEL: i32 = 256i32;
pub const ES_NUMBER: i32 = 8192i32;
pub const ES_OEMCONVERT: i32 = 1024i32;
pub const ES_PASSWORD: i32 = 32i32;
pub const ES_READONLY: i32 = 2048i32;
pub const ES_RIGHT: i32 = 2i32;
pub const ES_UPPERCASE: i32 = 8i32;
pub const ES_WANTRETURN: i32 = 4096i32;
pub const EVENT_AIA_END: u32 = 45055u32;
pub const EVENT_AIA_START: u32 = 40960u32;
pub const EVENT_CONSOLE_CARET: u32 = 16385u32;
pub const EVENT_CONSOLE_END: u32 = 16639u32;
pub const EVENT_CONSOLE_END_APPLICATION: u32 = 16391u32;
pub const EVENT_CONSOLE_LAYOUT: u32 = 16389u32;
pub const EVENT_CONSOLE_START_APPLICATION: u32 = 16390u32;
pub const EVENT_CONSOLE_UPDATE_REGION: u32 = 16386u32;
pub const EVENT_CONSOLE_UPDATE_SCROLL: u32 = 16388u32;
pub const EVENT_CONSOLE_UPDATE_SIMPLE: u32 = 16387u32;
pub const EVENT_MAX: u32 = 2147483647u32;
pub const EVENT_MIN: u32 = 1u32;
pub const EVENT_OBJECT_ACCELERATORCHANGE: u32 = 32786u32;
pub const EVENT_OBJECT_CLOAKED: u32 = 32791u32;
pub const EVENT_OBJECT_CONTENTSCROLLED: u32 = 32789u32;
pub const EVENT_OBJECT_CREATE: u32 = 32768u32;
pub const EVENT_OBJECT_DEFACTIONCHANGE: u32 = 32785u32;
pub const EVENT_OBJECT_DESCRIPTIONCHANGE: u32 = 32781u32;
pub const EVENT_OBJECT_DESTROY: u32 = 32769u32;
pub const EVENT_OBJECT_DRAGCANCEL: u32 = 32802u32;
pub const EVENT_OBJECT_DRAGCOMPLETE: u32 = 32803u32;
pub const EVENT_OBJECT_DRAGDROPPED: u32 = 32806u32;
pub const EVENT_OBJECT_DRAGENTER: u32 = 32804u32;
pub const EVENT_OBJECT_DRAGLEAVE: u32 = 32805u32;
pub const EVENT_OBJECT_DRAGSTART: u32 = 32801u32;
pub const EVENT_OBJECT_END: u32 = 33023u32;
pub const EVENT_OBJECT_FOCUS: u32 = 32773u32;
pub const EVENT_OBJECT_HELPCHANGE: u32 = 32784u32;
pub const EVENT_OBJECT_HIDE: u32 = 32771u32;
pub const EVENT_OBJECT_HOSTEDOBJECTSINVALIDATED: u32 = 32800u32;
pub const EVENT_OBJECT_IME_CHANGE: u32 = 32809u32;
pub const EVENT_OBJECT_IME_HIDE: u32 = 32808u32;
pub const EVENT_OBJECT_IME_SHOW: u32 = 32807u32;
pub const EVENT_OBJECT_INVOKED: u32 = 32787u32;
pub const EVENT_OBJECT_LIVEREGIONCHANGED: u32 = 32793u32;
pub const EVENT_OBJECT_LOCATIONCHANGE: u32 = 32779u32;
pub const EVENT_OBJECT_NAMECHANGE: u32 = 32780u32;
pub const EVENT_OBJECT_PARENTCHANGE: u32 = 32783u32;
pub const EVENT_OBJECT_REORDER: u32 = 32772u32;
pub const EVENT_OBJECT_SELECTION: u32 = 32774u32;
pub const EVENT_OBJECT_SELECTIONADD: u32 = 32775u32;
pub const EVENT_OBJECT_SELECTIONREMOVE: u32 = 32776u32;
pub const EVENT_OBJECT_SELECTIONWITHIN: u32 = 32777u32;
pub const EVENT_OBJECT_SHOW: u32 = 32770u32;
pub const EVENT_OBJECT_STATECHANGE: u32 = 32778u32;
pub const EVENT_OBJECT_TEXTEDIT_CONVERSIONTARGETCHANGED: u32 = 32816u32;
pub const EVENT_OBJECT_TEXTSELECTIONCHANGED: u32 = 32788u32;
pub const EVENT_OBJECT_UNCLOAKED: u32 = 32792u32;
pub const EVENT_OBJECT_VALUECHANGE: u32 = 32782u32;
pub const EVENT_OEM_DEFINED_END: u32 = 511u32;
pub const EVENT_OEM_DEFINED_START: u32 = 257u32;
pub const EVENT_SYSTEM_ALERT: u32 = 2u32;
pub const EVENT_SYSTEM_ARRANGMENTPREVIEW: u32 = 32790u32;
pub const EVENT_SYSTEM_CAPTUREEND: u32 = 9u32;
pub const EVENT_SYSTEM_CAPTURESTART: u32 = 8u32;
pub const EVENT_SYSTEM_CONTEXTHELPEND: u32 = 13u32;
pub const EVENT_SYSTEM_CONTEXTHELPSTART: u32 = 12u32;
pub const EVENT_SYSTEM_DESKTOPSWITCH: u32 = 32u32;
pub const EVENT_SYSTEM_DIALOGEND: u32 = 17u32;
pub const EVENT_SYSTEM_DIALOGSTART: u32 = 16u32;
pub const EVENT_SYSTEM_DRAGDROPEND: u32 = 15u32;
pub const EVENT_SYSTEM_DRAGDROPSTART: u32 = 14u32;
pub const EVENT_SYSTEM_END: u32 = 255u32;
pub const EVENT_SYSTEM_FOREGROUND: u32 = 3u32;
pub const EVENT_SYSTEM_IME_KEY_NOTIFICATION: u32 = 41u32;
pub const EVENT_SYSTEM_MENUEND: u32 = 5u32;
pub const EVENT_SYSTEM_MENUPOPUPEND: u32 = 7u32;
pub const EVENT_SYSTEM_MENUPOPUPSTART: u32 = 6u32;
pub const EVENT_SYSTEM_MENUSTART: u32 = 4u32;
pub const EVENT_SYSTEM_MINIMIZEEND: u32 = 23u32;
pub const EVENT_SYSTEM_MINIMIZESTART: u32 = 22u32;
pub const EVENT_SYSTEM_MOVESIZEEND: u32 = 11u32;
pub const EVENT_SYSTEM_MOVESIZESTART: u32 = 10u32;
pub const EVENT_SYSTEM_SCROLLINGEND: u32 = 19u32;
pub const EVENT_SYSTEM_SCROLLINGSTART: u32 = 18u32;
pub const EVENT_SYSTEM_SOUND: u32 = 1u32;
pub const EVENT_SYSTEM_SWITCHEND: u32 = 21u32;
pub const EVENT_SYSTEM_SWITCHER_APPDROPPED: u32 = 38u32;
pub const EVENT_SYSTEM_SWITCHER_APPGRABBED: u32 = 36u32;
pub const EVENT_SYSTEM_SWITCHER_APPOVERTARGET: u32 = 37u32;
pub const EVENT_SYSTEM_SWITCHER_CANCELLED: u32 = 39u32;
pub const EVENT_SYSTEM_SWITCHSTART: u32 = 20u32;
pub const EVENT_UIA_EVENTID_END: u32 = 20223u32;
pub const EVENT_UIA_EVENTID_START: u32 = 19968u32;
pub const EVENT_UIA_PROPID_END: u32 = 30207u32;
pub const EVENT_UIA_PROPID_START: u32 = 29952u32;
pub const FALT: ACCEL_VIRT_FLAGS = 16u8;
pub const FAPPCOMMAND_KEY: u32 = 0u32;
pub const FAPPCOMMAND_MASK: u32 = 61440u32;
pub const FAPPCOMMAND_MOUSE: u32 = 32768u32;
pub const FAPPCOMMAND_OEM: u32 = 4096u32;
pub const FCONTROL: ACCEL_VIRT_FLAGS = 8u8;
pub const FE_FONTSMOOTHINGCLEARTYPE: u32 = 2u32;
pub const FE_FONTSMOOTHINGORIENTATIONBGR: u32 = 0u32;
pub const FE_FONTSMOOTHINGORIENTATIONRGB: u32 = 1u32;
pub const FE_FONTSMOOTHINGSTANDARD: u32 = 1u32;
pub const FKF_AVAILABLE: u32 = 2u32;
pub const FKF_CLICKON: u32 = 64u32;
pub const FKF_CONFIRMHOTKEY: u32 = 8u32;
pub const FKF_FILTERKEYSON: u32 = 1u32;
pub const FKF_HOTKEYACTIVE: u32 = 4u32;
pub const FKF_HOTKEYSOUND: u32 = 16u32;
pub const FKF_INDICATOR: u32 = 32u32;
pub const FLASHW_ALL: FLASHWINFO_FLAGS = 3u32;
pub const FLASHW_CAPTION: FLASHWINFO_FLAGS = 1u32;
pub const FLASHW_STOP: FLASHWINFO_FLAGS = 0u32;
pub const FLASHW_TIMER: FLASHWINFO_FLAGS = 4u32;
pub const FLASHW_TIMERNOFG: FLASHWINFO_FLAGS = 12u32;
pub const FLASHW_TRAY: FLASHWINFO_FLAGS = 2u32;
pub const FNOINVERT: ACCEL_VIRT_FLAGS = 2u8;
pub const FSHIFT: ACCEL_VIRT_FLAGS = 4u8;
pub const FVIRTKEY: ACCEL_VIRT_FLAGS = 1u8;
pub const GA_PARENT: GET_ANCESTOR_FLAGS = 1u32;
pub const GA_ROOT: GET_ANCESTOR_FLAGS = 2u32;
pub const GA_ROOTOWNER: GET_ANCESTOR_FLAGS = 3u32;
pub const GCF_INCLUDE_ANCESTORS: u32 = 1u32;
pub const GCLP_HBRBACKGROUND: GET_CLASS_LONG_INDEX = -10i32;
pub const GCLP_HCURSOR: GET_CLASS_LONG_INDEX = -12i32;
pub const GCLP_HICON: GET_CLASS_LONG_INDEX = -14i32;
pub const GCLP_HICONSM: GET_CLASS_LONG_INDEX = -34i32;
pub const GCLP_HMODULE: GET_CLASS_LONG_INDEX = -16i32;
pub const GCLP_MENUNAME: GET_CLASS_LONG_INDEX = -8i32;
pub const GCLP_WNDPROC: GET_CLASS_LONG_INDEX = -24i32;
pub const GCL_CBCLSEXTRA: GET_CLASS_LONG_INDEX = -20i32;
pub const GCL_CBWNDEXTRA: GET_CLASS_LONG_INDEX = -18i32;
pub const GCL_HBRBACKGROUND: GET_CLASS_LONG_INDEX = -10i32;
pub const GCL_HCURSOR: GET_CLASS_LONG_INDEX = -12i32;
pub const GCL_HICON: GET_CLASS_LONG_INDEX = -14i32;
pub const GCL_HICONSM: GET_CLASS_LONG_INDEX = -34i32;
pub const GCL_HMODULE: GET_CLASS_LONG_INDEX = -16i32;
pub const GCL_MENUNAME: GET_CLASS_LONG_INDEX = -8i32;
pub const GCL_STYLE: GET_CLASS_LONG_INDEX = -26i32;
pub const GCL_WNDPROC: GET_CLASS_LONG_INDEX = -24i32;
pub const GCW_ATOM: GET_CLASS_LONG_INDEX = -32i32;
pub const GESTURECONFIGMAXCOUNT: u32 = 256u32;
pub const GESTUREVISUALIZATION_DOUBLETAP: u32 = 2u32;
pub const GESTUREVISUALIZATION_OFF: u32 = 0u32;
pub const GESTUREVISUALIZATION_ON: u32 = 31u32;
pub const GESTUREVISUALIZATION_PRESSANDHOLD: u32 = 8u32;
pub const GESTUREVISUALIZATION_PRESSANDTAP: u32 = 4u32;
pub const GESTUREVISUALIZATION_RIGHTTAP: u32 = 16u32;
pub const GESTUREVISUALIZATION_TAP: u32 = 1u32;
pub const GF_BEGIN: u32 = 1u32;
pub const GF_END: u32 = 4u32;
pub const GF_INERTIA: u32 = 2u32;
pub const GIDC_ARRIVAL: u32 = 1u32;
pub const GIDC_REMOVAL: u32 = 2u32;
pub const GMDI_GOINTOPOPUPS: GET_MENU_DEFAULT_ITEM_FLAGS = 2u32;
pub const GMDI_USEDISABLED: GET_MENU_DEFAULT_ITEM_FLAGS = 1u32;
pub const GUID_DEVICE_EVENT_RBC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd0744792_a98e_11d2_917a_00a0c9068ff3);
pub const GUID_IO_CDROM_EXCLUSIVE_LOCK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbc56c139_7a10_47ee_a294_4c6a38f0149a);
pub const GUID_IO_CDROM_EXCLUSIVE_UNLOCK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa3b6d27d_5e35_4885_81e5_ee18c00ed779);
pub const GUID_IO_DEVICE_BECOMING_READY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd07433f0_a98e_11d2_917a_00a0c9068ff3);
pub const GUID_IO_DEVICE_EXTERNAL_REQUEST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd07433d0_a98e_11d2_917a_00a0c9068ff3);
pub const GUID_IO_DISK_CLONE_ARRIVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6a61885b_7c39_43dd_9b56_b8ac22a549aa);
pub const GUID_IO_DISK_HEALTH_NOTIFICATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0f1bd644_3916_49c5_b063_991940118fb2);
pub const GUID_IO_DISK_LAYOUT_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x11dff54c_8469_41f9_b3de_ef836487c54a);
pub const GUID_IO_DRIVE_REQUIRES_CLEANING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7207877c_90ed_44e5_a000_81428d4c79bb);
pub const GUID_IO_MEDIA_ARRIVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd07433c0_a98e_11d2_917a_00a0c9068ff3);
pub const GUID_IO_MEDIA_EJECT_REQUEST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd07433d1_a98e_11d2_917a_00a0c9068ff3);
pub const GUID_IO_MEDIA_REMOVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd07433c1_a98e_11d2_917a_00a0c9068ff3);
pub const GUID_IO_TAPE_ERASE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x852d11eb_4bb8_4507_9d9b_417cc2b1b438);
pub const GUID_IO_VOLUME_BACKGROUND_FORMAT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa2e5fc86_d5cd_4038_b2e3_4445065c2377);
pub const GUID_IO_VOLUME_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7373654a_812a_11d0_bec7_08002be2092f);
pub const GUID_IO_VOLUME_CHANGE_SIZE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3a1625be_ad03_49f1_8ef8_6bbac182d1fd);
pub const GUID_IO_VOLUME_DEVICE_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x53f5630d_b6bf_11d0_94f2_00a0c91efb8b);
pub const GUID_IO_VOLUME_DISMOUNT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd16a55e8_1059_11d2_8ffd_00a0c9a06d32);
pub const GUID_IO_VOLUME_DISMOUNT_FAILED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe3c5b178_105d_11d2_8ffd_00a0c9a06d32);
pub const GUID_IO_VOLUME_FORCE_CLOSED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x411ad84f_433e_4dc2_a5ae_4a2d1a2de654);
pub const GUID_IO_VOLUME_FVE_STATUS_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x062998b2_ee1f_4b6a_b857_e76cbbe9a6da);
pub const GUID_IO_VOLUME_INFO_MAKE_COMPAT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3ab9a0d2_ef80_45cf_8cdc_cbe02a212906);
pub const GUID_IO_VOLUME_LOCK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x50708874_c9af_11d1_8fef_00a0c9a06d32);
pub const GUID_IO_VOLUME_LOCK_FAILED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xae2eed10_0ba8_11d2_8ffb_00a0c9a06d32);
pub const GUID_IO_VOLUME_MOUNT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb5804878_1a96_11d2_8ffd_00a0c9a06d32);
pub const GUID_IO_VOLUME_NAME_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2de97f83_4c06_11d2_a532_00609713055a);
pub const GUID_IO_VOLUME_NEED_CHKDSK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x799a0960_0a0b_4e03_ad88_2fa7c6ce748a);
pub const GUID_IO_VOLUME_PHYSICAL_CONFIGURATION_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2de97f84_4c06_11d2_a532_00609713055a);
pub const GUID_IO_VOLUME_PREPARING_EJECT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc79eb16e_0dac_4e7a_a86c_b25ceeaa88f6);
pub const GUID_IO_VOLUME_UNIQUE_ID_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaf39da42_6622_41f5_970b_139d092fa3d9);
pub const GUID_IO_VOLUME_UNLOCK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9a8c3d68_d0cb_11d1_8fef_00a0c9a06d32);
pub const GUID_IO_VOLUME_WEARING_OUT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x873113ca_1486_4508_82ac_c3b2e5297aaa);
pub const GUID_IO_VOLUME_WORM_NEAR_FULL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf3bfff82_f3de_48d2_af95_457f80b763f2);
pub const GUI_16BITTASK: u32 = 0u32;
pub const GUI_CARETBLINKING: GUITHREADINFO_FLAGS = 1u32;
pub const GUI_INMENUMODE: GUITHREADINFO_FLAGS = 4u32;
pub const GUI_INMOVESIZE: GUITHREADINFO_FLAGS = 2u32;
pub const GUI_POPUPMENUMODE: GUITHREADINFO_FLAGS = 16u32;
pub const GUI_SYSTEMMENUMODE: GUITHREADINFO_FLAGS = 8u32;
pub const GWFS_INCLUDE_ANCESTORS: u32 = 1u32;
pub const GWLP_HINSTANCE: WINDOW_LONG_PTR_INDEX = -6i32;
pub const GWLP_HWNDPARENT: WINDOW_LONG_PTR_INDEX = -8i32;
pub const GWLP_ID: WINDOW_LONG_PTR_INDEX = -12i32;
pub const GWLP_USERDATA: WINDOW_LONG_PTR_INDEX = -21i32;
pub const GWLP_WNDPROC: WINDOW_LONG_PTR_INDEX = -4i32;
pub const GWL_EXSTYLE: WINDOW_LONG_PTR_INDEX = -20i32;
pub const GWL_HINSTANCE: WINDOW_LONG_PTR_INDEX = -6i32;
pub const GWL_HWNDPARENT: WINDOW_LONG_PTR_INDEX = -8i32;
pub const GWL_ID: WINDOW_LONG_PTR_INDEX = -12i32;
pub const GWL_STYLE: WINDOW_LONG_PTR_INDEX = -16i32;
pub const GWL_USERDATA: WINDOW_LONG_PTR_INDEX = -21i32;
pub const GWL_WNDPROC: WINDOW_LONG_PTR_INDEX = -4i32;
pub const GW_CHILD: GET_WINDOW_CMD = 5u32;
pub const GW_ENABLEDPOPUP: GET_WINDOW_CMD = 6u32;
pub const GW_HWNDFIRST: GET_WINDOW_CMD = 0u32;
pub const GW_HWNDLAST: GET_WINDOW_CMD = 1u32;
pub const GW_HWNDNEXT: GET_WINDOW_CMD = 2u32;
pub const GW_HWNDPREV: GET_WINDOW_CMD = 3u32;
pub const GW_MAX: u32 = 5u32;
pub const GW_OWNER: GET_WINDOW_CMD = 4u32;
pub const HANDEDNESS_LEFT: HANDEDNESS = 0i32;
pub const HANDEDNESS_RIGHT: HANDEDNESS = 1i32;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_CALLBACK: super::super::Graphics::Gdi::HBITMAP = -1i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_MBAR_CLOSE: super::super::Graphics::Gdi::HBITMAP = 5i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_MBAR_CLOSE_D: super::super::Graphics::Gdi::HBITMAP = 6i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_MBAR_MINIMIZE: super::super::Graphics::Gdi::HBITMAP = 3i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_MBAR_MINIMIZE_D: super::super::Graphics::Gdi::HBITMAP = 7i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_MBAR_RESTORE: super::super::Graphics::Gdi::HBITMAP = 2i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_POPUP_CLOSE: super::super::Graphics::Gdi::HBITMAP = 8i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_POPUP_MAXIMIZE: super::super::Graphics::Gdi::HBITMAP = 10i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_POPUP_MINIMIZE: super::super::Graphics::Gdi::HBITMAP = 11i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_POPUP_RESTORE: super::super::Graphics::Gdi::HBITMAP = 9i32 as _;
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub const HBMMENU_SYSTEM: super::super::Graphics::Gdi::HBITMAP = 1i32 as _;
pub const HCBT_ACTIVATE: u32 = 5u32;
pub const HCBT_CLICKSKIPPED: u32 = 6u32;
pub const HCBT_CREATEWND: u32 = 3u32;
pub const HCBT_DESTROYWND: u32 = 4u32;
pub const HCBT_KEYSKIPPED: u32 = 7u32;
pub const HCBT_MINMAX: u32 = 1u32;
pub const HCBT_MOVESIZE: u32 = 0u32;
pub const HCBT_QS: u32 = 2u32;
pub const HCBT_SETFOCUS: u32 = 9u32;
pub const HCBT_SYSCOMMAND: u32 = 8u32;
pub const HCF_DEFAULTDESKTOP: u32 = 512u32;
pub const HCF_LOGONDESKTOP: u32 = 256u32;
pub const HC_ACTION: u32 = 0u32;
pub const HC_GETNEXT: u32 = 1u32;
pub const HC_NOREM: u32 = 3u32;
pub const HC_NOREMOVE: u32 = 3u32;
pub const HC_SKIP: u32 = 2u32;
pub const HC_SYSMODALOFF: u32 = 5u32;
pub const HC_SYSMODALON: u32 = 4u32;
pub const HELP_COMMAND: i32 = 258i32;
pub const HELP_CONTENTS: i32 = 3i32;
pub const HELP_CONTEXT: i32 = 1i32;
pub const HELP_CONTEXTMENU: u32 = 10u32;
pub const HELP_CONTEXTPOPUP: i32 = 8i32;
pub const HELP_FINDER: u32 = 11u32;
pub const HELP_FORCEFILE: i32 = 9i32;
pub const HELP_HELPONHELP: i32 = 4i32;
pub const HELP_INDEX: i32 = 3i32;
pub const HELP_KEY: i32 = 257i32;
pub const HELP_MULTIKEY: i32 = 513i32;
pub const HELP_PARTIALKEY: i32 = 261i32;
pub const HELP_QUIT: i32 = 2i32;
pub const HELP_SETCONTENTS: i32 = 5i32;
pub const HELP_SETINDEX: i32 = 5i32;
pub const HELP_SETPOPUP_POS: u32 = 13u32;
pub const HELP_SETWINPOS: i32 = 515i32;
pub const HELP_TCARD: u32 = 32768u32;
pub const HELP_TCARD_DATA: u32 = 16u32;
pub const HELP_TCARD_OTHER_CALLER: u32 = 17u32;
pub const HELP_WM_HELP: u32 = 12u32;
pub const HIDE_WINDOW: u32 = 0u32;
pub const HKL_NEXT: u32 = 1u32;
pub const HKL_PREV: u32 = 0u32;
pub const HSHELL_ACCESSIBILITYSTATE: u32 = 11u32;
pub const HSHELL_ACTIVATESHELLWINDOW: u32 = 3u32;
pub const HSHELL_APPCOMMAND: u32 = 12u32;
pub const HSHELL_ENDTASK: u32 = 10u32;
pub const HSHELL_GETMINRECT: u32 = 5u32;
pub const HSHELL_HIGHBIT: u32 = 32768u32;
pub const HSHELL_LANGUAGE: u32 = 8u32;
pub const HSHELL_MONITORCHANGED: u32 = 16u32;
pub const HSHELL_REDRAW: u32 = 6u32;
pub const HSHELL_SYSMENU: u32 = 9u32;
pub const HSHELL_TASKMAN: u32 = 7u32;
pub const HSHELL_WINDOWACTIVATED: u32 = 4u32;
pub const HSHELL_WINDOWCREATED: u32 = 1u32;
pub const HSHELL_WINDOWDESTROYED: u32 = 2u32;
pub const HSHELL_WINDOWREPLACED: u32 = 13u32;
pub const HSHELL_WINDOWREPLACING: u32 = 14u32;
pub const HTBORDER: u32 = 18u32;
pub const HTBOTTOM: u32 = 15u32;
pub const HTBOTTOMLEFT: u32 = 16u32;
pub const HTBOTTOMRIGHT: u32 = 17u32;
pub const HTCAPTION: u32 = 2u32;
pub const HTCLIENT: u32 = 1u32;
pub const HTCLOSE: u32 = 20u32;
pub const HTERROR: i32 = -2i32;
pub const HTGROWBOX: u32 = 4u32;
pub const HTHELP: u32 = 21u32;
pub const HTHSCROLL: u32 = 6u32;
pub const HTLEFT: u32 = 10u32;
pub const HTMAXBUTTON: u32 = 9u32;
pub const HTMENU: u32 = 5u32;
pub const HTMINBUTTON: u32 = 8u32;
pub const HTNOWHERE: u32 = 0u32;
pub const HTOBJECT: u32 = 19u32;
pub const HTREDUCE: u32 = 8u32;
pub const HTRIGHT: u32 = 11u32;
pub const HTSIZE: u32 = 4u32;
pub const HTSIZEFIRST: u32 = 10u32;
pub const HTSIZELAST: u32 = 17u32;
pub const HTSYSMENU: u32 = 3u32;
pub const HTTOP: u32 = 12u32;
pub const HTTOPLEFT: u32 = 13u32;
pub const HTTOPRIGHT: u32 = 14u32;
pub const HTTRANSPARENT: i32 = -1i32;
pub const HTVSCROLL: u32 = 7u32;
pub const HTZOOM: u32 = 9u32;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub const HWND_BOTTOM: super::super::Foundation::HWND = 1i32 as _;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub const HWND_BROADCAST: super::super::Foundation::HWND = 65535i32 as _;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub const HWND_DESKTOP: super::super::Foundation::HWND = 0i32 as _;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub const HWND_MESSAGE: super::super::Foundation::HWND = -3i32 as _;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub const HWND_NOTOPMOST: super::super::Foundation::HWND = -2i32 as _;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub const HWND_TOP: super::super::Foundation::HWND = 0i32 as _;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub const HWND_TOPMOST: super::super::Foundation::HWND = -1i32 as _;
pub const ICON_BIG: u32 = 1u32;
pub const ICON_SMALL: u32 = 0u32;
pub const ICON_SMALL2: u32 = 2u32;
pub const IDABORT: MESSAGEBOX_RESULT = 3i32;
pub const IDANI_CAPTION: u32 = 3u32;
pub const IDANI_OPEN: u32 = 1u32;
pub const IDASYNC: MESSAGEBOX_RESULT = 32001i32;
pub const IDCANCEL: MESSAGEBOX_RESULT = 2i32;
pub const IDCLOSE: MESSAGEBOX_RESULT = 8i32;
pub const IDCONTINUE: MESSAGEBOX_RESULT = 11i32;
pub const IDC_APPSTARTING: ::windows_sys::core::PCWSTR = 32650u16 as _;
pub const IDC_ARROW: ::windows_sys::core::PCWSTR = 32512u16 as _;
pub const IDC_CROSS: ::windows_sys::core::PCWSTR = 32515u16 as _;
pub const IDC_HAND: ::windows_sys::core::PCWSTR = 32649u16 as _;
pub const IDC_HELP: ::windows_sys::core::PCWSTR = 32651u16 as _;
pub const IDC_IBEAM: ::windows_sys::core::PCWSTR = 32513u16 as _;
pub const IDC_ICON: ::windows_sys::core::PCWSTR = 32641u16 as _;
pub const IDC_NO: ::windows_sys::core::PCWSTR = 32648u16 as _;
pub const IDC_PERSON: ::windows_sys::core::PCWSTR = 32672u16 as _;
pub const IDC_PIN: ::windows_sys::core::PCWSTR = 32671u16 as _;
pub const IDC_SIZE: ::windows_sys::core::PCWSTR = 32640u16 as _;
pub const IDC_SIZEALL: ::windows_sys::core::PCWSTR = 32646u16 as _;
pub const IDC_SIZENESW: ::windows_sys::core::PCWSTR = 32643u16 as _;
pub const IDC_SIZENS: ::windows_sys::core::PCWSTR = 32645u16 as _;
pub const IDC_SIZENWSE: ::windows_sys::core::PCWSTR = 32642u16 as _;
pub const IDC_SIZEWE: ::windows_sys::core::PCWSTR = 32644u16 as _;
pub const IDC_UPARROW: ::windows_sys::core::PCWSTR = 32516u16 as _;
pub const IDC_WAIT: ::windows_sys::core::PCWSTR = 32514u16 as _;
pub const IDHELP: MESSAGEBOX_RESULT = 9i32;
pub const IDHOT_SNAPDESKTOP: i32 = -2i32;
pub const IDHOT_SNAPWINDOW: i32 = -1i32;
pub const IDH_CANCEL: u32 = 28444u32;
pub const IDH_GENERIC_HELP_BUTTON: u32 = 28442u32;
pub const IDH_HELP: u32 = 28445u32;
pub const IDH_MISSING_CONTEXT: u32 = 28441u32;
pub const IDH_NO_HELP: u32 = 28440u32;
pub const IDH_OK: u32 = 28443u32;
pub const IDIGNORE: MESSAGEBOX_RESULT = 5i32;
pub const IDI_APPLICATION: ::windows_sys::core::PCWSTR = 32512u32 as _;
pub const IDI_ASTERISK: ::windows_sys::core::PCWSTR = 32516u32 as _;
pub const IDI_ERROR: ::windows_sys::core::PCWSTR = 32513u32 as _;
pub const IDI_EXCLAMATION: ::windows_sys::core::PCWSTR = 32515u32 as _;
pub const IDI_HAND: ::windows_sys::core::PCWSTR = 32513u32 as _;
pub const IDI_INFORMATION: ::windows_sys::core::PCWSTR = 32516u32 as _;
pub const IDI_QUESTION: ::windows_sys::core::PCWSTR = 32514u32 as _;
pub const IDI_SHIELD: ::windows_sys::core::PCWSTR = 32518u32 as _;
pub const IDI_WARNING: ::windows_sys::core::PCWSTR = 32515u32 as _;
pub const IDI_WINLOGO: ::windows_sys::core::PCWSTR = 32517u32 as _;
pub const IDNO: MESSAGEBOX_RESULT = 7i32;
pub const IDOK: MESSAGEBOX_RESULT = 1i32;
pub const IDRETRY: MESSAGEBOX_RESULT = 4i32;
pub const IDTIMEOUT: MESSAGEBOX_RESULT = 32000i32;
pub const IDTRYAGAIN: MESSAGEBOX_RESULT = 10i32;
pub const IDYES: MESSAGEBOX_RESULT = 6i32;
pub const IMAGE_BITMAP: GDI_IMAGE_TYPE = 0u32;
pub const IMAGE_CURSOR: GDI_IMAGE_TYPE = 2u32;
pub const IMAGE_ENHMETAFILE: u32 = 3u32;
pub const IMAGE_ICON: GDI_IMAGE_TYPE = 1u32;
pub const INDEXID_CONTAINER: u32 = 0u32;
pub const INDEXID_OBJECT: u32 = 0u32;
pub const INPUTLANGCHANGE_BACKWARD: u32 = 4u32;
pub const INPUTLANGCHANGE_FORWARD: u32 = 2u32;
pub const INPUTLANGCHANGE_SYSCHARSET: u32 = 1u32;
pub const ISMEX_CALLBACK: u32 = 4u32;
pub const ISMEX_NOSEND: u32 = 0u32;
pub const ISMEX_NOTIFY: u32 = 2u32;
pub const ISMEX_REPLIED: u32 = 8u32;
pub const ISMEX_SEND: u32 = 1u32;
pub const ISOLATIONAWARE_MANIFEST_RESOURCE_ID: u32 = 2u32;
pub const ISOLATIONAWARE_NOSTATICIMPORT_MANIFEST_RESOURCE_ID: u32 = 3u32;
pub const ISOLATIONPOLICY_BROWSER_MANIFEST_RESOURCE_ID: u32 = 5u32;
pub const ISOLATIONPOLICY_MANIFEST_RESOURCE_ID: u32 = 4u32;
pub const KF_ALTDOWN: u32 = 8192u32;
pub const KF_DLGMODE: u32 = 2048u32;
pub const KF_EXTENDED: u32 = 256u32;
pub const KF_MENUMODE: u32 = 4096u32;
pub const KF_REPEAT: u32 = 16384u32;
pub const KF_UP: u32 = 32768u32;
pub const KL_NAMELENGTH: u32 = 9u32;
pub const LBN_DBLCLK: u32 = 2u32;
pub const LBN_ERRSPACE: i32 = -2i32;
pub const LBN_KILLFOCUS: u32 = 5u32;
pub const LBN_SELCANCEL: u32 = 3u32;
pub const LBN_SELCHANGE: u32 = 1u32;
pub const LBN_SETFOCUS: u32 = 4u32;
pub const LBS_COMBOBOX: i32 = 32768i32;
pub const LBS_DISABLENOSCROLL: i32 = 4096i32;
pub const LBS_EXTENDEDSEL: i32 = 2048i32;
pub const LBS_HASSTRINGS: i32 = 64i32;
pub const LBS_MULTICOLUMN: i32 = 512i32;
pub const LBS_MULTIPLESEL: i32 = 8i32;
pub const LBS_NODATA: i32 = 8192i32;
pub const LBS_NOINTEGRALHEIGHT: i32 = 256i32;
pub const LBS_NOREDRAW: i32 = 4i32;
pub const LBS_NOSEL: i32 = 16384i32;
pub const LBS_NOTIFY: i32 = 1i32;
pub const LBS_OWNERDRAWFIXED: i32 = 16i32;
pub const LBS_OWNERDRAWVARIABLE: i32 = 32i32;
pub const LBS_SORT: i32 = 2i32;
pub const LBS_STANDARD: i32 = 10485763i32;
pub const LBS_USETABSTOPS: i32 = 128i32;
pub const LBS_WANTKEYBOARDINPUT: i32 = 1024i32;
pub const LB_ADDFILE: u32 = 406u32;
pub const LB_ADDSTRING: u32 = 384u32;
pub const LB_CTLCODE: i32 = 0i32;
pub const LB_DELETESTRING: u32 = 386u32;
pub const LB_DIR: u32 = 397u32;
pub const LB_ERR: i32 = -1i32;
pub const LB_ERRSPACE: i32 = -2i32;
pub const LB_FINDSTRING: u32 = 399u32;
pub const LB_FINDSTRINGEXACT: u32 = 418u32;
pub const LB_GETANCHORINDEX: u32 = 413u32;
pub const LB_GETCARETINDEX: u32 = 415u32;
pub const LB_GETCOUNT: u32 = 395u32;
pub const LB_GETCURSEL: u32 = 392u32;
pub const LB_GETHORIZONTALEXTENT: u32 = 403u32;
pub const LB_GETITEMDATA: u32 = 409u32;
pub const LB_GETITEMHEIGHT: u32 = 417u32;
pub const LB_GETITEMRECT: u32 = 408u32;
pub const LB_GETLISTBOXINFO: u32 = 434u32;
pub const LB_GETLOCALE: u32 = 422u32;
pub const LB_GETSEL: u32 = 391u32;
pub const LB_GETSELCOUNT: u32 = 400u32;
pub const LB_GETSELITEMS: u32 = 401u32;
pub const LB_GETTEXT: u32 = 393u32;
pub const LB_GETTEXTLEN: u32 = 394u32;
pub const LB_GETTOPINDEX: u32 = 398u32;
pub const LB_INITSTORAGE: u32 = 424u32;
pub const LB_INSERTSTRING: u32 = 385u32;
pub const LB_ITEMFROMPOINT: u32 = 425u32;
pub const LB_MSGMAX: u32 = 435u32;
pub const LB_MULTIPLEADDSTRING: u32 = 433u32;
pub const LB_OKAY: u32 = 0u32;
pub const LB_RESETCONTENT: u32 = 388u32;
pub const LB_SELECTSTRING: u32 = 396u32;
pub const LB_SELITEMRANGE: u32 = 411u32;
pub const LB_SELITEMRANGEEX: u32 = 387u32;
pub const LB_SETANCHORINDEX: u32 = 412u32;
pub const LB_SETCARETINDEX: u32 = 414u32;
pub const LB_SETCOLUMNWIDTH: u32 = 405u32;
pub const LB_SETCOUNT: u32 = 423u32;
pub const LB_SETCURSEL: u32 = 390u32;
pub const LB_SETHORIZONTALEXTENT: u32 = 404u32;
pub const LB_SETITEMDATA: u32 = 410u32;
pub const LB_SETITEMHEIGHT: u32 = 416u32;
pub const LB_SETLOCALE: u32 = 421u32;
pub const LB_SETSEL: u32 = 389u32;
pub const LB_SETTABSTOPS: u32 = 402u32;
pub const LB_SETTOPINDEX: u32 = 407u32;
pub const LLKHF_ALTDOWN: KBDLLHOOKSTRUCT_FLAGS = 32u32;
pub const LLKHF_EXTENDED: KBDLLHOOKSTRUCT_FLAGS = 1u32;
pub const LLKHF_INJECTED: KBDLLHOOKSTRUCT_FLAGS = 16u32;
pub const LLKHF_LOWER_IL_INJECTED: KBDLLHOOKSTRUCT_FLAGS = 2u32;
pub const LLKHF_UP: KBDLLHOOKSTRUCT_FLAGS = 128u32;
pub const LLMHF_INJECTED: u32 = 1u32;
pub const LLMHF_LOWER_IL_INJECTED: u32 = 2u32;
pub const LOCKF_LOGICAL_LOCK: u32 = 0u32;
pub const LOCKF_PHYSICAL_LOCK: u32 = 1u32;
pub const LOCKP_ALLOW_MEM_MAPPING: u32 = 0u32;
pub const LOCKP_ALLOW_WRITES: u32 = 1u32;
pub const LOCKP_FAIL_MEM_MAPPING: u32 = 2u32;
pub const LOCKP_FAIL_WRITES: u32 = 0u32;
pub const LOCKP_LOCK_FOR_FORMAT: u32 = 4u32;
pub const LOCKP_USER_MASK: u32 = 3u32;
pub const LR_COLOR: u32 = 2u32;
pub const LR_COPYDELETEORG: IMAGE_FLAGS = 8u32;
pub const LR_COPYFROMRESOURCE: IMAGE_FLAGS = 16384u32;
pub const LR_COPYRETURNORG: IMAGE_FLAGS = 4u32;
pub const LR_CREATEDIBSECTION: IMAGE_FLAGS = 8192u32;
pub const LR_DEFAULTCOLOR: IMAGE_FLAGS = 0u32;
pub const LR_DEFAULTSIZE: IMAGE_FLAGS = 64u32;
pub const LR_LOADFROMFILE: IMAGE_FLAGS = 16u32;
pub const LR_LOADMAP3DCOLORS: IMAGE_FLAGS = 4096u32;
pub const LR_LOADTRANSPARENT: IMAGE_FLAGS = 32u32;
pub const LR_MONOCHROME: IMAGE_FLAGS = 1u32;
pub const LR_SHARED: IMAGE_FLAGS = 32768u32;
pub const LR_VGACOLOR: IMAGE_FLAGS = 128u32;
pub const LSFW_LOCK: FOREGROUND_WINDOW_LOCK_CODE = 1u32;
pub const LSFW_UNLOCK: FOREGROUND_WINDOW_LOCK_CODE = 2u32;
pub const LWA_ALPHA: LAYERED_WINDOW_ATTRIBUTES_FLAGS = 2u32;
pub const LWA_COLORKEY: LAYERED_WINDOW_ATTRIBUTES_FLAGS = 1u32;
pub const MAXIMUM_RESERVED_MANIFEST_RESOURCE_ID: u32 = 16u32;
pub const MAX_LOGICALDPIOVERRIDE: u32 = 2u32;
pub const MAX_STR_BLOCKREASON: u32 = 256u32;
pub const MAX_TOUCH_COUNT: u32 = 256u32;
pub const MAX_TOUCH_PREDICTION_FILTER_TAPS: u32 = 3u32;
pub const MA_ACTIVATE: u32 = 1u32;
pub const MA_ACTIVATEANDEAT: u32 = 2u32;
pub const MA_NOACTIVATE: u32 = 3u32;
pub const MA_NOACTIVATEANDEAT: u32 = 4u32;
pub const MB_ABORTRETRYIGNORE: MESSAGEBOX_STYLE = 2u32;
pub const MB_APPLMODAL: MESSAGEBOX_STYLE = 0u32;
pub const MB_CANCELTRYCONTINUE: MESSAGEBOX_STYLE = 6u32;
pub const MB_DEFAULT_DESKTOP_ONLY: MESSAGEBOX_STYLE = 131072u32;
pub const MB_DEFBUTTON1: MESSAGEBOX_STYLE = 0u32;
pub const MB_DEFBUTTON2: MESSAGEBOX_STYLE = 256u32;
pub const MB_DEFBUTTON3: MESSAGEBOX_STYLE = 512u32;
pub const MB_DEFBUTTON4: MESSAGEBOX_STYLE = 768u32;
pub const MB_DEFMASK: MESSAGEBOX_STYLE = 3840u32;
pub const MB_HELP: MESSAGEBOX_STYLE = 16384u32;
pub const MB_ICONASTERISK: MESSAGEBOX_STYLE = 64u32;
pub const MB_ICONERROR: MESSAGEBOX_STYLE = 16u32;
pub const MB_ICONEXCLAMATION: MESSAGEBOX_STYLE = 48u32;
pub const MB_ICONHAND: MESSAGEBOX_STYLE = 16u32;
pub const MB_ICONINFORMATION: MESSAGEBOX_STYLE = 64u32;
pub const MB_ICONMASK: MESSAGEBOX_STYLE = 240u32;
pub const MB_ICONQUESTION: MESSAGEBOX_STYLE = 32u32;
pub const MB_ICONSTOP: MESSAGEBOX_STYLE = 16u32;
pub const MB_ICONWARNING: MESSAGEBOX_STYLE = 48u32;
pub const MB_MISCMASK: MESSAGEBOX_STYLE = 49152u32;
pub const MB_MODEMASK: MESSAGEBOX_STYLE = 12288u32;
pub const MB_NOFOCUS: MESSAGEBOX_STYLE = 32768u32;
pub const MB_OK: MESSAGEBOX_STYLE = 0u32;
pub const MB_OKCANCEL: MESSAGEBOX_STYLE = 1u32;
pub const MB_RETRYCANCEL: MESSAGEBOX_STYLE = 5u32;
pub const MB_RIGHT: MESSAGEBOX_STYLE = 524288u32;
pub const MB_RTLREADING: MESSAGEBOX_STYLE = 1048576u32;
pub const MB_SERVICE_NOTIFICATION: MESSAGEBOX_STYLE = 2097152u32;
pub const MB_SERVICE_NOTIFICATION_NT3X: MESSAGEBOX_STYLE = 262144u32;
pub const MB_SETFOREGROUND: MESSAGEBOX_STYLE = 65536u32;
pub const MB_SYSTEMMODAL: MESSAGEBOX_STYLE = 4096u32;
pub const MB_TASKMODAL: MESSAGEBOX_STYLE = 8192u32;
pub const MB_TOPMOST: MESSAGEBOX_STYLE = 262144u32;
pub const MB_TYPEMASK: MESSAGEBOX_STYLE = 15u32;
pub const MB_USERICON: MESSAGEBOX_STYLE = 128u32;
pub const MB_YESNO: MESSAGEBOX_STYLE = 4u32;
pub const MB_YESNOCANCEL: MESSAGEBOX_STYLE = 3u32;
pub const MDIS_ALLCHILDSTYLES: u32 = 1u32;
pub const MDITILE_HORIZONTAL: TILE_WINDOWS_HOW = 1u32;
pub const MDITILE_SKIPDISABLED: CASCADE_WINDOWS_HOW = 2u32;
pub const MDITILE_VERTICAL: TILE_WINDOWS_HOW = 0u32;
pub const MDITILE_ZORDER: CASCADE_WINDOWS_HOW = 4u32;
pub const METRICS_USEDEFAULT: i32 = -1i32;
pub const MFS_CHECKED: MENU_ITEM_STATE = 8u32;
pub const MFS_DEFAULT: MENU_ITEM_STATE = 4096u32;
pub const MFS_DISABLED: MENU_ITEM_STATE = 3u32;
pub const MFS_ENABLED: MENU_ITEM_STATE = 0u32;
pub const MFS_GRAYED: MENU_ITEM_STATE = 3u32;
pub const MFS_HILITE: MENU_ITEM_STATE = 128u32;
pub const MFS_UNCHECKED: MENU_ITEM_STATE = 0u32;
pub const MFS_UNHILITE: MENU_ITEM_STATE = 0u32;
pub const MFT_BITMAP: MENU_ITEM_TYPE = 4u32;
pub const MFT_MENUBARBREAK: MENU_ITEM_TYPE = 32u32;
pub const MFT_MENUBREAK: MENU_ITEM_TYPE = 64u32;
pub const MFT_OWNERDRAW: MENU_ITEM_TYPE = 256u32;
pub const MFT_RADIOCHECK: MENU_ITEM_TYPE = 512u32;
pub const MFT_RIGHTJUSTIFY: MENU_ITEM_TYPE = 16384u32;
pub const MFT_RIGHTORDER: MENU_ITEM_TYPE = 8192u32;
pub const MFT_SEPARATOR: MENU_ITEM_TYPE = 2048u32;
pub const MFT_STRING: MENU_ITEM_TYPE = 0u32;
pub const MF_APPEND: MENU_ITEM_FLAGS = 256u32;
pub const MF_BITMAP: MENU_ITEM_FLAGS = 4u32;
pub const MF_BYCOMMAND: MENU_ITEM_FLAGS = 0u32;
pub const MF_BYPOSITION: MENU_ITEM_FLAGS = 1024u32;
pub const MF_CHANGE: MENU_ITEM_FLAGS = 128u32;
pub const MF_CHECKED: MENU_ITEM_FLAGS = 8u32;
pub const MF_DEFAULT: MENU_ITEM_FLAGS = 4096u32;
pub const MF_DELETE: MENU_ITEM_FLAGS = 512u32;
pub const MF_DISABLED: MENU_ITEM_FLAGS = 2u32;
pub const MF_ENABLED: MENU_ITEM_FLAGS = 0u32;
pub const MF_END: MENU_ITEM_FLAGS = 128u32;
pub const MF_GRAYED: MENU_ITEM_FLAGS = 1u32;
pub const MF_HELP: MENU_ITEM_FLAGS = 16384u32;
pub const MF_HILITE: MENU_ITEM_FLAGS = 128u32;
pub const MF_INSERT: MENU_ITEM_FLAGS = 0u32;
pub const MF_MENUBARBREAK: MENU_ITEM_FLAGS = 32u32;
pub const MF_MENUBREAK: MENU_ITEM_FLAGS = 64u32;
pub const MF_MOUSESELECT: MENU_ITEM_FLAGS = 32768u32;
pub const MF_OWNERDRAW: MENU_ITEM_FLAGS = 256u32;
pub const MF_POPUP: MENU_ITEM_FLAGS = 16u32;
pub const MF_REMOVE: MENU_ITEM_FLAGS = 4096u32;
pub const MF_RIGHTJUSTIFY: MENU_ITEM_FLAGS = 16384u32;
pub const MF_SEPARATOR: MENU_ITEM_FLAGS = 2048u32;
pub const MF_STRING: MENU_ITEM_FLAGS = 0u32;
pub const MF_SYSMENU: MENU_ITEM_FLAGS = 8192u32;
pub const MF_UNCHECKED: MENU_ITEM_FLAGS = 0u32;
pub const MF_UNHILITE: MENU_ITEM_FLAGS = 0u32;
pub const MF_USECHECKBITMAPS: MENU_ITEM_FLAGS = 512u32;
pub const MIIM_BITMAP: MENU_ITEM_MASK = 128u32;
pub const MIIM_CHECKMARKS: MENU_ITEM_MASK = 8u32;
pub const MIIM_DATA: MENU_ITEM_MASK = 32u32;
pub const MIIM_FTYPE: MENU_ITEM_MASK = 256u32;
pub const MIIM_ID: MENU_ITEM_MASK = 2u32;
pub const MIIM_STATE: MENU_ITEM_MASK = 1u32;
pub const MIIM_STRING: MENU_ITEM_MASK = 64u32;
pub const MIIM_SUBMENU: MENU_ITEM_MASK = 4u32;
pub const MIIM_TYPE: MENU_ITEM_MASK = 16u32;
pub const MIM_APPLYTOSUBMENUS: MENUINFO_MASK = 2147483648u32;
pub const MIM_BACKGROUND: MENUINFO_MASK = 2u32;
pub const MIM_HELPID: MENUINFO_MASK = 4u32;
pub const MIM_MAXHEIGHT: MENUINFO_MASK = 1u32;
pub const MIM_MENUDATA: MENUINFO_MASK = 8u32;
pub const MIM_STYLE: MENUINFO_MASK = 16u32;
pub const MINIMUM_RESERVED_MANIFEST_RESOURCE_ID: u32 = 1u32;
pub const MIN_LOGICALDPIOVERRIDE: i32 = -2i32;
pub const MKF_AVAILABLE: u32 = 2u32;
pub const MKF_CONFIRMHOTKEY: u32 = 8u32;
pub const MKF_HOTKEYACTIVE: u32 = 4u32;
pub const MKF_HOTKEYSOUND: u32 = 16u32;
pub const MKF_INDICATOR: u32 = 32u32;
pub const MKF_LEFTBUTTONDOWN: u32 = 16777216u32;
pub const MKF_LEFTBUTTONSEL: u32 = 268435456u32;
pub const MKF_MODIFIERS: u32 = 64u32;
pub const MKF_MOUSEKEYSON: u32 = 1u32;
pub const MKF_MOUSEMODE: u32 = 2147483648u32;
pub const MKF_REPLACENUMBERS: u32 = 128u32;
pub const MKF_RIGHTBUTTONDOWN: u32 = 33554432u32;
pub const MKF_RIGHTBUTTONSEL: u32 = 536870912u32;
pub const MNC_CLOSE: u32 = 1u32;
pub const MNC_EXECUTE: u32 = 2u32;
pub const MNC_IGNORE: u32 = 0u32;
pub const MNC_SELECT: u32 = 3u32;
pub const MND_CONTINUE: u32 = 0u32;
pub const MND_ENDMENU: u32 = 1u32;
pub const MNGOF_BOTTOMGAP: MENUGETOBJECTINFO_FLAGS = 2u32;
pub const MNGOF_TOPGAP: MENUGETOBJECTINFO_FLAGS = 1u32;
pub const MNGO_NOERROR: u32 = 1u32;
pub const MNGO_NOINTERFACE: u32 = 0u32;
pub const MNS_AUTODISMISS: MENUINFO_STYLE = 268435456u32;
pub const MNS_CHECKORBMP: MENUINFO_STYLE = 67108864u32;
pub const MNS_DRAGDROP: MENUINFO_STYLE = 536870912u32;
pub const MNS_MODELESS: MENUINFO_STYLE = 1073741824u32;
pub const MNS_NOCHECK: MENUINFO_STYLE = 2147483648u32;
pub const MNS_NOTIFYBYPOS: MENUINFO_STYLE = 134217728u32;
pub const MN_GETHMENU: u32 = 481u32;
pub const MONITORINFOF_PRIMARY: u32 = 1u32;
pub const MOUSEWHEEL_ROUTING_FOCUS: u32 = 0u32;
pub const MOUSEWHEEL_ROUTING_HYBRID: u32 = 1u32;
pub const MOUSEWHEEL_ROUTING_MOUSE_POS: u32 = 2u32;
pub const MSGFLTINFO_ALLOWED_HIGHER: MSGFLTINFO_STATUS = 3u32;
pub const MSGFLTINFO_ALREADYALLOWED_FORWND: MSGFLTINFO_STATUS = 1u32;
pub const MSGFLTINFO_ALREADYDISALLOWED_FORWND: MSGFLTINFO_STATUS = 2u32;
pub const MSGFLTINFO_NONE: MSGFLTINFO_STATUS = 0u32;
pub const MSGFLT_ADD: CHANGE_WINDOW_MESSAGE_FILTER_FLAGS = 1u32;
pub const MSGFLT_ALLOW: WINDOW_MESSAGE_FILTER_ACTION = 1u32;
pub const MSGFLT_DISALLOW: WINDOW_MESSAGE_FILTER_ACTION = 2u32;
pub const MSGFLT_REMOVE: CHANGE_WINDOW_MESSAGE_FILTER_FLAGS = 2u32;
pub const MSGFLT_RESET: WINDOW_MESSAGE_FILTER_ACTION = 0u32;
pub const MSGF_DIALOGBOX: u32 = 0u32;
pub const MSGF_MAX: u32 = 8u32;
pub const MSGF_MENU: u32 = 2u32;
pub const MSGF_MESSAGEBOX: u32 = 1u32;
pub const MSGF_NEXTWINDOW: u32 = 6u32;
pub const MSGF_SCROLLBAR: u32 = 5u32;
pub const MSGF_USER: u32 = 4096u32;
pub const MWMO_ALERTABLE: MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS = 2u32;
pub const MWMO_INPUTAVAILABLE: MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS = 4u32;
pub const MWMO_NONE: MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS = 0u32;
pub const MWMO_WAITALL: MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS = 1u32;
pub const MrmDumpType_Basic: MrmDumpType = 0i32;
pub const MrmDumpType_Detailed: MrmDumpType = 1i32;
pub const MrmDumpType_Schema: MrmDumpType = 2i32;
pub const MrmIndexerFlagsAutoMerge: MrmIndexerFlags = 1i32;
pub const MrmIndexerFlagsCreateContentChecksum: MrmIndexerFlags = 2i32;
pub const MrmIndexerFlagsNone: MrmIndexerFlags = 0i32;
pub const MrmPackagingModeAutoSplit: MrmPackagingMode = 1i32;
pub const MrmPackagingModeResourcePack: MrmPackagingMode = 2i32;
pub const MrmPackagingModeStandaloneFile: MrmPackagingMode = 0i32;
pub const MrmPackagingOptionsNone: MrmPackagingOptions = 0i32;
pub const MrmPackagingOptionsOmitSchemaFromResourcePacks: MrmPackagingOptions = 1i32;
pub const MrmPackagingOptionsSplitLanguageVariants: MrmPackagingOptions = 2i32;
pub const MrmPlatformVersion_Default: MrmPlatformVersion = 0i32;
pub const MrmPlatformVersion_Windows10_0_0_0: MrmPlatformVersion = 17432576i32;
pub const MrmPlatformVersion_Windows10_0_0_5: MrmPlatformVersion = 17432581i32;
pub const MrmResourceIndexerMessageSeverityError: MrmResourceIndexerMessageSeverity = 3i32;
pub const MrmResourceIndexerMessageSeverityInfo: MrmResourceIndexerMessageSeverity = 1i32;
pub const MrmResourceIndexerMessageSeverityVerbose: MrmResourceIndexerMessageSeverity = 0i32;
pub const MrmResourceIndexerMessageSeverityWarning: MrmResourceIndexerMessageSeverity = 2i32;
pub const NFR_ANSI: u32 = 1u32;
pub const NFR_UNICODE: u32 = 2u32;
pub const NF_QUERY: u32 = 3u32;
pub const NF_REQUERY: u32 = 4u32;
pub const NID_EXTERNAL_PEN: u32 = 8u32;
pub const NID_EXTERNAL_TOUCH: u32 = 2u32;
pub const NID_INTEGRATED_PEN: u32 = 4u32;
pub const NID_INTEGRATED_TOUCH: u32 = 1u32;
pub const NID_MULTI_INPUT: u32 = 64u32;
pub const NID_READY: u32 = 128u32;
pub const OBJID_ALERT: OBJECT_IDENTIFIER = -10i32;
pub const OBJID_CARET: OBJECT_IDENTIFIER = -8i32;
pub const OBJID_CLIENT: OBJECT_IDENTIFIER = -4i32;
pub const OBJID_CURSOR: OBJECT_IDENTIFIER = -9i32;
pub const OBJID_HSCROLL: OBJECT_IDENTIFIER = -6i32;
pub const OBJID_MENU: OBJECT_IDENTIFIER = -3i32;
pub const OBJID_NATIVEOM: OBJECT_IDENTIFIER = -16i32;
pub const OBJID_QUERYCLASSNAMEIDX: OBJECT_IDENTIFIER = -12i32;
pub const OBJID_SIZEGRIP: OBJECT_IDENTIFIER = -7i32;
pub const OBJID_SOUND: OBJECT_IDENTIFIER = -11i32;
pub const OBJID_SYSMENU: OBJECT_IDENTIFIER = -1i32;
pub const OBJID_TITLEBAR: OBJECT_IDENTIFIER = -2i32;
pub const OBJID_VSCROLL: OBJECT_IDENTIFIER = -5i32;
pub const OBJID_WINDOW: OBJECT_IDENTIFIER = 0i32;
pub const OBM_BTNCORNERS: u32 = 32758u32;
pub const OBM_BTSIZE: u32 = 32761u32;
pub const OBM_CHECK: u32 = 32760u32;
pub const OBM_CHECKBOXES: u32 = 32759u32;
pub const OBM_CLOSE: u32 = 32754u32;
pub const OBM_COMBO: u32 = 32738u32;
pub const OBM_DNARROW: u32 = 32752u32;
pub const OBM_DNARROWD: u32 = 32742u32;
pub const OBM_DNARROWI: u32 = 32736u32;
pub const OBM_LFARROW: u32 = 32750u32;
pub const OBM_LFARROWD: u32 = 32740u32;
pub const OBM_LFARROWI: u32 = 32734u32;
pub const OBM_MNARROW: u32 = 32739u32;
pub const OBM_OLD_CLOSE: u32 = 32767u32;
pub const OBM_OLD_DNARROW: u32 = 32764u32;
pub const OBM_OLD_LFARROW: u32 = 32762u32;
pub const OBM_OLD_REDUCE: u32 = 32757u32;
pub const OBM_OLD_RESTORE: u32 = 32755u32;
pub const OBM_OLD_RGARROW: u32 = 32763u32;
pub const OBM_OLD_UPARROW: u32 = 32765u32;
pub const OBM_OLD_ZOOM: u32 = 32756u32;
pub const OBM_REDUCE: u32 = 32749u32;
pub const OBM_REDUCED: u32 = 32746u32;
pub const OBM_RESTORE: u32 = 32747u32;
pub const OBM_RESTORED: u32 = 32744u32;
pub const OBM_RGARROW: u32 = 32751u32;
pub const OBM_RGARROWD: u32 = 32741u32;
pub const OBM_RGARROWI: u32 = 32735u32;
pub const OBM_SIZE: u32 = 32766u32;
pub const OBM_UPARROW: u32 = 32753u32;
pub const OBM_UPARROWD: u32 = 32743u32;
pub const OBM_UPARROWI: u32 = 32737u32;
pub const OBM_ZOOM: u32 = 32748u32;
pub const OBM_ZOOMD: u32 = 32745u32;
pub const OCR_APPSTARTING: SYSTEM_CURSOR_ID = 32650u32;
pub const OCR_CROSS: SYSTEM_CURSOR_ID = 32515u32;
pub const OCR_HAND: SYSTEM_CURSOR_ID = 32649u32;
pub const OCR_HELP: SYSTEM_CURSOR_ID = 32651u32;
pub const OCR_IBEAM: SYSTEM_CURSOR_ID = 32513u32;
pub const OCR_ICOCUR: u32 = 32647u32;
pub const OCR_ICON: u32 = 32641u32;
pub const OCR_NO: SYSTEM_CURSOR_ID = 32648u32;
pub const OCR_NORMAL: SYSTEM_CURSOR_ID = 32512u32;
pub const OCR_SIZE: u32 = 32640u32;
pub const OCR_SIZEALL: SYSTEM_CURSOR_ID = 32646u32;
pub const OCR_SIZENESW: SYSTEM_CURSOR_ID = 32643u32;
pub const OCR_SIZENS: SYSTEM_CURSOR_ID = 32645u32;
pub const OCR_SIZENWSE: SYSTEM_CURSOR_ID = 32642u32;
pub const OCR_SIZEWE: SYSTEM_CURSOR_ID = 32644u32;
pub const OCR_UP: SYSTEM_CURSOR_ID = 32516u32;
pub const OCR_WAIT: SYSTEM_CURSOR_ID = 32514u32;
pub const OIC_BANG: u32 = 32515u32;
pub const OIC_ERROR: u32 = 32513u32;
pub const OIC_HAND: u32 = 32513u32;
pub const OIC_INFORMATION: u32 = 32516u32;
pub const OIC_NOTE: u32 = 32516u32;
pub const OIC_QUES: u32 = 32514u32;
pub const OIC_SAMPLE: u32 = 32512u32;
pub const OIC_SHIELD: u32 = 32518u32;
pub const OIC_WARNING: u32 = 32515u32;
pub const OIC_WINLOGO: u32 = 32517u32;
pub const ORD_LANGDRIVER: u32 = 1u32;
pub const PA_ACTIVATE: u32 = 1u32;
pub const PA_NOACTIVATE: u32 = 3u32;
pub const PBTF_APMRESUMEFROMFAILURE: u32 = 1u32;
pub const PBT_APMBATTERYLOW: u32 = 9u32;
pub const PBT_APMOEMEVENT: u32 = 11u32;
pub const PBT_APMPOWERSTATUSCHANGE: u32 = 10u32;
pub const PBT_APMQUERYSTANDBY: u32 = 1u32;
pub const PBT_APMQUERYSTANDBYFAILED: u32 = 3u32;
pub const PBT_APMQUERYSUSPEND: u32 = 0u32;
pub const PBT_APMQUERYSUSPENDFAILED: u32 = 2u32;
pub const PBT_APMRESUMEAUTOMATIC: u32 = 18u32;
pub const PBT_APMRESUMECRITICAL: u32 = 6u32;
pub const PBT_APMRESUMESTANDBY: u32 = 8u32;
pub const PBT_APMRESUMESUSPEND: u32 = 7u32;
pub const PBT_APMSTANDBY: u32 = 5u32;
pub const PBT_APMSUSPEND: u32 = 4u32;
pub const PBT_POWERSETTINGCHANGE: u32 = 32787u32;
pub const PDC_ARRIVAL: u32 = 1u32;
pub const PDC_MAPPING_CHANGE: u32 = 256u32;
pub const PDC_MODE_ASPECTRATIOPRESERVED: u32 = 2048u32;
pub const PDC_MODE_CENTERED: u32 = 128u32;
pub const PDC_MODE_DEFAULT: u32 = 64u32;
pub const PDC_ORIENTATION_0: u32 = 4u32;
pub const PDC_ORIENTATION_180: u32 = 16u32;
pub const PDC_ORIENTATION_270: u32 = 32u32;
pub const PDC_ORIENTATION_90: u32 = 8u32;
pub const PDC_ORIGIN: u32 = 1024u32;
pub const PDC_REMOVAL: u32 = 2u32;
pub const PDC_RESOLUTION: u32 = 512u32;
pub const PENARBITRATIONTYPE_FIS: u32 = 2u32;
pub const PENARBITRATIONTYPE_MAX: u32 = 4u32;
pub const PENARBITRATIONTYPE_NONE: u32 = 0u32;
pub const PENARBITRATIONTYPE_SPT: u32 = 3u32;
pub const PENARBITRATIONTYPE_WIN8: u32 = 1u32;
pub const PENVISUALIZATION_CURSOR: u32 = 32u32;
pub const PENVISUALIZATION_DOUBLETAP: u32 = 2u32;
pub const PENVISUALIZATION_OFF: u32 = 0u32;
pub const PENVISUALIZATION_ON: u32 = 35u32;
pub const PENVISUALIZATION_TAP: u32 = 1u32;
pub const PEN_FLAG_BARREL: u32 = 1u32;
pub const PEN_FLAG_ERASER: u32 = 4u32;
pub const PEN_FLAG_INVERTED: u32 = 2u32;
pub const PEN_FLAG_NONE: u32 = 0u32;
pub const PEN_MASK_NONE: u32 = 0u32;
pub const PEN_MASK_PRESSURE: u32 = 1u32;
pub const PEN_MASK_ROTATION: u32 = 2u32;
pub const PEN_MASK_TILT_X: u32 = 4u32;
pub const PEN_MASK_TILT_Y: u32 = 8u32;
pub const PMB_ACTIVE: u32 = 1u32;
pub const PM_NOREMOVE: PEEK_MESSAGE_REMOVE_TYPE = 0u32;
pub const PM_NOYIELD: PEEK_MESSAGE_REMOVE_TYPE = 2u32;
pub const PM_QS_INPUT: PEEK_MESSAGE_REMOVE_TYPE = 67567616u32;
pub const PM_QS_PAINT: PEEK_MESSAGE_REMOVE_TYPE = 2097152u32;
pub const PM_QS_POSTMESSAGE: PEEK_MESSAGE_REMOVE_TYPE = 9961472u32;
pub const PM_QS_SENDMESSAGE: PEEK_MESSAGE_REMOVE_TYPE = 4194304u32;
pub const PM_REMOVE: PEEK_MESSAGE_REMOVE_TYPE = 1u32;
pub const POINTER_DEVICE_PRODUCT_STRING_MAX: u32 = 520u32;
pub const POINTER_MESSAGE_FLAG_CANCELED: u32 = 32768u32;
pub const POINTER_MESSAGE_FLAG_CONFIDENCE: u32 = 16384u32;
pub const POINTER_MESSAGE_FLAG_FIFTHBUTTON: u32 = 256u32;
pub const POINTER_MESSAGE_FLAG_FIRSTBUTTON: u32 = 16u32;
pub const POINTER_MESSAGE_FLAG_FOURTHBUTTON: u32 = 128u32;
pub const POINTER_MESSAGE_FLAG_INCONTACT: u32 = 4u32;
pub const POINTER_MESSAGE_FLAG_INRANGE: u32 = 2u32;
pub const POINTER_MESSAGE_FLAG_NEW: u32 = 1u32;
pub const POINTER_MESSAGE_FLAG_PRIMARY: u32 = 8192u32;
pub const POINTER_MESSAGE_FLAG_SECONDBUTTON: u32 = 32u32;
pub const POINTER_MESSAGE_FLAG_THIRDBUTTON: u32 = 64u32;
pub const POINTER_MOD_CTRL: u32 = 8u32;
pub const POINTER_MOD_SHIFT: u32 = 4u32;
pub const PRF_CHECKVISIBLE: i32 = 1i32;
pub const PRF_CHILDREN: i32 = 16i32;
pub const PRF_CLIENT: i32 = 4i32;
pub const PRF_ERASEBKGND: i32 = 8i32;
pub const PRF_NONCLIENT: i32 = 2i32;
pub const PRF_OWNED: i32 = 32i32;
pub const PT_MOUSE: POINTER_INPUT_TYPE = 4i32;
pub const PT_PEN: POINTER_INPUT_TYPE = 3i32;
pub const PT_POINTER: POINTER_INPUT_TYPE = 1i32;
pub const PT_TOUCH: POINTER_INPUT_TYPE = 2i32;
pub const PT_TOUCHPAD: POINTER_INPUT_TYPE = 5i32;
pub const PWR_CRITICALRESUME: u32 = 3u32;
pub const PWR_FAIL: i32 = -1i32;
pub const PWR_OK: u32 = 1u32;
pub const PWR_SUSPENDREQUEST: u32 = 1u32;
pub const PWR_SUSPENDRESUME: u32 = 2u32;
pub const PW_RENDERFULLCONTENT: u32 = 2u32;
pub const QS_ALLEVENTS: QUEUE_STATUS_FLAGS = 1215u32;
pub const QS_ALLINPUT: QUEUE_STATUS_FLAGS = 1279u32;
pub const QS_ALLPOSTMESSAGE: QUEUE_STATUS_FLAGS = 256u32;
pub const QS_HOTKEY: QUEUE_STATUS_FLAGS = 128u32;
pub const QS_INPUT: QUEUE_STATUS_FLAGS = 1031u32;
pub const QS_KEY: QUEUE_STATUS_FLAGS = 1u32;
pub const QS_MOUSE: QUEUE_STATUS_FLAGS = 6u32;
pub const QS_MOUSEBUTTON: QUEUE_STATUS_FLAGS = 4u32;
pub const QS_MOUSEMOVE: QUEUE_STATUS_FLAGS = 2u32;
pub const QS_PAINT: QUEUE_STATUS_FLAGS = 32u32;
pub const QS_POINTER: u32 = 4096u32;
pub const QS_POSTMESSAGE: QUEUE_STATUS_FLAGS = 8u32;
pub const QS_RAWINPUT: QUEUE_STATUS_FLAGS = 1024u32;
pub const QS_SENDMESSAGE: QUEUE_STATUS_FLAGS = 64u32;
pub const QS_TIMER: QUEUE_STATUS_FLAGS = 16u32;
pub const QS_TOUCH: u32 = 2048u32;
pub const RES_CURSOR: u32 = 2u32;
pub const RES_ICON: u32 = 1u32;
pub const RIDEV_EXMODEMASK: u32 = 240u32;
pub const RIM_INPUT: u32 = 0u32;
pub const RIM_INPUTSINK: u32 = 1u32;
pub const RIM_TYPEMAX: u32 = 2u32;
pub const RI_KEY_BREAK: u32 = 1u32;
pub const RI_KEY_E0: u32 = 2u32;
pub const RI_KEY_E1: u32 = 4u32;
pub const RI_KEY_MAKE: u32 = 0u32;
pub const RI_KEY_TERMSRV_SET_LED: u32 = 8u32;
pub const RI_KEY_TERMSRV_SHADOW: u32 = 16u32;
pub const RI_MOUSE_BUTTON_1_DOWN: u32 = 1u32;
pub const RI_MOUSE_BUTTON_1_UP: u32 = 2u32;
pub const RI_MOUSE_BUTTON_2_DOWN: u32 = 4u32;
pub const RI_MOUSE_BUTTON_2_UP: u32 = 8u32;
pub const RI_MOUSE_BUTTON_3_DOWN: u32 = 16u32;
pub const RI_MOUSE_BUTTON_3_UP: u32 = 32u32;
pub const RI_MOUSE_BUTTON_4_DOWN: u32 = 64u32;
pub const RI_MOUSE_BUTTON_4_UP: u32 = 128u32;
pub const RI_MOUSE_BUTTON_5_DOWN: u32 = 256u32;
pub const RI_MOUSE_BUTTON_5_UP: u32 = 512u32;
pub const RI_MOUSE_HWHEEL: u32 = 2048u32;
pub const RI_MOUSE_LEFT_BUTTON_DOWN: u32 = 1u32;
pub const RI_MOUSE_LEFT_BUTTON_UP: u32 = 2u32;
pub const RI_MOUSE_MIDDLE_BUTTON_DOWN: u32 = 16u32;
pub const RI_MOUSE_MIDDLE_BUTTON_UP: u32 = 32u32;
pub const RI_MOUSE_RIGHT_BUTTON_DOWN: u32 = 4u32;
pub const RI_MOUSE_RIGHT_BUTTON_UP: u32 = 8u32;
pub const RI_MOUSE_WHEEL: u32 = 1024u32;
pub const RT_ACCELERATOR: ::windows_sys::core::PCWSTR = 9u16 as _;
pub const RT_ANICURSOR: ::windows_sys::core::PCWSTR = 21u16 as _;
pub const RT_ANIICON: ::windows_sys::core::PCWSTR = 22u16 as _;
pub const RT_BITMAP: ::windows_sys::core::PCWSTR = 2u16 as _;
pub const RT_CURSOR: ::windows_sys::core::PCWSTR = 1u16 as _;
pub const RT_DIALOG: ::windows_sys::core::PCWSTR = 5u16 as _;
pub const RT_DLGINCLUDE: ::windows_sys::core::PCWSTR = 17u16 as _;
pub const RT_FONT: ::windows_sys::core::PCWSTR = 8u16 as _;
pub const RT_FONTDIR: ::windows_sys::core::PCWSTR = 7u16 as _;
pub const RT_HTML: ::windows_sys::core::PCWSTR = 23u16 as _;
pub const RT_ICON: ::windows_sys::core::PCWSTR = 3u16 as _;
pub const RT_MANIFEST: u32 = 24u32;
pub const RT_MENU: ::windows_sys::core::PCWSTR = 4u16 as _;
pub const RT_MESSAGETABLE: ::windows_sys::core::PCWSTR = 11u16 as _;
pub const RT_PLUGPLAY: ::windows_sys::core::PCWSTR = 19u16 as _;
pub const RT_VERSION: ::windows_sys::core::PCWSTR = 16u16 as _;
pub const RT_VXD: ::windows_sys::core::PCWSTR = 20u16 as _;
pub const SBM_ENABLE_ARROWS: u32 = 228u32;
pub const SBM_GETPOS: u32 = 225u32;
pub const SBM_GETRANGE: u32 = 227u32;
pub const SBM_GETSCROLLBARINFO: u32 = 235u32;
pub const SBM_GETSCROLLINFO: u32 = 234u32;
pub const SBM_SETPOS: u32 = 224u32;
pub const SBM_SETRANGE: u32 = 226u32;
pub const SBM_SETRANGEREDRAW: u32 = 230u32;
pub const SBM_SETSCROLLINFO: u32 = 233u32;
pub const SBS_BOTTOMALIGN: i32 = 4i32;
pub const SBS_HORZ: i32 = 0i32;
pub const SBS_LEFTALIGN: i32 = 2i32;
pub const SBS_RIGHTALIGN: i32 = 4i32;
pub const SBS_SIZEBOX: i32 = 8i32;
pub const SBS_SIZEBOXBOTTOMRIGHTALIGN: i32 = 4i32;
pub const SBS_SIZEBOXTOPLEFTALIGN: i32 = 2i32;
pub const SBS_SIZEGRIP: i32 = 16i32;
pub const SBS_TOPALIGN: i32 = 2i32;
pub const SBS_VERT: i32 = 1i32;
pub const SB_BOTH: SCROLLBAR_CONSTANTS = 3i32;
pub const SB_BOTTOM: SCROLLBAR_COMMAND = 7i32;
pub const SB_CTL: SCROLLBAR_CONSTANTS = 2i32;
pub const SB_ENDSCROLL: SCROLLBAR_COMMAND = 8i32;
pub const SB_HORZ: SCROLLBAR_CONSTANTS = 0i32;
pub const SB_LEFT: SCROLLBAR_COMMAND = 6i32;
pub const SB_LINEDOWN: SCROLLBAR_COMMAND = 1i32;
pub const SB_LINELEFT: SCROLLBAR_COMMAND = 0i32;
pub const SB_LINERIGHT: SCROLLBAR_COMMAND = 1i32;
pub const SB_LINEUP: SCROLLBAR_COMMAND = 0i32;
pub const SB_PAGEDOWN: SCROLLBAR_COMMAND = 3i32;
pub const SB_PAGELEFT: SCROLLBAR_COMMAND = 2i32;
pub const SB_PAGERIGHT: SCROLLBAR_COMMAND = 3i32;
pub const SB_PAGEUP: SCROLLBAR_COMMAND = 2i32;
pub const SB_RIGHT: SCROLLBAR_COMMAND = 7i32;
pub const SB_THUMBPOSITION: SCROLLBAR_COMMAND = 4i32;
pub const SB_THUMBTRACK: SCROLLBAR_COMMAND = 5i32;
pub const SB_TOP: SCROLLBAR_COMMAND = 6i32;
pub const SB_VERT: SCROLLBAR_CONSTANTS = 1i32;
pub const SCF_ISSECURE: u32 = 1u32;
pub const SC_ARRANGE: u32 = 61712u32;
pub const SC_CLOSE: u32 = 61536u32;
pub const SC_CONTEXTHELP: u32 = 61824u32;
pub const SC_DEFAULT: u32 = 61792u32;
pub const SC_HOTKEY: u32 = 61776u32;
pub const SC_HSCROLL: u32 = 61568u32;
pub const SC_ICON: u32 = 61472u32;
pub const SC_KEYMENU: u32 = 61696u32;
pub const SC_MAXIMIZE: u32 = 61488u32;
pub const SC_MINIMIZE: u32 = 61472u32;
pub const SC_MONITORPOWER: u32 = 61808u32;
pub const SC_MOUSEMENU: u32 = 61584u32;
pub const SC_MOVE: u32 = 61456u32;
pub const SC_NEXTWINDOW: u32 = 61504u32;
pub const SC_PREVWINDOW: u32 = 61520u32;
pub const SC_RESTORE: u32 = 61728u32;
pub const SC_SEPARATOR: u32 = 61455u32;
pub const SC_SIZE: u32 = 61440u32;
pub const SC_TASKLIST: u32 = 61744u32;
pub const SC_VSCROLL: u32 = 61552u32;
pub const SC_ZOOM: u32 = 61488u32;
pub const SHOW_FULLSCREEN: u32 = 3u32;
pub const SHOW_ICONWINDOW: u32 = 2u32;
pub const SHOW_OPENNOACTIVATE: u32 = 4u32;
pub const SHOW_OPENWINDOW: u32 = 1u32;
pub const SIF_ALL: SCROLLINFO_MASK = 23u32;
pub const SIF_DISABLENOSCROLL: SCROLLINFO_MASK = 8u32;
pub const SIF_PAGE: SCROLLINFO_MASK = 2u32;
pub const SIF_POS: SCROLLINFO_MASK = 4u32;
pub const SIF_RANGE: SCROLLINFO_MASK = 1u32;
pub const SIF_TRACKPOS: SCROLLINFO_MASK = 16u32;
pub const SIZEFULLSCREEN: u32 = 2u32;
pub const SIZEICONIC: u32 = 1u32;
pub const SIZENORMAL: u32 = 0u32;
pub const SIZEZOOMHIDE: u32 = 4u32;
pub const SIZEZOOMSHOW: u32 = 3u32;
pub const SIZE_MAXHIDE: u32 = 4u32;
pub const SIZE_MAXIMIZED: u32 = 2u32;
pub const SIZE_MAXSHOW: u32 = 3u32;
pub const SIZE_MINIMIZED: u32 = 1u32;
pub const SIZE_RESTORED: u32 = 0u32;
pub const SMTO_ABORTIFHUNG: SEND_MESSAGE_TIMEOUT_FLAGS = 2u32;
pub const SMTO_BLOCK: SEND_MESSAGE_TIMEOUT_FLAGS = 1u32;
pub const SMTO_ERRORONEXIT: SEND_MESSAGE_TIMEOUT_FLAGS = 32u32;
pub const SMTO_NORMAL: SEND_MESSAGE_TIMEOUT_FLAGS = 0u32;
pub const SMTO_NOTIMEOUTIFNOTHUNG: SEND_MESSAGE_TIMEOUT_FLAGS = 8u32;
pub const SM_ARRANGE: SYSTEM_METRICS_INDEX = 56i32;
pub const SM_CARETBLINKINGENABLED: u32 = 8194u32;
pub const SM_CLEANBOOT: SYSTEM_METRICS_INDEX = 67i32;
pub const SM_CMETRICS: u32 = 76u32;
pub const SM_CMONITORS: SYSTEM_METRICS_INDEX = 80i32;
pub const SM_CMOUSEBUTTONS: SYSTEM_METRICS_INDEX = 43i32;
pub const SM_CONVERTIBLESLATEMODE: SYSTEM_METRICS_INDEX = 8195i32;
pub const SM_CXBORDER: SYSTEM_METRICS_INDEX = 5i32;
pub const SM_CXCURSOR: SYSTEM_METRICS_INDEX = 13i32;
pub const SM_CXDLGFRAME: SYSTEM_METRICS_INDEX = 7i32;
pub const SM_CXDOUBLECLK: SYSTEM_METRICS_INDEX = 36i32;
pub const SM_CXDRAG: SYSTEM_METRICS_INDEX = 68i32;
pub const SM_CXEDGE: SYSTEM_METRICS_INDEX = 45i32;
pub const SM_CXFIXEDFRAME: SYSTEM_METRICS_INDEX = 7i32;
pub const SM_CXFOCUSBORDER: SYSTEM_METRICS_INDEX = 83i32;
pub const SM_CXFRAME: SYSTEM_METRICS_INDEX = 32i32;
pub const SM_CXFULLSCREEN: SYSTEM_METRICS_INDEX = 16i32;
pub const SM_CXHSCROLL: SYSTEM_METRICS_INDEX = 21i32;
pub const SM_CXHTHUMB: SYSTEM_METRICS_INDEX = 10i32;
pub const SM_CXICON: SYSTEM_METRICS_INDEX = 11i32;
pub const SM_CXICONSPACING: SYSTEM_METRICS_INDEX = 38i32;
pub const SM_CXMAXIMIZED: SYSTEM_METRICS_INDEX = 61i32;
pub const SM_CXMAXTRACK: SYSTEM_METRICS_INDEX = 59i32;
pub const SM_CXMENUCHECK: SYSTEM_METRICS_INDEX = 71i32;
pub const SM_CXMENUSIZE: SYSTEM_METRICS_INDEX = 54i32;
pub const SM_CXMIN: SYSTEM_METRICS_INDEX = 28i32;
pub const SM_CXMINIMIZED: SYSTEM_METRICS_INDEX = 57i32;
pub const SM_CXMINSPACING: SYSTEM_METRICS_INDEX = 47i32;
pub const SM_CXMINTRACK: SYSTEM_METRICS_INDEX = 34i32;
pub const SM_CXPADDEDBORDER: SYSTEM_METRICS_INDEX = 92i32;
pub const SM_CXSCREEN: SYSTEM_METRICS_INDEX = 0i32;
pub const SM_CXSIZE: SYSTEM_METRICS_INDEX = 30i32;
pub const SM_CXSIZEFRAME: SYSTEM_METRICS_INDEX = 32i32;
pub const SM_CXSMICON: SYSTEM_METRICS_INDEX = 49i32;
pub const SM_CXSMSIZE: SYSTEM_METRICS_INDEX = 52i32;
pub const SM_CXVIRTUALSCREEN: SYSTEM_METRICS_INDEX = 78i32;
pub const SM_CXVSCROLL: SYSTEM_METRICS_INDEX = 2i32;
pub const SM_CYBORDER: SYSTEM_METRICS_INDEX = 6i32;
pub const SM_CYCAPTION: SYSTEM_METRICS_INDEX = 4i32;
pub const SM_CYCURSOR: SYSTEM_METRICS_INDEX = 14i32;
pub const SM_CYDLGFRAME: SYSTEM_METRICS_INDEX = 8i32;
pub const SM_CYDOUBLECLK: SYSTEM_METRICS_INDEX = 37i32;
pub const SM_CYDRAG: SYSTEM_METRICS_INDEX = 69i32;
pub const SM_CYEDGE: SYSTEM_METRICS_INDEX = 46i32;
pub const SM_CYFIXEDFRAME: SYSTEM_METRICS_INDEX = 8i32;
pub const SM_CYFOCUSBORDER: SYSTEM_METRICS_INDEX = 84i32;
pub const SM_CYFRAME: SYSTEM_METRICS_INDEX = 33i32;
pub const SM_CYFULLSCREEN: SYSTEM_METRICS_INDEX = 17i32;
pub const SM_CYHSCROLL: SYSTEM_METRICS_INDEX = 3i32;
pub const SM_CYICON: SYSTEM_METRICS_INDEX = 12i32;
pub const SM_CYICONSPACING: SYSTEM_METRICS_INDEX = 39i32;
pub const SM_CYKANJIWINDOW: SYSTEM_METRICS_INDEX = 18i32;
pub const SM_CYMAXIMIZED: SYSTEM_METRICS_INDEX = 62i32;
pub const SM_CYMAXTRACK: SYSTEM_METRICS_INDEX = 60i32;
pub const SM_CYMENU: SYSTEM_METRICS_INDEX = 15i32;
pub const SM_CYMENUCHECK: SYSTEM_METRICS_INDEX = 72i32;
pub const SM_CYMENUSIZE: SYSTEM_METRICS_INDEX = 55i32;
pub const SM_CYMIN: SYSTEM_METRICS_INDEX = 29i32;
pub const SM_CYMINIMIZED: SYSTEM_METRICS_INDEX = 58i32;
pub const SM_CYMINSPACING: SYSTEM_METRICS_INDEX = 48i32;
pub const SM_CYMINTRACK: SYSTEM_METRICS_INDEX = 35i32;
pub const SM_CYSCREEN: SYSTEM_METRICS_INDEX = 1i32;
pub const SM_CYSIZE: SYSTEM_METRICS_INDEX = 31i32;
pub const SM_CYSIZEFRAME: SYSTEM_METRICS_INDEX = 33i32;
pub const SM_CYSMCAPTION: SYSTEM_METRICS_INDEX = 51i32;
pub const SM_CYSMICON: SYSTEM_METRICS_INDEX = 50i32;
pub const SM_CYSMSIZE: SYSTEM_METRICS_INDEX = 53i32;
pub const SM_CYVIRTUALSCREEN: SYSTEM_METRICS_INDEX = 79i32;
pub const SM_CYVSCROLL: SYSTEM_METRICS_INDEX = 20i32;
pub const SM_CYVTHUMB: SYSTEM_METRICS_INDEX = 9i32;
pub const SM_DBCSENABLED: SYSTEM_METRICS_INDEX = 42i32;
pub const SM_DEBUG: SYSTEM_METRICS_INDEX = 22i32;
pub const SM_DIGITIZER: SYSTEM_METRICS_INDEX = 94i32;
pub const SM_IMMENABLED: SYSTEM_METRICS_INDEX = 82i32;
pub const SM_MAXIMUMTOUCHES: SYSTEM_METRICS_INDEX = 95i32;
pub const SM_MEDIACENTER: SYSTEM_METRICS_INDEX = 87i32;
pub const SM_MENUDROPALIGNMENT: SYSTEM_METRICS_INDEX = 40i32;
pub const SM_MIDEASTENABLED: SYSTEM_METRICS_INDEX = 74i32;
pub const SM_MOUSEHORIZONTALWHEELPRESENT: SYSTEM_METRICS_INDEX = 91i32;
pub const SM_MOUSEPRESENT: SYSTEM_METRICS_INDEX = 19i32;
pub const SM_MOUSEWHEELPRESENT: SYSTEM_METRICS_INDEX = 75i32;
pub const SM_NETWORK: SYSTEM_METRICS_INDEX = 63i32;
pub const SM_PENWINDOWS: SYSTEM_METRICS_INDEX = 41i32;
pub const SM_REMOTECONTROL: SYSTEM_METRICS_INDEX = 8193i32;
pub const SM_REMOTESESSION: SYSTEM_METRICS_INDEX = 4096i32;
pub const SM_RESERVED1: u32 = 24u32;
pub const SM_RESERVED2: u32 = 25u32;
pub const SM_RESERVED3: u32 = 26u32;
pub const SM_RESERVED4: u32 = 27u32;
pub const SM_SAMEDISPLAYFORMAT: SYSTEM_METRICS_INDEX = 81i32;
pub const SM_SECURE: SYSTEM_METRICS_INDEX = 44i32;
pub const SM_SERVERR2: SYSTEM_METRICS_INDEX = 89i32;
pub const SM_SHOWSOUNDS: SYSTEM_METRICS_INDEX = 70i32;
pub const SM_SHUTTINGDOWN: SYSTEM_METRICS_INDEX = 8192i32;
pub const SM_SLOWMACHINE: SYSTEM_METRICS_INDEX = 73i32;
pub const SM_STARTER: SYSTEM_METRICS_INDEX = 88i32;
pub const SM_SWAPBUTTON: SYSTEM_METRICS_INDEX = 23i32;
pub const SM_SYSTEMDOCKED: SYSTEM_METRICS_INDEX = 8196i32;
pub const SM_TABLETPC: SYSTEM_METRICS_INDEX = 86i32;
pub const SM_XVIRTUALSCREEN: SYSTEM_METRICS_INDEX = 76i32;
pub const SM_YVIRTUALSCREEN: SYSTEM_METRICS_INDEX = 77i32;
pub const SOUND_SYSTEM_APPEND: u32 = 14u32;
pub const SOUND_SYSTEM_APPSTART: u32 = 12u32;
pub const SOUND_SYSTEM_BEEP: u32 = 3u32;
pub const SOUND_SYSTEM_ERROR: u32 = 4u32;
pub const SOUND_SYSTEM_FAULT: u32 = 13u32;
pub const SOUND_SYSTEM_INFORMATION: u32 = 7u32;
pub const SOUND_SYSTEM_MAXIMIZE: u32 = 8u32;
pub const SOUND_SYSTEM_MENUCOMMAND: u32 = 15u32;
pub const SOUND_SYSTEM_MENUPOPUP: u32 = 16u32;
pub const SOUND_SYSTEM_MINIMIZE: u32 = 9u32;
pub const SOUND_SYSTEM_QUESTION: u32 = 5u32;
pub const SOUND_SYSTEM_RESTOREDOWN: u32 = 11u32;
pub const SOUND_SYSTEM_RESTOREUP: u32 = 10u32;
pub const SOUND_SYSTEM_SHUTDOWN: u32 = 2u32;
pub const SOUND_SYSTEM_STARTUP: u32 = 1u32;
pub const SOUND_SYSTEM_WARNING: u32 = 6u32;
pub const SPIF_SENDCHANGE: SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS = 2u32;
pub const SPIF_SENDWININICHANGE: SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS = 2u32;
pub const SPIF_UPDATEINIFILE: SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS = 1u32;
pub const SPI_GETACCESSTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 60u32;
pub const SPI_GETACTIVEWINDOWTRACKING: SYSTEM_PARAMETERS_INFO_ACTION = 4096u32;
pub const SPI_GETACTIVEWNDTRKTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 8194u32;
pub const SPI_GETACTIVEWNDTRKZORDER: SYSTEM_PARAMETERS_INFO_ACTION = 4108u32;
pub const SPI_GETANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 72u32;
pub const SPI_GETAUDIODESCRIPTION: SYSTEM_PARAMETERS_INFO_ACTION = 116u32;
pub const SPI_GETBEEP: SYSTEM_PARAMETERS_INFO_ACTION = 1u32;
pub const SPI_GETBLOCKSENDINPUTRESETS: SYSTEM_PARAMETERS_INFO_ACTION = 4134u32;
pub const SPI_GETBORDER: SYSTEM_PARAMETERS_INFO_ACTION = 5u32;
pub const SPI_GETCARETBROWSING: SYSTEM_PARAMETERS_INFO_ACTION = 4172u32;
pub const SPI_GETCARETTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 8226u32;
pub const SPI_GETCARETWIDTH: SYSTEM_PARAMETERS_INFO_ACTION = 8198u32;
pub const SPI_GETCLEARTYPE: SYSTEM_PARAMETERS_INFO_ACTION = 4168u32;
pub const SPI_GETCLIENTAREAANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 4162u32;
pub const SPI_GETCOMBOBOXANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 4100u32;
pub const SPI_GETCONTACTVISUALIZATION: SYSTEM_PARAMETERS_INFO_ACTION = 8216u32;
pub const SPI_GETCURSORSHADOW: SYSTEM_PARAMETERS_INFO_ACTION = 4122u32;
pub const SPI_GETDEFAULTINPUTLANG: SYSTEM_PARAMETERS_INFO_ACTION = 89u32;
pub const SPI_GETDESKWALLPAPER: SYSTEM_PARAMETERS_INFO_ACTION = 115u32;
pub const SPI_GETDISABLEOVERLAPPEDCONTENT: SYSTEM_PARAMETERS_INFO_ACTION = 4160u32;
pub const SPI_GETDOCKMOVING: SYSTEM_PARAMETERS_INFO_ACTION = 144u32;
pub const SPI_GETDRAGFROMMAXIMIZE: SYSTEM_PARAMETERS_INFO_ACTION = 140u32;
pub const SPI_GETDRAGFULLWINDOWS: SYSTEM_PARAMETERS_INFO_ACTION = 38u32;
pub const SPI_GETDROPSHADOW: SYSTEM_PARAMETERS_INFO_ACTION = 4132u32;
pub const SPI_GETFASTTASKSWITCH: SYSTEM_PARAMETERS_INFO_ACTION = 35u32;
pub const SPI_GETFILTERKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 50u32;
pub const SPI_GETFLATMENU: SYSTEM_PARAMETERS_INFO_ACTION = 4130u32;
pub const SPI_GETFOCUSBORDERHEIGHT: SYSTEM_PARAMETERS_INFO_ACTION = 8208u32;
pub const SPI_GETFOCUSBORDERWIDTH: SYSTEM_PARAMETERS_INFO_ACTION = 8206u32;
pub const SPI_GETFONTSMOOTHING: SYSTEM_PARAMETERS_INFO_ACTION = 74u32;
pub const SPI_GETFONTSMOOTHINGCONTRAST: SYSTEM_PARAMETERS_INFO_ACTION = 8204u32;
pub const SPI_GETFONTSMOOTHINGORIENTATION: SYSTEM_PARAMETERS_INFO_ACTION = 8210u32;
pub const SPI_GETFONTSMOOTHINGTYPE: SYSTEM_PARAMETERS_INFO_ACTION = 8202u32;
pub const SPI_GETFOREGROUNDFLASHCOUNT: SYSTEM_PARAMETERS_INFO_ACTION = 8196u32;
pub const SPI_GETFOREGROUNDLOCKTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 8192u32;
pub const SPI_GETGESTUREVISUALIZATION: SYSTEM_PARAMETERS_INFO_ACTION = 8218u32;
pub const SPI_GETGRADIENTCAPTIONS: SYSTEM_PARAMETERS_INFO_ACTION = 4104u32;
pub const SPI_GETGRIDGRANULARITY: SYSTEM_PARAMETERS_INFO_ACTION = 18u32;
pub const SPI_GETHANDEDNESS: SYSTEM_PARAMETERS_INFO_ACTION = 8228u32;
pub const SPI_GETHIGHCONTRAST: SYSTEM_PARAMETERS_INFO_ACTION = 66u32;
pub const SPI_GETHOTTRACKING: SYSTEM_PARAMETERS_INFO_ACTION = 4110u32;
pub const SPI_GETHUNGAPPTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 120u32;
pub const SPI_GETICONMETRICS: SYSTEM_PARAMETERS_INFO_ACTION = 45u32;
pub const SPI_GETICONTITLELOGFONT: SYSTEM_PARAMETERS_INFO_ACTION = 31u32;
pub const SPI_GETICONTITLEWRAP: SYSTEM_PARAMETERS_INFO_ACTION = 25u32;
pub const SPI_GETKEYBOARDCUES: SYSTEM_PARAMETERS_INFO_ACTION = 4106u32;
pub const SPI_GETKEYBOARDDELAY: SYSTEM_PARAMETERS_INFO_ACTION = 22u32;
pub const SPI_GETKEYBOARDPREF: SYSTEM_PARAMETERS_INFO_ACTION = 68u32;
pub const SPI_GETKEYBOARDSPEED: SYSTEM_PARAMETERS_INFO_ACTION = 10u32;
pub const SPI_GETLISTBOXSMOOTHSCROLLING: SYSTEM_PARAMETERS_INFO_ACTION = 4102u32;
pub const SPI_GETLOGICALDPIOVERRIDE: SYSTEM_PARAMETERS_INFO_ACTION = 158u32;
pub const SPI_GETLOWPOWERACTIVE: SYSTEM_PARAMETERS_INFO_ACTION = 83u32;
pub const SPI_GETLOWPOWERTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 79u32;
pub const SPI_GETMENUANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 4098u32;
pub const SPI_GETMENUDROPALIGNMENT: SYSTEM_PARAMETERS_INFO_ACTION = 27u32;
pub const SPI_GETMENUFADE: SYSTEM_PARAMETERS_INFO_ACTION = 4114u32;
pub const SPI_GETMENURECT: SYSTEM_PARAMETERS_INFO_ACTION = 162u32;
pub const SPI_GETMENUSHOWDELAY: SYSTEM_PARAMETERS_INFO_ACTION = 106u32;
pub const SPI_GETMENUUNDERLINES: SYSTEM_PARAMETERS_INFO_ACTION = 4106u32;
pub const SPI_GETMESSAGEDURATION: SYSTEM_PARAMETERS_INFO_ACTION = 8214u32;
pub const SPI_GETMINIMIZEDMETRICS: SYSTEM_PARAMETERS_INFO_ACTION = 43u32;
pub const SPI_GETMINIMUMHITRADIUS: SYSTEM_PARAMETERS_INFO_ACTION = 8212u32;
pub const SPI_GETMOUSE: SYSTEM_PARAMETERS_INFO_ACTION = 3u32;
pub const SPI_GETMOUSECLICKLOCK: SYSTEM_PARAMETERS_INFO_ACTION = 4126u32;
pub const SPI_GETMOUSECLICKLOCKTIME: SYSTEM_PARAMETERS_INFO_ACTION = 8200u32;
pub const SPI_GETMOUSEDOCKTHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 126u32;
pub const SPI_GETMOUSEDRAGOUTTHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 132u32;
pub const SPI_GETMOUSEHOVERHEIGHT: SYSTEM_PARAMETERS_INFO_ACTION = 100u32;
pub const SPI_GETMOUSEHOVERTIME: SYSTEM_PARAMETERS_INFO_ACTION = 102u32;
pub const SPI_GETMOUSEHOVERWIDTH: SYSTEM_PARAMETERS_INFO_ACTION = 98u32;
pub const SPI_GETMOUSEKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 54u32;
pub const SPI_GETMOUSESIDEMOVETHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 136u32;
pub const SPI_GETMOUSESONAR: SYSTEM_PARAMETERS_INFO_ACTION = 4124u32;
pub const SPI_GETMOUSESPEED: SYSTEM_PARAMETERS_INFO_ACTION = 112u32;
pub const SPI_GETMOUSETRAILS: SYSTEM_PARAMETERS_INFO_ACTION = 94u32;
pub const SPI_GETMOUSEVANISH: SYSTEM_PARAMETERS_INFO_ACTION = 4128u32;
pub const SPI_GETMOUSEWHEELROUTING: SYSTEM_PARAMETERS_INFO_ACTION = 8220u32;
pub const SPI_GETNONCLIENTMETRICS: SYSTEM_PARAMETERS_INFO_ACTION = 41u32;
pub const SPI_GETPENARBITRATIONTYPE: SYSTEM_PARAMETERS_INFO_ACTION = 8224u32;
pub const SPI_GETPENDOCKTHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 128u32;
pub const SPI_GETPENDRAGOUTTHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 134u32;
pub const SPI_GETPENSIDEMOVETHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 138u32;
pub const SPI_GETPENVISUALIZATION: SYSTEM_PARAMETERS_INFO_ACTION = 8222u32;
pub const SPI_GETPOWEROFFACTIVE: SYSTEM_PARAMETERS_INFO_ACTION = 84u32;
pub const SPI_GETPOWEROFFTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 80u32;
pub const SPI_GETSCREENREADER: SYSTEM_PARAMETERS_INFO_ACTION = 70u32;
pub const SPI_GETSCREENSAVEACTIVE: SYSTEM_PARAMETERS_INFO_ACTION = 16u32;
pub const SPI_GETSCREENSAVERRUNNING: SYSTEM_PARAMETERS_INFO_ACTION = 114u32;
pub const SPI_GETSCREENSAVESECURE: SYSTEM_PARAMETERS_INFO_ACTION = 118u32;
pub const SPI_GETSCREENSAVETIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 14u32;
pub const SPI_GETSELECTIONFADE: SYSTEM_PARAMETERS_INFO_ACTION = 4116u32;
pub const SPI_GETSERIALKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 62u32;
pub const SPI_GETSHOWIMEUI: SYSTEM_PARAMETERS_INFO_ACTION = 110u32;
pub const SPI_GETSHOWSOUNDS: SYSTEM_PARAMETERS_INFO_ACTION = 56u32;
pub const SPI_GETSNAPSIZING: SYSTEM_PARAMETERS_INFO_ACTION = 142u32;
pub const SPI_GETSNAPTODEFBUTTON: SYSTEM_PARAMETERS_INFO_ACTION = 95u32;
pub const SPI_GETSOUNDSENTRY: SYSTEM_PARAMETERS_INFO_ACTION = 64u32;
pub const SPI_GETSPEECHRECOGNITION: SYSTEM_PARAMETERS_INFO_ACTION = 4170u32;
pub const SPI_GETSTICKYKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 58u32;
pub const SPI_GETSYSTEMLANGUAGEBAR: SYSTEM_PARAMETERS_INFO_ACTION = 4176u32;
pub const SPI_GETTHREADLOCALINPUTSETTINGS: SYSTEM_PARAMETERS_INFO_ACTION = 4174u32;
pub const SPI_GETTOGGLEKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 52u32;
pub const SPI_GETTOOLTIPANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 4118u32;
pub const SPI_GETTOOLTIPFADE: SYSTEM_PARAMETERS_INFO_ACTION = 4120u32;
pub const SPI_GETTOUCHPREDICTIONPARAMETERS: SYSTEM_PARAMETERS_INFO_ACTION = 156u32;
pub const SPI_GETUIEFFECTS: SYSTEM_PARAMETERS_INFO_ACTION = 4158u32;
pub const SPI_GETWAITTOKILLSERVICETIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 124u32;
pub const SPI_GETWAITTOKILLTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 122u32;
pub const SPI_GETWHEELSCROLLCHARS: SYSTEM_PARAMETERS_INFO_ACTION = 108u32;
pub const SPI_GETWHEELSCROLLLINES: SYSTEM_PARAMETERS_INFO_ACTION = 104u32;
pub const SPI_GETWINARRANGING: SYSTEM_PARAMETERS_INFO_ACTION = 130u32;
pub const SPI_GETWINDOWSEXTENSION: SYSTEM_PARAMETERS_INFO_ACTION = 92u32;
pub const SPI_GETWORKAREA: SYSTEM_PARAMETERS_INFO_ACTION = 48u32;
pub const SPI_ICONHORIZONTALSPACING: SYSTEM_PARAMETERS_INFO_ACTION = 13u32;
pub const SPI_ICONVERTICALSPACING: SYSTEM_PARAMETERS_INFO_ACTION = 24u32;
pub const SPI_LANGDRIVER: SYSTEM_PARAMETERS_INFO_ACTION = 12u32;
pub const SPI_SCREENSAVERRUNNING: SYSTEM_PARAMETERS_INFO_ACTION = 97u32;
pub const SPI_SETACCESSTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 61u32;
pub const SPI_SETACTIVEWINDOWTRACKING: SYSTEM_PARAMETERS_INFO_ACTION = 4097u32;
pub const SPI_SETACTIVEWNDTRKTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 8195u32;
pub const SPI_SETACTIVEWNDTRKZORDER: SYSTEM_PARAMETERS_INFO_ACTION = 4109u32;
pub const SPI_SETANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 73u32;
pub const SPI_SETAUDIODESCRIPTION: SYSTEM_PARAMETERS_INFO_ACTION = 117u32;
pub const SPI_SETBEEP: SYSTEM_PARAMETERS_INFO_ACTION = 2u32;
pub const SPI_SETBLOCKSENDINPUTRESETS: SYSTEM_PARAMETERS_INFO_ACTION = 4135u32;
pub const SPI_SETBORDER: SYSTEM_PARAMETERS_INFO_ACTION = 6u32;
pub const SPI_SETCARETBROWSING: SYSTEM_PARAMETERS_INFO_ACTION = 4173u32;
pub const SPI_SETCARETTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 8227u32;
pub const SPI_SETCARETWIDTH: SYSTEM_PARAMETERS_INFO_ACTION = 8199u32;
pub const SPI_SETCLEARTYPE: SYSTEM_PARAMETERS_INFO_ACTION = 4169u32;
pub const SPI_SETCLIENTAREAANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 4163u32;
pub const SPI_SETCOMBOBOXANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 4101u32;
pub const SPI_SETCONTACTVISUALIZATION: SYSTEM_PARAMETERS_INFO_ACTION = 8217u32;
pub const SPI_SETCURSORS: SYSTEM_PARAMETERS_INFO_ACTION = 87u32;
pub const SPI_SETCURSORSHADOW: SYSTEM_PARAMETERS_INFO_ACTION = 4123u32;
pub const SPI_SETDEFAULTINPUTLANG: SYSTEM_PARAMETERS_INFO_ACTION = 90u32;
pub const SPI_SETDESKPATTERN: SYSTEM_PARAMETERS_INFO_ACTION = 21u32;
pub const SPI_SETDESKWALLPAPER: SYSTEM_PARAMETERS_INFO_ACTION = 20u32;
pub const SPI_SETDISABLEOVERLAPPEDCONTENT: SYSTEM_PARAMETERS_INFO_ACTION = 4161u32;
pub const SPI_SETDOCKMOVING: SYSTEM_PARAMETERS_INFO_ACTION = 145u32;
pub const SPI_SETDOUBLECLICKTIME: SYSTEM_PARAMETERS_INFO_ACTION = 32u32;
pub const SPI_SETDOUBLECLKHEIGHT: SYSTEM_PARAMETERS_INFO_ACTION = 30u32;
pub const SPI_SETDOUBLECLKWIDTH: SYSTEM_PARAMETERS_INFO_ACTION = 29u32;
pub const SPI_SETDRAGFROMMAXIMIZE: SYSTEM_PARAMETERS_INFO_ACTION = 141u32;
pub const SPI_SETDRAGFULLWINDOWS: SYSTEM_PARAMETERS_INFO_ACTION = 37u32;
pub const SPI_SETDRAGHEIGHT: SYSTEM_PARAMETERS_INFO_ACTION = 77u32;
pub const SPI_SETDRAGWIDTH: SYSTEM_PARAMETERS_INFO_ACTION = 76u32;
pub const SPI_SETDROPSHADOW: SYSTEM_PARAMETERS_INFO_ACTION = 4133u32;
pub const SPI_SETFASTTASKSWITCH: SYSTEM_PARAMETERS_INFO_ACTION = 36u32;
pub const SPI_SETFILTERKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 51u32;
pub const SPI_SETFLATMENU: SYSTEM_PARAMETERS_INFO_ACTION = 4131u32;
pub const SPI_SETFOCUSBORDERHEIGHT: SYSTEM_PARAMETERS_INFO_ACTION = 8209u32;
pub const SPI_SETFOCUSBORDERWIDTH: SYSTEM_PARAMETERS_INFO_ACTION = 8207u32;
pub const SPI_SETFONTSMOOTHING: SYSTEM_PARAMETERS_INFO_ACTION = 75u32;
pub const SPI_SETFONTSMOOTHINGCONTRAST: SYSTEM_PARAMETERS_INFO_ACTION = 8205u32;
pub const SPI_SETFONTSMOOTHINGORIENTATION: SYSTEM_PARAMETERS_INFO_ACTION = 8211u32;
pub const SPI_SETFONTSMOOTHINGTYPE: SYSTEM_PARAMETERS_INFO_ACTION = 8203u32;
pub const SPI_SETFOREGROUNDFLASHCOUNT: SYSTEM_PARAMETERS_INFO_ACTION = 8197u32;
pub const SPI_SETFOREGROUNDLOCKTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 8193u32;
pub const SPI_SETGESTUREVISUALIZATION: SYSTEM_PARAMETERS_INFO_ACTION = 8219u32;
pub const SPI_SETGRADIENTCAPTIONS: SYSTEM_PARAMETERS_INFO_ACTION = 4105u32;
pub const SPI_SETGRIDGRANULARITY: SYSTEM_PARAMETERS_INFO_ACTION = 19u32;
pub const SPI_SETHANDEDNESS: SYSTEM_PARAMETERS_INFO_ACTION = 8229u32;
pub const SPI_SETHANDHELD: SYSTEM_PARAMETERS_INFO_ACTION = 78u32;
pub const SPI_SETHIGHCONTRAST: SYSTEM_PARAMETERS_INFO_ACTION = 67u32;
pub const SPI_SETHOTTRACKING: SYSTEM_PARAMETERS_INFO_ACTION = 4111u32;
pub const SPI_SETHUNGAPPTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 121u32;
pub const SPI_SETICONMETRICS: SYSTEM_PARAMETERS_INFO_ACTION = 46u32;
pub const SPI_SETICONS: SYSTEM_PARAMETERS_INFO_ACTION = 88u32;
pub const SPI_SETICONTITLELOGFONT: SYSTEM_PARAMETERS_INFO_ACTION = 34u32;
pub const SPI_SETICONTITLEWRAP: SYSTEM_PARAMETERS_INFO_ACTION = 26u32;
pub const SPI_SETKEYBOARDCUES: SYSTEM_PARAMETERS_INFO_ACTION = 4107u32;
pub const SPI_SETKEYBOARDDELAY: SYSTEM_PARAMETERS_INFO_ACTION = 23u32;
pub const SPI_SETKEYBOARDPREF: SYSTEM_PARAMETERS_INFO_ACTION = 69u32;
pub const SPI_SETKEYBOARDSPEED: SYSTEM_PARAMETERS_INFO_ACTION = 11u32;
pub const SPI_SETLANGTOGGLE: SYSTEM_PARAMETERS_INFO_ACTION = 91u32;
pub const SPI_SETLISTBOXSMOOTHSCROLLING: SYSTEM_PARAMETERS_INFO_ACTION = 4103u32;
pub const SPI_SETLOGICALDPIOVERRIDE: SYSTEM_PARAMETERS_INFO_ACTION = 159u32;
pub const SPI_SETLOWPOWERACTIVE: SYSTEM_PARAMETERS_INFO_ACTION = 85u32;
pub const SPI_SETLOWPOWERTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 81u32;
pub const SPI_SETMENUANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 4099u32;
pub const SPI_SETMENUDROPALIGNMENT: SYSTEM_PARAMETERS_INFO_ACTION = 28u32;
pub const SPI_SETMENUFADE: SYSTEM_PARAMETERS_INFO_ACTION = 4115u32;
pub const SPI_SETMENURECT: SYSTEM_PARAMETERS_INFO_ACTION = 163u32;
pub const SPI_SETMENUSHOWDELAY: SYSTEM_PARAMETERS_INFO_ACTION = 107u32;
pub const SPI_SETMENUUNDERLINES: SYSTEM_PARAMETERS_INFO_ACTION = 4107u32;
pub const SPI_SETMESSAGEDURATION: SYSTEM_PARAMETERS_INFO_ACTION = 8215u32;
pub const SPI_SETMINIMIZEDMETRICS: SYSTEM_PARAMETERS_INFO_ACTION = 44u32;
pub const SPI_SETMINIMUMHITRADIUS: SYSTEM_PARAMETERS_INFO_ACTION = 8213u32;
pub const SPI_SETMOUSE: SYSTEM_PARAMETERS_INFO_ACTION = 4u32;
pub const SPI_SETMOUSEBUTTONSWAP: SYSTEM_PARAMETERS_INFO_ACTION = 33u32;
pub const SPI_SETMOUSECLICKLOCK: SYSTEM_PARAMETERS_INFO_ACTION = 4127u32;
pub const SPI_SETMOUSECLICKLOCKTIME: SYSTEM_PARAMETERS_INFO_ACTION = 8201u32;
pub const SPI_SETMOUSEDOCKTHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 127u32;
pub const SPI_SETMOUSEDRAGOUTTHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 133u32;
pub const SPI_SETMOUSEHOVERHEIGHT: SYSTEM_PARAMETERS_INFO_ACTION = 101u32;
pub const SPI_SETMOUSEHOVERTIME: SYSTEM_PARAMETERS_INFO_ACTION = 103u32;
pub const SPI_SETMOUSEHOVERWIDTH: SYSTEM_PARAMETERS_INFO_ACTION = 99u32;
pub const SPI_SETMOUSEKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 55u32;
pub const SPI_SETMOUSESIDEMOVETHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 137u32;
pub const SPI_SETMOUSESONAR: SYSTEM_PARAMETERS_INFO_ACTION = 4125u32;
pub const SPI_SETMOUSESPEED: SYSTEM_PARAMETERS_INFO_ACTION = 113u32;
pub const SPI_SETMOUSETRAILS: SYSTEM_PARAMETERS_INFO_ACTION = 93u32;
pub const SPI_SETMOUSEVANISH: SYSTEM_PARAMETERS_INFO_ACTION = 4129u32;
pub const SPI_SETMOUSEWHEELROUTING: SYSTEM_PARAMETERS_INFO_ACTION = 8221u32;
pub const SPI_SETNONCLIENTMETRICS: SYSTEM_PARAMETERS_INFO_ACTION = 42u32;
pub const SPI_SETPENARBITRATIONTYPE: SYSTEM_PARAMETERS_INFO_ACTION = 8225u32;
pub const SPI_SETPENDOCKTHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 129u32;
pub const SPI_SETPENDRAGOUTTHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 135u32;
pub const SPI_SETPENSIDEMOVETHRESHOLD: SYSTEM_PARAMETERS_INFO_ACTION = 139u32;
pub const SPI_SETPENVISUALIZATION: SYSTEM_PARAMETERS_INFO_ACTION = 8223u32;
pub const SPI_SETPENWINDOWS: SYSTEM_PARAMETERS_INFO_ACTION = 49u32;
pub const SPI_SETPOWEROFFACTIVE: SYSTEM_PARAMETERS_INFO_ACTION = 86u32;
pub const SPI_SETPOWEROFFTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 82u32;
pub const SPI_SETSCREENREADER: SYSTEM_PARAMETERS_INFO_ACTION = 71u32;
pub const SPI_SETSCREENSAVEACTIVE: SYSTEM_PARAMETERS_INFO_ACTION = 17u32;
pub const SPI_SETSCREENSAVERRUNNING: SYSTEM_PARAMETERS_INFO_ACTION = 97u32;
pub const SPI_SETSCREENSAVESECURE: SYSTEM_PARAMETERS_INFO_ACTION = 119u32;
pub const SPI_SETSCREENSAVETIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 15u32;
pub const SPI_SETSELECTIONFADE: SYSTEM_PARAMETERS_INFO_ACTION = 4117u32;
pub const SPI_SETSERIALKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 63u32;
pub const SPI_SETSHOWIMEUI: SYSTEM_PARAMETERS_INFO_ACTION = 111u32;
pub const SPI_SETSHOWSOUNDS: SYSTEM_PARAMETERS_INFO_ACTION = 57u32;
pub const SPI_SETSNAPSIZING: SYSTEM_PARAMETERS_INFO_ACTION = 143u32;
pub const SPI_SETSNAPTODEFBUTTON: SYSTEM_PARAMETERS_INFO_ACTION = 96u32;
pub const SPI_SETSOUNDSENTRY: SYSTEM_PARAMETERS_INFO_ACTION = 65u32;
pub const SPI_SETSPEECHRECOGNITION: SYSTEM_PARAMETERS_INFO_ACTION = 4171u32;
pub const SPI_SETSTICKYKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 59u32;
pub const SPI_SETSYSTEMLANGUAGEBAR: SYSTEM_PARAMETERS_INFO_ACTION = 4177u32;
pub const SPI_SETTHREADLOCALINPUTSETTINGS: SYSTEM_PARAMETERS_INFO_ACTION = 4175u32;
pub const SPI_SETTOGGLEKEYS: SYSTEM_PARAMETERS_INFO_ACTION = 53u32;
pub const SPI_SETTOOLTIPANIMATION: SYSTEM_PARAMETERS_INFO_ACTION = 4119u32;
pub const SPI_SETTOOLTIPFADE: SYSTEM_PARAMETERS_INFO_ACTION = 4121u32;
pub const SPI_SETTOUCHPREDICTIONPARAMETERS: SYSTEM_PARAMETERS_INFO_ACTION = 157u32;
pub const SPI_SETUIEFFECTS: SYSTEM_PARAMETERS_INFO_ACTION = 4159u32;
pub const SPI_SETWAITTOKILLSERVICETIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 125u32;
pub const SPI_SETWAITTOKILLTIMEOUT: SYSTEM_PARAMETERS_INFO_ACTION = 123u32;
pub const SPI_SETWHEELSCROLLCHARS: SYSTEM_PARAMETERS_INFO_ACTION = 109u32;
pub const SPI_SETWHEELSCROLLLINES: SYSTEM_PARAMETERS_INFO_ACTION = 105u32;
pub const SPI_SETWINARRANGING: SYSTEM_PARAMETERS_INFO_ACTION = 131u32;
pub const SPI_SETWORKAREA: SYSTEM_PARAMETERS_INFO_ACTION = 47u32;
pub const STATE_SYSTEM_ALERT_HIGH: u32 = 268435456u32;
pub const STATE_SYSTEM_ALERT_LOW: u32 = 67108864u32;
pub const STATE_SYSTEM_ALERT_MEDIUM: u32 = 134217728u32;
pub const STATE_SYSTEM_ANIMATED: u32 = 16384u32;
pub const STATE_SYSTEM_BUSY: u32 = 2048u32;
pub const STATE_SYSTEM_CHECKED: u32 = 16u32;
pub const STATE_SYSTEM_COLLAPSED: u32 = 1024u32;
pub const STATE_SYSTEM_DEFAULT: u32 = 256u32;
pub const STATE_SYSTEM_EXPANDED: u32 = 512u32;
pub const STATE_SYSTEM_EXTSELECTABLE: u32 = 33554432u32;
pub const STATE_SYSTEM_FLOATING: u32 = 4096u32;
pub const STATE_SYSTEM_FOCUSED: u32 = 4u32;
pub const STATE_SYSTEM_HOTTRACKED: u32 = 128u32;
pub const STATE_SYSTEM_INDETERMINATE: u32 = 32u32;
pub const STATE_SYSTEM_LINKED: u32 = 4194304u32;
pub const STATE_SYSTEM_MARQUEED: u32 = 8192u32;
pub const STATE_SYSTEM_MIXED: u32 = 32u32;
pub const STATE_SYSTEM_MOVEABLE: u32 = 262144u32;
pub const STATE_SYSTEM_MULTISELECTABLE: u32 = 16777216u32;
pub const STATE_SYSTEM_PROTECTED: u32 = 536870912u32;
pub const STATE_SYSTEM_READONLY: u32 = 64u32;
pub const STATE_SYSTEM_SELECTABLE: u32 = 2097152u32;
pub const STATE_SYSTEM_SELECTED: u32 = 2u32;
pub const STATE_SYSTEM_SELFVOICING: u32 = 524288u32;
pub const STATE_SYSTEM_SIZEABLE: u32 = 131072u32;
pub const STATE_SYSTEM_TRAVERSED: u32 = 8388608u32;
pub const STATE_SYSTEM_VALID: u32 = 1073741823u32;
pub const STM_GETICON: u32 = 369u32;
pub const STM_GETIMAGE: u32 = 371u32;
pub const STM_MSGMAX: u32 = 372u32;
pub const STM_SETICON: u32 = 368u32;
pub const STM_SETIMAGE: u32 = 370u32;
pub const STN_CLICKED: u32 = 0u32;
pub const STN_DBLCLK: u32 = 1u32;
pub const STN_DISABLE: u32 = 3u32;
pub const STN_ENABLE: u32 = 2u32;
pub const STRSAFE_E_END_OF_FILE: ::windows_sys::core::HRESULT = -2147024858i32;
pub const STRSAFE_E_INSUFFICIENT_BUFFER: ::windows_sys::core::HRESULT = -2147024774i32;
pub const STRSAFE_E_INVALID_PARAMETER: ::windows_sys::core::HRESULT = -2147024809i32;
pub const STRSAFE_FILL_BEHIND_NULL: u32 = 512u32;
pub const STRSAFE_FILL_ON_FAILURE: u32 = 1024u32;
pub const STRSAFE_IGNORE_NULLS: u32 = 256u32;
pub const STRSAFE_MAX_CCH: u32 = 2147483647u32;
pub const STRSAFE_MAX_LENGTH: u32 = 2147483646u32;
pub const STRSAFE_NO_TRUNCATION: u32 = 4096u32;
pub const STRSAFE_NULL_ON_FAILURE: u32 = 2048u32;
pub const STRSAFE_USE_SECURE_CRT: u32 = 0u32;
pub const SWP_ASYNCWINDOWPOS: SET_WINDOW_POS_FLAGS = 16384u32;
pub const SWP_DEFERERASE: SET_WINDOW_POS_FLAGS = 8192u32;
pub const SWP_DRAWFRAME: SET_WINDOW_POS_FLAGS = 32u32;
pub const SWP_FRAMECHANGED: SET_WINDOW_POS_FLAGS = 32u32;
pub const SWP_HIDEWINDOW: SET_WINDOW_POS_FLAGS = 128u32;
pub const SWP_NOACTIVATE: SET_WINDOW_POS_FLAGS = 16u32;
pub const SWP_NOCOPYBITS: SET_WINDOW_POS_FLAGS = 256u32;
pub const SWP_NOMOVE: SET_WINDOW_POS_FLAGS = 2u32;
pub const SWP_NOOWNERZORDER: SET_WINDOW_POS_FLAGS = 512u32;
pub const SWP_NOREDRAW: SET_WINDOW_POS_FLAGS = 8u32;
pub const SWP_NOREPOSITION: SET_WINDOW_POS_FLAGS = 512u32;
pub const SWP_NOSENDCHANGING: SET_WINDOW_POS_FLAGS = 1024u32;
pub const SWP_NOSIZE: SET_WINDOW_POS_FLAGS = 1u32;
pub const SWP_NOZORDER: SET_WINDOW_POS_FLAGS = 4u32;
pub const SWP_SHOWWINDOW: SET_WINDOW_POS_FLAGS = 64u32;
pub const SW_ERASE: SCROLL_WINDOW_FLAGS = 4u32;
pub const SW_FORCEMINIMIZE: SHOW_WINDOW_CMD = 11i32;
pub const SW_HIDE: SHOW_WINDOW_CMD = 0i32;
pub const SW_INVALIDATE: SCROLL_WINDOW_FLAGS = 2u32;
pub const SW_MAX: SHOW_WINDOW_CMD = 11i32;
pub const SW_MAXIMIZE: SHOW_WINDOW_CMD = 3i32;
pub const SW_MINIMIZE: SHOW_WINDOW_CMD = 6i32;
pub const SW_NORMAL: SHOW_WINDOW_CMD = 1i32;
pub const SW_OTHERUNZOOM: SHOW_WINDOW_STATUS = 4u32;
pub const SW_OTHERZOOM: SHOW_WINDOW_STATUS = 2u32;
pub const SW_PARENTCLOSING: SHOW_WINDOW_STATUS = 1u32;
pub const SW_PARENTOPENING: SHOW_WINDOW_STATUS = 3u32;
pub const SW_RESTORE: SHOW_WINDOW_CMD = 9i32;
pub const SW_SCROLLCHILDREN: SCROLL_WINDOW_FLAGS = 1u32;
pub const SW_SHOW: SHOW_WINDOW_CMD = 5i32;
pub const SW_SHOWDEFAULT: SHOW_WINDOW_CMD = 10i32;
pub const SW_SHOWMAXIMIZED: SHOW_WINDOW_CMD = 3i32;
pub const SW_SHOWMINIMIZED: SHOW_WINDOW_CMD = 2i32;
pub const SW_SHOWMINNOACTIVE: SHOW_WINDOW_CMD = 7i32;
pub const SW_SHOWNA: SHOW_WINDOW_CMD = 8i32;
pub const SW_SHOWNOACTIVATE: SHOW_WINDOW_CMD = 4i32;
pub const SW_SHOWNORMAL: SHOW_WINDOW_CMD = 1i32;
pub const SW_SMOOTHSCROLL: SCROLL_WINDOW_FLAGS = 16u32;
pub const TDF_REGISTER: TOOLTIP_DISMISS_FLAGS = 1i32;
pub const TDF_UNREGISTER: TOOLTIP_DISMISS_FLAGS = 2i32;
pub const TIMERV_COALESCING_MAX: u32 = 2147483637u32;
pub const TIMERV_COALESCING_MIN: u32 = 1u32;
pub const TIMERV_DEFAULT_COALESCING: u32 = 0u32;
pub const TIMERV_NO_COALESCING: u32 = 4294967295u32;
pub const TKF_AVAILABLE: u32 = 2u32;
pub const TKF_CONFIRMHOTKEY: u32 = 8u32;
pub const TKF_HOTKEYACTIVE: u32 = 4u32;
pub const TKF_HOTKEYSOUND: u32 = 16u32;
pub const TKF_INDICATOR: u32 = 32u32;
pub const TKF_TOGGLEKEYSON: u32 = 1u32;
pub const TOUCHPREDICTIONPARAMETERS_DEFAULT_LATENCY: u32 = 8u32;
pub const TOUCHPREDICTIONPARAMETERS_DEFAULT_RLS_DELTA: f32 = 0.001f32;
pub const TOUCHPREDICTIONPARAMETERS_DEFAULT_RLS_EXPO_SMOOTH_ALPHA: f32 = 0.99f32;
pub const TOUCHPREDICTIONPARAMETERS_DEFAULT_RLS_LAMBDA_LEARNING_RATE: f32 = 0.001f32;
pub const TOUCHPREDICTIONPARAMETERS_DEFAULT_RLS_LAMBDA_MAX: f32 = 0.999f32;
pub const TOUCHPREDICTIONPARAMETERS_DEFAULT_RLS_LAMBDA_MIN: f32 = 0.9f32;
pub const TOUCHPREDICTIONPARAMETERS_DEFAULT_SAMPLETIME: u32 = 8u32;
pub const TOUCHPREDICTIONPARAMETERS_DEFAULT_USE_HW_TIMESTAMP: u32 = 1u32;
pub const TOUCH_FLAG_NONE: u32 = 0u32;
pub const TOUCH_HIT_TESTING_CLIENT: u32 = 1u32;
pub const TOUCH_HIT_TESTING_DEFAULT: u32 = 0u32;
pub const TOUCH_HIT_TESTING_NONE: u32 = 2u32;
pub const TOUCH_HIT_TESTING_PROXIMITY_CLOSEST: u32 = 0u32;
pub const TOUCH_HIT_TESTING_PROXIMITY_FARTHEST: u32 = 4095u32;
pub const TOUCH_MASK_CONTACTAREA: u32 = 1u32;
pub const TOUCH_MASK_NONE: u32 = 0u32;
pub const TOUCH_MASK_ORIENTATION: u32 = 2u32;
pub const TOUCH_MASK_PRESSURE: u32 = 4u32;
pub const TPM_BOTTOMALIGN: TRACK_POPUP_MENU_FLAGS = 32u32;
pub const TPM_CENTERALIGN: TRACK_POPUP_MENU_FLAGS = 4u32;
pub const TPM_HORIZONTAL: TRACK_POPUP_MENU_FLAGS = 0u32;
pub const TPM_HORNEGANIMATION: TRACK_POPUP_MENU_FLAGS = 2048u32;
pub const TPM_HORPOSANIMATION: TRACK_POPUP_MENU_FLAGS = 1024u32;
pub const TPM_LAYOUTRTL: TRACK_POPUP_MENU_FLAGS = 32768u32;
pub const TPM_LEFTALIGN: TRACK_POPUP_MENU_FLAGS = 0u32;
pub const TPM_LEFTBUTTON: TRACK_POPUP_MENU_FLAGS = 0u32;
pub const TPM_NOANIMATION: TRACK_POPUP_MENU_FLAGS = 16384u32;
pub const TPM_NONOTIFY: TRACK_POPUP_MENU_FLAGS = 128u32;
pub const TPM_RECURSE: TRACK_POPUP_MENU_FLAGS = 1u32;
pub const TPM_RETURNCMD: TRACK_POPUP_MENU_FLAGS = 256u32;
pub const TPM_RIGHTALIGN: TRACK_POPUP_MENU_FLAGS = 8u32;
pub const TPM_RIGHTBUTTON: TRACK_POPUP_MENU_FLAGS = 2u32;
pub const TPM_TOPALIGN: TRACK_POPUP_MENU_FLAGS = 0u32;
pub const TPM_VCENTERALIGN: TRACK_POPUP_MENU_FLAGS = 16u32;
pub const TPM_VERNEGANIMATION: TRACK_POPUP_MENU_FLAGS = 8192u32;
pub const TPM_VERPOSANIMATION: TRACK_POPUP_MENU_FLAGS = 4096u32;
pub const TPM_VERTICAL: TRACK_POPUP_MENU_FLAGS = 64u32;
pub const TPM_WORKAREA: TRACK_POPUP_MENU_FLAGS = 65536u32;
pub const UISF_ACTIVE: u32 = 4u32;
pub const UISF_HIDEACCEL: u32 = 2u32;
pub const UISF_HIDEFOCUS: u32 = 1u32;
pub const UIS_CLEAR: u32 = 2u32;
pub const UIS_INITIALIZE: u32 = 3u32;
pub const UIS_SET: u32 = 1u32;
pub const ULW_ALPHA: UPDATE_LAYERED_WINDOW_FLAGS = 2u32;
pub const ULW_COLORKEY: UPDATE_LAYERED_WINDOW_FLAGS = 1u32;
pub const ULW_EX_NORESIZE: UPDATE_LAYERED_WINDOW_FLAGS = 8u32;
pub const ULW_OPAQUE: UPDATE_LAYERED_WINDOW_FLAGS = 4u32;
pub const UNICODE_NOCHAR: u32 = 65535u32;
pub const UOI_TIMERPROC_EXCEPTION_SUPPRESSION: u32 = 7u32;
pub const USER_DEFAULT_SCREEN_DPI: u32 = 96u32;
pub const USER_TIMER_MAXIMUM: u32 = 2147483647u32;
pub const USER_TIMER_MINIMUM: u32 = 10u32;
pub const WA_ACTIVE: u32 = 1u32;
pub const WA_CLICKACTIVE: u32 = 2u32;
pub const WA_INACTIVE: u32 = 0u32;
pub const WDA_EXCLUDEFROMCAPTURE: WINDOW_DISPLAY_AFFINITY = 17u32;
pub const WDA_MONITOR: WINDOW_DISPLAY_AFFINITY = 1u32;
pub const WDA_NONE: WINDOW_DISPLAY_AFFINITY = 0u32;
pub const WHEEL_DELTA: u32 = 120u32;
pub const WH_CALLWNDPROC: WINDOWS_HOOK_ID = 4i32;
pub const WH_CALLWNDPROCRET: WINDOWS_HOOK_ID = 12i32;
pub const WH_CBT: WINDOWS_HOOK_ID = 5i32;
pub const WH_DEBUG: WINDOWS_HOOK_ID = 9i32;
pub const WH_FOREGROUNDIDLE: WINDOWS_HOOK_ID = 11i32;
pub const WH_GETMESSAGE: WINDOWS_HOOK_ID = 3i32;
pub const WH_HARDWARE: u32 = 8u32;
pub const WH_JOURNALPLAYBACK: WINDOWS_HOOK_ID = 1i32;
pub const WH_JOURNALRECORD: WINDOWS_HOOK_ID = 0i32;
pub const WH_KEYBOARD: WINDOWS_HOOK_ID = 2i32;
pub const WH_KEYBOARD_LL: WINDOWS_HOOK_ID = 13i32;
pub const WH_MAX: u32 = 14u32;
pub const WH_MAXHOOK: u32 = 14u32;
pub const WH_MIN: i32 = -1i32;
pub const WH_MINHOOK: i32 = -1i32;
pub const WH_MOUSE: WINDOWS_HOOK_ID = 7i32;
pub const WH_MOUSE_LL: WINDOWS_HOOK_ID = 14i32;
pub const WH_MSGFILTER: WINDOWS_HOOK_ID = -1i32;
pub const WH_SHELL: WINDOWS_HOOK_ID = 10i32;
pub const WH_SYSMSGFILTER: WINDOWS_HOOK_ID = 6i32;
pub const WINEVENT_INCONTEXT: u32 = 4u32;
pub const WINEVENT_OUTOFCONTEXT: u32 = 0u32;
pub const WINEVENT_SKIPOWNPROCESS: u32 = 2u32;
pub const WINEVENT_SKIPOWNTHREAD: u32 = 1u32;
pub const WINSTA_ACCESSCLIPBOARD: i32 = 4i32;
pub const WINSTA_ACCESSGLOBALATOMS: i32 = 32i32;
pub const WINSTA_CREATEDESKTOP: i32 = 8i32;
pub const WINSTA_ENUMDESKTOPS: i32 = 1i32;
pub const WINSTA_ENUMERATE: i32 = 256i32;
pub const WINSTA_EXITWINDOWS: i32 = 64i32;
pub const WINSTA_READATTRIBUTES: i32 = 2i32;
pub const WINSTA_READSCREEN: i32 = 512i32;
pub const WINSTA_WRITEATTRIBUTES: i32 = 16i32;
pub const WMSZ_BOTTOM: u32 = 6u32;
pub const WMSZ_BOTTOMLEFT: u32 = 7u32;
pub const WMSZ_BOTTOMRIGHT: u32 = 8u32;
pub const WMSZ_LEFT: u32 = 1u32;
pub const WMSZ_RIGHT: u32 = 2u32;
pub const WMSZ_TOP: u32 = 3u32;
pub const WMSZ_TOPLEFT: u32 = 4u32;
pub const WMSZ_TOPRIGHT: u32 = 5u32;
pub const WM_ACTIVATE: u32 = 6u32;
pub const WM_ACTIVATEAPP: u32 = 28u32;
pub const WM_AFXFIRST: u32 = 864u32;
pub const WM_AFXLAST: u32 = 895u32;
pub const WM_APP: u32 = 32768u32;
pub const WM_APPCOMMAND: u32 = 793u32;
pub const WM_ASKCBFORMATNAME: u32 = 780u32;
pub const WM_CANCELJOURNAL: u32 = 75u32;
pub const WM_CANCELMODE: u32 = 31u32;
pub const WM_CAPTURECHANGED: u32 = 533u32;
pub const WM_CHANGECBCHAIN: u32 = 781u32;
pub const WM_CHANGEUISTATE: u32 = 295u32;
pub const WM_CHAR: u32 = 258u32;
pub const WM_CHARTOITEM: u32 = 47u32;
pub const WM_CHILDACTIVATE: u32 = 34u32;
pub const WM_CLEAR: u32 = 771u32;
pub const WM_CLIPBOARDUPDATE: u32 = 797u32;
pub const WM_CLOSE: u32 = 16u32;
pub const WM_COMMAND: u32 = 273u32;
pub const WM_COMMNOTIFY: u32 = 68u32;
pub const WM_COMPACTING: u32 = 65u32;
pub const WM_COMPAREITEM: u32 = 57u32;
pub const WM_CONTEXTMENU: u32 = 123u32;
pub const WM_COPY: u32 = 769u32;
pub const WM_COPYDATA: u32 = 74u32;
pub const WM_CREATE: u32 = 1u32;
pub const WM_CTLCOLORBTN: u32 = 309u32;
pub const WM_CTLCOLORDLG: u32 = 310u32;
pub const WM_CTLCOLOREDIT: u32 = 307u32;
pub const WM_CTLCOLORLISTBOX: u32 = 308u32;
pub const WM_CTLCOLORMSGBOX: u32 = 306u32;
pub const WM_CTLCOLORSCROLLBAR: u32 = 311u32;
pub const WM_CTLCOLORSTATIC: u32 = 312u32;
pub const WM_CUT: u32 = 768u32;
pub const WM_DEADCHAR: u32 = 259u32;
pub const WM_DELETEITEM: u32 = 45u32;
pub const WM_DESTROY: u32 = 2u32;
pub const WM_DESTROYCLIPBOARD: u32 = 775u32;
pub const WM_DEVICECHANGE: u32 = 537u32;
pub const WM_DEVMODECHANGE: u32 = 27u32;
pub const WM_DISPLAYCHANGE: u32 = 126u32;
pub const WM_DPICHANGED: u32 = 736u32;
pub const WM_DPICHANGED_AFTERPARENT: u32 = 739u32;
pub const WM_DPICHANGED_BEFOREPARENT: u32 = 738u32;
pub const WM_DRAWCLIPBOARD: u32 = 776u32;
pub const WM_DRAWITEM: u32 = 43u32;
pub const WM_DROPFILES: u32 = 563u32;
pub const WM_DWMCOLORIZATIONCOLORCHANGED: u32 = 800u32;
pub const WM_DWMCOMPOSITIONCHANGED: u32 = 798u32;
pub const WM_DWMNCRENDERINGCHANGED: u32 = 799u32;
pub const WM_DWMSENDICONICLIVEPREVIEWBITMAP: u32 = 806u32;
pub const WM_DWMSENDICONICTHUMBNAIL: u32 = 803u32;
pub const WM_DWMWINDOWMAXIMIZEDCHANGE: u32 = 801u32;
pub const WM_ENABLE: u32 = 10u32;
pub const WM_ENDSESSION: u32 = 22u32;
pub const WM_ENTERIDLE: u32 = 289u32;
pub const WM_ENTERMENULOOP: u32 = 529u32;
pub const WM_ENTERSIZEMOVE: u32 = 561u32;
pub const WM_ERASEBKGND: u32 = 20u32;
pub const WM_EXITMENULOOP: u32 = 530u32;
pub const WM_EXITSIZEMOVE: u32 = 562u32;
pub const WM_FONTCHANGE: u32 = 29u32;
pub const WM_GESTURE: u32 = 281u32;
pub const WM_GESTURENOTIFY: u32 = 282u32;
pub const WM_GETDLGCODE: u32 = 135u32;
pub const WM_GETDPISCALEDSIZE: u32 = 740u32;
pub const WM_GETFONT: u32 = 49u32;
pub const WM_GETHOTKEY: u32 = 51u32;
pub const WM_GETICON: u32 = 127u32;
pub const WM_GETMINMAXINFO: u32 = 36u32;
pub const WM_GETOBJECT: u32 = 61u32;
pub const WM_GETTEXT: u32 = 13u32;
pub const WM_GETTEXTLENGTH: u32 = 14u32;
pub const WM_GETTITLEBARINFOEX: u32 = 831u32;
pub const WM_HANDHELDFIRST: u32 = 856u32;
pub const WM_HANDHELDLAST: u32 = 863u32;
pub const WM_HELP: u32 = 83u32;
pub const WM_HOTKEY: u32 = 786u32;
pub const WM_HSCROLL: u32 = 276u32;
pub const WM_HSCROLLCLIPBOARD: u32 = 782u32;
pub const WM_ICONERASEBKGND: u32 = 39u32;
pub const WM_IME_CHAR: u32 = 646u32;
pub const WM_IME_COMPOSITION: u32 = 271u32;
pub const WM_IME_COMPOSITIONFULL: u32 = 644u32;
pub const WM_IME_CONTROL: u32 = 643u32;
pub const WM_IME_ENDCOMPOSITION: u32 = 270u32;
pub const WM_IME_KEYDOWN: u32 = 656u32;
pub const WM_IME_KEYLAST: u32 = 271u32;
pub const WM_IME_KEYUP: u32 = 657u32;
pub const WM_IME_NOTIFY: u32 = 642u32;
pub const WM_IME_REQUEST: u32 = 648u32;
pub const WM_IME_SELECT: u32 = 645u32;
pub const WM_IME_SETCONTEXT: u32 = 641u32;
pub const WM_IME_STARTCOMPOSITION: u32 = 269u32;
pub const WM_INITDIALOG: u32 = 272u32;
pub const WM_INITMENU: u32 = 278u32;
pub const WM_INITMENUPOPUP: u32 = 279u32;
pub const WM_INPUT: u32 = 255u32;
pub const WM_INPUTLANGCHANGE: u32 = 81u32;
pub const WM_INPUTLANGCHANGEREQUEST: u32 = 80u32;
pub const WM_INPUT_DEVICE_CHANGE: u32 = 254u32;
pub const WM_KEYDOWN: u32 = 256u32;
pub const WM_KEYFIRST: u32 = 256u32;
pub const WM_KEYLAST: u32 = 265u32;
pub const WM_KEYUP: u32 = 257u32;
pub const WM_KILLFOCUS: u32 = 8u32;
pub const WM_LBUTTONDBLCLK: u32 = 515u32;
pub const WM_LBUTTONDOWN: u32 = 513u32;
pub const WM_LBUTTONUP: u32 = 514u32;
pub const WM_MBUTTONDBLCLK: u32 = 521u32;
pub const WM_MBUTTONDOWN: u32 = 519u32;
pub const WM_MBUTTONUP: u32 = 520u32;
pub const WM_MDIACTIVATE: u32 = 546u32;
pub const WM_MDICASCADE: u32 = 551u32;
pub const WM_MDICREATE: u32 = 544u32;
pub const WM_MDIDESTROY: u32 = 545u32;
pub const WM_MDIGETACTIVE: u32 = 553u32;
pub const WM_MDIICONARRANGE: u32 = 552u32;
pub const WM_MDIMAXIMIZE: u32 = 549u32;
pub const WM_MDINEXT: u32 = 548u32;
pub const WM_MDIREFRESHMENU: u32 = 564u32;
pub const WM_MDIRESTORE: u32 = 547u32;
pub const WM_MDISETMENU: u32 = 560u32;
pub const WM_MDITILE: u32 = 550u32;
pub const WM_MEASUREITEM: u32 = 44u32;
pub const WM_MENUCHAR: u32 = 288u32;
pub const WM_MENUCOMMAND: u32 = 294u32;
pub const WM_MENUDRAG: u32 = 291u32;
pub const WM_MENUGETOBJECT: u32 = 292u32;
pub const WM_MENURBUTTONUP: u32 = 290u32;
pub const WM_MENUSELECT: u32 = 287u32;
pub const WM_MOUSEACTIVATE: u32 = 33u32;
pub const WM_MOUSEFIRST: u32 = 512u32;
pub const WM_MOUSEHWHEEL: u32 = 526u32;
pub const WM_MOUSELAST: u32 = 526u32;
pub const WM_MOUSEMOVE: u32 = 512u32;
pub const WM_MOUSEWHEEL: u32 = 522u32;
pub const WM_MOVE: u32 = 3u32;
pub const WM_MOVING: u32 = 534u32;
pub const WM_NCACTIVATE: u32 = 134u32;
pub const WM_NCCALCSIZE: u32 = 131u32;
pub const WM_NCCREATE: u32 = 129u32;
pub const WM_NCDESTROY: u32 = 130u32;
pub const WM_NCHITTEST: u32 = 132u32;
pub const WM_NCLBUTTONDBLCLK: u32 = 163u32;
pub const WM_NCLBUTTONDOWN: u32 = 161u32;
pub const WM_NCLBUTTONUP: u32 = 162u32;
pub const WM_NCMBUTTONDBLCLK: u32 = 169u32;
pub const WM_NCMBUTTONDOWN: u32 = 167u32;
pub const WM_NCMBUTTONUP: u32 = 168u32;
pub const WM_NCMOUSEHOVER: u32 = 672u32;
pub const WM_NCMOUSELEAVE: u32 = 674u32;
pub const WM_NCMOUSEMOVE: u32 = 160u32;
pub const WM_NCPAINT: u32 = 133u32;
pub const WM_NCPOINTERDOWN: u32 = 578u32;
pub const WM_NCPOINTERUP: u32 = 579u32;
pub const WM_NCPOINTERUPDATE: u32 = 577u32;
pub const WM_NCRBUTTONDBLCLK: u32 = 166u32;
pub const WM_NCRBUTTONDOWN: u32 = 164u32;
pub const WM_NCRBUTTONUP: u32 = 165u32;
pub const WM_NCXBUTTONDBLCLK: u32 = 173u32;
pub const WM_NCXBUTTONDOWN: u32 = 171u32;
pub const WM_NCXBUTTONUP: u32 = 172u32;
pub const WM_NEXTDLGCTL: u32 = 40u32;
pub const WM_NEXTMENU: u32 = 531u32;
pub const WM_NOTIFY: u32 = 78u32;
pub const WM_NOTIFYFORMAT: u32 = 85u32;
pub const WM_NULL: u32 = 0u32;
pub const WM_PAINT: u32 = 15u32;
pub const WM_PAINTCLIPBOARD: u32 = 777u32;
pub const WM_PAINTICON: u32 = 38u32;
pub const WM_PALETTECHANGED: u32 = 785u32;
pub const WM_PALETTEISCHANGING: u32 = 784u32;
pub const WM_PARENTNOTIFY: u32 = 528u32;
pub const WM_PASTE: u32 = 770u32;
pub const WM_PENWINFIRST: u32 = 896u32;
pub const WM_PENWINLAST: u32 = 911u32;
pub const WM_POINTERACTIVATE: u32 = 587u32;
pub const WM_POINTERCAPTURECHANGED: u32 = 588u32;
pub const WM_POINTERDEVICECHANGE: u32 = 568u32;
pub const WM_POINTERDEVICEINRANGE: u32 = 569u32;
pub const WM_POINTERDEVICEOUTOFRANGE: u32 = 570u32;
pub const WM_POINTERDOWN: u32 = 582u32;
pub const WM_POINTERENTER: u32 = 585u32;
pub const WM_POINTERHWHEEL: u32 = 591u32;
pub const WM_POINTERLEAVE: u32 = 586u32;
pub const WM_POINTERROUTEDAWAY: u32 = 594u32;
pub const WM_POINTERROUTEDRELEASED: u32 = 595u32;
pub const WM_POINTERROUTEDTO: u32 = 593u32;
pub const WM_POINTERUP: u32 = 583u32;
pub const WM_POINTERUPDATE: u32 = 581u32;
pub const WM_POINTERWHEEL: u32 = 590u32;
pub const WM_POWER: u32 = 72u32;
pub const WM_POWERBROADCAST: u32 = 536u32;
pub const WM_PRINT: u32 = 791u32;
pub const WM_PRINTCLIENT: u32 = 792u32;
pub const WM_QUERYDRAGICON: u32 = 55u32;
pub const WM_QUERYENDSESSION: u32 = 17u32;
pub const WM_QUERYNEWPALETTE: u32 = 783u32;
pub const WM_QUERYOPEN: u32 = 19u32;
pub const WM_QUERYUISTATE: u32 = 297u32;
pub const WM_QUEUESYNC: u32 = 35u32;
pub const WM_QUIT: u32 = 18u32;
pub const WM_RBUTTONDBLCLK: u32 = 518u32;
pub const WM_RBUTTONDOWN: u32 = 516u32;
pub const WM_RBUTTONUP: u32 = 517u32;
pub const WM_RENDERALLFORMATS: u32 = 774u32;
pub const WM_RENDERFORMAT: u32 = 773u32;
pub const WM_SETCURSOR: u32 = 32u32;
pub const WM_SETFOCUS: u32 = 7u32;
pub const WM_SETFONT: u32 = 48u32;
pub const WM_SETHOTKEY: u32 = 50u32;
pub const WM_SETICON: u32 = 128u32;
pub const WM_SETREDRAW: u32 = 11u32;
pub const WM_SETTEXT: u32 = 12u32;
pub const WM_SETTINGCHANGE: u32 = 26u32;
pub const WM_SHOWWINDOW: u32 = 24u32;
pub const WM_SIZE: u32 = 5u32;
pub const WM_SIZECLIPBOARD: u32 = 779u32;
pub const WM_SIZING: u32 = 532u32;
pub const WM_SPOOLERSTATUS: u32 = 42u32;
pub const WM_STYLECHANGED: u32 = 125u32;
pub const WM_STYLECHANGING: u32 = 124u32;
pub const WM_SYNCPAINT: u32 = 136u32;
pub const WM_SYSCHAR: u32 = 262u32;
pub const WM_SYSCOLORCHANGE: u32 = 21u32;
pub const WM_SYSCOMMAND: u32 = 274u32;
pub const WM_SYSDEADCHAR: u32 = 263u32;
pub const WM_SYSKEYDOWN: u32 = 260u32;
pub const WM_SYSKEYUP: u32 = 261u32;
pub const WM_TABLET_FIRST: u32 = 704u32;
pub const WM_TABLET_LAST: u32 = 735u32;
pub const WM_TCARD: u32 = 82u32;
pub const WM_THEMECHANGED: u32 = 794u32;
pub const WM_TIMECHANGE: u32 = 30u32;
pub const WM_TIMER: u32 = 275u32;
pub const WM_TOOLTIPDISMISS: u32 = 837u32;
pub const WM_TOUCH: u32 = 576u32;
pub const WM_TOUCHHITTESTING: u32 = 589u32;
pub const WM_UNDO: u32 = 772u32;
pub const WM_UNICHAR: u32 = 265u32;
pub const WM_UNINITMENUPOPUP: u32 = 293u32;
pub const WM_UPDATEUISTATE: u32 = 296u32;
pub const WM_USER: u32 = 1024u32;
pub const WM_USERCHANGED: u32 = 84u32;
pub const WM_VKEYTOITEM: u32 = 46u32;
pub const WM_VSCROLL: u32 = 277u32;
pub const WM_VSCROLLCLIPBOARD: u32 = 778u32;
pub const WM_WINDOWPOSCHANGED: u32 = 71u32;
pub const WM_WINDOWPOSCHANGING: u32 = 70u32;
pub const WM_WININICHANGE: u32 = 26u32;
pub const WM_WTSSESSION_CHANGE: u32 = 689u32;
pub const WM_XBUTTONDBLCLK: u32 = 525u32;
pub const WM_XBUTTONDOWN: u32 = 523u32;
pub const WM_XBUTTONUP: u32 = 524u32;
pub const WPF_ASYNCWINDOWPLACEMENT: WINDOWPLACEMENT_FLAGS = 4u32;
pub const WPF_RESTORETOMAXIMIZED: WINDOWPLACEMENT_FLAGS = 2u32;
pub const WPF_SETMINPOSITION: WINDOWPLACEMENT_FLAGS = 1u32;
pub const WSF_VISIBLE: i32 = 1i32;
pub const WS_ACTIVECAPTION: WINDOW_STYLE = 1u32;
pub const WS_BORDER: WINDOW_STYLE = 8388608u32;
pub const WS_CAPTION: WINDOW_STYLE = 12582912u32;
pub const WS_CHILD: WINDOW_STYLE = 1073741824u32;
pub const WS_CHILDWINDOW: WINDOW_STYLE = 1073741824u32;
pub const WS_CLIPCHILDREN: WINDOW_STYLE = 33554432u32;
pub const WS_CLIPSIBLINGS: WINDOW_STYLE = 67108864u32;
pub const WS_DISABLED: WINDOW_STYLE = 134217728u32;
pub const WS_DLGFRAME: WINDOW_STYLE = 4194304u32;
pub const WS_EX_ACCEPTFILES: WINDOW_EX_STYLE = 16u32;
pub const WS_EX_APPWINDOW: WINDOW_EX_STYLE = 262144u32;
pub const WS_EX_CLIENTEDGE: WINDOW_EX_STYLE = 512u32;
pub const WS_EX_COMPOSITED: WINDOW_EX_STYLE = 33554432u32;
pub const WS_EX_CONTEXTHELP: WINDOW_EX_STYLE = 1024u32;
pub const WS_EX_CONTROLPARENT: WINDOW_EX_STYLE = 65536u32;
pub const WS_EX_DLGMODALFRAME: WINDOW_EX_STYLE = 1u32;
pub const WS_EX_LAYERED: WINDOW_EX_STYLE = 524288u32;
pub const WS_EX_LAYOUTRTL: WINDOW_EX_STYLE = 4194304u32;
pub const WS_EX_LEFT: WINDOW_EX_STYLE = 0u32;
pub const WS_EX_LEFTSCROLLBAR: WINDOW_EX_STYLE = 16384u32;
pub const WS_EX_LTRREADING: WINDOW_EX_STYLE = 0u32;
pub const WS_EX_MDICHILD: WINDOW_EX_STYLE = 64u32;
pub const WS_EX_NOACTIVATE: WINDOW_EX_STYLE = 134217728u32;
pub const WS_EX_NOINHERITLAYOUT: WINDOW_EX_STYLE = 1048576u32;
pub const WS_EX_NOPARENTNOTIFY: WINDOW_EX_STYLE = 4u32;
pub const WS_EX_NOREDIRECTIONBITMAP: WINDOW_EX_STYLE = 2097152u32;
pub const WS_EX_OVERLAPPEDWINDOW: WINDOW_EX_STYLE = 768u32;
pub const WS_EX_PALETTEWINDOW: WINDOW_EX_STYLE = 392u32;
pub const WS_EX_RIGHT: WINDOW_EX_STYLE = 4096u32;
pub const WS_EX_RIGHTSCROLLBAR: WINDOW_EX_STYLE = 0u32;
pub const WS_EX_RTLREADING: WINDOW_EX_STYLE = 8192u32;
pub const WS_EX_STATICEDGE: WINDOW_EX_STYLE = 131072u32;
pub const WS_EX_TOOLWINDOW: WINDOW_EX_STYLE = 128u32;
pub const WS_EX_TOPMOST: WINDOW_EX_STYLE = 8u32;
pub const WS_EX_TRANSPARENT: WINDOW_EX_STYLE = 32u32;
pub const WS_EX_WINDOWEDGE: WINDOW_EX_STYLE = 256u32;
pub const WS_GROUP: WINDOW_STYLE = 131072u32;
pub const WS_HSCROLL: WINDOW_STYLE = 1048576u32;
pub const WS_ICONIC: WINDOW_STYLE = 536870912u32;
pub const WS_MAXIMIZE: WINDOW_STYLE = 16777216u32;
pub const WS_MAXIMIZEBOX: WINDOW_STYLE = 65536u32;
pub const WS_MINIMIZE: WINDOW_STYLE = 536870912u32;
pub const WS_MINIMIZEBOX: WINDOW_STYLE = 131072u32;
pub const WS_OVERLAPPED: WINDOW_STYLE = 0u32;
pub const WS_OVERLAPPEDWINDOW: WINDOW_STYLE = 13565952u32;
pub const WS_POPUP: WINDOW_STYLE = 2147483648u32;
pub const WS_POPUPWINDOW: WINDOW_STYLE = 2156396544u32;
pub const WS_SIZEBOX: WINDOW_STYLE = 262144u32;
pub const WS_SYSMENU: WINDOW_STYLE = 524288u32;
pub const WS_TABSTOP: WINDOW_STYLE = 65536u32;
pub const WS_THICKFRAME: WINDOW_STYLE = 262144u32;
pub const WS_TILED: WINDOW_STYLE = 0u32;
pub const WS_TILEDWINDOW: WINDOW_STYLE = 13565952u32;
pub const WS_VISIBLE: WINDOW_STYLE = 268435456u32;
pub const WS_VSCROLL: WINDOW_STYLE = 2097152u32;
pub const WTS_CONSOLE_CONNECT: u32 = 1u32;
pub const WTS_CONSOLE_DISCONNECT: u32 = 2u32;
pub const WTS_REMOTE_CONNECT: u32 = 3u32;
pub const WTS_REMOTE_DISCONNECT: u32 = 4u32;
pub const WTS_SESSION_CREATE: u32 = 10u32;
pub const WTS_SESSION_LOCK: u32 = 7u32;
pub const WTS_SESSION_LOGOFF: u32 = 6u32;
pub const WTS_SESSION_LOGON: u32 = 5u32;
pub const WTS_SESSION_REMOTE_CONTROL: u32 = 9u32;
pub const WTS_SESSION_TERMINATE: u32 = 11u32;
pub const WTS_SESSION_UNLOCK: u32 = 8u32;
pub const WVR_ALIGNBOTTOM: u32 = 64u32;
pub const WVR_ALIGNLEFT: u32 = 32u32;
pub const WVR_ALIGNRIGHT: u32 = 128u32;
pub const WVR_ALIGNTOP: u32 = 16u32;
pub const WVR_HREDRAW: u32 = 256u32;
pub const WVR_VALIDRECTS: u32 = 1024u32;
pub const WVR_VREDRAW: u32 = 512u32;
pub const XBUTTON1: u16 = 1u16;
pub const XBUTTON2: u16 = 2u16;
pub const __WARNING_BANNED_API_USAGE: u32 = 28719u32;
pub const __WARNING_CYCLOMATIC_COMPLEXITY: u32 = 28734u32;
pub const __WARNING_DEREF_NULL_PTR: u32 = 6011u32;
pub const __WARNING_HIGH_PRIORITY_OVERFLOW_POSTCONDITION: u32 = 26045u32;
pub const __WARNING_INCORRECT_ANNOTATION: u32 = 26007u32;
pub const __WARNING_INVALID_PARAM_VALUE_1: u32 = 6387u32;
pub const __WARNING_INVALID_PARAM_VALUE_3: u32 = 28183u32;
pub const __WARNING_MISSING_ZERO_TERMINATION2: u32 = 6054u32;
pub const __WARNING_POSTCONDITION_NULLTERMINATION_VIOLATION: u32 = 26036u32;
pub const __WARNING_POST_EXPECTED: u32 = 28210u32;
pub const __WARNING_POTENTIAL_BUFFER_OVERFLOW_HIGH_PRIORITY: u32 = 26015u32;
pub const __WARNING_POTENTIAL_RANGE_POSTCONDITION_VIOLATION: u32 = 26071u32;
pub const __WARNING_PRECONDITION_NULLTERMINATION_VIOLATION: u32 = 26035u32;
pub const __WARNING_RANGE_POSTCONDITION_VIOLATION: u32 = 26061u32;
pub const __WARNING_RETURNING_BAD_RESULT: u32 = 28196u32;
pub const __WARNING_RETURN_UNINIT_VAR: u32 = 6101u32;
pub const __WARNING_USING_UNINIT_VAR: u32 = 6001u32;
pub type ACCEL_VIRT_FLAGS = u8;
pub type ANIMATE_WINDOW_FLAGS = u32;
pub type CASCADE_WINDOWS_HOW = u32;
pub type CHANGE_WINDOW_MESSAGE_FILTER_FLAGS = u32;
pub type CURSORINFO_FLAGS = u32;
pub type CWP_FLAGS = u32;
pub type DEV_BROADCAST_HDR_DEVICE_TYPE = u32;
pub type DEV_BROADCAST_VOLUME_FLAGS = u16;
pub type DI_FLAGS = u32;
pub type EDIT_CONTROL_FEATURE = i32;
pub type FLASHWINFO_FLAGS = u32;
pub type FOREGROUND_WINDOW_LOCK_CODE = u32;
pub type GDI_IMAGE_TYPE = u32;
pub type GET_ANCESTOR_FLAGS = u32;
pub type GET_CLASS_LONG_INDEX = i32;
pub type GET_MENU_DEFAULT_ITEM_FLAGS = u32;
pub type GET_WINDOW_CMD = u32;
pub type GUITHREADINFO_FLAGS = u32;
pub type HANDEDNESS = i32;
pub type IMAGE_FLAGS = u32;
pub type KBDLLHOOKSTRUCT_FLAGS = u32;
pub type LAYERED_WINDOW_ATTRIBUTES_FLAGS = u32;
pub type MENUGETOBJECTINFO_FLAGS = u32;
pub type MENUINFO_MASK = u32;
pub type MENUINFO_STYLE = u32;
pub type MENU_ITEM_FLAGS = u32;
pub type MENU_ITEM_MASK = u32;
pub type MENU_ITEM_STATE = u32;
pub type MENU_ITEM_TYPE = u32;
pub type MESSAGEBOX_RESULT = i32;
pub type MESSAGEBOX_STYLE = u32;
pub type MINIMIZEDMETRICS_ARRANGE = i32;
pub type MSGFLTINFO_STATUS = u32;
pub type MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS = u32;
pub type MrmDumpType = i32;
pub type MrmIndexerFlags = i32;
pub type MrmPackagingMode = i32;
pub type MrmPackagingOptions = i32;
pub type MrmPlatformVersion = i32;
pub type MrmResourceIndexerMessageSeverity = i32;
pub type OBJECT_IDENTIFIER = i32;
pub type PEEK_MESSAGE_REMOVE_TYPE = u32;
pub type POINTER_INPUT_TYPE = i32;
pub type QUEUE_STATUS_FLAGS = u32;
pub type REGISTER_NOTIFICATION_FLAGS = u32;
pub type SCROLLBAR_COMMAND = i32;
pub type SCROLLBAR_CONSTANTS = i32;
pub type SCROLLINFO_MASK = u32;
pub type SCROLL_WINDOW_FLAGS = u32;
pub type SEND_MESSAGE_TIMEOUT_FLAGS = u32;
pub type SET_WINDOW_POS_FLAGS = u32;
pub type SHOW_WINDOW_CMD = i32;
pub type SHOW_WINDOW_STATUS = u32;
pub type SYSTEM_CURSOR_ID = u32;
pub type SYSTEM_METRICS_INDEX = i32;
pub type SYSTEM_PARAMETERS_INFO_ACTION = u32;
pub type SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS = u32;
pub type TILE_WINDOWS_HOW = u32;
pub type TOOLTIP_DISMISS_FLAGS = i32;
pub type TRACK_POPUP_MENU_FLAGS = u32;
pub type UPDATE_LAYERED_WINDOW_FLAGS = u32;
pub type WINDOWPLACEMENT_FLAGS = u32;
pub type WINDOWS_HOOK_ID = i32;
pub type WINDOW_DISPLAY_AFFINITY = u32;
pub type WINDOW_EX_STYLE = u32;
pub type WINDOW_LONG_PTR_INDEX = i32;
pub type WINDOW_MESSAGE_FILTER_ACTION = u32;
pub type WINDOW_STYLE = u32;
pub type WNDCLASS_STYLES = u32;
#[repr(C)]
pub struct ACCEL {
    pub fVirt: ACCEL_VIRT_FLAGS,
    pub key: u16,
    pub cmd: u16,
}
impl ::core::marker::Copy for ACCEL {}
impl ::core::clone::Clone for ACCEL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ALTTABINFO {
    pub cbSize: u32,
    pub cItems: i32,
    pub cColumns: i32,
    pub cRows: i32,
    pub iColFocus: i32,
    pub iRowFocus: i32,
    pub cxItem: i32,
    pub cyItem: i32,
    pub ptStart: super::super::Foundation::POINT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ALTTABINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ALTTABINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ANIMATIONINFO {
    pub cbSize: u32,
    pub iMinAnimate: i32,
}
impl ::core::marker::Copy for ANIMATIONINFO {}
impl ::core::clone::Clone for ANIMATIONINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct AUDIODESCRIPTION {
    pub cbSize: u32,
    pub Enabled: super::super::Foundation::BOOL,
    pub Locale: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for AUDIODESCRIPTION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for AUDIODESCRIPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CBTACTIVATESTRUCT {
    pub fMouse: super::super::Foundation::BOOL,
    pub hWndActive: super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CBTACTIVATESTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CBTACTIVATESTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CBT_CREATEWNDA {
    pub lpcs: *mut CREATESTRUCTA,
    pub hwndInsertAfter: super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CBT_CREATEWNDA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CBT_CREATEWNDA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CBT_CREATEWNDW {
    pub lpcs: *mut CREATESTRUCTW,
    pub hwndInsertAfter: super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CBT_CREATEWNDW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CBT_CREATEWNDW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CHANGEFILTERSTRUCT {
    pub cbSize: u32,
    pub ExtStatus: MSGFLTINFO_STATUS,
}
impl ::core::marker::Copy for CHANGEFILTERSTRUCT {}
impl ::core::clone::Clone for CHANGEFILTERSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLIENTCREATESTRUCT {
    pub hWindowMenu: super::super::Foundation::HANDLE,
    pub idFirstChild: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLIENTCREATESTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLIENTCREATESTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CREATESTRUCTA {
    pub lpCreateParams: *mut ::core::ffi::c_void,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub hMenu: HMENU,
    pub hwndParent: super::super::Foundation::HWND,
    pub cy: i32,
    pub cx: i32,
    pub y: i32,
    pub x: i32,
    pub style: i32,
    pub lpszName: ::windows_sys::core::PCSTR,
    pub lpszClass: ::windows_sys::core::PCSTR,
    pub dwExStyle: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CREATESTRUCTA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CREATESTRUCTA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CREATESTRUCTW {
    pub lpCreateParams: *mut ::core::ffi::c_void,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub hMenu: HMENU,
    pub hwndParent: super::super::Foundation::HWND,
    pub cy: i32,
    pub cx: i32,
    pub y: i32,
    pub x: i32,
    pub style: i32,
    pub lpszName: ::windows_sys::core::PCWSTR,
    pub lpszClass: ::windows_sys::core::PCWSTR,
    pub dwExStyle: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CREATESTRUCTW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CREATESTRUCTW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CURSORINFO {
    pub cbSize: u32,
    pub flags: CURSORINFO_FLAGS,
    pub hCursor: HCURSOR,
    pub ptScreenPos: super::super::Foundation::POINT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CURSORINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CURSORINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CURSORSHAPE {
    pub xHotSpot: i32,
    pub yHotSpot: i32,
    pub cx: i32,
    pub cy: i32,
    pub cbWidth: i32,
    pub Planes: u8,
    pub BitsPixel: u8,
}
impl ::core::marker::Copy for CURSORSHAPE {}
impl ::core::clone::Clone for CURSORSHAPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CWPRETSTRUCT {
    pub lResult: super::super::Foundation::LRESULT,
    pub lParam: super::super::Foundation::LPARAM,
    pub wParam: super::super::Foundation::WPARAM,
    pub message: u32,
    pub hwnd: super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CWPRETSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CWPRETSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CWPSTRUCT {
    pub lParam: super::super::Foundation::LPARAM,
    pub wParam: super::super::Foundation::WPARAM,
    pub message: u32,
    pub hwnd: super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CWPSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CWPSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUGHOOKINFO {
    pub idThread: u32,
    pub idThreadInstaller: u32,
    pub lParam: super::super::Foundation::LPARAM,
    pub wParam: super::super::Foundation::WPARAM,
    pub code: i32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUGHOOKINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUGHOOKINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEVICE_EVENT_BECOMING_READY {
    pub Version: u32,
    pub Reason: u32,
    pub Estimated100msToReady: u32,
}
impl ::core::marker::Copy for DEVICE_EVENT_BECOMING_READY {}
impl ::core::clone::Clone for DEVICE_EVENT_BECOMING_READY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEVICE_EVENT_EXTERNAL_REQUEST {
    pub Version: u32,
    pub DeviceClass: u32,
    pub ButtonStatus: u16,
    pub Request: u16,
    pub SystemTime: i64,
}
impl ::core::marker::Copy for DEVICE_EVENT_EXTERNAL_REQUEST {}
impl ::core::clone::Clone for DEVICE_EVENT_EXTERNAL_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEVICE_EVENT_GENERIC_DATA {
    pub EventNumber: u32,
}
impl ::core::marker::Copy for DEVICE_EVENT_GENERIC_DATA {}
impl ::core::clone::Clone for DEVICE_EVENT_GENERIC_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEVICE_EVENT_MOUNT {
    pub Version: u32,
    pub Flags: u32,
    pub FileSystemNameLength: u32,
    pub FileSystemNameOffset: u32,
}
impl ::core::marker::Copy for DEVICE_EVENT_MOUNT {}
impl ::core::clone::Clone for DEVICE_EVENT_MOUNT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEVICE_EVENT_RBC_DATA {
    pub EventNumber: u32,
    pub SenseQualifier: u8,
    pub SenseCode: u8,
    pub SenseKey: u8,
    pub Reserved: u8,
    pub Information: u32,
}
impl ::core::marker::Copy for DEVICE_EVENT_RBC_DATA {}
impl ::core::clone::Clone for DEVICE_EVENT_RBC_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_DEVICEINTERFACE_A {
    pub dbcc_size: u32,
    pub dbcc_devicetype: u32,
    pub dbcc_reserved: u32,
    pub dbcc_classguid: ::windows_sys::core::GUID,
    pub dbcc_name: [u8; 1],
}
impl ::core::marker::Copy for DEV_BROADCAST_DEVICEINTERFACE_A {}
impl ::core::clone::Clone for DEV_BROADCAST_DEVICEINTERFACE_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_DEVICEINTERFACE_W {
    pub dbcc_size: u32,
    pub dbcc_devicetype: u32,
    pub dbcc_reserved: u32,
    pub dbcc_classguid: ::windows_sys::core::GUID,
    pub dbcc_name: [u16; 1],
}
impl ::core::marker::Copy for DEV_BROADCAST_DEVICEINTERFACE_W {}
impl ::core::clone::Clone for DEV_BROADCAST_DEVICEINTERFACE_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_DEVNODE {
    pub dbcd_size: u32,
    pub dbcd_devicetype: u32,
    pub dbcd_reserved: u32,
    pub dbcd_devnode: u32,
}
impl ::core::marker::Copy for DEV_BROADCAST_DEVNODE {}
impl ::core::clone::Clone for DEV_BROADCAST_DEVNODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEV_BROADCAST_HANDLE {
    pub dbch_size: u32,
    pub dbch_devicetype: u32,
    pub dbch_reserved: u32,
    pub dbch_handle: super::super::Foundation::HANDLE,
    pub dbch_hdevnotify: HDEVNOTIFY,
    pub dbch_eventguid: ::windows_sys::core::GUID,
    pub dbch_nameoffset: i32,
    pub dbch_data: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEV_BROADCAST_HANDLE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEV_BROADCAST_HANDLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_HANDLE32 {
    pub dbch_size: u32,
    pub dbch_devicetype: u32,
    pub dbch_reserved: u32,
    pub dbch_handle: u32,
    pub dbch_hdevnotify: u32,
    pub dbch_eventguid: ::windows_sys::core::GUID,
    pub dbch_nameoffset: i32,
    pub dbch_data: [u8; 1],
}
impl ::core::marker::Copy for DEV_BROADCAST_HANDLE32 {}
impl ::core::clone::Clone for DEV_BROADCAST_HANDLE32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_HANDLE64 {
    pub dbch_size: u32,
    pub dbch_devicetype: u32,
    pub dbch_reserved: u32,
    pub dbch_handle: u64,
    pub dbch_hdevnotify: u64,
    pub dbch_eventguid: ::windows_sys::core::GUID,
    pub dbch_nameoffset: i32,
    pub dbch_data: [u8; 1],
}
impl ::core::marker::Copy for DEV_BROADCAST_HANDLE64 {}
impl ::core::clone::Clone for DEV_BROADCAST_HANDLE64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_HDR {
    pub dbch_size: u32,
    pub dbch_devicetype: DEV_BROADCAST_HDR_DEVICE_TYPE,
    pub dbch_reserved: u32,
}
impl ::core::marker::Copy for DEV_BROADCAST_HDR {}
impl ::core::clone::Clone for DEV_BROADCAST_HDR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_NET {
    pub dbcn_size: u32,
    pub dbcn_devicetype: u32,
    pub dbcn_reserved: u32,
    pub dbcn_resource: u32,
    pub dbcn_flags: u32,
}
impl ::core::marker::Copy for DEV_BROADCAST_NET {}
impl ::core::clone::Clone for DEV_BROADCAST_NET {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_OEM {
    pub dbco_size: u32,
    pub dbco_devicetype: u32,
    pub dbco_reserved: u32,
    pub dbco_identifier: u32,
    pub dbco_suppfunc: u32,
}
impl ::core::marker::Copy for DEV_BROADCAST_OEM {}
impl ::core::clone::Clone for DEV_BROADCAST_OEM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_PORT_A {
    pub dbcp_size: u32,
    pub dbcp_devicetype: u32,
    pub dbcp_reserved: u32,
    pub dbcp_name: [u8; 1],
}
impl ::core::marker::Copy for DEV_BROADCAST_PORT_A {}
impl ::core::clone::Clone for DEV_BROADCAST_PORT_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_PORT_W {
    pub dbcp_size: u32,
    pub dbcp_devicetype: u32,
    pub dbcp_reserved: u32,
    pub dbcp_name: [u16; 1],
}
impl ::core::marker::Copy for DEV_BROADCAST_PORT_W {}
impl ::core::clone::Clone for DEV_BROADCAST_PORT_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEV_BROADCAST_VOLUME {
    pub dbcv_size: u32,
    pub dbcv_devicetype: u32,
    pub dbcv_reserved: u32,
    pub dbcv_unitmask: u32,
    pub dbcv_flags: DEV_BROADCAST_VOLUME_FLAGS,
}
impl ::core::marker::Copy for DEV_BROADCAST_VOLUME {}
impl ::core::clone::Clone for DEV_BROADCAST_VOLUME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DISK_HEALTH_NOTIFICATION_DATA {
    pub DeviceGuid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for DISK_HEALTH_NOTIFICATION_DATA {}
impl ::core::clone::Clone for DISK_HEALTH_NOTIFICATION_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(2))]
pub struct DLGITEMTEMPLATE {
    pub style: u32,
    pub dwExtendedStyle: u32,
    pub x: i16,
    pub y: i16,
    pub cx: i16,
    pub cy: i16,
    pub id: u16,
}
impl ::core::marker::Copy for DLGITEMTEMPLATE {}
impl ::core::clone::Clone for DLGITEMTEMPLATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(2))]
pub struct DLGTEMPLATE {
    pub style: u32,
    pub dwExtendedStyle: u32,
    pub cdit: u16,
    pub x: i16,
    pub y: i16,
    pub cx: i16,
    pub cy: i16,
}
impl ::core::marker::Copy for DLGTEMPLATE {}
impl ::core::clone::Clone for DLGTEMPLATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DROPSTRUCT {
    pub hwndSource: super::super::Foundation::HWND,
    pub hwndSink: super::super::Foundation::HWND,
    pub wFmt: u32,
    pub dwData: usize,
    pub ptDrop: super::super::Foundation::POINT,
    pub dwControlData: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DROPSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DROPSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EVENTMSG {
    pub message: u32,
    pub paramL: u32,
    pub paramH: u32,
    pub time: u32,
    pub hwnd: super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EVENTMSG {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EVENTMSG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FLASHWINFO {
    pub cbSize: u32,
    pub hwnd: super::super::Foundation::HWND,
    pub dwFlags: FLASHWINFO_FLAGS,
    pub uCount: u32,
    pub dwTimeout: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FLASHWINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FLASHWINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct GETCLIPBMETADATA {
    pub Version: u32,
    pub IsDelayRendered: super::super::Foundation::BOOL,
    pub IsSynthetic: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for GETCLIPBMETADATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for GETCLIPBMETADATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GUID_IO_DISK_CLONE_ARRIVAL_INFORMATION {
    pub DiskNumber: u32,
}
impl ::core::marker::Copy for GUID_IO_DISK_CLONE_ARRIVAL_INFORMATION {}
impl ::core::clone::Clone for GUID_IO_DISK_CLONE_ARRIVAL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct GUITHREADINFO {
    pub cbSize: u32,
    pub flags: GUITHREADINFO_FLAGS,
    pub hwndActive: super::super::Foundation::HWND,
    pub hwndFocus: super::super::Foundation::HWND,
    pub hwndCapture: super::super::Foundation::HWND,
    pub hwndMenuOwner: super::super::Foundation::HWND,
    pub hwndMoveSize: super::super::Foundation::HWND,
    pub hwndCaret: super::super::Foundation::HWND,
    pub rcCaret: super::super::Foundation::RECT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for GUITHREADINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for GUITHREADINFO {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HACCEL = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct HARDWAREHOOKSTRUCT {
    pub hwnd: super::super::Foundation::HWND,
    pub message: u32,
    pub wParam: super::super::Foundation::WPARAM,
    pub lParam: super::super::Foundation::LPARAM,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for HARDWAREHOOKSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for HARDWAREHOOKSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HCURSOR = isize;
pub type HDEVNOTIFY = *mut ::core::ffi::c_void;
pub type HDWP = isize;
pub type HHOOK = isize;
pub type HICON = isize;
pub type HMENU = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct ICONINFO {
    pub fIcon: super::super::Foundation::BOOL,
    pub xHotspot: u32,
    pub yHotspot: u32,
    pub hbmMask: super::super::Graphics::Gdi::HBITMAP,
    pub hbmColor: super::super::Graphics::Gdi::HBITMAP,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for ICONINFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for ICONINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct ICONINFOEXA {
    pub cbSize: u32,
    pub fIcon: super::super::Foundation::BOOL,
    pub xHotspot: u32,
    pub yHotspot: u32,
    pub hbmMask: super::super::Graphics::Gdi::HBITMAP,
    pub hbmColor: super::super::Graphics::Gdi::HBITMAP,
    pub wResID: u16,
    pub szModName: [u8; 260],
    pub szResName: [u8; 260],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for ICONINFOEXA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for ICONINFOEXA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct ICONINFOEXW {
    pub cbSize: u32,
    pub fIcon: super::super::Foundation::BOOL,
    pub xHotspot: u32,
    pub yHotspot: u32,
    pub hbmMask: super::super::Graphics::Gdi::HBITMAP,
    pub hbmColor: super::super::Graphics::Gdi::HBITMAP,
    pub wResID: u16,
    pub szModName: [u16; 260],
    pub szResName: [u16; 260],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for ICONINFOEXW {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for ICONINFOEXW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct ICONMETRICSA {
    pub cbSize: u32,
    pub iHorzSpacing: i32,
    pub iVertSpacing: i32,
    pub iTitleWrap: i32,
    pub lfFont: super::super::Graphics::Gdi::LOGFONTA,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for ICONMETRICSA {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for ICONMETRICSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct ICONMETRICSW {
    pub cbSize: u32,
    pub iHorzSpacing: i32,
    pub iVertSpacing: i32,
    pub iTitleWrap: i32,
    pub lfFont: super::super::Graphics::Gdi::LOGFONTW,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for ICONMETRICSW {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for ICONMETRICSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IndexedResourceQualifier {
    pub name: ::windows_sys::core::PWSTR,
    pub value: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for IndexedResourceQualifier {}
impl ::core::clone::Clone for IndexedResourceQualifier {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct KBDLLHOOKSTRUCT {
    pub vkCode: u32,
    pub scanCode: u32,
    pub flags: KBDLLHOOKSTRUCT_FLAGS,
    pub time: u32,
    pub dwExtraInfo: usize,
}
impl ::core::marker::Copy for KBDLLHOOKSTRUCT {}
impl ::core::clone::Clone for KBDLLHOOKSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MDICREATESTRUCTA {
    pub szClass: ::windows_sys::core::PCSTR,
    pub szTitle: ::windows_sys::core::PCSTR,
    pub hOwner: super::super::Foundation::HANDLE,
    pub x: i32,
    pub y: i32,
    pub cx: i32,
    pub cy: i32,
    pub style: WINDOW_STYLE,
    pub lParam: super::super::Foundation::LPARAM,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MDICREATESTRUCTA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MDICREATESTRUCTA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MDICREATESTRUCTW {
    pub szClass: ::windows_sys::core::PCWSTR,
    pub szTitle: ::windows_sys::core::PCWSTR,
    pub hOwner: super::super::Foundation::HANDLE,
    pub x: i32,
    pub y: i32,
    pub cx: i32,
    pub cy: i32,
    pub style: WINDOW_STYLE,
    pub lParam: super::super::Foundation::LPARAM,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MDICREATESTRUCTW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MDICREATESTRUCTW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MDINEXTMENU {
    pub hmenuIn: HMENU,
    pub hmenuNext: HMENU,
    pub hwndNext: super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MDINEXTMENU {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MDINEXTMENU {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MENUBARINFO {
    pub cbSize: u32,
    pub rcBar: super::super::Foundation::RECT,
    pub hMenu: HMENU,
    pub hwndMenu: super::super::Foundation::HWND,
    pub _bitfield: i32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MENUBARINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MENUBARINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MENUEX_TEMPLATE_HEADER {
    pub wVersion: u16,
    pub wOffset: u16,
    pub dwHelpId: u32,
}
impl ::core::marker::Copy for MENUEX_TEMPLATE_HEADER {}
impl ::core::clone::Clone for MENUEX_TEMPLATE_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MENUEX_TEMPLATE_ITEM {
    pub dwType: u32,
    pub dwState: u32,
    pub uId: u32,
    pub wFlags: u16,
    pub szText: [u16; 1],
}
impl ::core::marker::Copy for MENUEX_TEMPLATE_ITEM {}
impl ::core::clone::Clone for MENUEX_TEMPLATE_ITEM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MENUGETOBJECTINFO {
    pub dwFlags: MENUGETOBJECTINFO_FLAGS,
    pub uPos: u32,
    pub hmenu: HMENU,
    pub riid: *mut ::core::ffi::c_void,
    pub pvObj: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for MENUGETOBJECTINFO {}
impl ::core::clone::Clone for MENUGETOBJECTINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct MENUINFO {
    pub cbSize: u32,
    pub fMask: MENUINFO_MASK,
    pub dwStyle: MENUINFO_STYLE,
    pub cyMax: u32,
    pub hbrBack: super::super::Graphics::Gdi::HBRUSH,
    pub dwContextHelpID: u32,
    pub dwMenuData: usize,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for MENUINFO {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for MENUINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct MENUITEMINFOA {
    pub cbSize: u32,
    pub fMask: MENU_ITEM_MASK,
    pub fType: MENU_ITEM_TYPE,
    pub fState: MENU_ITEM_STATE,
    pub wID: u32,
    pub hSubMenu: HMENU,
    pub hbmpChecked: super::super::Graphics::Gdi::HBITMAP,
    pub hbmpUnchecked: super::super::Graphics::Gdi::HBITMAP,
    pub dwItemData: usize,
    pub dwTypeData: ::windows_sys::core::PSTR,
    pub cch: u32,
    pub hbmpItem: super::super::Graphics::Gdi::HBITMAP,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for MENUITEMINFOA {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for MENUITEMINFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct MENUITEMINFOW {
    pub cbSize: u32,
    pub fMask: MENU_ITEM_MASK,
    pub fType: MENU_ITEM_TYPE,
    pub fState: MENU_ITEM_STATE,
    pub wID: u32,
    pub hSubMenu: HMENU,
    pub hbmpChecked: super::super::Graphics::Gdi::HBITMAP,
    pub hbmpUnchecked: super::super::Graphics::Gdi::HBITMAP,
    pub dwItemData: usize,
    pub dwTypeData: ::windows_sys::core::PWSTR,
    pub cch: u32,
    pub hbmpItem: super::super::Graphics::Gdi::HBITMAP,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for MENUITEMINFOW {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for MENUITEMINFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MENUITEMTEMPLATE {
    pub mtOption: u16,
    pub mtID: u16,
    pub mtString: [u16; 1],
}
impl ::core::marker::Copy for MENUITEMTEMPLATE {}
impl ::core::clone::Clone for MENUITEMTEMPLATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MENUITEMTEMPLATEHEADER {
    pub versionNumber: u16,
    pub offset: u16,
}
impl ::core::marker::Copy for MENUITEMTEMPLATEHEADER {}
impl ::core::clone::Clone for MENUITEMTEMPLATEHEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MENUTEMPLATEEX {
    pub Anonymous: MENUTEMPLATEEX_0,
}
impl ::core::marker::Copy for MENUTEMPLATEEX {}
impl ::core::clone::Clone for MENUTEMPLATEEX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union MENUTEMPLATEEX_0 {
    pub Menu: MENUTEMPLATEEX_0_1,
    pub MenuEx: MENUTEMPLATEEX_0_0,
}
impl ::core::marker::Copy for MENUTEMPLATEEX_0 {}
impl ::core::clone::Clone for MENUTEMPLATEEX_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MENUTEMPLATEEX_0_0 {
    pub mexHeader: MENUEX_TEMPLATE_HEADER,
    pub mexItem: [MENUEX_TEMPLATE_ITEM; 1],
}
impl ::core::marker::Copy for MENUTEMPLATEEX_0_0 {}
impl ::core::clone::Clone for MENUTEMPLATEEX_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MENUTEMPLATEEX_0_1 {
    pub mitHeader: MENUITEMTEMPLATEHEADER,
    pub miTemplate: [MENUITEMTEMPLATE; 1],
}
impl ::core::marker::Copy for MENUTEMPLATEEX_0_1 {}
impl ::core::clone::Clone for MENUTEMPLATEEX_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MESSAGE_RESOURCE_BLOCK {
    pub LowId: u32,
    pub HighId: u32,
    pub OffsetToEntries: u32,
}
impl ::core::marker::Copy for MESSAGE_RESOURCE_BLOCK {}
impl ::core::clone::Clone for MESSAGE_RESOURCE_BLOCK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MESSAGE_RESOURCE_DATA {
    pub NumberOfBlocks: u32,
    pub Blocks: [MESSAGE_RESOURCE_BLOCK; 1],
}
impl ::core::marker::Copy for MESSAGE_RESOURCE_DATA {}
impl ::core::clone::Clone for MESSAGE_RESOURCE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MESSAGE_RESOURCE_ENTRY {
    pub Length: u16,
    pub Flags: u16,
    pub Text: [u8; 1],
}
impl ::core::marker::Copy for MESSAGE_RESOURCE_ENTRY {}
impl ::core::clone::Clone for MESSAGE_RESOURCE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MINIMIZEDMETRICS {
    pub cbSize: u32,
    pub iWidth: i32,
    pub iHorzGap: i32,
    pub iVertGap: i32,
    pub iArrange: MINIMIZEDMETRICS_ARRANGE,
}
impl ::core::marker::Copy for MINIMIZEDMETRICS {}
impl ::core::clone::Clone for MINIMIZEDMETRICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MINMAXINFO {
    pub ptReserved: super::super::Foundation::POINT,
    pub ptMaxSize: super::super::Foundation::POINT,
    pub ptMaxPosition: super::super::Foundation::POINT,
    pub ptMinTrackSize: super::super::Foundation::POINT,
    pub ptMaxTrackSize: super::super::Foundation::POINT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MINMAXINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MINMAXINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MOUSEHOOKSTRUCT {
    pub pt: super::super::Foundation::POINT,
    pub hwnd: super::super::Foundation::HWND,
    pub wHitTestCode: u32,
    pub dwExtraInfo: usize,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MOUSEHOOKSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MOUSEHOOKSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MOUSEHOOKSTRUCTEX {
    pub Base: MOUSEHOOKSTRUCT,
    pub mouseData: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MOUSEHOOKSTRUCTEX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MOUSEHOOKSTRUCTEX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MSG {
    pub hwnd: super::super::Foundation::HWND,
    pub message: u32,
    pub wParam: super::super::Foundation::WPARAM,
    pub lParam: super::super::Foundation::LPARAM,
    pub time: u32,
    pub pt: super::super::Foundation::POINT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MSG {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MSG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
pub struct MSGBOXPARAMSA {
    pub cbSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub lpszText: ::windows_sys::core::PCSTR,
    pub lpszCaption: ::windows_sys::core::PCSTR,
    pub dwStyle: MESSAGEBOX_STYLE,
    pub lpszIcon: ::windows_sys::core::PCSTR,
    pub dwContextHelpId: usize,
    pub lpfnMsgBoxCallback: MSGBOXCALLBACK,
    pub dwLanguageId: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
impl ::core::marker::Copy for MSGBOXPARAMSA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
impl ::core::clone::Clone for MSGBOXPARAMSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
pub struct MSGBOXPARAMSW {
    pub cbSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub lpszText: ::windows_sys::core::PCWSTR,
    pub lpszCaption: ::windows_sys::core::PCWSTR,
    pub dwStyle: MESSAGEBOX_STYLE,
    pub lpszIcon: ::windows_sys::core::PCWSTR,
    pub dwContextHelpId: usize,
    pub lpfnMsgBoxCallback: MSGBOXCALLBACK,
    pub dwLanguageId: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
impl ::core::marker::Copy for MSGBOXPARAMSW {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
impl ::core::clone::Clone for MSGBOXPARAMSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MSLLHOOKSTRUCT {
    pub pt: super::super::Foundation::POINT,
    pub mouseData: u32,
    pub flags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MSLLHOOKSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MSLLHOOKSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MrmResourceIndexerHandle {
    pub handle: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for MrmResourceIndexerHandle {}
impl ::core::clone::Clone for MrmResourceIndexerHandle {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MrmResourceIndexerMessage {
    pub severity: MrmResourceIndexerMessageSeverity,
    pub id: u32,
    pub text: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for MrmResourceIndexerMessage {}
impl ::core::clone::Clone for MrmResourceIndexerMessage {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct NCCALCSIZE_PARAMS {
    pub rgrc: [super::super::Foundation::RECT; 3],
    pub lppos: *mut WINDOWPOS,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for NCCALCSIZE_PARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for NCCALCSIZE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct NONCLIENTMETRICSA {
    pub cbSize: u32,
    pub iBorderWidth: i32,
    pub iScrollWidth: i32,
    pub iScrollHeight: i32,
    pub iCaptionWidth: i32,
    pub iCaptionHeight: i32,
    pub lfCaptionFont: super::super::Graphics::Gdi::LOGFONTA,
    pub iSmCaptionWidth: i32,
    pub iSmCaptionHeight: i32,
    pub lfSmCaptionFont: super::super::Graphics::Gdi::LOGFONTA,
    pub iMenuWidth: i32,
    pub iMenuHeight: i32,
    pub lfMenuFont: super::super::Graphics::Gdi::LOGFONTA,
    pub lfStatusFont: super::super::Graphics::Gdi::LOGFONTA,
    pub lfMessageFont: super::super::Graphics::Gdi::LOGFONTA,
    pub iPaddedBorderWidth: i32,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for NONCLIENTMETRICSA {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for NONCLIENTMETRICSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct NONCLIENTMETRICSW {
    pub cbSize: u32,
    pub iBorderWidth: i32,
    pub iScrollWidth: i32,
    pub iScrollHeight: i32,
    pub iCaptionWidth: i32,
    pub iCaptionHeight: i32,
    pub lfCaptionFont: super::super::Graphics::Gdi::LOGFONTW,
    pub iSmCaptionWidth: i32,
    pub iSmCaptionHeight: i32,
    pub lfSmCaptionFont: super::super::Graphics::Gdi::LOGFONTW,
    pub iMenuWidth: i32,
    pub iMenuHeight: i32,
    pub lfMenuFont: super::super::Graphics::Gdi::LOGFONTW,
    pub lfStatusFont: super::super::Graphics::Gdi::LOGFONTW,
    pub lfMessageFont: super::super::Graphics::Gdi::LOGFONTW,
    pub iPaddedBorderWidth: i32,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for NONCLIENTMETRICSW {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for NONCLIENTMETRICSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SCROLLBARINFO {
    pub cbSize: u32,
    pub rcScrollBar: super::super::Foundation::RECT,
    pub dxyLineButton: i32,
    pub xyThumbTop: i32,
    pub xyThumbBottom: i32,
    pub reserved: i32,
    pub rgstate: [u32; 6],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SCROLLBARINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SCROLLBARINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCROLLINFO {
    pub cbSize: u32,
    pub fMask: SCROLLINFO_MASK,
    pub nMin: i32,
    pub nMax: i32,
    pub nPage: u32,
    pub nPos: i32,
    pub nTrackPos: i32,
}
impl ::core::marker::Copy for SCROLLINFO {}
impl ::core::clone::Clone for SCROLLINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SHELLHOOKINFO {
    pub hwnd: super::super::Foundation::HWND,
    pub rc: super::super::Foundation::RECT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SHELLHOOKINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SHELLHOOKINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct STYLESTRUCT {
    pub styleOld: u32,
    pub styleNew: u32,
}
impl ::core::marker::Copy for STYLESTRUCT {}
impl ::core::clone::Clone for STYLESTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TITLEBARINFO {
    pub cbSize: u32,
    pub rcTitleBar: super::super::Foundation::RECT,
    pub rgstate: [u32; 6],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TITLEBARINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TITLEBARINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TITLEBARINFOEX {
    pub cbSize: u32,
    pub rcTitleBar: super::super::Foundation::RECT,
    pub rgstate: [u32; 6],
    pub rgrect: [super::super::Foundation::RECT; 6],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TITLEBARINFOEX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TITLEBARINFOEX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TOUCHPREDICTIONPARAMETERS {
    pub cbSize: u32,
    pub dwLatency: u32,
    pub dwSampleTime: u32,
    pub bUseHWTimeStamp: u32,
}
impl ::core::marker::Copy for TOUCHPREDICTIONPARAMETERS {}
impl ::core::clone::Clone for TOUCHPREDICTIONPARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TPMPARAMS {
    pub cbSize: u32,
    pub rcExclude: super::super::Foundation::RECT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TPMPARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TPMPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct UPDATELAYEREDWINDOWINFO {
    pub cbSize: u32,
    pub hdcDst: super::super::Graphics::Gdi::HDC,
    pub pptDst: *const super::super::Foundation::POINT,
    pub psize: *const super::super::Foundation::SIZE,
    pub hdcSrc: super::super::Graphics::Gdi::HDC,
    pub pptSrc: *const super::super::Foundation::POINT,
    pub crKey: super::super::Foundation::COLORREF,
    pub pblend: *const super::super::Graphics::Gdi::BLENDFUNCTION,
    pub dwFlags: UPDATE_LAYERED_WINDOW_FLAGS,
    pub prcDirty: *const super::super::Foundation::RECT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for UPDATELAYEREDWINDOWINFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for UPDATELAYEREDWINDOWINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VolLockBroadcast {
    pub vlb_dbh: DEV_BROADCAST_HDR,
    pub vlb_owner: u32,
    pub vlb_perms: u8,
    pub vlb_lockType: u8,
    pub vlb_drive: u8,
    pub vlb_flags: u8,
}
impl ::core::marker::Copy for VolLockBroadcast {}
impl ::core::clone::Clone for VolLockBroadcast {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WINDOWINFO {
    pub cbSize: u32,
    pub rcWindow: super::super::Foundation::RECT,
    pub rcClient: super::super::Foundation::RECT,
    pub dwStyle: WINDOW_STYLE,
    pub dwExStyle: WINDOW_EX_STYLE,
    pub dwWindowStatus: u32,
    pub cxWindowBorders: u32,
    pub cyWindowBorders: u32,
    pub atomWindowType: u16,
    pub wCreatorVersion: u16,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WINDOWINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WINDOWINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WINDOWPLACEMENT {
    pub length: u32,
    pub flags: WINDOWPLACEMENT_FLAGS,
    pub showCmd: u32,
    pub ptMinPosition: super::super::Foundation::POINT,
    pub ptMaxPosition: super::super::Foundation::POINT,
    pub rcNormalPosition: super::super::Foundation::RECT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WINDOWPLACEMENT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WINDOWPLACEMENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WINDOWPOS {
    pub hwnd: super::super::Foundation::HWND,
    pub hwndInsertAfter: super::super::Foundation::HWND,
    pub x: i32,
    pub y: i32,
    pub cx: i32,
    pub cy: i32,
    pub flags: SET_WINDOW_POS_FLAGS,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WINDOWPOS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WINDOWPOS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct WNDCLASSA {
    pub style: WNDCLASS_STYLES,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: super::super::Graphics::Gdi::HBRUSH,
    pub lpszMenuName: ::windows_sys::core::PCSTR,
    pub lpszClassName: ::windows_sys::core::PCSTR,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for WNDCLASSA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for WNDCLASSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct WNDCLASSEXA {
    pub cbSize: u32,
    pub style: WNDCLASS_STYLES,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: super::super::Graphics::Gdi::HBRUSH,
    pub lpszMenuName: ::windows_sys::core::PCSTR,
    pub lpszClassName: ::windows_sys::core::PCSTR,
    pub hIconSm: HICON,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for WNDCLASSEXA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for WNDCLASSEXA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: WNDCLASS_STYLES,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: super::super::Graphics::Gdi::HBRUSH,
    pub lpszMenuName: ::windows_sys::core::PCWSTR,
    pub lpszClassName: ::windows_sys::core::PCWSTR,
    pub hIconSm: HICON,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for WNDCLASSEXW {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for WNDCLASSEXW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct WNDCLASSW {
    pub style: WNDCLASS_STYLES,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: super::super::Graphics::Gdi::HBRUSH,
    pub lpszMenuName: ::windows_sys::core::PCWSTR,
    pub lpszClassName: ::windows_sys::core::PCWSTR,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for WNDCLASSW {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for WNDCLASSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct _DEV_BROADCAST_HEADER {
    pub dbcd_size: u32,
    pub dbcd_devicetype: u32,
    pub dbcd_reserved: u32,
}
impl ::core::marker::Copy for _DEV_BROADCAST_HEADER {}
impl ::core::clone::Clone for _DEV_BROADCAST_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct _DEV_BROADCAST_USERDEFINED {
    pub dbud_dbh: DEV_BROADCAST_HDR,
    pub dbud_szName: [u8; 1],
}
impl ::core::marker::Copy for _DEV_BROADCAST_USERDEFINED {}
impl ::core::clone::Clone for _DEV_BROADCAST_USERDEFINED {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DLGPROC = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: u32, param2: super::super::Foundation::WPARAM, param3: super::super::Foundation::LPARAM) -> isize>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type HOOKPROC = ::core::option::Option<unsafe extern "system" fn(code: i32, wparam: super::super::Foundation::WPARAM, lparam: super::super::Foundation::LPARAM) -> super::super::Foundation::LRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
pub type MSGBOXCALLBACK = ::core::option::Option<unsafe extern "system" fn(lphelpinfo: *mut super::Shell::HELPINFO) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type NAMEENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: super::super::Foundation::LPARAM) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type NAMEENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: super::super::Foundation::LPARAM) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PREGISTERCLASSNAMEW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> super::super::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PROPENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: ::windows_sys::core::PCSTR, param2: super::super::Foundation::HANDLE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PROPENUMPROCEXA = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: ::windows_sys::core::PCSTR, param2: super::super::Foundation::HANDLE, param3: usize) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PROPENUMPROCEXW = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: ::windows_sys::core::PCWSTR, param2: super::super::Foundation::HANDLE, param3: usize) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PROPENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: ::windows_sys::core::PCWSTR, param2: super::super::Foundation::HANDLE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type SENDASYNCPROC = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: u32, param2: usize, param3: super::super::Foundation::LRESULT) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type TIMERPROC = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: u32, param2: usize, param3: u32) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type WNDENUMPROC = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: super::super::Foundation::LPARAM) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type WNDPROC = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: u32, param2: super::super::Foundation::WPARAM, param3: super::super::Foundation::LPARAM) -> super::super::Foundation::LRESULT>;
#[cfg(target_pointer_width = "32")]
#[cfg(feature = "Win32_Foundation")]
pub use GetWindowLongA as GetWindowLongPtrA;
#[cfg(target_pointer_width = "32")]
#[cfg(feature = "Win32_Foundation")]
pub use GetWindowLongW as GetWindowLongPtrW;
#[cfg(target_pointer_width = "32")]
#[cfg(feature = "Win32_Foundation")]
pub use SetWindowLongA as SetWindowLongPtrA;
#[cfg(target_pointer_width = "32")]
#[cfg(feature = "Win32_Foundation")]
pub use SetWindowLongW as SetWindowLongPtrW;
