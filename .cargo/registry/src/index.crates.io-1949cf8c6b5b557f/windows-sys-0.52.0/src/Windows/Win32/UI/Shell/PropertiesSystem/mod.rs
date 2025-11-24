#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSCoerceToCanonicalValue(key : *const PROPERTYKEY, ppropvar : *mut super::super::super::System::Com::StructuredStorage:: PROPVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSCreateAdapterFromPropertyStore(pps : IPropertyStore, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSCreateDelayedMultiplexPropertyStore(flags : GETPROPERTYSTOREFLAGS, pdpsf : IDelayedPropertyStoreFactory, rgstoreids : *const u32, cstores : u32, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSCreateMemoryPropertyStore(riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSCreateMultiplexPropertyStore(prgpunkstores : *const ::windows_sys::core::IUnknown, cstores : u32, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSCreatePropertyChangeArray(rgpropkey : *const PROPERTYKEY, rgflags : *const PKA_FLAGS, rgpropvar : *const super::super::super::System::Com::StructuredStorage:: PROPVARIANT, cchanges : u32, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSCreatePropertyStoreFromObject(punk : ::windows_sys::core::IUnknown, grfmode : u32, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSCreatePropertyStoreFromPropertySetStorage(ppss : super::super::super::System::Com::StructuredStorage:: IPropertySetStorage, grfmode : u32, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSCreateSimplePropertyChange(flags : PKA_FLAGS, key : *const PROPERTYKEY, propvar : *const super::super::super::System::Com::StructuredStorage:: PROPVARIANT, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSEnumeratePropertyDescriptions(filteron : PROPDESC_ENUMFILTER, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSFormatForDisplay(propkey : *const PROPERTYKEY, propvar : *const super::super::super::System::Com::StructuredStorage:: PROPVARIANT, pdfflags : PROPDESC_FORMAT_FLAGS, pwsztext : ::windows_sys::core::PWSTR, cchtext : u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSFormatForDisplayAlloc(key : *const PROPERTYKEY, propvar : *const super::super::super::System::Com::StructuredStorage:: PROPVARIANT, pdff : PROPDESC_FORMAT_FLAGS, ppszdisplay : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSFormatPropertyValue(pps : IPropertyStore, ppd : IPropertyDescription, pdff : PROPDESC_FORMAT_FLAGS, ppszdisplay : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSGetImageReferenceForValue(propkey : *const PROPERTYKEY, propvar : *const super::super::super::System::Com::StructuredStorage:: PROPVARIANT, ppszimageres : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PSGetItemPropertyHandler(punkitem : ::windows_sys::core::IUnknown, freadwrite : super::super::super::Foundation:: BOOL, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PSGetItemPropertyHandlerWithCreateObject(punkitem : ::windows_sys::core::IUnknown, freadwrite : super::super::super::Foundation:: BOOL, punkcreateobject : ::windows_sys::core::IUnknown, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSGetNameFromPropertyKey(propkey : *const PROPERTYKEY, ppszcanonicalname : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSGetNamedPropertyFromPropertyStorage(psps : PCUSERIALIZEDPROPSTORAGE, cb : u32, pszname : ::windows_sys::core::PCWSTR, ppropvar : *mut super::super::super::System::Com::StructuredStorage:: PROPVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSGetPropertyDescription(propkey : *const PROPERTYKEY, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSGetPropertyDescriptionByName(pszcanonicalname : ::windows_sys::core::PCWSTR, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSGetPropertyDescriptionListFromString(pszproplist : ::windows_sys::core::PCWSTR, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSGetPropertyFromPropertyStorage(psps : PCUSERIALIZEDPROPSTORAGE, cb : u32, rpkey : *const PROPERTYKEY, ppropvar : *mut super::super::super::System::Com::StructuredStorage:: PROPVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSGetPropertyKeyFromName(pszname : ::windows_sys::core::PCWSTR, ppropkey : *mut PROPERTYKEY) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSGetPropertySystem(riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSGetPropertyValue(pps : IPropertyStore, ppd : IPropertyDescription, ppropvar : *mut super::super::super::System::Com::StructuredStorage:: PROPVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSLookupPropertyHandlerCLSID(pszfilepath : ::windows_sys::core::PCWSTR, pclsid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_Delete(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadBOOL(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut super::super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadBSTR(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadDWORD(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadGUID(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadInt(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadLONG(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadPOINTL(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut super::super::super::Foundation:: POINTL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadPOINTS(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut super::super::super::Foundation:: POINTS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadPropertyKey(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut PROPERTYKEY) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadRECTL(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut super::super::super::Foundation:: RECTL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadSHORT(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut i16) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadStr(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : ::windows_sys::core::PWSTR, charactercount : i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadStrAlloc(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadStream(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut super::super::super::System::Com:: IStream) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn PSPropertyBag_ReadType(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, var : *mut super::super::super::System::Variant:: VARIANT, r#type : super::super::super::System::Variant:: VARENUM) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadULONGLONG(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_ReadUnknown(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteBOOL(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : super::super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteBSTR(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteDWORD(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteGUID(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *const ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteInt(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteLONG(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : i32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WritePOINTL(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *const super::super::super::Foundation:: POINTL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WritePOINTS(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *const super::super::super::Foundation:: POINTS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WritePropertyKey(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *const PROPERTYKEY) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteRECTL(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : *const super::super::super::Foundation:: RECTL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteSHORT(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : i16) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteStr(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteStream(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : super::super::super::System::Com:: IStream) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteULONGLONG(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, value : u64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn PSPropertyBag_WriteUnknown(propbag : super::super::super::System::Com::StructuredStorage:: IPropertyBag, propname : ::windows_sys::core::PCWSTR, punk : ::windows_sys::core::IUnknown) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSPropertyKeyFromString(pszstring : ::windows_sys::core::PCWSTR, pkey : *mut PROPERTYKEY) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSRefreshPropertySchema() -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSRegisterPropertySchema(pszpath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("propsys.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PSSetPropertyValue(pps : IPropertyStore, ppd : IPropertyDescription, propvar : *const super::super::super::System::Com::StructuredStorage:: PROPVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSStringFromPropertyKey(pkey : *const PROPERTYKEY, psz : ::windows_sys::core::PWSTR, cch : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("propsys.dll" "system" fn PSUnregisterPropertySchema(pszpath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PifMgr_CloseProperties(hprops : super::super::super::Foundation:: HANDLE, flopt : u32) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PifMgr_GetProperties(hprops : super::super::super::Foundation:: HANDLE, pszgroup : ::windows_sys::core::PCSTR, lpprops : *mut ::core::ffi::c_void, cbprops : i32, flopt : u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PifMgr_OpenProperties(pszapp : ::windows_sys::core::PCWSTR, pszpif : ::windows_sys::core::PCWSTR, hinf : u32, flopt : u32) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PifMgr_SetProperties(hprops : super::super::super::Foundation:: HANDLE, pszgroup : ::windows_sys::core::PCSTR, lpprops : *const ::core::ffi::c_void, cbprops : i32, flopt : u32) -> i32);
::windows_targets::link!("shell32.dll" "system" fn SHAddDefaultPropertiesByExt(pszext : ::windows_sys::core::PCWSTR, ppropstore : IPropertyStore) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SHGetPropertyStoreForWindow(hwnd : super::super::super::Foundation:: HWND, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_UI_Shell_Common")]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_UI_Shell_Common\"`"] fn SHGetPropertyStoreFromIDList(pidl : *const super::Common:: ITEMIDLIST, flags : GETPROPERTYSTOREFLAGS, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn SHGetPropertyStoreFromParsingName(pszpath : ::windows_sys::core::PCWSTR, pbc : super::super::super::System::Com:: IBindCtx, flags : GETPROPERTYSTOREFLAGS, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn SHPropStgCreate(psstg : super::super::super::System::Com::StructuredStorage:: IPropertySetStorage, fmtid : *const ::windows_sys::core::GUID, pclsid : *const ::windows_sys::core::GUID, grfflags : u32, grfmode : u32, dwdisposition : u32, ppstg : *mut super::super::super::System::Com::StructuredStorage:: IPropertyStorage, pucodepage : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn SHPropStgReadMultiple(pps : super::super::super::System::Com::StructuredStorage:: IPropertyStorage, ucodepage : u32, cpspec : u32, rgpspec : *const super::super::super::System::Com::StructuredStorage:: PROPSPEC, rgvar : *mut super::super::super::System::Com::StructuredStorage:: PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("shell32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn SHPropStgWriteMultiple(pps : super::super::super::System::Com::StructuredStorage:: IPropertyStorage, pucodepage : *mut u32, cpspec : u32, rgpspec : *const super::super::super::System::Com::StructuredStorage:: PROPSPEC, rgvar : *mut super::super::super::System::Com::StructuredStorage:: PROPVARIANT, propidnamefirst : u32) -> ::windows_sys::core::HRESULT);
pub type ICreateObject = *mut ::core::ffi::c_void;
pub type IDelayedPropertyStoreFactory = *mut ::core::ffi::c_void;
pub type IInitializeWithFile = *mut ::core::ffi::c_void;
pub type IInitializeWithStream = *mut ::core::ffi::c_void;
pub type INamedPropertyStore = *mut ::core::ffi::c_void;
pub type IObjectWithPropertyKey = *mut ::core::ffi::c_void;
pub type IPersistSerializedPropStorage = *mut ::core::ffi::c_void;
pub type IPersistSerializedPropStorage2 = *mut ::core::ffi::c_void;
pub type IPropertyChange = *mut ::core::ffi::c_void;
pub type IPropertyChangeArray = *mut ::core::ffi::c_void;
pub type IPropertyDescription = *mut ::core::ffi::c_void;
pub type IPropertyDescription2 = *mut ::core::ffi::c_void;
pub type IPropertyDescriptionAliasInfo = *mut ::core::ffi::c_void;
pub type IPropertyDescriptionList = *mut ::core::ffi::c_void;
pub type IPropertyDescriptionRelatedPropertyInfo = *mut ::core::ffi::c_void;
pub type IPropertyDescriptionSearchInfo = *mut ::core::ffi::c_void;
pub type IPropertyEnumType = *mut ::core::ffi::c_void;
pub type IPropertyEnumType2 = *mut ::core::ffi::c_void;
pub type IPropertyEnumTypeList = *mut ::core::ffi::c_void;
pub type IPropertyStore = *mut ::core::ffi::c_void;
pub type IPropertyStoreCache = *mut ::core::ffi::c_void;
pub type IPropertyStoreCapabilities = *mut ::core::ffi::c_void;
pub type IPropertyStoreFactory = *mut ::core::ffi::c_void;
pub type IPropertySystem = *mut ::core::ffi::c_void;
pub type IPropertySystemChangeNotify = *mut ::core::ffi::c_void;
pub type IPropertyUI = *mut ::core::ffi::c_void;
pub const FPSPS_DEFAULT: _PERSIST_SPROPSTORE_FLAGS = 0i32;
pub const FPSPS_READONLY: _PERSIST_SPROPSTORE_FLAGS = 1i32;
pub const FPSPS_TREAT_NEW_VALUES_AS_DIRTY: _PERSIST_SPROPSTORE_FLAGS = 2i32;
pub const GPS_BESTEFFORT: GETPROPERTYSTOREFLAGS = 64i32;
pub const GPS_DEFAULT: GETPROPERTYSTOREFLAGS = 0i32;
pub const GPS_DELAYCREATION: GETPROPERTYSTOREFLAGS = 32i32;
pub const GPS_EXTRINSICPROPERTIES: GETPROPERTYSTOREFLAGS = 512i32;
pub const GPS_EXTRINSICPROPERTIESONLY: GETPROPERTYSTOREFLAGS = 1024i32;
pub const GPS_FASTPROPERTIESONLY: GETPROPERTYSTOREFLAGS = 8i32;
pub const GPS_HANDLERPROPERTIESONLY: GETPROPERTYSTOREFLAGS = 1i32;
pub const GPS_MASK_VALID: GETPROPERTYSTOREFLAGS = 8191i32;
pub const GPS_NO_OPLOCK: GETPROPERTYSTOREFLAGS = 128i32;
pub const GPS_OPENSLOWITEM: GETPROPERTYSTOREFLAGS = 16i32;
pub const GPS_PREFERQUERYPROPERTIES: GETPROPERTYSTOREFLAGS = 256i32;
pub const GPS_READWRITE: GETPROPERTYSTOREFLAGS = 2i32;
pub const GPS_TEMPORARY: GETPROPERTYSTOREFLAGS = 4i32;
pub const GPS_VOLATILEPROPERTIES: GETPROPERTYSTOREFLAGS = 2048i32;
pub const GPS_VOLATILEPROPERTIESONLY: GETPROPERTYSTOREFLAGS = 4096i32;
pub const InMemoryPropertyStore: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9a02e012_6303_4e1e_b9a1_630f802592c5);
pub const InMemoryPropertyStoreMarshalByValue: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd4ca0e2d_6da7_4b75_a97c_5f306f0eaedc);
pub const PDAT_AVERAGE: PROPDESC_AGGREGATION_TYPE = 3i32;
pub const PDAT_DATERANGE: PROPDESC_AGGREGATION_TYPE = 4i32;
pub const PDAT_DEFAULT: PROPDESC_AGGREGATION_TYPE = 0i32;
pub const PDAT_FIRST: PROPDESC_AGGREGATION_TYPE = 1i32;
pub const PDAT_MAX: PROPDESC_AGGREGATION_TYPE = 6i32;
pub const PDAT_MIN: PROPDESC_AGGREGATION_TYPE = 7i32;
pub const PDAT_SUM: PROPDESC_AGGREGATION_TYPE = 2i32;
pub const PDAT_UNION: PROPDESC_AGGREGATION_TYPE = 5i32;
pub const PDCIT_INMEMORY: PROPDESC_COLUMNINDEX_TYPE = 2i32;
pub const PDCIT_NONE: PROPDESC_COLUMNINDEX_TYPE = 0i32;
pub const PDCIT_ONDEMAND: PROPDESC_COLUMNINDEX_TYPE = 3i32;
pub const PDCIT_ONDISK: PROPDESC_COLUMNINDEX_TYPE = 1i32;
pub const PDCIT_ONDISKALL: PROPDESC_COLUMNINDEX_TYPE = 4i32;
pub const PDCIT_ONDISKVECTOR: PROPDESC_COLUMNINDEX_TYPE = 5i32;
pub const PDCOT_BOOLEAN: PROPDESC_CONDITION_TYPE = 4i32;
pub const PDCOT_DATETIME: PROPDESC_CONDITION_TYPE = 3i32;
pub const PDCOT_NONE: PROPDESC_CONDITION_TYPE = 0i32;
pub const PDCOT_NUMBER: PROPDESC_CONDITION_TYPE = 5i32;
pub const PDCOT_SIZE: PROPDESC_CONDITION_TYPE = 2i32;
pub const PDCOT_STRING: PROPDESC_CONDITION_TYPE = 1i32;
pub const PDDT_BOOLEAN: PROPDESC_DISPLAYTYPE = 2i32;
pub const PDDT_DATETIME: PROPDESC_DISPLAYTYPE = 3i32;
pub const PDDT_ENUMERATED: PROPDESC_DISPLAYTYPE = 4i32;
pub const PDDT_NUMBER: PROPDESC_DISPLAYTYPE = 1i32;
pub const PDDT_STRING: PROPDESC_DISPLAYTYPE = 0i32;
pub const PDEF_ALL: PROPDESC_ENUMFILTER = 0i32;
pub const PDEF_COLUMN: PROPDESC_ENUMFILTER = 6i32;
pub const PDEF_INFULLTEXTQUERY: PROPDESC_ENUMFILTER = 5i32;
pub const PDEF_NONSYSTEM: PROPDESC_ENUMFILTER = 2i32;
pub const PDEF_QUERYABLE: PROPDESC_ENUMFILTER = 4i32;
pub const PDEF_SYSTEM: PROPDESC_ENUMFILTER = 1i32;
pub const PDEF_VIEWABLE: PROPDESC_ENUMFILTER = 3i32;
pub const PDFF_ALWAYSKB: PROPDESC_FORMAT_FLAGS = 4i32;
pub const PDFF_DEFAULT: PROPDESC_FORMAT_FLAGS = 0i32;
pub const PDFF_FILENAME: PROPDESC_FORMAT_FLAGS = 2i32;
pub const PDFF_HIDEDATE: PROPDESC_FORMAT_FLAGS = 512i32;
pub const PDFF_HIDETIME: PROPDESC_FORMAT_FLAGS = 64i32;
pub const PDFF_LONGDATE: PROPDESC_FORMAT_FLAGS = 256i32;
pub const PDFF_LONGTIME: PROPDESC_FORMAT_FLAGS = 32i32;
pub const PDFF_NOAUTOREADINGORDER: PROPDESC_FORMAT_FLAGS = 8192i32;
pub const PDFF_PREFIXNAME: PROPDESC_FORMAT_FLAGS = 1i32;
pub const PDFF_READONLY: PROPDESC_FORMAT_FLAGS = 4096i32;
pub const PDFF_RELATIVEDATE: PROPDESC_FORMAT_FLAGS = 1024i32;
pub const PDFF_RESERVED_RIGHTTOLEFT: PROPDESC_FORMAT_FLAGS = 8i32;
pub const PDFF_SHORTDATE: PROPDESC_FORMAT_FLAGS = 128i32;
pub const PDFF_SHORTTIME: PROPDESC_FORMAT_FLAGS = 16i32;
pub const PDFF_USEEDITINVITATION: PROPDESC_FORMAT_FLAGS = 2048i32;
pub const PDGR_ALPHANUMERIC: PROPDESC_GROUPING_RANGE = 1i32;
pub const PDGR_DATE: PROPDESC_GROUPING_RANGE = 4i32;
pub const PDGR_DISCRETE: PROPDESC_GROUPING_RANGE = 0i32;
pub const PDGR_DYNAMIC: PROPDESC_GROUPING_RANGE = 3i32;
pub const PDGR_ENUMERATED: PROPDESC_GROUPING_RANGE = 6i32;
pub const PDGR_PERCENT: PROPDESC_GROUPING_RANGE = 5i32;
pub const PDGR_SIZE: PROPDESC_GROUPING_RANGE = 2i32;
pub const PDOPS_CANCELLED: PDOPSTATUS = 3i32;
pub const PDOPS_ERRORS: PDOPSTATUS = 5i32;
pub const PDOPS_PAUSED: PDOPSTATUS = 2i32;
pub const PDOPS_RUNNING: PDOPSTATUS = 1i32;
pub const PDOPS_STOPPED: PDOPSTATUS = 4i32;
pub const PDRDT_COUNT: PROPDESC_RELATIVEDESCRIPTION_TYPE = 3i32;
pub const PDRDT_DATE: PROPDESC_RELATIVEDESCRIPTION_TYPE = 1i32;
pub const PDRDT_DURATION: PROPDESC_RELATIVEDESCRIPTION_TYPE = 6i32;
pub const PDRDT_GENERAL: PROPDESC_RELATIVEDESCRIPTION_TYPE = 0i32;
pub const PDRDT_LENGTH: PROPDESC_RELATIVEDESCRIPTION_TYPE = 5i32;
pub const PDRDT_PRIORITY: PROPDESC_RELATIVEDESCRIPTION_TYPE = 10i32;
pub const PDRDT_RATE: PROPDESC_RELATIVEDESCRIPTION_TYPE = 8i32;
pub const PDRDT_RATING: PROPDESC_RELATIVEDESCRIPTION_TYPE = 9i32;
pub const PDRDT_REVISION: PROPDESC_RELATIVEDESCRIPTION_TYPE = 4i32;
pub const PDRDT_SIZE: PROPDESC_RELATIVEDESCRIPTION_TYPE = 2i32;
pub const PDRDT_SPEED: PROPDESC_RELATIVEDESCRIPTION_TYPE = 7i32;
pub const PDSD_A_Z: PROPDESC_SORTDESCRIPTION = 1i32;
pub const PDSD_GENERAL: PROPDESC_SORTDESCRIPTION = 0i32;
pub const PDSD_LOWEST_HIGHEST: PROPDESC_SORTDESCRIPTION = 2i32;
pub const PDSD_OLDEST_NEWEST: PROPDESC_SORTDESCRIPTION = 4i32;
pub const PDSD_SMALLEST_BIGGEST: PROPDESC_SORTDESCRIPTION = 3i32;
pub const PDSIF_ALWAYSINCLUDE: PROPDESC_SEARCHINFO_FLAGS = 8i32;
pub const PDSIF_DEFAULT: PROPDESC_SEARCHINFO_FLAGS = 0i32;
pub const PDSIF_ININVERTEDINDEX: PROPDESC_SEARCHINFO_FLAGS = 1i32;
pub const PDSIF_ISCOLUMN: PROPDESC_SEARCHINFO_FLAGS = 2i32;
pub const PDSIF_ISCOLUMNSPARSE: PROPDESC_SEARCHINFO_FLAGS = 4i32;
pub const PDSIF_USEFORTYPEAHEAD: PROPDESC_SEARCHINFO_FLAGS = 16i32;
pub const PDTF_ALWAYSINSUPPLEMENTALSTORE: PROPDESC_TYPE_FLAGS = 4096u32;
pub const PDTF_CANBEPURGED: PROPDESC_TYPE_FLAGS = 512u32;
pub const PDTF_CANGROUPBY: PROPDESC_TYPE_FLAGS = 8u32;
pub const PDTF_CANSTACKBY: PROPDESC_TYPE_FLAGS = 16u32;
pub const PDTF_DEFAULT: PROPDESC_TYPE_FLAGS = 0u32;
pub const PDTF_DONTCOERCEEMPTYSTRINGS: PROPDESC_TYPE_FLAGS = 2048u32;
pub const PDTF_INCLUDEINFULLTEXTQUERY: PROPDESC_TYPE_FLAGS = 64u32;
pub const PDTF_ISGROUP: PROPDESC_TYPE_FLAGS = 4u32;
pub const PDTF_ISINNATE: PROPDESC_TYPE_FLAGS = 2u32;
pub const PDTF_ISQUERYABLE: PROPDESC_TYPE_FLAGS = 256u32;
pub const PDTF_ISSYSTEMPROPERTY: PROPDESC_TYPE_FLAGS = 2147483648u32;
pub const PDTF_ISTREEPROPERTY: PROPDESC_TYPE_FLAGS = 32u32;
pub const PDTF_ISVIEWABLE: PROPDESC_TYPE_FLAGS = 128u32;
pub const PDTF_MASK_ALL: PROPDESC_TYPE_FLAGS = 2147491839u32;
pub const PDTF_MULTIPLEVALUES: PROPDESC_TYPE_FLAGS = 1u32;
pub const PDTF_SEARCHRAWVALUE: PROPDESC_TYPE_FLAGS = 1024u32;
pub const PDVF_BEGINNEWGROUP: PROPDESC_VIEW_FLAGS = 4i32;
pub const PDVF_CANWRAP: PROPDESC_VIEW_FLAGS = 4096i32;
pub const PDVF_CENTERALIGN: PROPDESC_VIEW_FLAGS = 1i32;
pub const PDVF_DEFAULT: PROPDESC_VIEW_FLAGS = 0i32;
pub const PDVF_FILLAREA: PROPDESC_VIEW_FLAGS = 8i32;
pub const PDVF_HIDDEN: PROPDESC_VIEW_FLAGS = 2048i32;
pub const PDVF_HIDELABEL: PROPDESC_VIEW_FLAGS = 512i32;
pub const PDVF_MASK_ALL: PROPDESC_VIEW_FLAGS = 7167i32;
pub const PDVF_RIGHTALIGN: PROPDESC_VIEW_FLAGS = 2i32;
pub const PDVF_SHOWBYDEFAULT: PROPDESC_VIEW_FLAGS = 64i32;
pub const PDVF_SHOWINPRIMARYLIST: PROPDESC_VIEW_FLAGS = 128i32;
pub const PDVF_SHOWINSECONDARYLIST: PROPDESC_VIEW_FLAGS = 256i32;
pub const PDVF_SHOWONLYIFPRESENT: PROPDESC_VIEW_FLAGS = 32i32;
pub const PDVF_SORTDESCENDING: PROPDESC_VIEW_FLAGS = 16i32;
pub const PET_DEFAULTVALUE: PROPENUMTYPE = 2i32;
pub const PET_DISCRETEVALUE: PROPENUMTYPE = 0i32;
pub const PET_ENDRANGE: PROPENUMTYPE = 3i32;
pub const PET_RANGEDVALUE: PROPENUMTYPE = 1i32;
pub const PKA_APPEND: PKA_FLAGS = 1i32;
pub const PKA_DELETE: PKA_FLAGS = 2i32;
pub const PKA_SET: PKA_FLAGS = 0i32;
pub const PKEY_PIDSTR_MAX: u32 = 10u32;
pub const PSC_DIRTY: PSC_STATE = 2i32;
pub const PSC_NORMAL: PSC_STATE = 0i32;
pub const PSC_NOTINSOURCE: PSC_STATE = 1i32;
pub const PSC_READONLY: PSC_STATE = 3i32;
pub const PS_ALL: PLACEHOLDER_STATES = 15i32;
pub const PS_CLOUDFILE_PLACEHOLDER: PLACEHOLDER_STATES = 8i32;
pub const PS_CREATE_FILE_ACCESSIBLE: PLACEHOLDER_STATES = 4i32;
pub const PS_DEFAULT: PLACEHOLDER_STATES = 7i32;
pub const PS_FULL_PRIMARY_STREAM_AVAILABLE: PLACEHOLDER_STATES = 2i32;
pub const PS_MARKED_FOR_OFFLINE_AVAILABILITY: PLACEHOLDER_STATES = 1i32;
pub const PS_NONE: PLACEHOLDER_STATES = 0i32;
pub const PUIFFDF_DEFAULT: PROPERTYUI_FORMAT_FLAGS = 0i32;
pub const PUIFFDF_FRIENDLYDATE: PROPERTYUI_FORMAT_FLAGS = 8i32;
pub const PUIFFDF_NOTIME: PROPERTYUI_FORMAT_FLAGS = 4i32;
pub const PUIFFDF_RIGHTTOLEFT: PROPERTYUI_FORMAT_FLAGS = 1i32;
pub const PUIFFDF_SHORTFORMAT: PROPERTYUI_FORMAT_FLAGS = 2i32;
pub const PUIFNF_DEFAULT: PROPERTYUI_NAME_FLAGS = 0i32;
pub const PUIFNF_MNEMONIC: PROPERTYUI_NAME_FLAGS = 1i32;
pub const PUIF_DEFAULT: PROPERTYUI_FLAGS = 0i32;
pub const PUIF_NOLABELININFOTIP: PROPERTYUI_FLAGS = 2i32;
pub const PUIF_RIGHTALIGN: PROPERTYUI_FLAGS = 1i32;
pub const PropertySystem: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb8967f85_58ae_4f46_9fb2_5d7904798f4b);
pub const SESF_ALL_FLAGS: SYNC_ENGINE_STATE_FLAGS = 511i32;
pub const SESF_AUTHENTICATION_ERROR: SYNC_ENGINE_STATE_FLAGS = 4i32;
pub const SESF_NONE: SYNC_ENGINE_STATE_FLAGS = 0i32;
pub const SESF_PAUSED_DUE_TO_CLIENT_POLICY: SYNC_ENGINE_STATE_FLAGS = 32i32;
pub const SESF_PAUSED_DUE_TO_DISK_SPACE_FULL: SYNC_ENGINE_STATE_FLAGS = 16i32;
pub const SESF_PAUSED_DUE_TO_METERED_NETWORK: SYNC_ENGINE_STATE_FLAGS = 8i32;
pub const SESF_PAUSED_DUE_TO_SERVICE_POLICY: SYNC_ENGINE_STATE_FLAGS = 64i32;
pub const SESF_PAUSED_DUE_TO_USER_REQUEST: SYNC_ENGINE_STATE_FLAGS = 256i32;
pub const SESF_SERVICE_QUOTA_EXCEEDED_LIMIT: SYNC_ENGINE_STATE_FLAGS = 2i32;
pub const SESF_SERVICE_QUOTA_NEARING_LIMIT: SYNC_ENGINE_STATE_FLAGS = 1i32;
pub const SESF_SERVICE_UNAVAILABLE: SYNC_ENGINE_STATE_FLAGS = 128i32;
pub const STS_EXCLUDED: SYNC_TRANSFER_STATUS = 256i32;
pub const STS_FETCHING_METADATA: SYNC_TRANSFER_STATUS = 32i32;
pub const STS_HASERROR: SYNC_TRANSFER_STATUS = 16i32;
pub const STS_HASWARNING: SYNC_TRANSFER_STATUS = 128i32;
pub const STS_INCOMPLETE: SYNC_TRANSFER_STATUS = 512i32;
pub const STS_NEEDSDOWNLOAD: SYNC_TRANSFER_STATUS = 2i32;
pub const STS_NEEDSUPLOAD: SYNC_TRANSFER_STATUS = 1i32;
pub const STS_NONE: SYNC_TRANSFER_STATUS = 0i32;
pub const STS_PAUSED: SYNC_TRANSFER_STATUS = 8i32;
pub const STS_PLACEHOLDER_IFEMPTY: SYNC_TRANSFER_STATUS = 1024i32;
pub const STS_TRANSFERRING: SYNC_TRANSFER_STATUS = 4i32;
pub const STS_USER_REQUESTED_REFRESH: SYNC_TRANSFER_STATUS = 64i32;
pub type GETPROPERTYSTOREFLAGS = i32;
pub type PDOPSTATUS = i32;
pub type PKA_FLAGS = i32;
pub type PLACEHOLDER_STATES = i32;
pub type PROPDESC_AGGREGATION_TYPE = i32;
pub type PROPDESC_COLUMNINDEX_TYPE = i32;
pub type PROPDESC_CONDITION_TYPE = i32;
pub type PROPDESC_DISPLAYTYPE = i32;
pub type PROPDESC_ENUMFILTER = i32;
pub type PROPDESC_FORMAT_FLAGS = i32;
pub type PROPDESC_GROUPING_RANGE = i32;
pub type PROPDESC_RELATIVEDESCRIPTION_TYPE = i32;
pub type PROPDESC_SEARCHINFO_FLAGS = i32;
pub type PROPDESC_SORTDESCRIPTION = i32;
pub type PROPDESC_TYPE_FLAGS = u32;
pub type PROPDESC_VIEW_FLAGS = i32;
pub type PROPENUMTYPE = i32;
pub type PROPERTYUI_FLAGS = i32;
pub type PROPERTYUI_FORMAT_FLAGS = i32;
pub type PROPERTYUI_NAME_FLAGS = i32;
pub type PSC_STATE = i32;
pub type SYNC_ENGINE_STATE_FLAGS = i32;
pub type SYNC_TRANSFER_STATUS = i32;
pub type _PERSIST_SPROPSTORE_FLAGS = i32;
pub type PCUSERIALIZEDPROPSTORAGE = isize;
#[repr(C)]
pub struct PROPERTYKEY {
    pub fmtid: ::windows_sys::core::GUID,
    pub pid: u32,
}
impl ::core::marker::Copy for PROPERTYKEY {}
impl ::core::clone::Clone for PROPERTYKEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PROPPRG {
    pub flPrg: u16,
    pub flPrgInit: u16,
    pub achTitle: [u8; 30],
    pub achCmdLine: [u8; 128],
    pub achWorkDir: [u8; 64],
    pub wHotKey: u16,
    pub achIconFile: [u8; 80],
    pub wIconIndex: u16,
    pub dwEnhModeFlags: u32,
    pub dwRealModeFlags: u32,
    pub achOtherFile: [u8; 80],
    pub achPIFFile: [u8; 260],
}
impl ::core::marker::Copy for PROPPRG {}
impl ::core::clone::Clone for PROPPRG {
    fn clone(&self) -> Self {
        *self
    }
}
pub type SERIALIZEDPROPSTORAGE = isize;
