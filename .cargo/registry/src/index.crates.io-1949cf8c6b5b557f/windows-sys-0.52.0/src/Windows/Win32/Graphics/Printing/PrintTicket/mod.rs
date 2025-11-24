#[cfg(feature = "Win32_Storage_Xps")]
::windows_targets::link!("prntvpt.dll" "system" #[doc = "Required features: `\"Win32_Storage_Xps\"`"] fn PTCloseProvider(hprovider : super::super::super::Storage::Xps:: HPTPROVIDER) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_Storage_Xps", feature = "Win32_System_Com"))]
::windows_targets::link!("prntvpt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_Storage_Xps\"`, `\"Win32_System_Com\"`"] fn PTConvertDevModeToPrintTicket(hprovider : super::super::super::Storage::Xps:: HPTPROVIDER, cbdevmode : u32, pdevmode : *const super::super::Gdi:: DEVMODEA, scope : EPrintTicketScope, pprintticket : super::super::super::System::Com:: IStream) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_Storage_Xps", feature = "Win32_System_Com"))]
::windows_targets::link!("prntvpt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_Storage_Xps\"`, `\"Win32_System_Com\"`"] fn PTConvertPrintTicketToDevMode(hprovider : super::super::super::Storage::Xps:: HPTPROVIDER, pprintticket : super::super::super::System::Com:: IStream, basedevmodetype : EDefaultDevmodeType, scope : EPrintTicketScope, pcbdevmode : *mut u32, ppdevmode : *mut *mut super::super::Gdi:: DEVMODEA, pbstrerrormessage : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Storage_Xps", feature = "Win32_System_Com"))]
::windows_targets::link!("prntvpt.dll" "system" #[doc = "Required features: `\"Win32_Storage_Xps\"`, `\"Win32_System_Com\"`"] fn PTGetPrintCapabilities(hprovider : super::super::super::Storage::Xps:: HPTPROVIDER, pprintticket : super::super::super::System::Com:: IStream, pcapabilities : super::super::super::System::Com:: IStream, pbstrerrormessage : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Storage_Xps", feature = "Win32_System_Com"))]
::windows_targets::link!("prntvpt.dll" "system" #[doc = "Required features: `\"Win32_Storage_Xps\"`, `\"Win32_System_Com\"`"] fn PTGetPrintDeviceCapabilities(hprovider : super::super::super::Storage::Xps:: HPTPROVIDER, pprintticket : super::super::super::System::Com:: IStream, pdevicecapabilities : super::super::super::System::Com:: IStream, pbstrerrormessage : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Storage_Xps", feature = "Win32_System_Com"))]
::windows_targets::link!("prntvpt.dll" "system" #[doc = "Required features: `\"Win32_Storage_Xps\"`, `\"Win32_System_Com\"`"] fn PTGetPrintDeviceResources(hprovider : super::super::super::Storage::Xps:: HPTPROVIDER, pszlocalename : ::windows_sys::core::PCWSTR, pprintticket : super::super::super::System::Com:: IStream, pdeviceresources : super::super::super::System::Com:: IStream, pbstrerrormessage : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Storage_Xps", feature = "Win32_System_Com"))]
::windows_targets::link!("prntvpt.dll" "system" #[doc = "Required features: `\"Win32_Storage_Xps\"`, `\"Win32_System_Com\"`"] fn PTMergeAndValidatePrintTicket(hprovider : super::super::super::Storage::Xps:: HPTPROVIDER, pbaseticket : super::super::super::System::Com:: IStream, pdeltaticket : super::super::super::System::Com:: IStream, scope : EPrintTicketScope, presultticket : super::super::super::System::Com:: IStream, pbstrerrormessage : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Storage_Xps")]
::windows_targets::link!("prntvpt.dll" "system" #[doc = "Required features: `\"Win32_Storage_Xps\"`"] fn PTOpenProvider(pszprintername : ::windows_sys::core::PCWSTR, dwversion : u32, phprovider : *mut super::super::super::Storage::Xps:: HPTPROVIDER) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Storage_Xps")]
::windows_targets::link!("prntvpt.dll" "system" #[doc = "Required features: `\"Win32_Storage_Xps\"`"] fn PTOpenProviderEx(pszprintername : ::windows_sys::core::PCWSTR, dwmaxversion : u32, dwprefversion : u32, phprovider : *mut super::super::super::Storage::Xps:: HPTPROVIDER, pusedversion : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("prntvpt.dll" "system" fn PTQuerySchemaVersionSupport(pszprintername : ::windows_sys::core::PCWSTR, pmaxversion : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("prntvpt.dll" "system" fn PTReleaseMemory(pbuffer : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
pub const E_DELTA_PRINTTICKET_FORMAT: u32 = 2147745797u32;
pub const E_PRINTCAPABILITIES_FORMAT: u32 = 2147745796u32;
pub const E_PRINTDEVICECAPABILITIES_FORMAT: u32 = 2147745798u32;
pub const E_PRINTTICKET_FORMAT: u32 = 2147745795u32;
pub const PRINTTICKET_ISTREAM_APIS: u32 = 1u32;
pub const S_PT_CONFLICT_RESOLVED: u32 = 262146u32;
pub const S_PT_NO_CONFLICT: u32 = 262145u32;
pub const kPTDocumentScope: EPrintTicketScope = 1i32;
pub const kPTJobScope: EPrintTicketScope = 2i32;
pub const kPTPageScope: EPrintTicketScope = 0i32;
pub const kPrinterDefaultDevmode: EDefaultDevmodeType = 1i32;
pub const kUserDefaultDevmode: EDefaultDevmodeType = 0i32;
pub type EDefaultDevmodeType = i32;
pub type EPrintTicketScope = i32;
