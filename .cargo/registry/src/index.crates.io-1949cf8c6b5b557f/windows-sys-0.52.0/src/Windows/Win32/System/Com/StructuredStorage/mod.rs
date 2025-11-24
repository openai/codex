#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn ClearPropVariantArray(rgpropvar : *mut PROPVARIANT, cvars : u32) -> ());
::windows_targets::link!("ole32.dll" "system" fn CoGetInstanceFromFile(pserverinfo : *const super:: COSERVERINFO, pclsid : *const ::windows_sys::core::GUID, punkouter : ::windows_sys::core::IUnknown, dwclsctx : super:: CLSCTX, grfmode : u32, pwszname : ::windows_sys::core::PCWSTR, dwcount : u32, presults : *mut super:: MULTI_QI) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoGetInstanceFromIStorage(pserverinfo : *const super:: COSERVERINFO, pclsid : *const ::windows_sys::core::GUID, punkouter : ::windows_sys::core::IUnknown, dwclsctx : super:: CLSCTX, pstg : IStorage, dwcount : u32, presults : *mut super:: MULTI_QI) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn CoGetInterfaceAndReleaseStream(pstm : super:: IStream, iid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateILockBytesOnHGlobal(hglobal : super::super::super::Foundation:: HGLOBAL, fdeleteonrelease : super::super::super::Foundation:: BOOL, pplkbyt : *mut ILockBytes) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateStreamOnHGlobal(hglobal : super::super::super::Foundation:: HGLOBAL, fdeleteonrelease : super::super::super::Foundation:: BOOL, ppstm : *mut super:: IStream) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn FmtIdToPropStgName(pfmtid : *const ::windows_sys::core::GUID, oszname : ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn FreePropVariantArray(cvariants : u32, rgvars : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn GetConvertStg(pstg : IStorage) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetHGlobalFromILockBytes(plkbyt : ILockBytes, phglobal : *mut super::super::super::Foundation:: HGLOBAL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetHGlobalFromStream(pstm : super:: IStream, phglobal : *mut super::super::super::Foundation:: HGLOBAL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromBooleanVector(prgf : *const super::super::super::Foundation:: BOOL, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromBuffer(pv : *const ::core::ffi::c_void, cb : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromCLSID(clsid : *const ::windows_sys::core::GUID, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromDoubleVector(prgn : *const f64, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromFileTime(pftin : *const super::super::super::Foundation:: FILETIME, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromFileTimeVector(prgft : *const super::super::super::Foundation:: FILETIME, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromGUIDAsString(guid : *const ::windows_sys::core::GUID, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromInt16Vector(prgn : *const i16, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromInt32Vector(prgn : *const i32, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromInt64Vector(prgn : *const i64, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromPropVariantVectorElem(propvarin : *const PROPVARIANT, ielem : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromResource(hinst : super::super::super::Foundation:: HINSTANCE, id : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromStringAsVector(psz : ::windows_sys::core::PCWSTR, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromStringVector(prgsz : *const ::windows_sys::core::PCWSTR, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromUInt16Vector(prgn : *const u16, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromUInt32Vector(prgn : *const u32, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromUInt64Vector(prgn : *const u64, celems : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantVectorFromPropVariant(propvarsingle : *const PROPVARIANT, ppropvarvector : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn OleConvertIStorageToOLESTREAM(pstg : IStorage, lpolestream : *mut OLESTREAM) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn OleConvertIStorageToOLESTREAMEx(pstg : IStorage, cfformat : u16, lwidth : i32, lheight : i32, dwsize : u32, pmedium : *const super:: STGMEDIUM, polestm : *mut OLESTREAM) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn OleConvertOLESTREAMToIStorage(lpolestream : *const OLESTREAM, pstg : IStorage, ptd : *const super:: DVTARGETDEVICE) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn OleConvertOLESTREAMToIStorageEx(polestm : *const OLESTREAM, pstg : IStorage, pcfformat : *mut u16, plwwidth : *mut i32, plheight : *mut i32, pdwsize : *mut u32, pmedium : *mut super:: STGMEDIUM) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn PropStgNameToFmtId(oszname : ::windows_sys::core::PCWSTR, pfmtid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantChangeType(ppropvardest : *mut PROPVARIANT, propvarsrc : *const PROPVARIANT, flags : PROPVAR_CHANGE_FLAGS, vt : super::super::Variant:: VARENUM) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantClear(pvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantCompareEx(propvar1 : *const PROPVARIANT, propvar2 : *const PROPVARIANT, unit : PROPVAR_COMPARE_UNIT, flags : PROPVAR_COMPARE_FLAGS) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantCopy(pvardest : *mut PROPVARIANT, pvarsrc : *const PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetBooleanElem(propvar : *const PROPVARIANT, ielem : u32, pfval : *mut super::super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetDoubleElem(propvar : *const PROPVARIANT, ielem : u32, pnval : *mut f64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetElementCount(propvar : *const PROPVARIANT) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetFileTimeElem(propvar : *const PROPVARIANT, ielem : u32, pftval : *mut super::super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetInt16Elem(propvar : *const PROPVARIANT, ielem : u32, pnval : *mut i16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetInt32Elem(propvar : *const PROPVARIANT, ielem : u32, pnval : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetInt64Elem(propvar : *const PROPVARIANT, ielem : u32, pnval : *mut i64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetStringElem(propvar : *const PROPVARIANT, ielem : u32, ppszval : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetUInt16Elem(propvar : *const PROPVARIANT, ielem : u32, pnval : *mut u16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetUInt32Elem(propvar : *const PROPVARIANT, ielem : u32, pnval : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetUInt64Elem(propvar : *const PROPVARIANT, ielem : u32, pnval : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToBSTR(propvar : *const PROPVARIANT, pbstrout : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToBoolean(propvarin : *const PROPVARIANT, pfret : *mut super::super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToBooleanVector(propvar : *const PROPVARIANT, prgf : *mut super::super::super::Foundation:: BOOL, crgf : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToBooleanVectorAlloc(propvar : *const PROPVARIANT, pprgf : *mut *mut super::super::super::Foundation:: BOOL, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToBooleanWithDefault(propvarin : *const PROPVARIANT, fdefault : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToBuffer(propvar : *const PROPVARIANT, pv : *mut ::core::ffi::c_void, cb : u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToDouble(propvarin : *const PROPVARIANT, pdblret : *mut f64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToDoubleVector(propvar : *const PROPVARIANT, prgn : *mut f64, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToDoubleVectorAlloc(propvar : *const PROPVARIANT, pprgn : *mut *mut f64, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToDoubleWithDefault(propvarin : *const PROPVARIANT, dbldefault : f64) -> f64);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToFileTime(propvar : *const PROPVARIANT, pstfout : super::super::Variant:: PSTIME_FLAGS, pftout : *mut super::super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToFileTimeVector(propvar : *const PROPVARIANT, prgft : *mut super::super::super::Foundation:: FILETIME, crgft : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToFileTimeVectorAlloc(propvar : *const PROPVARIANT, pprgft : *mut *mut super::super::super::Foundation:: FILETIME, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToGUID(propvar : *const PROPVARIANT, pguid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt16(propvarin : *const PROPVARIANT, piret : *mut i16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt16Vector(propvar : *const PROPVARIANT, prgn : *mut i16, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt16VectorAlloc(propvar : *const PROPVARIANT, pprgn : *mut *mut i16, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt16WithDefault(propvarin : *const PROPVARIANT, idefault : i16) -> i16);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt32(propvarin : *const PROPVARIANT, plret : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt32Vector(propvar : *const PROPVARIANT, prgn : *mut i32, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt32VectorAlloc(propvar : *const PROPVARIANT, pprgn : *mut *mut i32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt32WithDefault(propvarin : *const PROPVARIANT, ldefault : i32) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt64(propvarin : *const PROPVARIANT, pllret : *mut i64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt64Vector(propvar : *const PROPVARIANT, prgn : *mut i64, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt64VectorAlloc(propvar : *const PROPVARIANT, pprgn : *mut *mut i64, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToInt64WithDefault(propvarin : *const PROPVARIANT, lldefault : i64) -> i64);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToString(propvar : *const PROPVARIANT, psz : ::windows_sys::core::PWSTR, cch : u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToStringAlloc(propvar : *const PROPVARIANT, ppszout : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToStringVector(propvar : *const PROPVARIANT, prgsz : *mut ::windows_sys::core::PWSTR, crgsz : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToStringVectorAlloc(propvar : *const PROPVARIANT, pprgsz : *mut *mut ::windows_sys::core::PWSTR, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToStringWithDefault(propvarin : *const PROPVARIANT, pszdefault : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PCWSTR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt16(propvarin : *const PROPVARIANT, puiret : *mut u16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt16Vector(propvar : *const PROPVARIANT, prgn : *mut u16, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt16VectorAlloc(propvar : *const PROPVARIANT, pprgn : *mut *mut u16, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt16WithDefault(propvarin : *const PROPVARIANT, uidefault : u16) -> u16);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt32(propvarin : *const PROPVARIANT, pulret : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt32Vector(propvar : *const PROPVARIANT, prgn : *mut u32, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt32VectorAlloc(propvar : *const PROPVARIANT, pprgn : *mut *mut u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt32WithDefault(propvarin : *const PROPVARIANT, uldefault : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt64(propvarin : *const PROPVARIANT, pullret : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt64Vector(propvar : *const PROPVARIANT, prgn : *mut u64, crgn : u32, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt64VectorAlloc(propvar : *const PROPVARIANT, pprgn : *mut *mut u64, pcelem : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToUInt64WithDefault(propvarin : *const PROPVARIANT, ulldefault : u64) -> u64);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn PropVariantToVariant(ppropvar : *const PROPVARIANT, pvar : *mut super::super::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn PropVariantToWinRTPropertyValue(propvar : *const PROPVARIANT, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn ReadClassStg(pstg : IStorage, pclsid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn ReadClassStm(pstm : super:: IStream, pclsid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn ReadFmtUserTypeStg(pstg : IStorage, pcf : *mut u16, lplpszusertype : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetConvertStg(pstg : IStorage, fconvert : super::super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn StgConvertVariantToProperty(pvar : *const PROPVARIANT, codepage : u16, pprop : *mut SERIALIZEDPROPERTYVALUE, pcb : *mut u32, pid : u32, freserved : super::super::super::Foundation:: BOOLEAN, pcindirect : *mut u32) -> *mut SERIALIZEDPROPERTYVALUE);
::windows_targets::link!("ole32.dll" "system" fn StgCreateDocfile(pwcsname : ::windows_sys::core::PCWSTR, grfmode : super:: STGM, reserved : u32, ppstgopen : *mut IStorage) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgCreateDocfileOnILockBytes(plkbyt : ILockBytes, grfmode : super:: STGM, reserved : u32, ppstgopen : *mut IStorage) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgCreatePropSetStg(pstorage : IStorage, dwreserved : u32, pppropsetstg : *mut IPropertySetStorage) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgCreatePropStg(punk : ::windows_sys::core::IUnknown, fmtid : *const ::windows_sys::core::GUID, pclsid : *const ::windows_sys::core::GUID, grfflags : u32, dwreserved : u32, pppropstg : *mut IPropertyStorage) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn StgCreateStorageEx(pwcsname : ::windows_sys::core::PCWSTR, grfmode : super:: STGM, stgfmt : STGFMT, grfattrs : u32, pstgoptions : *mut STGOPTIONS, psecuritydescriptor : super::super::super::Security:: PSECURITY_DESCRIPTOR, riid : *const ::windows_sys::core::GUID, ppobjectopen : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn StgDeserializePropVariant(pprop : *const SERIALIZEDPROPERTYVALUE, cbmax : u32, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgGetIFillLockBytesOnFile(pwcsname : ::windows_sys::core::PCWSTR, ppflb : *mut IFillLockBytes) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgGetIFillLockBytesOnILockBytes(pilb : ILockBytes, ppflb : *mut IFillLockBytes) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgIsStorageFile(pwcsname : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgIsStorageILockBytes(plkbyt : ILockBytes) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgOpenAsyncDocfileOnIFillLockBytes(pflb : IFillLockBytes, grfmode : u32, asyncflags : u32, ppstgopen : *mut IStorage) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("dflayout.dll" "system" fn StgOpenLayoutDocfile(pwcsdfname : ::windows_sys::core::PCWSTR, grfmode : u32, reserved : u32, ppstgopen : *mut IStorage) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgOpenPropStg(punk : ::windows_sys::core::IUnknown, fmtid : *const ::windows_sys::core::GUID, grfflags : u32, dwreserved : u32, pppropstg : *mut IPropertyStorage) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgOpenStorage(pwcsname : ::windows_sys::core::PCWSTR, pstgpriority : IStorage, grfmode : super:: STGM, snbexclude : *const *const u16, reserved : u32, ppstgopen : *mut IStorage) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn StgOpenStorageEx(pwcsname : ::windows_sys::core::PCWSTR, grfmode : super:: STGM, stgfmt : STGFMT, grfattrs : u32, pstgoptions : *mut STGOPTIONS, psecuritydescriptor : super::super::super::Security:: PSECURITY_DESCRIPTOR, riid : *const ::windows_sys::core::GUID, ppobjectopen : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgOpenStorageOnILockBytes(plkbyt : ILockBytes, pstgpriority : IStorage, grfmode : super:: STGM, snbexclude : *const *const u16, reserved : u32, ppstgopen : *mut IStorage) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn StgPropertyLengthAsVariant(pprop : *const SERIALIZEDPROPERTYVALUE, cbprop : u32, codepage : u16, breserved : u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn StgSerializePropVariant(ppropvar : *const PROPVARIANT, ppprop : *mut *mut SERIALIZEDPROPERTYVALUE, pcb : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn StgSetTimes(lpszname : ::windows_sys::core::PCWSTR, pctime : *const super::super::super::Foundation:: FILETIME, patime : *const super::super::super::Foundation:: FILETIME, pmtime : *const super::super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn VariantToPropVariant(pvar : *const super::super::Variant:: VARIANT, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"] fn WinRTPropertyValueToPropVariant(punkpropertyvalue : ::windows_sys::core::IUnknown, ppropvar : *mut PROPVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn WriteClassStg(pstg : IStorage, rclsid : *const ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn WriteClassStm(pstm : super:: IStream, rclsid : *const ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ole32.dll" "system" fn WriteFmtUserTypeStg(pstg : IStorage, cf : u16, lpszusertype : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
pub type IDirectWriterLock = *mut ::core::ffi::c_void;
pub type IEnumSTATPROPSETSTG = *mut ::core::ffi::c_void;
pub type IEnumSTATPROPSTG = *mut ::core::ffi::c_void;
pub type IEnumSTATSTG = *mut ::core::ffi::c_void;
pub type IFillLockBytes = *mut ::core::ffi::c_void;
pub type ILayoutStorage = *mut ::core::ffi::c_void;
pub type ILockBytes = *mut ::core::ffi::c_void;
pub type IPersistStorage = *mut ::core::ffi::c_void;
pub type IPropertyBag = *mut ::core::ffi::c_void;
pub type IPropertyBag2 = *mut ::core::ffi::c_void;
pub type IPropertySetStorage = *mut ::core::ffi::c_void;
pub type IPropertyStorage = *mut ::core::ffi::c_void;
pub type IRootStorage = *mut ::core::ffi::c_void;
pub type IStorage = *mut ::core::ffi::c_void;
pub const CCH_MAX_PROPSTG_NAME: u32 = 31u32;
pub const CWCSTORAGENAME: u32 = 32u32;
pub const PIDDI_THUMBNAIL: i32 = 2i32;
pub const PIDDSI_BYTECOUNT: u32 = 4u32;
pub const PIDDSI_CATEGORY: u32 = 2u32;
pub const PIDDSI_COMPANY: u32 = 15u32;
pub const PIDDSI_DOCPARTS: u32 = 13u32;
pub const PIDDSI_HEADINGPAIR: u32 = 12u32;
pub const PIDDSI_HIDDENCOUNT: u32 = 9u32;
pub const PIDDSI_LINECOUNT: u32 = 5u32;
pub const PIDDSI_LINKSDIRTY: u32 = 16u32;
pub const PIDDSI_MANAGER: u32 = 14u32;
pub const PIDDSI_MMCLIPCOUNT: u32 = 10u32;
pub const PIDDSI_NOTECOUNT: u32 = 8u32;
pub const PIDDSI_PARCOUNT: u32 = 6u32;
pub const PIDDSI_PRESFORMAT: u32 = 3u32;
pub const PIDDSI_SCALE: u32 = 11u32;
pub const PIDDSI_SLIDECOUNT: u32 = 7u32;
pub const PIDMSI_COPYRIGHT: i32 = 11i32;
pub const PIDMSI_EDITOR: i32 = 2i32;
pub const PIDMSI_OWNER: i32 = 8i32;
pub const PIDMSI_PRODUCTION: i32 = 10i32;
pub const PIDMSI_PROJECT: i32 = 6i32;
pub const PIDMSI_RATING: i32 = 9i32;
pub const PIDMSI_SEQUENCE_NO: i32 = 5i32;
pub const PIDMSI_SOURCE: i32 = 4i32;
pub const PIDMSI_STATUS: i32 = 7i32;
pub const PIDMSI_STATUS_DRAFT: PIDMSI_STATUS_VALUE = 3i32;
pub const PIDMSI_STATUS_EDIT: PIDMSI_STATUS_VALUE = 5i32;
pub const PIDMSI_STATUS_FINAL: PIDMSI_STATUS_VALUE = 8i32;
pub const PIDMSI_STATUS_INPROGRESS: PIDMSI_STATUS_VALUE = 4i32;
pub const PIDMSI_STATUS_NEW: PIDMSI_STATUS_VALUE = 1i32;
pub const PIDMSI_STATUS_NORMAL: PIDMSI_STATUS_VALUE = 0i32;
pub const PIDMSI_STATUS_OTHER: PIDMSI_STATUS_VALUE = 32767i32;
pub const PIDMSI_STATUS_PRELIM: PIDMSI_STATUS_VALUE = 2i32;
pub const PIDMSI_STATUS_PROOF: PIDMSI_STATUS_VALUE = 7i32;
pub const PIDMSI_STATUS_REVIEW: PIDMSI_STATUS_VALUE = 6i32;
pub const PIDMSI_SUPPLIER: i32 = 3i32;
pub const PIDSI_APPNAME: i32 = 18i32;
pub const PIDSI_AUTHOR: i32 = 4i32;
pub const PIDSI_CHARCOUNT: i32 = 16i32;
pub const PIDSI_COMMENTS: i32 = 6i32;
pub const PIDSI_CREATE_DTM: i32 = 12i32;
pub const PIDSI_DOC_SECURITY: i32 = 19i32;
pub const PIDSI_EDITTIME: i32 = 10i32;
pub const PIDSI_KEYWORDS: i32 = 5i32;
pub const PIDSI_LASTAUTHOR: i32 = 8i32;
pub const PIDSI_LASTPRINTED: i32 = 11i32;
pub const PIDSI_LASTSAVE_DTM: i32 = 13i32;
pub const PIDSI_PAGECOUNT: i32 = 14i32;
pub const PIDSI_REVNUMBER: i32 = 9i32;
pub const PIDSI_SUBJECT: i32 = 3i32;
pub const PIDSI_TEMPLATE: i32 = 7i32;
pub const PIDSI_THUMBNAIL: i32 = 17i32;
pub const PIDSI_TITLE: i32 = 2i32;
pub const PIDSI_WORDCOUNT: i32 = 15i32;
pub const PID_BEHAVIOR: u32 = 2147483651u32;
pub const PID_CODEPAGE: u32 = 1u32;
pub const PID_DICTIONARY: u32 = 0u32;
pub const PID_FIRST_NAME_DEFAULT: u32 = 4095u32;
pub const PID_FIRST_USABLE: u32 = 2u32;
pub const PID_ILLEGAL: u32 = 4294967295u32;
pub const PID_LOCALE: u32 = 2147483648u32;
pub const PID_MAX_READONLY: u32 = 3221225471u32;
pub const PID_MIN_READONLY: u32 = 2147483648u32;
pub const PID_MODIFY_TIME: u32 = 2147483649u32;
pub const PID_SECURITY: u32 = 2147483650u32;
pub const PROPSETFLAG_ANSI: u32 = 2u32;
pub const PROPSETFLAG_CASE_SENSITIVE: u32 = 8u32;
pub const PROPSETFLAG_DEFAULT: u32 = 0u32;
pub const PROPSETFLAG_NONSIMPLE: u32 = 1u32;
pub const PROPSETFLAG_UNBUFFERED: u32 = 4u32;
pub const PROPSETHDR_OSVERSION_UNKNOWN: u32 = 4294967295u32;
pub const PROPSET_BEHAVIOR_CASE_SENSITIVE: u32 = 1u32;
pub const PRSPEC_INVALID: u32 = 4294967295u32;
pub const PRSPEC_LPWSTR: PROPSPEC_KIND = 0u32;
pub const PRSPEC_PROPID: PROPSPEC_KIND = 1u32;
pub const PVCF_DEFAULT: PROPVAR_COMPARE_FLAGS = 0i32;
pub const PVCF_DIGITSASNUMBERS_CASESENSITIVE: PROPVAR_COMPARE_FLAGS = 32i32;
pub const PVCF_TREATEMPTYASGREATERTHAN: PROPVAR_COMPARE_FLAGS = 1i32;
pub const PVCF_USESTRCMP: PROPVAR_COMPARE_FLAGS = 2i32;
pub const PVCF_USESTRCMPC: PROPVAR_COMPARE_FLAGS = 4i32;
pub const PVCF_USESTRCMPI: PROPVAR_COMPARE_FLAGS = 8i32;
pub const PVCF_USESTRCMPIC: PROPVAR_COMPARE_FLAGS = 16i32;
pub const PVCHF_ALPHABOOL: PROPVAR_CHANGE_FLAGS = 2i32;
pub const PVCHF_DEFAULT: PROPVAR_CHANGE_FLAGS = 0i32;
pub const PVCHF_LOCALBOOL: PROPVAR_CHANGE_FLAGS = 8i32;
pub const PVCHF_NOHEXSTRING: PROPVAR_CHANGE_FLAGS = 16i32;
pub const PVCHF_NOUSEROVERRIDE: PROPVAR_CHANGE_FLAGS = 4i32;
pub const PVCHF_NOVALUEPROP: PROPVAR_CHANGE_FLAGS = 1i32;
pub const PVCU_DAY: PROPVAR_COMPARE_UNIT = 4i32;
pub const PVCU_DEFAULT: PROPVAR_COMPARE_UNIT = 0i32;
pub const PVCU_HOUR: PROPVAR_COMPARE_UNIT = 3i32;
pub const PVCU_MINUTE: PROPVAR_COMPARE_UNIT = 2i32;
pub const PVCU_MONTH: PROPVAR_COMPARE_UNIT = 5i32;
pub const PVCU_SECOND: PROPVAR_COMPARE_UNIT = 1i32;
pub const PVCU_YEAR: PROPVAR_COMPARE_UNIT = 6i32;
pub const STGFMT_ANY: STGFMT = 4u32;
pub const STGFMT_DOCFILE: STGFMT = 5u32;
pub const STGFMT_DOCUMENT: STGFMT = 0u32;
pub const STGFMT_FILE: STGFMT = 3u32;
pub const STGFMT_NATIVE: STGFMT = 1u32;
pub const STGFMT_STORAGE: STGFMT = 0u32;
pub const STGMOVE_COPY: STGMOVE = 1i32;
pub const STGMOVE_MOVE: STGMOVE = 0i32;
pub const STGMOVE_SHALLOWCOPY: STGMOVE = 2i32;
pub const STGOPTIONS_VERSION: u32 = 1u32;
pub type PIDMSI_STATUS_VALUE = i32;
pub type PROPSPEC_KIND = u32;
pub type PROPVAR_CHANGE_FLAGS = i32;
pub type PROPVAR_COMPARE_FLAGS = i32;
pub type PROPVAR_COMPARE_UNIT = i32;
pub type STGFMT = u32;
pub type STGMOVE = i32;
#[repr(C)]
pub struct BSTRBLOB {
    pub cbSize: u32,
    pub pData: *mut u8,
}
impl ::core::marker::Copy for BSTRBLOB {}
impl ::core::clone::Clone for BSTRBLOB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CABOOL {
    pub cElems: u32,
    pub pElems: *mut super::super::super::Foundation::VARIANT_BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CABOOL {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CABOOL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CABSTR {
    pub cElems: u32,
    pub pElems: *mut ::windows_sys::core::BSTR,
}
impl ::core::marker::Copy for CABSTR {}
impl ::core::clone::Clone for CABSTR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CABSTRBLOB {
    pub cElems: u32,
    pub pElems: *mut BSTRBLOB,
}
impl ::core::marker::Copy for CABSTRBLOB {}
impl ::core::clone::Clone for CABSTRBLOB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAC {
    pub cElems: u32,
    pub pElems: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for CAC {}
impl ::core::clone::Clone for CAC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CACLIPDATA {
    pub cElems: u32,
    pub pElems: *mut CLIPDATA,
}
impl ::core::marker::Copy for CACLIPDATA {}
impl ::core::clone::Clone for CACLIPDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CACLSID {
    pub cElems: u32,
    pub pElems: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for CACLSID {}
impl ::core::clone::Clone for CACLSID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CACY {
    pub cElems: u32,
    pub pElems: *mut super::CY,
}
impl ::core::marker::Copy for CACY {}
impl ::core::clone::Clone for CACY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CADATE {
    pub cElems: u32,
    pub pElems: *mut f64,
}
impl ::core::marker::Copy for CADATE {}
impl ::core::clone::Clone for CADATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CADBL {
    pub cElems: u32,
    pub pElems: *mut f64,
}
impl ::core::marker::Copy for CADBL {}
impl ::core::clone::Clone for CADBL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CAFILETIME {
    pub cElems: u32,
    pub pElems: *mut super::super::super::Foundation::FILETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CAFILETIME {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CAFILETIME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAFLT {
    pub cElems: u32,
    pub pElems: *mut f32,
}
impl ::core::marker::Copy for CAFLT {}
impl ::core::clone::Clone for CAFLT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAH {
    pub cElems: u32,
    pub pElems: *mut i64,
}
impl ::core::marker::Copy for CAH {}
impl ::core::clone::Clone for CAH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAI {
    pub cElems: u32,
    pub pElems: *mut i16,
}
impl ::core::marker::Copy for CAI {}
impl ::core::clone::Clone for CAI {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAL {
    pub cElems: u32,
    pub pElems: *mut i32,
}
impl ::core::marker::Copy for CAL {}
impl ::core::clone::Clone for CAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CALPSTR {
    pub cElems: u32,
    pub pElems: *mut ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for CALPSTR {}
impl ::core::clone::Clone for CALPSTR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CALPWSTR {
    pub cElems: u32,
    pub pElems: *mut ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for CALPWSTR {}
impl ::core::clone::Clone for CALPWSTR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
pub struct CAPROPVARIANT {
    pub cElems: u32,
    pub pElems: *mut PROPVARIANT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for CAPROPVARIANT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for CAPROPVARIANT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CASCODE {
    pub cElems: u32,
    pub pElems: *mut i32,
}
impl ::core::marker::Copy for CASCODE {}
impl ::core::clone::Clone for CASCODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAUB {
    pub cElems: u32,
    pub pElems: *mut u8,
}
impl ::core::marker::Copy for CAUB {}
impl ::core::clone::Clone for CAUB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAUH {
    pub cElems: u32,
    pub pElems: *mut u64,
}
impl ::core::marker::Copy for CAUH {}
impl ::core::clone::Clone for CAUH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAUI {
    pub cElems: u32,
    pub pElems: *mut u16,
}
impl ::core::marker::Copy for CAUI {}
impl ::core::clone::Clone for CAUI {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAUL {
    pub cElems: u32,
    pub pElems: *mut u32,
}
impl ::core::marker::Copy for CAUL {}
impl ::core::clone::Clone for CAUL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLIPDATA {
    pub cbSize: u32,
    pub ulClipFmt: i32,
    pub pClipData: *mut u8,
}
impl ::core::marker::Copy for CLIPDATA {}
impl ::core::clone::Clone for CLIPDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OLESTREAM {
    pub lpstbl: *mut OLESTREAMVTBL,
}
impl ::core::marker::Copy for OLESTREAM {}
impl ::core::clone::Clone for OLESTREAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OLESTREAMVTBL {
    pub Get: isize,
    pub Put: isize,
}
impl ::core::marker::Copy for OLESTREAMVTBL {}
impl ::core::clone::Clone for OLESTREAMVTBL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Variant\"`"]
#[cfg(feature = "Win32_System_Variant")]
pub struct PROPBAG2 {
    pub dwType: u32,
    pub vt: super::super::Variant::VARENUM,
    pub cfType: u16,
    pub dwHint: u32,
    pub pstrName: ::windows_sys::core::PWSTR,
    pub clsid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_System_Variant")]
impl ::core::marker::Copy for PROPBAG2 {}
#[cfg(feature = "Win32_System_Variant")]
impl ::core::clone::Clone for PROPBAG2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROPSPEC {
    pub ulKind: PROPSPEC_KIND,
    pub Anonymous: PROPSPEC_0,
}
impl ::core::marker::Copy for PROPSPEC {}
impl ::core::clone::Clone for PROPSPEC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union PROPSPEC_0 {
    pub propid: u32,
    pub lpwstr: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for PROPSPEC_0 {}
impl ::core::clone::Clone for PROPSPEC_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
pub struct PROPVARIANT {
    pub Anonymous: PROPVARIANT_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for PROPVARIANT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for PROPVARIANT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
pub union PROPVARIANT_0 {
    pub Anonymous: PROPVARIANT_0_0,
    pub decVal: super::super::super::Foundation::DECIMAL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for PROPVARIANT_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for PROPVARIANT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
pub struct PROPVARIANT_0_0 {
    pub vt: super::super::Variant::VARENUM,
    pub wReserved1: u16,
    pub wReserved2: u16,
    pub wReserved3: u16,
    pub Anonymous: PROPVARIANT_0_0_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for PROPVARIANT_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for PROPVARIANT_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
pub union PROPVARIANT_0_0_0 {
    pub cVal: u8,
    pub bVal: u8,
    pub iVal: i16,
    pub uiVal: u16,
    pub lVal: i32,
    pub ulVal: u32,
    pub intVal: i32,
    pub uintVal: u32,
    pub hVal: i64,
    pub uhVal: u64,
    pub fltVal: f32,
    pub dblVal: f64,
    pub boolVal: super::super::super::Foundation::VARIANT_BOOL,
    pub __OBSOLETE__VARIANT_BOOL: super::super::super::Foundation::VARIANT_BOOL,
    pub scode: i32,
    pub cyVal: super::CY,
    pub date: f64,
    pub filetime: super::super::super::Foundation::FILETIME,
    pub puuid: *mut ::windows_sys::core::GUID,
    pub pclipdata: *mut CLIPDATA,
    pub bstrVal: ::windows_sys::core::BSTR,
    pub bstrblobVal: BSTRBLOB,
    pub blob: super::BLOB,
    pub pszVal: ::windows_sys::core::PSTR,
    pub pwszVal: ::windows_sys::core::PWSTR,
    pub punkVal: ::windows_sys::core::IUnknown,
    pub pdispVal: super::IDispatch,
    pub pStream: super::IStream,
    pub pStorage: IStorage,
    pub pVersionedStream: *mut VERSIONEDSTREAM,
    pub parray: *mut super::SAFEARRAY,
    pub cac: CAC,
    pub caub: CAUB,
    pub cai: CAI,
    pub caui: CAUI,
    pub cal: CAL,
    pub caul: CAUL,
    pub cah: CAH,
    pub cauh: CAUH,
    pub caflt: CAFLT,
    pub cadbl: CADBL,
    pub cabool: CABOOL,
    pub cascode: CASCODE,
    pub cacy: CACY,
    pub cadate: CADATE,
    pub cafiletime: CAFILETIME,
    pub cauuid: CACLSID,
    pub caclipdata: CACLIPDATA,
    pub cabstr: CABSTR,
    pub cabstrblob: CABSTRBLOB,
    pub calpstr: CALPSTR,
    pub calpwstr: CALPWSTR,
    pub capropvar: CAPROPVARIANT,
    pub pcVal: ::windows_sys::core::PSTR,
    pub pbVal: *mut u8,
    pub piVal: *mut i16,
    pub puiVal: *mut u16,
    pub plVal: *mut i32,
    pub pulVal: *mut u32,
    pub pintVal: *mut i32,
    pub puintVal: *mut u32,
    pub pfltVal: *mut f32,
    pub pdblVal: *mut f64,
    pub pboolVal: *mut super::super::super::Foundation::VARIANT_BOOL,
    pub pdecVal: *mut super::super::super::Foundation::DECIMAL,
    pub pscode: *mut i32,
    pub pcyVal: *mut super::CY,
    pub pdate: *mut f64,
    pub pbstrVal: *mut ::windows_sys::core::BSTR,
    pub ppunkVal: *mut ::windows_sys::core::IUnknown,
    pub ppdispVal: *mut super::IDispatch,
    pub pparray: *mut *mut super::SAFEARRAY,
    pub pvarVal: *mut PROPVARIANT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for PROPVARIANT_0_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for PROPVARIANT_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RemSNB {
    pub ulCntStr: u32,
    pub ulCntChar: u32,
    pub rgString: [u16; 1],
}
impl ::core::marker::Copy for RemSNB {}
impl ::core::clone::Clone for RemSNB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SERIALIZEDPROPERTYVALUE {
    pub dwType: u32,
    pub rgb: [u8; 1],
}
impl ::core::marker::Copy for SERIALIZEDPROPERTYVALUE {}
impl ::core::clone::Clone for SERIALIZEDPROPERTYVALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct STATPROPSETSTG {
    pub fmtid: ::windows_sys::core::GUID,
    pub clsid: ::windows_sys::core::GUID,
    pub grfFlags: u32,
    pub mtime: super::super::super::Foundation::FILETIME,
    pub ctime: super::super::super::Foundation::FILETIME,
    pub atime: super::super::super::Foundation::FILETIME,
    pub dwOSVersion: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for STATPROPSETSTG {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for STATPROPSETSTG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Variant\"`"]
#[cfg(feature = "Win32_System_Variant")]
pub struct STATPROPSTG {
    pub lpwstrName: ::windows_sys::core::PWSTR,
    pub propid: u32,
    pub vt: super::super::Variant::VARENUM,
}
#[cfg(feature = "Win32_System_Variant")]
impl ::core::marker::Copy for STATPROPSTG {}
#[cfg(feature = "Win32_System_Variant")]
impl ::core::clone::Clone for STATPROPSTG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct STGOPTIONS {
    pub usVersion: u16,
    pub reserved: u16,
    pub ulSectorSize: u32,
    pub pwcsTemplateFile: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for STGOPTIONS {}
impl ::core::clone::Clone for STGOPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VERSIONEDSTREAM {
    pub guidVersion: ::windows_sys::core::GUID,
    pub pStream: super::IStream,
}
impl ::core::marker::Copy for VERSIONEDSTREAM {}
impl ::core::clone::Clone for VERSIONEDSTREAM {
    fn clone(&self) -> Self {
        *self
    }
}
