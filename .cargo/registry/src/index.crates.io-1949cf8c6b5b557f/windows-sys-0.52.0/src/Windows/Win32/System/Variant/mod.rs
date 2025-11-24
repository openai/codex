#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn ClearVariantArray(pvars : *mut VARIANT, cvars : u32) -> ());
::windows_targets::link!("oleaut32.dll" "system" fn DosDateTimeToVariantTime(wdosdate : u16, wdostime : u16, pvtime : *mut f64) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromBooleanArray(prgf : *const super::super::Foundation:: BOOL, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromBuffer(pv : *const ::core::ffi::c_void, cb : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromDoubleArray(prgn : *const f64, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromFileTime(pft : *const super::super::Foundation:: FILETIME, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromFileTimeArray(prgft : *const super::super::Foundation:: FILETIME, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromGUIDAsString(guid : *const ::windows_sys::core::GUID, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromInt16Array(prgn : *const i16, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromInt32Array(prgn : *const i32, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromInt64Array(prgn : *const i64, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromResource(hinst : super::super::Foundation:: HINSTANCE, id : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromStringArray(prgsz : *const ::windows_sys::core::PCWSTR, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromUInt16Array(prgn : *const u16, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromUInt32Array(prgn : *const u32, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromUInt64Array(prgn : *const u64, celems : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn InitVariantFromVariantArrayElem(varin : *const VARIANT, ielem : u32, pvar : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SystemTimeToVariantTime(lpsystemtime : *const super::super::Foundation:: SYSTEMTIME, pvtime : *mut f64) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VARIANT_UserFree(param0 : *const u32, param1 : *const VARIANT) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VARIANT_UserFree64(param0 : *const u32, param1 : *const VARIANT) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VARIANT_UserMarshal(param0 : *const u32, param1 : *mut u8, param2 : *const VARIANT) -> *mut u8);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VARIANT_UserMarshal64(param0 : *const u32, param1 : *mut u8, param2 : *const VARIANT) -> *mut u8);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VARIANT_UserSize(param0 : *const u32, param1 : u32, param2 : *const VARIANT) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VARIANT_UserSize64(param0 : *const u32, param1 : u32, param2 : *const VARIANT) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VARIANT_UserUnmarshal(param0 : *const u32, param1 : *const u8, param2 : *mut VARIANT) -> *mut u8);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VARIANT_UserUnmarshal64(param0 : *const u32, param1 : *const u8, param2 : *mut VARIANT) -> *mut u8);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantChangeType(pvargdest : *mut VARIANT, pvarsrc : *const VARIANT, wflags : VAR_CHANGE_FLAGS, vt : VARENUM) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantChangeTypeEx(pvargdest : *mut VARIANT, pvarsrc : *const VARIANT, lcid : u32, wflags : VAR_CHANGE_FLAGS, vt : VARENUM) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantClear(pvarg : *mut VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantCompare(var1 : *const VARIANT, var2 : *const VARIANT) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantCopy(pvargdest : *mut VARIANT, pvargsrc : *const VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantCopyInd(pvardest : *mut VARIANT, pvargsrc : *const VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetBooleanElem(var : *const VARIANT, ielem : u32, pfval : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetDoubleElem(var : *const VARIANT, ielem : u32, pnval : *mut f64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetElementCount(varin : *const VARIANT) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetInt16Elem(var : *const VARIANT, ielem : u32, pnval : *mut i16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetInt32Elem(var : *const VARIANT, ielem : u32, pnval : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetInt64Elem(var : *const VARIANT, ielem : u32, pnval : *mut i64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetStringElem(var : *const VARIANT, ielem : u32, ppszval : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetUInt16Elem(var : *const VARIANT, ielem : u32, pnval : *mut u16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetUInt32Elem(var : *const VARIANT, ielem : u32, pnval : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantGetUInt64Elem(var : *const VARIANT, ielem : u32, pnval : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantInit(pvarg : *mut VARIANT) -> ());
::windows_targets::link!("oleaut32.dll" "system" fn VariantTimeToDosDateTime(vtime : f64, pwdosdate : *mut u16, pwdostime : *mut u16) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleaut32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn VariantTimeToSystemTime(vtime : f64, lpsystemtime : *mut super::super::Foundation:: SYSTEMTIME) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToBoolean(varin : *const VARIANT, pfret : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToBooleanArray(var : *const VARIANT, prgf : *mut super::super::Foundation:: BOOL, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToBooleanArrayAlloc(var : *const VARIANT, pprgf : *mut *mut super::super::Foundation:: BOOL, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToBooleanWithDefault(varin : *const VARIANT, fdefault : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToBuffer(varin : *const VARIANT, pv : *mut ::core::ffi::c_void, cb : u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToDosDateTime(varin : *const VARIANT, pwdate : *mut u16, pwtime : *mut u16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToDouble(varin : *const VARIANT, pdblret : *mut f64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToDoubleArray(var : *const VARIANT, prgn : *mut f64, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToDoubleArrayAlloc(var : *const VARIANT, pprgn : *mut *mut f64, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToDoubleWithDefault(varin : *const VARIANT, dbldefault : f64) -> f64);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToFileTime(varin : *const VARIANT, stfout : PSTIME_FLAGS, pftout : *mut super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToGUID(varin : *const VARIANT, pguid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt16(varin : *const VARIANT, piret : *mut i16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt16Array(var : *const VARIANT, prgn : *mut i16, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt16ArrayAlloc(var : *const VARIANT, pprgn : *mut *mut i16, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt16WithDefault(varin : *const VARIANT, idefault : i16) -> i16);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt32(varin : *const VARIANT, plret : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt32Array(var : *const VARIANT, prgn : *mut i32, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt32ArrayAlloc(var : *const VARIANT, pprgn : *mut *mut i32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt32WithDefault(varin : *const VARIANT, ldefault : i32) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt64(varin : *const VARIANT, pllret : *mut i64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt64Array(var : *const VARIANT, prgn : *mut i64, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt64ArrayAlloc(var : *const VARIANT, pprgn : *mut *mut i64, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToInt64WithDefault(varin : *const VARIANT, lldefault : i64) -> i64);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToString(varin : *const VARIANT, pszbuf : ::windows_sys::core::PWSTR, cchbuf : u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToStringAlloc(varin : *const VARIANT, ppszbuf : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToStringArray(var : *const VARIANT, prgsz : *mut ::windows_sys::core::PWSTR, crgsz : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToStringArrayAlloc(var : *const VARIANT, pprgsz : *mut *mut ::windows_sys::core::PWSTR, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToStringWithDefault(varin : *const VARIANT, pszdefault : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PCWSTR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt16(varin : *const VARIANT, puiret : *mut u16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt16Array(var : *const VARIANT, prgn : *mut u16, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt16ArrayAlloc(var : *const VARIANT, pprgn : *mut *mut u16, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt16WithDefault(varin : *const VARIANT, uidefault : u16) -> u16);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt32(varin : *const VARIANT, pulret : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt32Array(var : *const VARIANT, prgn : *mut u32, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt32ArrayAlloc(var : *const VARIANT, pprgn : *mut *mut u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt32WithDefault(varin : *const VARIANT, uldefault : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt64(varin : *const VARIANT, pullret : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt64Array(var : *const VARIANT, prgn : *mut u64, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt64ArrayAlloc(var : *const VARIANT, pprgn : *mut *mut u64, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn VariantToUInt64WithDefault(varin : *const VARIANT, ulldefault : u64) -> u64);
pub const DPF_ERROR: DRAWPROGRESSFLAGS = 4i32;
pub const DPF_MARQUEE: DRAWPROGRESSFLAGS = 1i32;
pub const DPF_MARQUEE_COMPLETE: DRAWPROGRESSFLAGS = 2i32;
pub const DPF_NONE: DRAWPROGRESSFLAGS = 0i32;
pub const DPF_STOPPED: DRAWPROGRESSFLAGS = 16i32;
pub const DPF_WARNING: DRAWPROGRESSFLAGS = 8i32;
pub const PSTF_LOCAL: PSTIME_FLAGS = 1i32;
pub const PSTF_UTC: PSTIME_FLAGS = 0i32;
pub const VARIANT_ALPHABOOL: VAR_CHANGE_FLAGS = 2u16;
pub const VARIANT_CALENDAR_GREGORIAN: VAR_CHANGE_FLAGS = 64u16;
pub const VARIANT_CALENDAR_HIJRI: VAR_CHANGE_FLAGS = 8u16;
pub const VARIANT_CALENDAR_THAI: VAR_CHANGE_FLAGS = 32u16;
pub const VARIANT_LOCALBOOL: VAR_CHANGE_FLAGS = 16u16;
pub const VARIANT_NOUSEROVERRIDE: VAR_CHANGE_FLAGS = 4u16;
pub const VARIANT_NOVALUEPROP: VAR_CHANGE_FLAGS = 1u16;
pub const VARIANT_USE_NLS: VAR_CHANGE_FLAGS = 128u16;
pub const VT_ARRAY: VARENUM = 8192u16;
pub const VT_BLOB: VARENUM = 65u16;
pub const VT_BLOB_OBJECT: VARENUM = 70u16;
pub const VT_BOOL: VARENUM = 11u16;
pub const VT_BSTR: VARENUM = 8u16;
pub const VT_BSTR_BLOB: VARENUM = 4095u16;
pub const VT_BYREF: VARENUM = 16384u16;
pub const VT_CARRAY: VARENUM = 28u16;
pub const VT_CF: VARENUM = 71u16;
pub const VT_CLSID: VARENUM = 72u16;
pub const VT_CY: VARENUM = 6u16;
pub const VT_DATE: VARENUM = 7u16;
pub const VT_DECIMAL: VARENUM = 14u16;
pub const VT_DISPATCH: VARENUM = 9u16;
pub const VT_EMPTY: VARENUM = 0u16;
pub const VT_ERROR: VARENUM = 10u16;
pub const VT_FILETIME: VARENUM = 64u16;
pub const VT_HRESULT: VARENUM = 25u16;
pub const VT_I1: VARENUM = 16u16;
pub const VT_I2: VARENUM = 2u16;
pub const VT_I4: VARENUM = 3u16;
pub const VT_I8: VARENUM = 20u16;
pub const VT_ILLEGAL: VARENUM = 65535u16;
pub const VT_ILLEGALMASKED: VARENUM = 4095u16;
pub const VT_INT: VARENUM = 22u16;
pub const VT_INT_PTR: VARENUM = 37u16;
pub const VT_LPSTR: VARENUM = 30u16;
pub const VT_LPWSTR: VARENUM = 31u16;
pub const VT_NULL: VARENUM = 1u16;
pub const VT_PTR: VARENUM = 26u16;
pub const VT_R4: VARENUM = 4u16;
pub const VT_R8: VARENUM = 5u16;
pub const VT_RECORD: VARENUM = 36u16;
pub const VT_RESERVED: VARENUM = 32768u16;
pub const VT_SAFEARRAY: VARENUM = 27u16;
pub const VT_STORAGE: VARENUM = 67u16;
pub const VT_STORED_OBJECT: VARENUM = 69u16;
pub const VT_STREAM: VARENUM = 66u16;
pub const VT_STREAMED_OBJECT: VARENUM = 68u16;
pub const VT_TYPEMASK: VARENUM = 4095u16;
pub const VT_UI1: VARENUM = 17u16;
pub const VT_UI2: VARENUM = 18u16;
pub const VT_UI4: VARENUM = 19u16;
pub const VT_UI8: VARENUM = 21u16;
pub const VT_UINT: VARENUM = 23u16;
pub const VT_UINT_PTR: VARENUM = 38u16;
pub const VT_UNKNOWN: VARENUM = 13u16;
pub const VT_USERDEFINED: VARENUM = 29u16;
pub const VT_VARIANT: VARENUM = 12u16;
pub const VT_VECTOR: VARENUM = 4096u16;
pub const VT_VERSIONED_STREAM: VARENUM = 73u16;
pub const VT_VOID: VARENUM = 24u16;
pub type DRAWPROGRESSFLAGS = i32;
pub type PSTIME_FLAGS = i32;
pub type VARENUM = u16;
pub type VAR_CHANGE_FLAGS = u16;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
pub struct VARIANT {
    pub Anonymous: VARIANT_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::marker::Copy for VARIANT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::clone::Clone for VARIANT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
pub union VARIANT_0 {
    pub Anonymous: VARIANT_0_0,
    pub decVal: super::super::Foundation::DECIMAL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::marker::Copy for VARIANT_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::clone::Clone for VARIANT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
pub struct VARIANT_0_0 {
    pub vt: VARENUM,
    pub wReserved1: u16,
    pub wReserved2: u16,
    pub wReserved3: u16,
    pub Anonymous: VARIANT_0_0_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::marker::Copy for VARIANT_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::clone::Clone for VARIANT_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
pub union VARIANT_0_0_0 {
    pub llVal: i64,
    pub lVal: i32,
    pub bVal: u8,
    pub iVal: i16,
    pub fltVal: f32,
    pub dblVal: f64,
    pub boolVal: super::super::Foundation::VARIANT_BOOL,
    pub __OBSOLETE__VARIANT_BOOL: super::super::Foundation::VARIANT_BOOL,
    pub scode: i32,
    pub cyVal: super::Com::CY,
    pub date: f64,
    pub bstrVal: ::windows_sys::core::BSTR,
    pub punkVal: ::windows_sys::core::IUnknown,
    pub pdispVal: super::Com::IDispatch,
    pub parray: *mut super::Com::SAFEARRAY,
    pub pbVal: *mut u8,
    pub piVal: *mut i16,
    pub plVal: *mut i32,
    pub pllVal: *mut i64,
    pub pfltVal: *mut f32,
    pub pdblVal: *mut f64,
    pub pboolVal: *mut super::super::Foundation::VARIANT_BOOL,
    pub __OBSOLETE__VARIANT_PBOOL: *mut super::super::Foundation::VARIANT_BOOL,
    pub pscode: *mut i32,
    pub pcyVal: *mut super::Com::CY,
    pub pdate: *mut f64,
    pub pbstrVal: *mut ::windows_sys::core::BSTR,
    pub ppunkVal: *mut ::windows_sys::core::IUnknown,
    pub ppdispVal: *mut super::Com::IDispatch,
    pub pparray: *mut *mut super::Com::SAFEARRAY,
    pub pvarVal: *mut VARIANT,
    pub byref: *mut ::core::ffi::c_void,
    pub cVal: u8,
    pub uiVal: u16,
    pub ulVal: u32,
    pub ullVal: u64,
    pub intVal: i32,
    pub uintVal: u32,
    pub pdecVal: *mut super::super::Foundation::DECIMAL,
    pub pcVal: ::windows_sys::core::PSTR,
    pub puiVal: *mut u16,
    pub pulVal: *mut u32,
    pub pullVal: *mut u64,
    pub pintVal: *mut i32,
    pub puintVal: *mut u32,
    pub Anonymous: VARIANT_0_0_0_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::marker::Copy for VARIANT_0_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::clone::Clone for VARIANT_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
pub struct VARIANT_0_0_0_0 {
    pub pvRecord: *mut ::core::ffi::c_void,
    pub pRecInfo: super::Ole::IRecordInfo,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::marker::Copy for VARIANT_0_0_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
impl ::core::clone::Clone for VARIANT_0_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
