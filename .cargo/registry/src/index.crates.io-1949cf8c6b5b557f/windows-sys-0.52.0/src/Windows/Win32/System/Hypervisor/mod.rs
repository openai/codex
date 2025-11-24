::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ApplyGuestMemoryFix(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, virtualaddress : u64, fixbuffer : *const ::core::ffi::c_void, fixbuffersize : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ApplyPendingSavedStateFileReplayLog(vmrsfile : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn CallStackUnwind(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, imageinfo : *const MODULE_INFO, imageinfocount : u32, framecount : u32, callstack : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindSavedStateSymbolFieldInType(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, typename : ::windows_sys::core::PCSTR, fieldname : ::windows_sys::core::PCWSTR, offset : *mut u32, found : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ForceActiveVirtualTrustLevel(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, virtualtrustlevel : u8) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ForceArchitecture(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, architecture : VIRTUAL_PROCESSOR_ARCH) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ForceNestedHostMode(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, hostmode : super::super::Foundation:: BOOL, oldmode : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ForcePagingMode(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, pagingmode : PAGING_MODE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetActiveVirtualTrustLevel(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, virtualtrustlevel : *mut u8) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetArchitecture(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, architecture : *mut VIRTUAL_PROCESSOR_ARCH) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetEnabledVirtualTrustLevels(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, virtualtrustlevels : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetGuestEnabledVirtualTrustLevels(vmsavedstatedumphandle : *mut ::core::ffi::c_void, virtualtrustlevels : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetGuestOsInfo(vmsavedstatedumphandle : *mut ::core::ffi::c_void, virtualtrustlevel : u8, guestosinfo : *mut GUEST_OS_INFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetGuestPhysicalMemoryChunks(vmsavedstatedumphandle : *mut ::core::ffi::c_void, memorychunkpagesize : *mut u64, memorychunks : *mut GPA_MEMORY_CHUNK, memorychunkcount : *mut u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetGuestRawSavedMemorySize(vmsavedstatedumphandle : *mut ::core::ffi::c_void, guestrawsavedmemorysize : *mut u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetMemoryBlockCacheLimit(vmsavedstatedumphandle : *mut ::core::ffi::c_void, memoryblockcachelimit : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetNestedVirtualizationMode(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, enabled : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetPagingMode(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, pagingmode : *mut PAGING_MODE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetRegisterValue(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, registerid : u32, registervalue : *mut VIRTUAL_PROCESSOR_REGISTER) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetSavedStateSymbolFieldInfo(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, typename : ::windows_sys::core::PCSTR, typefieldinfomap : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetSavedStateSymbolProviderHandle(vmsavedstatedumphandle : *mut ::core::ffi::c_void) -> super::super::Foundation:: HANDLE);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetSavedStateSymbolTypeSize(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, typename : ::windows_sys::core::PCSTR, size : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GetVpCount(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpcount : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GuestPhysicalAddressToRawSavedMemoryOffset(vmsavedstatedumphandle : *mut ::core::ffi::c_void, physicaladdress : u64, rawsavedmemoryoffset : *mut u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn GuestVirtualAddressToPhysicalAddress(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, virtualaddress : u64, physicaladdress : *mut u64, unmappedregionsize : *mut u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmdevicehost.dll" "system" fn HdvCreateDeviceInstance(devicehosthandle : *const ::core::ffi::c_void, devicetype : HDV_DEVICE_TYPE, deviceclassid : *const ::windows_sys::core::GUID, deviceinstanceid : *const ::windows_sys::core::GUID, deviceinterface : *const ::core::ffi::c_void, devicecontext : *const ::core::ffi::c_void, devicehandle : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmdevicehost.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HdvCreateGuestMemoryAperture(requestor : *const ::core::ffi::c_void, guestphysicaladdress : u64, bytecount : u32, writeprotected : super::super::Foundation:: BOOL, mappedaddress : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmdevicehost.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HdvCreateSectionBackedMmioRange(requestor : *const ::core::ffi::c_void, barindex : HDV_PCI_BAR_SELECTOR, offsetinpages : u64, lengthinpages : u64, mappingflags : HDV_MMIO_MAPPING_FLAGS, sectionhandle : super::super::Foundation:: HANDLE, sectionoffsetinpages : u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmdevicehost.dll" "system" fn HdvDeliverGuestInterrupt(requestor : *const ::core::ffi::c_void, msiaddress : u64, msidata : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmdevicehost.dll" "system" fn HdvDestroyGuestMemoryAperture(requestor : *const ::core::ffi::c_void, mappedaddress : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmdevicehost.dll" "system" fn HdvDestroySectionBackedMmioRange(requestor : *const ::core::ffi::c_void, barindex : HDV_PCI_BAR_SELECTOR, offsetinpages : u64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_HostComputeSystem")]
::windows_targets::link!("vmdevicehost.dll" "system" #[doc = "Required features: `\"Win32_System_HostComputeSystem\"`"] fn HdvInitializeDeviceHost(computesystem : super::HostComputeSystem:: HCS_SYSTEM, devicehosthandle : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_HostComputeSystem")]
::windows_targets::link!("vmdevicehost.dll" "system" #[doc = "Required features: `\"Win32_System_HostComputeSystem\"`"] fn HdvInitializeDeviceHostEx(computesystem : super::HostComputeSystem:: HCS_SYSTEM, flags : HDV_DEVICE_HOST_FLAGS, devicehosthandle : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmdevicehost.dll" "system" fn HdvReadGuestMemory(requestor : *const ::core::ffi::c_void, guestphysicaladdress : u64, bytecount : u32, buffer : *mut u8) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmdevicehost.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HdvRegisterDoorbell(requestor : *const ::core::ffi::c_void, barindex : HDV_PCI_BAR_SELECTOR, baroffset : u64, triggervalue : u64, flags : u64, doorbellevent : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmdevicehost.dll" "system" fn HdvTeardownDeviceHost(devicehosthandle : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmdevicehost.dll" "system" fn HdvUnregisterDoorbell(requestor : *const ::core::ffi::c_void, barindex : HDV_PCI_BAR_SELECTOR, baroffset : u64, triggervalue : u64, flags : u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmdevicehost.dll" "system" fn HdvWriteGuestMemory(requestor : *const ::core::ffi::c_void, guestphysicaladdress : u64, bytecount : u32, buffer : *const u8) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InKernelSpace(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, inkernelspace : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsActiveVirtualTrustLevelEnabled(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, activevirtualtrustlevelenabled : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsNestedVirtualizationEnabled(vmsavedstatedumphandle : *mut ::core::ffi::c_void, enabled : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn LoadSavedStateFile(vmrsfile : ::windows_sys::core::PCWSTR, vmsavedstatedumphandle : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn LoadSavedStateFiles(binfile : ::windows_sys::core::PCWSTR, vsvfile : ::windows_sys::core::PCWSTR, vmsavedstatedumphandle : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn LoadSavedStateModuleSymbols(vmsavedstatedumphandle : *mut ::core::ffi::c_void, imagename : ::windows_sys::core::PCSTR, modulename : ::windows_sys::core::PCSTR, baseaddress : u64, sizeofbase : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn LoadSavedStateModuleSymbolsEx(vmsavedstatedumphandle : *mut ::core::ffi::c_void, imagename : ::windows_sys::core::PCSTR, imagetimestamp : u32, modulename : ::windows_sys::core::PCSTR, baseaddress : u64, sizeofbase : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadSavedStateSymbolProvider(vmsavedstatedumphandle : *mut ::core::ffi::c_void, usersymbols : ::windows_sys::core::PCWSTR, force : super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn LocateSavedStateFiles(vmname : ::windows_sys::core::PCWSTR, snapshotname : ::windows_sys::core::PCWSTR, binpath : *mut ::windows_sys::core::PWSTR, vsvpath : *mut ::windows_sys::core::PWSTR, vmrspath : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ReadGuestPhysicalAddress(vmsavedstatedumphandle : *mut ::core::ffi::c_void, physicaladdress : u64, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesread : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ReadGuestRawSavedMemory(vmsavedstatedumphandle : *mut ::core::ffi::c_void, rawsavedmemoryoffset : u64, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesread : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ReadSavedStateGlobalVariable(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, globalname : ::windows_sys::core::PCSTR, buffer : *mut ::core::ffi::c_void, buffersize : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ReleaseSavedStateFiles(vmsavedstatedumphandle : *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ReleaseSavedStateSymbolProvider(vmsavedstatedumphandle : *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn ResolveSavedStateGlobalVariableAddress(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, globalname : ::windows_sys::core::PCSTR, virtualaddress : *mut u64, size : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ScanMemoryForDosImages(vmsavedstatedumphandle : *mut ::core::ffi::c_void, vpid : u32, startaddress : u64, endaddress : u64, callbackcontext : *mut ::core::ffi::c_void, foundimagecallback : FOUND_IMAGE_CALLBACK, standaloneaddress : *const u64, standaloneaddresscount : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn SetMemoryBlockCacheLimit(vmsavedstatedumphandle : *mut ::core::ffi::c_void, memoryblockcachelimit : u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("vmsavedstatedumpprovider.dll" "system" fn SetSavedStateSymbolProviderDebugInfoCallback(vmsavedstatedumphandle : *mut ::core::ffi::c_void, callback : GUEST_SYMBOLS_PROVIDER_DEBUG_INFO_CALLBACK) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WHvAcceptPartitionMigration(migrationhandle : super::super::Foundation:: HANDLE, partition : *mut WHV_PARTITION_HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvAdviseGpaRange(partition : WHV_PARTITION_HANDLE, gparanges : *const WHV_MEMORY_RANGE_ENTRY, gparangescount : u32, advice : WHV_ADVISE_GPA_RANGE_CODE, advicebuffer : *const ::core::ffi::c_void, advicebuffersizeinbytes : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WHvAllocateVpciResource(providerid : *const ::windows_sys::core::GUID, flags : WHV_ALLOCATE_VPCI_RESOURCE_FLAGS, resourcedescriptor : *const ::core::ffi::c_void, resourcedescriptorsizeinbytes : u32, vpciresource : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvCancelPartitionMigration(partition : WHV_PARTITION_HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvCancelRunVirtualProcessor(partition : WHV_PARTITION_HANDLE, vpindex : u32, flags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvCompletePartitionMigration(partition : WHV_PARTITION_HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WHvCreateNotificationPort(partition : WHV_PARTITION_HANDLE, parameters : *const WHV_NOTIFICATION_PORT_PARAMETERS, eventhandle : super::super::Foundation:: HANDLE, porthandle : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvCreatePartition(partition : *mut WHV_PARTITION_HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WHvCreateTrigger(partition : WHV_PARTITION_HANDLE, parameters : *const WHV_TRIGGER_PARAMETERS, triggerhandle : *mut *mut ::core::ffi::c_void, eventhandle : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvCreateVirtualProcessor(partition : WHV_PARTITION_HANDLE, vpindex : u32, flags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvCreateVirtualProcessor2(partition : WHV_PARTITION_HANDLE, vpindex : u32, properties : *const WHV_VIRTUAL_PROCESSOR_PROPERTY, propertycount : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WHvCreateVpciDevice(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, vpciresource : super::super::Foundation:: HANDLE, flags : WHV_CREATE_VPCI_DEVICE_FLAGS, notificationeventhandle : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvDeleteNotificationPort(partition : WHV_PARTITION_HANDLE, porthandle : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvDeletePartition(partition : WHV_PARTITION_HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvDeleteTrigger(partition : WHV_PARTITION_HANDLE, triggerhandle : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvDeleteVirtualProcessor(partition : WHV_PARTITION_HANDLE, vpindex : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvDeleteVpciDevice(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvemulation.dll" "system" fn WHvEmulatorCreateEmulator(callbacks : *const WHV_EMULATOR_CALLBACKS, emulator : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvemulation.dll" "system" fn WHvEmulatorDestroyEmulator(emulator : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvemulation.dll" "system" fn WHvEmulatorTryIoEmulation(emulator : *const ::core::ffi::c_void, context : *const ::core::ffi::c_void, vpcontext : *const WHV_VP_EXIT_CONTEXT, ioinstructioncontext : *const WHV_X64_IO_PORT_ACCESS_CONTEXT, emulatorreturnstatus : *mut WHV_EMULATOR_STATUS) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvemulation.dll" "system" fn WHvEmulatorTryMmioEmulation(emulator : *const ::core::ffi::c_void, context : *const ::core::ffi::c_void, vpcontext : *const WHV_VP_EXIT_CONTEXT, mmioinstructioncontext : *const WHV_MEMORY_ACCESS_CONTEXT, emulatorreturnstatus : *mut WHV_EMULATOR_STATUS) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetCapability(capabilitycode : WHV_CAPABILITY_CODE, capabilitybuffer : *mut ::core::ffi::c_void, capabilitybuffersizeinbytes : u32, writtensizeinbytes : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetInterruptTargetVpSet(partition : WHV_PARTITION_HANDLE, destination : u64, destinationmode : WHV_INTERRUPT_DESTINATION_MODE, targetvps : *mut u32, vpcount : u32, targetvpcount : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetPartitionCounters(partition : WHV_PARTITION_HANDLE, counterset : WHV_PARTITION_COUNTER_SET, buffer : *mut ::core::ffi::c_void, buffersizeinbytes : u32, byteswritten : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetPartitionProperty(partition : WHV_PARTITION_HANDLE, propertycode : WHV_PARTITION_PROPERTY_CODE, propertybuffer : *mut ::core::ffi::c_void, propertybuffersizeinbytes : u32, writtensizeinbytes : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVirtualProcessorCounters(partition : WHV_PARTITION_HANDLE, vpindex : u32, counterset : WHV_PROCESSOR_COUNTER_SET, buffer : *mut ::core::ffi::c_void, buffersizeinbytes : u32, byteswritten : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVirtualProcessorCpuidOutput(partition : WHV_PARTITION_HANDLE, vpindex : u32, eax : u32, ecx : u32, cpuidoutput : *mut WHV_CPUID_OUTPUT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVirtualProcessorInterruptControllerState(partition : WHV_PARTITION_HANDLE, vpindex : u32, state : *mut ::core::ffi::c_void, statesize : u32, writtensize : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVirtualProcessorInterruptControllerState2(partition : WHV_PARTITION_HANDLE, vpindex : u32, state : *mut ::core::ffi::c_void, statesize : u32, writtensize : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVirtualProcessorRegisters(partition : WHV_PARTITION_HANDLE, vpindex : u32, registernames : *const WHV_REGISTER_NAME, registercount : u32, registervalues : *mut WHV_REGISTER_VALUE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVirtualProcessorState(partition : WHV_PARTITION_HANDLE, vpindex : u32, statetype : WHV_VIRTUAL_PROCESSOR_STATE_TYPE, buffer : *mut ::core::ffi::c_void, buffersizeinbytes : u32, byteswritten : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVirtualProcessorXsaveState(partition : WHV_PARTITION_HANDLE, vpindex : u32, buffer : *mut ::core::ffi::c_void, buffersizeinbytes : u32, byteswritten : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVpciDeviceInterruptTarget(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, index : u32, multimessagenumber : u32, target : *mut WHV_VPCI_INTERRUPT_TARGET, targetsizeinbytes : u32, byteswritten : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVpciDeviceNotification(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, notification : *mut WHV_VPCI_DEVICE_NOTIFICATION, notificationsizeinbytes : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvGetVpciDeviceProperty(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, propertycode : WHV_VPCI_DEVICE_PROPERTY_CODE, propertybuffer : *mut ::core::ffi::c_void, propertybuffersizeinbytes : u32, writtensizeinbytes : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvMapGpaRange(partition : WHV_PARTITION_HANDLE, sourceaddress : *const ::core::ffi::c_void, guestaddress : u64, sizeinbytes : u64, flags : WHV_MAP_GPA_RANGE_FLAGS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WHvMapGpaRange2(partition : WHV_PARTITION_HANDLE, process : super::super::Foundation:: HANDLE, sourceaddress : *const ::core::ffi::c_void, guestaddress : u64, sizeinbytes : u64, flags : WHV_MAP_GPA_RANGE_FLAGS) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvMapVpciDeviceInterrupt(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, index : u32, messagecount : u32, target : *const WHV_VPCI_INTERRUPT_TARGET, msiaddress : *mut u64, msidata : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvMapVpciDeviceMmioRanges(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, mappingcount : *mut u32, mappings : *mut *mut WHV_VPCI_MMIO_MAPPING) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvPostVirtualProcessorSynicMessage(partition : WHV_PARTITION_HANDLE, vpindex : u32, sintindex : u32, message : *const ::core::ffi::c_void, messagesizeinbytes : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvQueryGpaRangeDirtyBitmap(partition : WHV_PARTITION_HANDLE, guestaddress : u64, rangesizeinbytes : u64, bitmap : *mut u64, bitmapsizeinbytes : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvReadGpaRange(partition : WHV_PARTITION_HANDLE, vpindex : u32, guestaddress : u64, controls : WHV_ACCESS_GPA_CONTROLS, data : *mut ::core::ffi::c_void, datasizeinbytes : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvReadVpciDeviceRegister(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, register : *const WHV_VPCI_DEVICE_REGISTER, data : *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WHvRegisterPartitionDoorbellEvent(partition : WHV_PARTITION_HANDLE, matchdata : *const WHV_DOORBELL_MATCH_DATA, eventhandle : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvRequestInterrupt(partition : WHV_PARTITION_HANDLE, interrupt : *const WHV_INTERRUPT_CONTROL, interruptcontrolsize : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvRequestVpciDeviceInterrupt(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, msiaddress : u64, msidata : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvResetPartition(partition : WHV_PARTITION_HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvResumePartitionTime(partition : WHV_PARTITION_HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvRetargetVpciDeviceInterrupt(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, msiaddress : u64, msidata : u32, target : *const WHV_VPCI_INTERRUPT_TARGET) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvRunVirtualProcessor(partition : WHV_PARTITION_HANDLE, vpindex : u32, exitcontext : *mut ::core::ffi::c_void, exitcontextsizeinbytes : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvSetNotificationPortProperty(partition : WHV_PARTITION_HANDLE, porthandle : *const ::core::ffi::c_void, propertycode : WHV_NOTIFICATION_PORT_PROPERTY_CODE, propertyvalue : u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvSetPartitionProperty(partition : WHV_PARTITION_HANDLE, propertycode : WHV_PARTITION_PROPERTY_CODE, propertybuffer : *const ::core::ffi::c_void, propertybuffersizeinbytes : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvSetVirtualProcessorInterruptControllerState(partition : WHV_PARTITION_HANDLE, vpindex : u32, state : *const ::core::ffi::c_void, statesize : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvSetVirtualProcessorInterruptControllerState2(partition : WHV_PARTITION_HANDLE, vpindex : u32, state : *const ::core::ffi::c_void, statesize : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvSetVirtualProcessorRegisters(partition : WHV_PARTITION_HANDLE, vpindex : u32, registernames : *const WHV_REGISTER_NAME, registercount : u32, registervalues : *const WHV_REGISTER_VALUE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvSetVirtualProcessorState(partition : WHV_PARTITION_HANDLE, vpindex : u32, statetype : WHV_VIRTUAL_PROCESSOR_STATE_TYPE, buffer : *const ::core::ffi::c_void, buffersizeinbytes : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvSetVirtualProcessorXsaveState(partition : WHV_PARTITION_HANDLE, vpindex : u32, buffer : *const ::core::ffi::c_void, buffersizeinbytes : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Power")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_System_Power\"`"] fn WHvSetVpciDevicePowerState(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, powerstate : super::Power:: DEVICE_POWER_STATE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvSetupPartition(partition : WHV_PARTITION_HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WHvSignalVirtualProcessorSynicEvent(partition : WHV_PARTITION_HANDLE, synicevent : WHV_SYNIC_EVENT_PARAMETERS, newlysignaled : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("winhvplatform.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WHvStartPartitionMigration(partition : WHV_PARTITION_HANDLE, migrationhandle : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvSuspendPartitionTime(partition : WHV_PARTITION_HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvTranslateGva(partition : WHV_PARTITION_HANDLE, vpindex : u32, gva : u64, translateflags : WHV_TRANSLATE_GVA_FLAGS, translationresult : *mut WHV_TRANSLATE_GVA_RESULT, gpa : *mut u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvUnmapGpaRange(partition : WHV_PARTITION_HANDLE, guestaddress : u64, sizeinbytes : u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvUnmapVpciDeviceInterrupt(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, index : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvUnmapVpciDeviceMmioRanges(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvUnregisterPartitionDoorbellEvent(partition : WHV_PARTITION_HANDLE, matchdata : *const WHV_DOORBELL_MATCH_DATA) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvUpdateTriggerParameters(partition : WHV_PARTITION_HANDLE, parameters : *const WHV_TRIGGER_PARAMETERS, triggerhandle : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvWriteGpaRange(partition : WHV_PARTITION_HANDLE, vpindex : u32, guestaddress : u64, controls : WHV_ACCESS_GPA_CONTROLS, data : *const ::core::ffi::c_void, datasizeinbytes : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("winhvplatform.dll" "system" fn WHvWriteVpciDeviceRegister(partition : WHV_PARTITION_HANDLE, logicaldeviceid : u64, register : *const WHV_VPCI_DEVICE_REGISTER, data : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
pub const ARM64_RegisterActlrEl1: REGISTER_ID = 145i32;
pub const ARM64_RegisterAmairEl1: REGISTER_ID = 148i32;
pub const ARM64_RegisterCntkctlEl1: REGISTER_ID = 155i32;
pub const ARM64_RegisterCntvCtlEl0: REGISTER_ID = 157i32;
pub const ARM64_RegisterCntvCvalEl0: REGISTER_ID = 156i32;
pub const ARM64_RegisterContextIdrEl1: REGISTER_ID = 152i32;
pub const ARM64_RegisterCpacrEl1: REGISTER_ID = 153i32;
pub const ARM64_RegisterCpsr: REGISTER_ID = 101i32;
pub const ARM64_RegisterCsselrEl1: REGISTER_ID = 154i32;
pub const ARM64_RegisterElrEl1: REGISTER_ID = 140i32;
pub const ARM64_RegisterEsrEl1: REGISTER_ID = 136i32;
pub const ARM64_RegisterFarEl1: REGISTER_ID = 138i32;
pub const ARM64_RegisterFpControl: REGISTER_ID = 135i32;
pub const ARM64_RegisterFpStatus: REGISTER_ID = 134i32;
pub const ARM64_RegisterMairEl1: REGISTER_ID = 147i32;
pub const ARM64_RegisterMax: REGISTER_ID = 158i32;
pub const ARM64_RegisterParEl1: REGISTER_ID = 139i32;
pub const ARM64_RegisterPc: REGISTER_ID = 98i32;
pub const ARM64_RegisterQ0: REGISTER_ID = 102i32;
pub const ARM64_RegisterQ1: REGISTER_ID = 103i32;
pub const ARM64_RegisterQ10: REGISTER_ID = 112i32;
pub const ARM64_RegisterQ11: REGISTER_ID = 113i32;
pub const ARM64_RegisterQ12: REGISTER_ID = 114i32;
pub const ARM64_RegisterQ13: REGISTER_ID = 115i32;
pub const ARM64_RegisterQ14: REGISTER_ID = 116i32;
pub const ARM64_RegisterQ15: REGISTER_ID = 117i32;
pub const ARM64_RegisterQ16: REGISTER_ID = 118i32;
pub const ARM64_RegisterQ17: REGISTER_ID = 119i32;
pub const ARM64_RegisterQ18: REGISTER_ID = 120i32;
pub const ARM64_RegisterQ19: REGISTER_ID = 121i32;
pub const ARM64_RegisterQ2: REGISTER_ID = 104i32;
pub const ARM64_RegisterQ20: REGISTER_ID = 122i32;
pub const ARM64_RegisterQ21: REGISTER_ID = 123i32;
pub const ARM64_RegisterQ22: REGISTER_ID = 124i32;
pub const ARM64_RegisterQ23: REGISTER_ID = 125i32;
pub const ARM64_RegisterQ24: REGISTER_ID = 126i32;
pub const ARM64_RegisterQ25: REGISTER_ID = 127i32;
pub const ARM64_RegisterQ26: REGISTER_ID = 128i32;
pub const ARM64_RegisterQ27: REGISTER_ID = 129i32;
pub const ARM64_RegisterQ28: REGISTER_ID = 130i32;
pub const ARM64_RegisterQ29: REGISTER_ID = 131i32;
pub const ARM64_RegisterQ3: REGISTER_ID = 105i32;
pub const ARM64_RegisterQ30: REGISTER_ID = 132i32;
pub const ARM64_RegisterQ31: REGISTER_ID = 133i32;
pub const ARM64_RegisterQ4: REGISTER_ID = 106i32;
pub const ARM64_RegisterQ5: REGISTER_ID = 107i32;
pub const ARM64_RegisterQ6: REGISTER_ID = 108i32;
pub const ARM64_RegisterQ7: REGISTER_ID = 109i32;
pub const ARM64_RegisterQ8: REGISTER_ID = 110i32;
pub const ARM64_RegisterQ9: REGISTER_ID = 111i32;
pub const ARM64_RegisterSctlrEl1: REGISTER_ID = 144i32;
pub const ARM64_RegisterSpEl0: REGISTER_ID = 99i32;
pub const ARM64_RegisterSpEl1: REGISTER_ID = 100i32;
pub const ARM64_RegisterSpsrEl1: REGISTER_ID = 137i32;
pub const ARM64_RegisterTcrEl1: REGISTER_ID = 146i32;
pub const ARM64_RegisterTpidrEl0: REGISTER_ID = 149i32;
pub const ARM64_RegisterTpidrEl1: REGISTER_ID = 151i32;
pub const ARM64_RegisterTpidrroEl0: REGISTER_ID = 150i32;
pub const ARM64_RegisterTtbr0El1: REGISTER_ID = 141i32;
pub const ARM64_RegisterTtbr1El1: REGISTER_ID = 142i32;
pub const ARM64_RegisterVbarEl1: REGISTER_ID = 143i32;
pub const ARM64_RegisterX0: REGISTER_ID = 67i32;
pub const ARM64_RegisterX1: REGISTER_ID = 68i32;
pub const ARM64_RegisterX10: REGISTER_ID = 77i32;
pub const ARM64_RegisterX11: REGISTER_ID = 78i32;
pub const ARM64_RegisterX12: REGISTER_ID = 79i32;
pub const ARM64_RegisterX13: REGISTER_ID = 80i32;
pub const ARM64_RegisterX14: REGISTER_ID = 81i32;
pub const ARM64_RegisterX15: REGISTER_ID = 82i32;
pub const ARM64_RegisterX16: REGISTER_ID = 83i32;
pub const ARM64_RegisterX17: REGISTER_ID = 84i32;
pub const ARM64_RegisterX18: REGISTER_ID = 85i32;
pub const ARM64_RegisterX19: REGISTER_ID = 86i32;
pub const ARM64_RegisterX2: REGISTER_ID = 69i32;
pub const ARM64_RegisterX20: REGISTER_ID = 87i32;
pub const ARM64_RegisterX21: REGISTER_ID = 88i32;
pub const ARM64_RegisterX22: REGISTER_ID = 89i32;
pub const ARM64_RegisterX23: REGISTER_ID = 90i32;
pub const ARM64_RegisterX24: REGISTER_ID = 91i32;
pub const ARM64_RegisterX25: REGISTER_ID = 92i32;
pub const ARM64_RegisterX26: REGISTER_ID = 93i32;
pub const ARM64_RegisterX27: REGISTER_ID = 94i32;
pub const ARM64_RegisterX28: REGISTER_ID = 95i32;
pub const ARM64_RegisterX3: REGISTER_ID = 70i32;
pub const ARM64_RegisterX4: REGISTER_ID = 71i32;
pub const ARM64_RegisterX5: REGISTER_ID = 72i32;
pub const ARM64_RegisterX6: REGISTER_ID = 73i32;
pub const ARM64_RegisterX7: REGISTER_ID = 74i32;
pub const ARM64_RegisterX8: REGISTER_ID = 75i32;
pub const ARM64_RegisterX9: REGISTER_ID = 76i32;
pub const ARM64_RegisterXFp: REGISTER_ID = 96i32;
pub const ARM64_RegisterXLr: REGISTER_ID = 97i32;
pub const Arch_Armv8: VIRTUAL_PROCESSOR_ARCH = 3i32;
pub const Arch_Unknown: VIRTUAL_PROCESSOR_ARCH = 0i32;
pub const Arch_x64: VIRTUAL_PROCESSOR_ARCH = 2i32;
pub const Arch_x86: VIRTUAL_PROCESSOR_ARCH = 1i32;
pub const GUID_DEVINTERFACE_VM_GENCOUNTER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3ff2c92b_6598_4e60_8e1c_0ccf4927e319);
pub const GuestOsMicrosoftMSDOS: GUEST_OS_MICROSOFT_IDS = 1i32;
pub const GuestOsMicrosoftUndefined: GUEST_OS_MICROSOFT_IDS = 0i32;
pub const GuestOsMicrosoftWindows3x: GUEST_OS_MICROSOFT_IDS = 2i32;
pub const GuestOsMicrosoftWindows9x: GUEST_OS_MICROSOFT_IDS = 3i32;
pub const GuestOsMicrosoftWindowsCE: GUEST_OS_MICROSOFT_IDS = 5i32;
pub const GuestOsMicrosoftWindowsNT: GUEST_OS_MICROSOFT_IDS = 4i32;
pub const GuestOsOpenSourceFreeBSD: GUEST_OS_OPENSOURCE_IDS = 2i32;
pub const GuestOsOpenSourceIllumos: GUEST_OS_OPENSOURCE_IDS = 4i32;
pub const GuestOsOpenSourceLinux: GUEST_OS_OPENSOURCE_IDS = 1i32;
pub const GuestOsOpenSourceUndefined: GUEST_OS_OPENSOURCE_IDS = 0i32;
pub const GuestOsOpenSourceXen: GUEST_OS_OPENSOURCE_IDS = 3i32;
pub const GuestOsVendorHPE: GUEST_OS_VENDOR = 2i32;
pub const GuestOsVendorLANCOM: GUEST_OS_VENDOR = 512i32;
pub const GuestOsVendorMicrosoft: GUEST_OS_VENDOR = 1i32;
pub const GuestOsVendorUndefined: GUEST_OS_VENDOR = 0i32;
pub const HDV_DOORBELL_FLAG_TRIGGER_ANY_VALUE: HDV_DOORBELL_FLAGS = -2147483648i32;
pub const HDV_DOORBELL_FLAG_TRIGGER_SIZE_ANY: HDV_DOORBELL_FLAGS = 0i32;
pub const HDV_DOORBELL_FLAG_TRIGGER_SIZE_BYTE: HDV_DOORBELL_FLAGS = 1i32;
pub const HDV_DOORBELL_FLAG_TRIGGER_SIZE_DWORD: HDV_DOORBELL_FLAGS = 3i32;
pub const HDV_DOORBELL_FLAG_TRIGGER_SIZE_QWORD: HDV_DOORBELL_FLAGS = 4i32;
pub const HDV_DOORBELL_FLAG_TRIGGER_SIZE_WORD: HDV_DOORBELL_FLAGS = 2i32;
pub const HDV_PCI_BAR0: HDV_PCI_BAR_SELECTOR = 0i32;
pub const HDV_PCI_BAR1: HDV_PCI_BAR_SELECTOR = 1i32;
pub const HDV_PCI_BAR2: HDV_PCI_BAR_SELECTOR = 2i32;
pub const HDV_PCI_BAR3: HDV_PCI_BAR_SELECTOR = 3i32;
pub const HDV_PCI_BAR4: HDV_PCI_BAR_SELECTOR = 4i32;
pub const HDV_PCI_BAR5: HDV_PCI_BAR_SELECTOR = 5i32;
pub const HDV_PCI_BAR_COUNT: u32 = 6u32;
pub const HVSOCKET_ADDRESS_FLAG_PASSTHRU: u32 = 1u32;
pub const HVSOCKET_CONNECTED_SUSPEND: u32 = 4u32;
pub const HVSOCKET_CONNECT_TIMEOUT: u32 = 1u32;
pub const HVSOCKET_CONNECT_TIMEOUT_MAX: u32 = 300000u32;
pub const HVSOCKET_HIGH_VTL: u32 = 8u32;
pub const HV_GUID_BROADCAST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xffffffff_ffff_ffff_ffff_ffffffffffff);
pub const HV_GUID_CHILDREN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x90db8b89_0d35_4f79_8ce9_49ea0ac8b7cd);
pub const HV_GUID_LOOPBACK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe0e16197_dd56_4a10_9195_5ee7a155a838);
pub const HV_GUID_PARENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa42e7cda_d03f_480c_9cc2_a4de20abb878);
pub const HV_GUID_SILOHOST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x36bd0c5c_7276_4223_88ba_7d03b654c568);
pub const HV_GUID_VSOCK_TEMPLATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000000_facb_11e6_bd58_64006a7986d3);
pub const HV_GUID_ZERO: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000000_0000_0000_0000_000000000000);
pub const HV_PROTOCOL_RAW: u32 = 1u32;
pub const HdvDeviceHostFlagInitializeComSecurity: HDV_DEVICE_HOST_FLAGS = 1i32;
pub const HdvDeviceHostFlagNone: HDV_DEVICE_HOST_FLAGS = 0i32;
pub const HdvDeviceTypePCI: HDV_DEVICE_TYPE = 1i32;
pub const HdvDeviceTypeUndefined: HDV_DEVICE_TYPE = 0i32;
pub const HdvMmioMappingFlagExecutable: HDV_MMIO_MAPPING_FLAGS = 2i32;
pub const HdvMmioMappingFlagNone: HDV_MMIO_MAPPING_FLAGS = 0i32;
pub const HdvMmioMappingFlagWriteable: HDV_MMIO_MAPPING_FLAGS = 1i32;
pub const HdvPciDeviceInterfaceVersion1: HDV_PCI_INTERFACE_VERSION = 1i32;
pub const HdvPciDeviceInterfaceVersionInvalid: HDV_PCI_INTERFACE_VERSION = 0i32;
pub const IOCTL_VMGENCOUNTER_READ: u32 = 3325956u32;
pub const Paging_32Bit: PAGING_MODE = 2i32;
pub const Paging_Armv8: PAGING_MODE = 5i32;
pub const Paging_Invalid: PAGING_MODE = 0i32;
pub const Paging_Long: PAGING_MODE = 4i32;
pub const Paging_NonPaged: PAGING_MODE = 1i32;
pub const Paging_Pae: PAGING_MODE = 3i32;
pub const ProcessorVendor_Amd: VIRTUAL_PROCESSOR_VENDOR = 1i32;
pub const ProcessorVendor_Arm: VIRTUAL_PROCESSOR_VENDOR = 4i32;
pub const ProcessorVendor_Hygon: VIRTUAL_PROCESSOR_VENDOR = 3i32;
pub const ProcessorVendor_Intel: VIRTUAL_PROCESSOR_VENDOR = 2i32;
pub const ProcessorVendor_Unknown: VIRTUAL_PROCESSOR_VENDOR = 0i32;
pub const VM_GENCOUNTER_SYMBOLIC_LINK_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("\\VmGenerationCounter");
pub const WHV_ANY_VP: u32 = 4294967295u32;
pub const WHV_HYPERCALL_CONTEXT_MAX_XMM_REGISTERS: u32 = 6u32;
pub const WHV_MAX_DEVICE_ID_SIZE_IN_CHARS: u32 = 200u32;
pub const WHV_PROCESSOR_FEATURES_BANKS_COUNT: u32 = 2u32;
pub const WHV_READ_WRITE_GPA_RANGE_MAX_SIZE: u32 = 16u32;
pub const WHV_SYNIC_MESSAGE_SIZE: u32 = 256u32;
pub const WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS_COUNT: u32 = 1u32;
pub const WHV_VPCI_TYPE0_BAR_COUNT: u32 = 6u32;
pub const WHvAdviseGpaRangeCodePin: WHV_ADVISE_GPA_RANGE_CODE = 1i32;
pub const WHvAdviseGpaRangeCodePopulate: WHV_ADVISE_GPA_RANGE_CODE = 0i32;
pub const WHvAdviseGpaRangeCodeUnpin: WHV_ADVISE_GPA_RANGE_CODE = 2i32;
pub const WHvAllocateVpciResourceFlagAllowDirectP2P: WHV_ALLOCATE_VPCI_RESOURCE_FLAGS = 1i32;
pub const WHvAllocateVpciResourceFlagNone: WHV_ALLOCATE_VPCI_RESOURCE_FLAGS = 0i32;
pub const WHvCacheTypeUncached: WHV_CACHE_TYPE = 0i32;
pub const WHvCacheTypeWriteBack: WHV_CACHE_TYPE = 6i32;
pub const WHvCacheTypeWriteCombining: WHV_CACHE_TYPE = 1i32;
pub const WHvCacheTypeWriteThrough: WHV_CACHE_TYPE = 4i32;
pub const WHvCapabilityCodeExceptionExitBitmap: WHV_CAPABILITY_CODE = 3i32;
pub const WHvCapabilityCodeExtendedVmExits: WHV_CAPABILITY_CODE = 2i32;
pub const WHvCapabilityCodeFeatures: WHV_CAPABILITY_CODE = 1i32;
pub const WHvCapabilityCodeGpaRangePopulateFlags: WHV_CAPABILITY_CODE = 5i32;
pub const WHvCapabilityCodeHypervisorPresent: WHV_CAPABILITY_CODE = 0i32;
pub const WHvCapabilityCodeInterruptClockFrequency: WHV_CAPABILITY_CODE = 4101i32;
pub const WHvCapabilityCodeProcessorClFlushSize: WHV_CAPABILITY_CODE = 4098i32;
pub const WHvCapabilityCodeProcessorClockFrequency: WHV_CAPABILITY_CODE = 4100i32;
pub const WHvCapabilityCodeProcessorFeatures: WHV_CAPABILITY_CODE = 4097i32;
pub const WHvCapabilityCodeProcessorFeaturesBanks: WHV_CAPABILITY_CODE = 4102i32;
pub const WHvCapabilityCodeProcessorFrequencyCap: WHV_CAPABILITY_CODE = 4103i32;
pub const WHvCapabilityCodeProcessorPerfmonFeatures: WHV_CAPABILITY_CODE = 4105i32;
pub const WHvCapabilityCodeProcessorVendor: WHV_CAPABILITY_CODE = 4096i32;
pub const WHvCapabilityCodeProcessorXsaveFeatures: WHV_CAPABILITY_CODE = 4099i32;
pub const WHvCapabilityCodeSchedulerFeatures: WHV_CAPABILITY_CODE = 6i32;
pub const WHvCapabilityCodeSyntheticProcessorFeaturesBanks: WHV_CAPABILITY_CODE = 4104i32;
pub const WHvCapabilityCodeX64MsrExitBitmap: WHV_CAPABILITY_CODE = 4i32;
pub const WHvCreateVpciDeviceFlagNone: WHV_CREATE_VPCI_DEVICE_FLAGS = 0i32;
pub const WHvCreateVpciDeviceFlagPhysicallyBacked: WHV_CREATE_VPCI_DEVICE_FLAGS = 1i32;
pub const WHvCreateVpciDeviceFlagUseLogicalInterrupts: WHV_CREATE_VPCI_DEVICE_FLAGS = 2i32;
pub const WHvMapGpaRangeFlagExecute: WHV_MAP_GPA_RANGE_FLAGS = 4i32;
pub const WHvMapGpaRangeFlagNone: WHV_MAP_GPA_RANGE_FLAGS = 0i32;
pub const WHvMapGpaRangeFlagRead: WHV_MAP_GPA_RANGE_FLAGS = 1i32;
pub const WHvMapGpaRangeFlagTrackDirtyPages: WHV_MAP_GPA_RANGE_FLAGS = 8i32;
pub const WHvMapGpaRangeFlagWrite: WHV_MAP_GPA_RANGE_FLAGS = 2i32;
pub const WHvMemoryAccessExecute: WHV_MEMORY_ACCESS_TYPE = 2i32;
pub const WHvMemoryAccessRead: WHV_MEMORY_ACCESS_TYPE = 0i32;
pub const WHvMemoryAccessWrite: WHV_MEMORY_ACCESS_TYPE = 1i32;
pub const WHvMsrActionArchitectureDefault: WHV_MSR_ACTION = 0i32;
pub const WHvMsrActionExit: WHV_MSR_ACTION = 2i32;
pub const WHvMsrActionIgnoreWriteReadZero: WHV_MSR_ACTION = 1i32;
pub const WHvNotificationPortPropertyPreferredTargetDuration: WHV_NOTIFICATION_PORT_PROPERTY_CODE = 5i32;
pub const WHvNotificationPortPropertyPreferredTargetVp: WHV_NOTIFICATION_PORT_PROPERTY_CODE = 1i32;
pub const WHvNotificationPortTypeDoorbell: WHV_NOTIFICATION_PORT_TYPE = 4i32;
pub const WHvNotificationPortTypeEvent: WHV_NOTIFICATION_PORT_TYPE = 2i32;
pub const WHvPartitionCounterSetMemory: WHV_PARTITION_COUNTER_SET = 0i32;
pub const WHvPartitionPropertyCodeAllowDeviceAssignment: WHV_PARTITION_PROPERTY_CODE = 12i32;
pub const WHvPartitionPropertyCodeApicRemoteReadSupport: WHV_PARTITION_PROPERTY_CODE = 4105i32;
pub const WHvPartitionPropertyCodeCpuCap: WHV_PARTITION_PROPERTY_CODE = 8i32;
pub const WHvPartitionPropertyCodeCpuGroupId: WHV_PARTITION_PROPERTY_CODE = 10i32;
pub const WHvPartitionPropertyCodeCpuReserve: WHV_PARTITION_PROPERTY_CODE = 7i32;
pub const WHvPartitionPropertyCodeCpuWeight: WHV_PARTITION_PROPERTY_CODE = 9i32;
pub const WHvPartitionPropertyCodeCpuidExitList: WHV_PARTITION_PROPERTY_CODE = 4099i32;
pub const WHvPartitionPropertyCodeCpuidResultList: WHV_PARTITION_PROPERTY_CODE = 4100i32;
pub const WHvPartitionPropertyCodeCpuidResultList2: WHV_PARTITION_PROPERTY_CODE = 4109i32;
pub const WHvPartitionPropertyCodeDisableSmt: WHV_PARTITION_PROPERTY_CODE = 13i32;
pub const WHvPartitionPropertyCodeExceptionExitBitmap: WHV_PARTITION_PROPERTY_CODE = 2i32;
pub const WHvPartitionPropertyCodeExtendedVmExits: WHV_PARTITION_PROPERTY_CODE = 1i32;
pub const WHvPartitionPropertyCodeInterruptClockFrequency: WHV_PARTITION_PROPERTY_CODE = 4104i32;
pub const WHvPartitionPropertyCodeLocalApicEmulationMode: WHV_PARTITION_PROPERTY_CODE = 4101i32;
pub const WHvPartitionPropertyCodeMsrActionList: WHV_PARTITION_PROPERTY_CODE = 4111i32;
pub const WHvPartitionPropertyCodeNestedVirtualization: WHV_PARTITION_PROPERTY_CODE = 4i32;
pub const WHvPartitionPropertyCodePrimaryNumaNode: WHV_PARTITION_PROPERTY_CODE = 6i32;
pub const WHvPartitionPropertyCodeProcessorClFlushSize: WHV_PARTITION_PROPERTY_CODE = 4098i32;
pub const WHvPartitionPropertyCodeProcessorClockFrequency: WHV_PARTITION_PROPERTY_CODE = 4103i32;
pub const WHvPartitionPropertyCodeProcessorCount: WHV_PARTITION_PROPERTY_CODE = 8191i32;
pub const WHvPartitionPropertyCodeProcessorFeatures: WHV_PARTITION_PROPERTY_CODE = 4097i32;
pub const WHvPartitionPropertyCodeProcessorFeaturesBanks: WHV_PARTITION_PROPERTY_CODE = 4106i32;
pub const WHvPartitionPropertyCodeProcessorFrequencyCap: WHV_PARTITION_PROPERTY_CODE = 11i32;
pub const WHvPartitionPropertyCodeProcessorPerfmonFeatures: WHV_PARTITION_PROPERTY_CODE = 4110i32;
pub const WHvPartitionPropertyCodeProcessorXsaveFeatures: WHV_PARTITION_PROPERTY_CODE = 4102i32;
pub const WHvPartitionPropertyCodeReferenceTime: WHV_PARTITION_PROPERTY_CODE = 4107i32;
pub const WHvPartitionPropertyCodeSeparateSecurityDomain: WHV_PARTITION_PROPERTY_CODE = 3i32;
pub const WHvPartitionPropertyCodeSyntheticProcessorFeaturesBanks: WHV_PARTITION_PROPERTY_CODE = 4108i32;
pub const WHvPartitionPropertyCodeUnimplementedMsrAction: WHV_PARTITION_PROPERTY_CODE = 4112i32;
pub const WHvPartitionPropertyCodeX64MsrExitBitmap: WHV_PARTITION_PROPERTY_CODE = 5i32;
pub const WHvProcessorCounterSetApic: WHV_PROCESSOR_COUNTER_SET = 3i32;
pub const WHvProcessorCounterSetEvents: WHV_PROCESSOR_COUNTER_SET = 2i32;
pub const WHvProcessorCounterSetIntercepts: WHV_PROCESSOR_COUNTER_SET = 1i32;
pub const WHvProcessorCounterSetRuntime: WHV_PROCESSOR_COUNTER_SET = 0i32;
pub const WHvProcessorCounterSetSyntheticFeatures: WHV_PROCESSOR_COUNTER_SET = 4i32;
pub const WHvProcessorVendorAmd: WHV_PROCESSOR_VENDOR = 0i32;
pub const WHvProcessorVendorHygon: WHV_PROCESSOR_VENDOR = 2i32;
pub const WHvProcessorVendorIntel: WHV_PROCESSOR_VENDOR = 1i32;
pub const WHvRegisterEom: WHV_REGISTER_NAME = 16404i32;
pub const WHvRegisterGuestOsId: WHV_REGISTER_NAME = 20482i32;
pub const WHvRegisterInternalActivityState: WHV_REGISTER_NAME = -2147483643i32;
pub const WHvRegisterInterruptState: WHV_REGISTER_NAME = -2147483647i32;
pub const WHvRegisterPendingEvent: WHV_REGISTER_NAME = -2147483646i32;
pub const WHvRegisterPendingInterruption: WHV_REGISTER_NAME = -2147483648i32;
pub const WHvRegisterReferenceTsc: WHV_REGISTER_NAME = 20503i32;
pub const WHvRegisterReferenceTscSequence: WHV_REGISTER_NAME = 20506i32;
pub const WHvRegisterScontrol: WHV_REGISTER_NAME = 16400i32;
pub const WHvRegisterSiefp: WHV_REGISTER_NAME = 16402i32;
pub const WHvRegisterSimp: WHV_REGISTER_NAME = 16403i32;
pub const WHvRegisterSint0: WHV_REGISTER_NAME = 16384i32;
pub const WHvRegisterSint1: WHV_REGISTER_NAME = 16385i32;
pub const WHvRegisterSint10: WHV_REGISTER_NAME = 16394i32;
pub const WHvRegisterSint11: WHV_REGISTER_NAME = 16395i32;
pub const WHvRegisterSint12: WHV_REGISTER_NAME = 16396i32;
pub const WHvRegisterSint13: WHV_REGISTER_NAME = 16397i32;
pub const WHvRegisterSint14: WHV_REGISTER_NAME = 16398i32;
pub const WHvRegisterSint15: WHV_REGISTER_NAME = 16399i32;
pub const WHvRegisterSint2: WHV_REGISTER_NAME = 16386i32;
pub const WHvRegisterSint3: WHV_REGISTER_NAME = 16387i32;
pub const WHvRegisterSint4: WHV_REGISTER_NAME = 16388i32;
pub const WHvRegisterSint5: WHV_REGISTER_NAME = 16389i32;
pub const WHvRegisterSint6: WHV_REGISTER_NAME = 16390i32;
pub const WHvRegisterSint7: WHV_REGISTER_NAME = 16391i32;
pub const WHvRegisterSint8: WHV_REGISTER_NAME = 16392i32;
pub const WHvRegisterSint9: WHV_REGISTER_NAME = 16393i32;
pub const WHvRegisterSversion: WHV_REGISTER_NAME = 16401i32;
pub const WHvRegisterVpAssistPage: WHV_REGISTER_NAME = 20499i32;
pub const WHvRegisterVpRuntime: WHV_REGISTER_NAME = 20480i32;
pub const WHvRunVpCancelReasonUser: WHV_RUN_VP_CANCEL_REASON = 0i32;
pub const WHvRunVpExitReasonCanceled: WHV_RUN_VP_EXIT_REASON = 8193i32;
pub const WHvRunVpExitReasonException: WHV_RUN_VP_EXIT_REASON = 4098i32;
pub const WHvRunVpExitReasonHypercall: WHV_RUN_VP_EXIT_REASON = 4101i32;
pub const WHvRunVpExitReasonInvalidVpRegisterValue: WHV_RUN_VP_EXIT_REASON = 5i32;
pub const WHvRunVpExitReasonMemoryAccess: WHV_RUN_VP_EXIT_REASON = 1i32;
pub const WHvRunVpExitReasonNone: WHV_RUN_VP_EXIT_REASON = 0i32;
pub const WHvRunVpExitReasonSynicSintDeliverable: WHV_RUN_VP_EXIT_REASON = 10i32;
pub const WHvRunVpExitReasonUnrecoverableException: WHV_RUN_VP_EXIT_REASON = 4i32;
pub const WHvRunVpExitReasonUnsupportedFeature: WHV_RUN_VP_EXIT_REASON = 6i32;
pub const WHvRunVpExitReasonX64ApicEoi: WHV_RUN_VP_EXIT_REASON = 9i32;
pub const WHvRunVpExitReasonX64ApicInitSipiTrap: WHV_RUN_VP_EXIT_REASON = 4102i32;
pub const WHvRunVpExitReasonX64ApicSmiTrap: WHV_RUN_VP_EXIT_REASON = 4100i32;
pub const WHvRunVpExitReasonX64ApicWriteTrap: WHV_RUN_VP_EXIT_REASON = 4103i32;
pub const WHvRunVpExitReasonX64Cpuid: WHV_RUN_VP_EXIT_REASON = 4097i32;
pub const WHvRunVpExitReasonX64Halt: WHV_RUN_VP_EXIT_REASON = 8i32;
pub const WHvRunVpExitReasonX64InterruptWindow: WHV_RUN_VP_EXIT_REASON = 7i32;
pub const WHvRunVpExitReasonX64IoPortAccess: WHV_RUN_VP_EXIT_REASON = 2i32;
pub const WHvRunVpExitReasonX64MsrAccess: WHV_RUN_VP_EXIT_REASON = 4096i32;
pub const WHvRunVpExitReasonX64Rdtsc: WHV_RUN_VP_EXIT_REASON = 4099i32;
pub const WHvTranslateGvaFlagEnforceSmap: WHV_TRANSLATE_GVA_FLAGS = 256i32;
pub const WHvTranslateGvaFlagNone: WHV_TRANSLATE_GVA_FLAGS = 0i32;
pub const WHvTranslateGvaFlagOverrideSmap: WHV_TRANSLATE_GVA_FLAGS = 512i32;
pub const WHvTranslateGvaFlagPrivilegeExempt: WHV_TRANSLATE_GVA_FLAGS = 8i32;
pub const WHvTranslateGvaFlagSetPageTableBits: WHV_TRANSLATE_GVA_FLAGS = 16i32;
pub const WHvTranslateGvaFlagValidateExecute: WHV_TRANSLATE_GVA_FLAGS = 4i32;
pub const WHvTranslateGvaFlagValidateRead: WHV_TRANSLATE_GVA_FLAGS = 1i32;
pub const WHvTranslateGvaFlagValidateWrite: WHV_TRANSLATE_GVA_FLAGS = 2i32;
pub const WHvTranslateGvaResultGpaIllegalOverlayAccess: WHV_TRANSLATE_GVA_RESULT_CODE = 7i32;
pub const WHvTranslateGvaResultGpaNoReadAccess: WHV_TRANSLATE_GVA_RESULT_CODE = 5i32;
pub const WHvTranslateGvaResultGpaNoWriteAccess: WHV_TRANSLATE_GVA_RESULT_CODE = 6i32;
pub const WHvTranslateGvaResultGpaUnmapped: WHV_TRANSLATE_GVA_RESULT_CODE = 4i32;
pub const WHvTranslateGvaResultIntercept: WHV_TRANSLATE_GVA_RESULT_CODE = 8i32;
pub const WHvTranslateGvaResultInvalidPageTableFlags: WHV_TRANSLATE_GVA_RESULT_CODE = 3i32;
pub const WHvTranslateGvaResultPageNotPresent: WHV_TRANSLATE_GVA_RESULT_CODE = 1i32;
pub const WHvTranslateGvaResultPrivilegeViolation: WHV_TRANSLATE_GVA_RESULT_CODE = 2i32;
pub const WHvTranslateGvaResultSuccess: WHV_TRANSLATE_GVA_RESULT_CODE = 0i32;
pub const WHvTriggerTypeDeviceInterrupt: WHV_TRIGGER_TYPE = 2i32;
pub const WHvTriggerTypeInterrupt: WHV_TRIGGER_TYPE = 0i32;
pub const WHvTriggerTypeSynicEvent: WHV_TRIGGER_TYPE = 1i32;
pub const WHvUnsupportedFeatureIntercept: WHV_X64_UNSUPPORTED_FEATURE_CODE = 1i32;
pub const WHvUnsupportedFeatureTaskSwitchTss: WHV_X64_UNSUPPORTED_FEATURE_CODE = 2i32;
pub const WHvVirtualProcessorPropertyCodeNumaNode: WHV_VIRTUAL_PROCESSOR_PROPERTY_CODE = 0i32;
pub const WHvVirtualProcessorStateTypeInterruptControllerState2: WHV_VIRTUAL_PROCESSOR_STATE_TYPE = 4096i32;
pub const WHvVirtualProcessorStateTypeSynicEventFlagPage: WHV_VIRTUAL_PROCESSOR_STATE_TYPE = 1i32;
pub const WHvVirtualProcessorStateTypeSynicMessagePage: WHV_VIRTUAL_PROCESSOR_STATE_TYPE = 0i32;
pub const WHvVirtualProcessorStateTypeSynicTimerState: WHV_VIRTUAL_PROCESSOR_STATE_TYPE = 2i32;
pub const WHvVirtualProcessorStateTypeXsaveState: WHV_VIRTUAL_PROCESSOR_STATE_TYPE = 4097i32;
pub const WHvVpciBar0: WHV_VPCI_DEVICE_REGISTER_SPACE = 0i32;
pub const WHvVpciBar1: WHV_VPCI_DEVICE_REGISTER_SPACE = 1i32;
pub const WHvVpciBar2: WHV_VPCI_DEVICE_REGISTER_SPACE = 2i32;
pub const WHvVpciBar3: WHV_VPCI_DEVICE_REGISTER_SPACE = 3i32;
pub const WHvVpciBar4: WHV_VPCI_DEVICE_REGISTER_SPACE = 4i32;
pub const WHvVpciBar5: WHV_VPCI_DEVICE_REGISTER_SPACE = 5i32;
pub const WHvVpciConfigSpace: WHV_VPCI_DEVICE_REGISTER_SPACE = -1i32;
pub const WHvVpciDeviceNotificationMmioRemapping: WHV_VPCI_DEVICE_NOTIFICATION_TYPE = 1i32;
pub const WHvVpciDeviceNotificationSurpriseRemoval: WHV_VPCI_DEVICE_NOTIFICATION_TYPE = 2i32;
pub const WHvVpciDeviceNotificationUndefined: WHV_VPCI_DEVICE_NOTIFICATION_TYPE = 0i32;
pub const WHvVpciDevicePropertyCodeHardwareIDs: WHV_VPCI_DEVICE_PROPERTY_CODE = 1i32;
pub const WHvVpciDevicePropertyCodeProbedBARs: WHV_VPCI_DEVICE_PROPERTY_CODE = 2i32;
pub const WHvVpciDevicePropertyCodeUndefined: WHV_VPCI_DEVICE_PROPERTY_CODE = 0i32;
pub const WHvVpciInterruptTargetFlagMulticast: WHV_VPCI_INTERRUPT_TARGET_FLAGS = 1i32;
pub const WHvVpciInterruptTargetFlagNone: WHV_VPCI_INTERRUPT_TARGET_FLAGS = 0i32;
pub const WHvVpciMmioRangeFlagReadAccess: WHV_VPCI_MMIO_RANGE_FLAGS = 1i32;
pub const WHvVpciMmioRangeFlagWriteAccess: WHV_VPCI_MMIO_RANGE_FLAGS = 2i32;
pub const WHvX64ApicWriteTypeDfr: WHV_X64_APIC_WRITE_TYPE = 224i32;
pub const WHvX64ApicWriteTypeLdr: WHV_X64_APIC_WRITE_TYPE = 208i32;
pub const WHvX64ApicWriteTypeLint0: WHV_X64_APIC_WRITE_TYPE = 848i32;
pub const WHvX64ApicWriteTypeLint1: WHV_X64_APIC_WRITE_TYPE = 864i32;
pub const WHvX64ApicWriteTypeSvr: WHV_X64_APIC_WRITE_TYPE = 240i32;
pub const WHvX64CpuidResult2FlagSubleafSpecific: WHV_X64_CPUID_RESULT2_FLAGS = 1i32;
pub const WHvX64CpuidResult2FlagVpSpecific: WHV_X64_CPUID_RESULT2_FLAGS = 2i32;
pub const WHvX64ExceptionTypeAlignmentCheckFault: WHV_EXCEPTION_TYPE = 17i32;
pub const WHvX64ExceptionTypeBoundRangeFault: WHV_EXCEPTION_TYPE = 5i32;
pub const WHvX64ExceptionTypeBreakpointTrap: WHV_EXCEPTION_TYPE = 3i32;
pub const WHvX64ExceptionTypeDebugTrapOrFault: WHV_EXCEPTION_TYPE = 1i32;
pub const WHvX64ExceptionTypeDeviceNotAvailableFault: WHV_EXCEPTION_TYPE = 7i32;
pub const WHvX64ExceptionTypeDivideErrorFault: WHV_EXCEPTION_TYPE = 0i32;
pub const WHvX64ExceptionTypeDoubleFaultAbort: WHV_EXCEPTION_TYPE = 8i32;
pub const WHvX64ExceptionTypeFloatingPointErrorFault: WHV_EXCEPTION_TYPE = 16i32;
pub const WHvX64ExceptionTypeGeneralProtectionFault: WHV_EXCEPTION_TYPE = 13i32;
pub const WHvX64ExceptionTypeInvalidOpcodeFault: WHV_EXCEPTION_TYPE = 6i32;
pub const WHvX64ExceptionTypeInvalidTaskStateSegmentFault: WHV_EXCEPTION_TYPE = 10i32;
pub const WHvX64ExceptionTypeMachineCheckAbort: WHV_EXCEPTION_TYPE = 18i32;
pub const WHvX64ExceptionTypeOverflowTrap: WHV_EXCEPTION_TYPE = 4i32;
pub const WHvX64ExceptionTypePageFault: WHV_EXCEPTION_TYPE = 14i32;
pub const WHvX64ExceptionTypeSegmentNotPresentFault: WHV_EXCEPTION_TYPE = 11i32;
pub const WHvX64ExceptionTypeSimdFloatingPointFault: WHV_EXCEPTION_TYPE = 19i32;
pub const WHvX64ExceptionTypeStackFault: WHV_EXCEPTION_TYPE = 12i32;
pub const WHvX64InterruptDestinationModeLogical: WHV_INTERRUPT_DESTINATION_MODE = 1i32;
pub const WHvX64InterruptDestinationModePhysical: WHV_INTERRUPT_DESTINATION_MODE = 0i32;
pub const WHvX64InterruptTriggerModeEdge: WHV_INTERRUPT_TRIGGER_MODE = 0i32;
pub const WHvX64InterruptTriggerModeLevel: WHV_INTERRUPT_TRIGGER_MODE = 1i32;
pub const WHvX64InterruptTypeFixed: WHV_INTERRUPT_TYPE = 0i32;
pub const WHvX64InterruptTypeInit: WHV_INTERRUPT_TYPE = 5i32;
pub const WHvX64InterruptTypeLocalInt1: WHV_INTERRUPT_TYPE = 9i32;
pub const WHvX64InterruptTypeLowestPriority: WHV_INTERRUPT_TYPE = 1i32;
pub const WHvX64InterruptTypeNmi: WHV_INTERRUPT_TYPE = 4i32;
pub const WHvX64InterruptTypeSipi: WHV_INTERRUPT_TYPE = 6i32;
pub const WHvX64LocalApicEmulationModeNone: WHV_X64_LOCAL_APIC_EMULATION_MODE = 0i32;
pub const WHvX64LocalApicEmulationModeX2Apic: WHV_X64_LOCAL_APIC_EMULATION_MODE = 2i32;
pub const WHvX64LocalApicEmulationModeXApic: WHV_X64_LOCAL_APIC_EMULATION_MODE = 1i32;
pub const WHvX64PendingEventException: WHV_X64_PENDING_EVENT_TYPE = 0i32;
pub const WHvX64PendingEventExtInt: WHV_X64_PENDING_EVENT_TYPE = 5i32;
pub const WHvX64PendingException: WHV_X64_PENDING_INTERRUPTION_TYPE = 3i32;
pub const WHvX64PendingInterrupt: WHV_X64_PENDING_INTERRUPTION_TYPE = 0i32;
pub const WHvX64PendingNmi: WHV_X64_PENDING_INTERRUPTION_TYPE = 2i32;
pub const WHvX64RegisterACount: WHV_REGISTER_NAME = 8319i32;
pub const WHvX64RegisterApicBase: WHV_REGISTER_NAME = 8195i32;
pub const WHvX64RegisterApicCurrentCount: WHV_REGISTER_NAME = 12345i32;
pub const WHvX64RegisterApicDivide: WHV_REGISTER_NAME = 12350i32;
pub const WHvX64RegisterApicEoi: WHV_REGISTER_NAME = 12299i32;
pub const WHvX64RegisterApicEse: WHV_REGISTER_NAME = 12328i32;
pub const WHvX64RegisterApicIcr: WHV_REGISTER_NAME = 12336i32;
pub const WHvX64RegisterApicId: WHV_REGISTER_NAME = 12290i32;
pub const WHvX64RegisterApicInitCount: WHV_REGISTER_NAME = 12344i32;
pub const WHvX64RegisterApicIrr0: WHV_REGISTER_NAME = 12320i32;
pub const WHvX64RegisterApicIrr1: WHV_REGISTER_NAME = 12321i32;
pub const WHvX64RegisterApicIrr2: WHV_REGISTER_NAME = 12322i32;
pub const WHvX64RegisterApicIrr3: WHV_REGISTER_NAME = 12323i32;
pub const WHvX64RegisterApicIrr4: WHV_REGISTER_NAME = 12324i32;
pub const WHvX64RegisterApicIrr5: WHV_REGISTER_NAME = 12325i32;
pub const WHvX64RegisterApicIrr6: WHV_REGISTER_NAME = 12326i32;
pub const WHvX64RegisterApicIrr7: WHV_REGISTER_NAME = 12327i32;
pub const WHvX64RegisterApicIsr0: WHV_REGISTER_NAME = 12304i32;
pub const WHvX64RegisterApicIsr1: WHV_REGISTER_NAME = 12305i32;
pub const WHvX64RegisterApicIsr2: WHV_REGISTER_NAME = 12306i32;
pub const WHvX64RegisterApicIsr3: WHV_REGISTER_NAME = 12307i32;
pub const WHvX64RegisterApicIsr4: WHV_REGISTER_NAME = 12308i32;
pub const WHvX64RegisterApicIsr5: WHV_REGISTER_NAME = 12309i32;
pub const WHvX64RegisterApicIsr6: WHV_REGISTER_NAME = 12310i32;
pub const WHvX64RegisterApicIsr7: WHV_REGISTER_NAME = 12311i32;
pub const WHvX64RegisterApicLdr: WHV_REGISTER_NAME = 12301i32;
pub const WHvX64RegisterApicLvtError: WHV_REGISTER_NAME = 12343i32;
pub const WHvX64RegisterApicLvtLint0: WHV_REGISTER_NAME = 12341i32;
pub const WHvX64RegisterApicLvtLint1: WHV_REGISTER_NAME = 12342i32;
pub const WHvX64RegisterApicLvtPerfmon: WHV_REGISTER_NAME = 12340i32;
pub const WHvX64RegisterApicLvtThermal: WHV_REGISTER_NAME = 12339i32;
pub const WHvX64RegisterApicLvtTimer: WHV_REGISTER_NAME = 12338i32;
pub const WHvX64RegisterApicPpr: WHV_REGISTER_NAME = 12298i32;
pub const WHvX64RegisterApicSelfIpi: WHV_REGISTER_NAME = 12351i32;
pub const WHvX64RegisterApicSpurious: WHV_REGISTER_NAME = 12303i32;
pub const WHvX64RegisterApicTmr0: WHV_REGISTER_NAME = 12312i32;
pub const WHvX64RegisterApicTmr1: WHV_REGISTER_NAME = 12313i32;
pub const WHvX64RegisterApicTmr2: WHV_REGISTER_NAME = 12314i32;
pub const WHvX64RegisterApicTmr3: WHV_REGISTER_NAME = 12315i32;
pub const WHvX64RegisterApicTmr4: WHV_REGISTER_NAME = 12316i32;
pub const WHvX64RegisterApicTmr5: WHV_REGISTER_NAME = 12317i32;
pub const WHvX64RegisterApicTmr6: WHV_REGISTER_NAME = 12318i32;
pub const WHvX64RegisterApicTmr7: WHV_REGISTER_NAME = 12319i32;
pub const WHvX64RegisterApicTpr: WHV_REGISTER_NAME = 12296i32;
pub const WHvX64RegisterApicVersion: WHV_REGISTER_NAME = 12291i32;
pub const WHvX64RegisterBndcfgs: WHV_REGISTER_NAME = 8316i32;
pub const WHvX64RegisterCr0: WHV_REGISTER_NAME = 28i32;
pub const WHvX64RegisterCr2: WHV_REGISTER_NAME = 29i32;
pub const WHvX64RegisterCr3: WHV_REGISTER_NAME = 30i32;
pub const WHvX64RegisterCr4: WHV_REGISTER_NAME = 31i32;
pub const WHvX64RegisterCr8: WHV_REGISTER_NAME = 32i32;
pub const WHvX64RegisterCs: WHV_REGISTER_NAME = 19i32;
pub const WHvX64RegisterCstar: WHV_REGISTER_NAME = 8202i32;
pub const WHvX64RegisterDeliverabilityNotifications: WHV_REGISTER_NAME = -2147483644i32;
pub const WHvX64RegisterDr0: WHV_REGISTER_NAME = 33i32;
pub const WHvX64RegisterDr1: WHV_REGISTER_NAME = 34i32;
pub const WHvX64RegisterDr2: WHV_REGISTER_NAME = 35i32;
pub const WHvX64RegisterDr3: WHV_REGISTER_NAME = 36i32;
pub const WHvX64RegisterDr6: WHV_REGISTER_NAME = 37i32;
pub const WHvX64RegisterDr7: WHV_REGISTER_NAME = 38i32;
pub const WHvX64RegisterDs: WHV_REGISTER_NAME = 21i32;
pub const WHvX64RegisterEfer: WHV_REGISTER_NAME = 8193i32;
pub const WHvX64RegisterEs: WHV_REGISTER_NAME = 18i32;
pub const WHvX64RegisterFpControlStatus: WHV_REGISTER_NAME = 4120i32;
pub const WHvX64RegisterFpMmx0: WHV_REGISTER_NAME = 4112i32;
pub const WHvX64RegisterFpMmx1: WHV_REGISTER_NAME = 4113i32;
pub const WHvX64RegisterFpMmx2: WHV_REGISTER_NAME = 4114i32;
pub const WHvX64RegisterFpMmx3: WHV_REGISTER_NAME = 4115i32;
pub const WHvX64RegisterFpMmx4: WHV_REGISTER_NAME = 4116i32;
pub const WHvX64RegisterFpMmx5: WHV_REGISTER_NAME = 4117i32;
pub const WHvX64RegisterFpMmx6: WHV_REGISTER_NAME = 4118i32;
pub const WHvX64RegisterFpMmx7: WHV_REGISTER_NAME = 4119i32;
pub const WHvX64RegisterFs: WHV_REGISTER_NAME = 22i32;
pub const WHvX64RegisterGdtr: WHV_REGISTER_NAME = 27i32;
pub const WHvX64RegisterGs: WHV_REGISTER_NAME = 23i32;
pub const WHvX64RegisterHypercall: WHV_REGISTER_NAME = 20481i32;
pub const WHvX64RegisterIdtr: WHV_REGISTER_NAME = 26i32;
pub const WHvX64RegisterInitialApicId: WHV_REGISTER_NAME = 8204i32;
pub const WHvX64RegisterInterruptSspTableAddr: WHV_REGISTER_NAME = 8339i32;
pub const WHvX64RegisterKernelGsBase: WHV_REGISTER_NAME = 8194i32;
pub const WHvX64RegisterLdtr: WHV_REGISTER_NAME = 24i32;
pub const WHvX64RegisterLstar: WHV_REGISTER_NAME = 8201i32;
pub const WHvX64RegisterMCount: WHV_REGISTER_NAME = 8318i32;
pub const WHvX64RegisterMsrMtrrCap: WHV_REGISTER_NAME = 8205i32;
pub const WHvX64RegisterMsrMtrrDefType: WHV_REGISTER_NAME = 8206i32;
pub const WHvX64RegisterMsrMtrrFix16k80000: WHV_REGISTER_NAME = 8305i32;
pub const WHvX64RegisterMsrMtrrFix16kA0000: WHV_REGISTER_NAME = 8306i32;
pub const WHvX64RegisterMsrMtrrFix4kC0000: WHV_REGISTER_NAME = 8307i32;
pub const WHvX64RegisterMsrMtrrFix4kC8000: WHV_REGISTER_NAME = 8308i32;
pub const WHvX64RegisterMsrMtrrFix4kD0000: WHV_REGISTER_NAME = 8309i32;
pub const WHvX64RegisterMsrMtrrFix4kD8000: WHV_REGISTER_NAME = 8310i32;
pub const WHvX64RegisterMsrMtrrFix4kE0000: WHV_REGISTER_NAME = 8311i32;
pub const WHvX64RegisterMsrMtrrFix4kE8000: WHV_REGISTER_NAME = 8312i32;
pub const WHvX64RegisterMsrMtrrFix4kF0000: WHV_REGISTER_NAME = 8313i32;
pub const WHvX64RegisterMsrMtrrFix4kF8000: WHV_REGISTER_NAME = 8314i32;
pub const WHvX64RegisterMsrMtrrFix64k00000: WHV_REGISTER_NAME = 8304i32;
pub const WHvX64RegisterMsrMtrrPhysBase0: WHV_REGISTER_NAME = 8208i32;
pub const WHvX64RegisterMsrMtrrPhysBase1: WHV_REGISTER_NAME = 8209i32;
pub const WHvX64RegisterMsrMtrrPhysBase2: WHV_REGISTER_NAME = 8210i32;
pub const WHvX64RegisterMsrMtrrPhysBase3: WHV_REGISTER_NAME = 8211i32;
pub const WHvX64RegisterMsrMtrrPhysBase4: WHV_REGISTER_NAME = 8212i32;
pub const WHvX64RegisterMsrMtrrPhysBase5: WHV_REGISTER_NAME = 8213i32;
pub const WHvX64RegisterMsrMtrrPhysBase6: WHV_REGISTER_NAME = 8214i32;
pub const WHvX64RegisterMsrMtrrPhysBase7: WHV_REGISTER_NAME = 8215i32;
pub const WHvX64RegisterMsrMtrrPhysBase8: WHV_REGISTER_NAME = 8216i32;
pub const WHvX64RegisterMsrMtrrPhysBase9: WHV_REGISTER_NAME = 8217i32;
pub const WHvX64RegisterMsrMtrrPhysBaseA: WHV_REGISTER_NAME = 8218i32;
pub const WHvX64RegisterMsrMtrrPhysBaseB: WHV_REGISTER_NAME = 8219i32;
pub const WHvX64RegisterMsrMtrrPhysBaseC: WHV_REGISTER_NAME = 8220i32;
pub const WHvX64RegisterMsrMtrrPhysBaseD: WHV_REGISTER_NAME = 8221i32;
pub const WHvX64RegisterMsrMtrrPhysBaseE: WHV_REGISTER_NAME = 8222i32;
pub const WHvX64RegisterMsrMtrrPhysBaseF: WHV_REGISTER_NAME = 8223i32;
pub const WHvX64RegisterMsrMtrrPhysMask0: WHV_REGISTER_NAME = 8256i32;
pub const WHvX64RegisterMsrMtrrPhysMask1: WHV_REGISTER_NAME = 8257i32;
pub const WHvX64RegisterMsrMtrrPhysMask2: WHV_REGISTER_NAME = 8258i32;
pub const WHvX64RegisterMsrMtrrPhysMask3: WHV_REGISTER_NAME = 8259i32;
pub const WHvX64RegisterMsrMtrrPhysMask4: WHV_REGISTER_NAME = 8260i32;
pub const WHvX64RegisterMsrMtrrPhysMask5: WHV_REGISTER_NAME = 8261i32;
pub const WHvX64RegisterMsrMtrrPhysMask6: WHV_REGISTER_NAME = 8262i32;
pub const WHvX64RegisterMsrMtrrPhysMask7: WHV_REGISTER_NAME = 8263i32;
pub const WHvX64RegisterMsrMtrrPhysMask8: WHV_REGISTER_NAME = 8264i32;
pub const WHvX64RegisterMsrMtrrPhysMask9: WHV_REGISTER_NAME = 8265i32;
pub const WHvX64RegisterMsrMtrrPhysMaskA: WHV_REGISTER_NAME = 8266i32;
pub const WHvX64RegisterMsrMtrrPhysMaskB: WHV_REGISTER_NAME = 8267i32;
pub const WHvX64RegisterMsrMtrrPhysMaskC: WHV_REGISTER_NAME = 8268i32;
pub const WHvX64RegisterMsrMtrrPhysMaskD: WHV_REGISTER_NAME = 8269i32;
pub const WHvX64RegisterMsrMtrrPhysMaskE: WHV_REGISTER_NAME = 8270i32;
pub const WHvX64RegisterMsrMtrrPhysMaskF: WHV_REGISTER_NAME = 8271i32;
pub const WHvX64RegisterPat: WHV_REGISTER_NAME = 8196i32;
pub const WHvX64RegisterPendingDebugException: WHV_REGISTER_NAME = -2147483642i32;
pub const WHvX64RegisterPl0Ssp: WHV_REGISTER_NAME = 8335i32;
pub const WHvX64RegisterPl1Ssp: WHV_REGISTER_NAME = 8336i32;
pub const WHvX64RegisterPl2Ssp: WHV_REGISTER_NAME = 8337i32;
pub const WHvX64RegisterPl3Ssp: WHV_REGISTER_NAME = 8338i32;
pub const WHvX64RegisterPredCmd: WHV_REGISTER_NAME = 8325i32;
pub const WHvX64RegisterR10: WHV_REGISTER_NAME = 10i32;
pub const WHvX64RegisterR11: WHV_REGISTER_NAME = 11i32;
pub const WHvX64RegisterR12: WHV_REGISTER_NAME = 12i32;
pub const WHvX64RegisterR13: WHV_REGISTER_NAME = 13i32;
pub const WHvX64RegisterR14: WHV_REGISTER_NAME = 14i32;
pub const WHvX64RegisterR15: WHV_REGISTER_NAME = 15i32;
pub const WHvX64RegisterR8: WHV_REGISTER_NAME = 8i32;
pub const WHvX64RegisterR9: WHV_REGISTER_NAME = 9i32;
pub const WHvX64RegisterRax: WHV_REGISTER_NAME = 0i32;
pub const WHvX64RegisterRbp: WHV_REGISTER_NAME = 5i32;
pub const WHvX64RegisterRbx: WHV_REGISTER_NAME = 3i32;
pub const WHvX64RegisterRcx: WHV_REGISTER_NAME = 1i32;
pub const WHvX64RegisterRdi: WHV_REGISTER_NAME = 7i32;
pub const WHvX64RegisterRdx: WHV_REGISTER_NAME = 2i32;
pub const WHvX64RegisterRflags: WHV_REGISTER_NAME = 17i32;
pub const WHvX64RegisterRip: WHV_REGISTER_NAME = 16i32;
pub const WHvX64RegisterRsi: WHV_REGISTER_NAME = 6i32;
pub const WHvX64RegisterRsp: WHV_REGISTER_NAME = 4i32;
pub const WHvX64RegisterSCet: WHV_REGISTER_NAME = 8333i32;
pub const WHvX64RegisterSfmask: WHV_REGISTER_NAME = 8203i32;
pub const WHvX64RegisterSpecCtrl: WHV_REGISTER_NAME = 8324i32;
pub const WHvX64RegisterSs: WHV_REGISTER_NAME = 20i32;
pub const WHvX64RegisterSsp: WHV_REGISTER_NAME = 8334i32;
pub const WHvX64RegisterStar: WHV_REGISTER_NAME = 8200i32;
pub const WHvX64RegisterSysenterCs: WHV_REGISTER_NAME = 8197i32;
pub const WHvX64RegisterSysenterEip: WHV_REGISTER_NAME = 8198i32;
pub const WHvX64RegisterSysenterEsp: WHV_REGISTER_NAME = 8199i32;
pub const WHvX64RegisterTr: WHV_REGISTER_NAME = 25i32;
pub const WHvX64RegisterTsc: WHV_REGISTER_NAME = 8192i32;
pub const WHvX64RegisterTscAdjust: WHV_REGISTER_NAME = 8342i32;
pub const WHvX64RegisterTscAux: WHV_REGISTER_NAME = 8315i32;
pub const WHvX64RegisterTscDeadline: WHV_REGISTER_NAME = 8341i32;
pub const WHvX64RegisterTscVirtualOffset: WHV_REGISTER_NAME = 8327i32;
pub const WHvX64RegisterTsxCtrl: WHV_REGISTER_NAME = 8328i32;
pub const WHvX64RegisterUCet: WHV_REGISTER_NAME = 8332i32;
pub const WHvX64RegisterUmwaitControl: WHV_REGISTER_NAME = 8344i32;
pub const WHvX64RegisterVirtualCr0: WHV_REGISTER_NAME = 40i32;
pub const WHvX64RegisterVirtualCr3: WHV_REGISTER_NAME = 41i32;
pub const WHvX64RegisterVirtualCr4: WHV_REGISTER_NAME = 42i32;
pub const WHvX64RegisterVirtualCr8: WHV_REGISTER_NAME = 43i32;
pub const WHvX64RegisterXCr0: WHV_REGISTER_NAME = 39i32;
pub const WHvX64RegisterXfd: WHV_REGISTER_NAME = 8345i32;
pub const WHvX64RegisterXfdErr: WHV_REGISTER_NAME = 8346i32;
pub const WHvX64RegisterXmm0: WHV_REGISTER_NAME = 4096i32;
pub const WHvX64RegisterXmm1: WHV_REGISTER_NAME = 4097i32;
pub const WHvX64RegisterXmm10: WHV_REGISTER_NAME = 4106i32;
pub const WHvX64RegisterXmm11: WHV_REGISTER_NAME = 4107i32;
pub const WHvX64RegisterXmm12: WHV_REGISTER_NAME = 4108i32;
pub const WHvX64RegisterXmm13: WHV_REGISTER_NAME = 4109i32;
pub const WHvX64RegisterXmm14: WHV_REGISTER_NAME = 4110i32;
pub const WHvX64RegisterXmm15: WHV_REGISTER_NAME = 4111i32;
pub const WHvX64RegisterXmm2: WHV_REGISTER_NAME = 4098i32;
pub const WHvX64RegisterXmm3: WHV_REGISTER_NAME = 4099i32;
pub const WHvX64RegisterXmm4: WHV_REGISTER_NAME = 4100i32;
pub const WHvX64RegisterXmm5: WHV_REGISTER_NAME = 4101i32;
pub const WHvX64RegisterXmm6: WHV_REGISTER_NAME = 4102i32;
pub const WHvX64RegisterXmm7: WHV_REGISTER_NAME = 4103i32;
pub const WHvX64RegisterXmm8: WHV_REGISTER_NAME = 4104i32;
pub const WHvX64RegisterXmm9: WHV_REGISTER_NAME = 4105i32;
pub const WHvX64RegisterXmmControlStatus: WHV_REGISTER_NAME = 4121i32;
pub const WHvX64RegisterXss: WHV_REGISTER_NAME = 8331i32;
pub const X64_RegisterCr0: REGISTER_ID = 44i32;
pub const X64_RegisterCr2: REGISTER_ID = 45i32;
pub const X64_RegisterCr3: REGISTER_ID = 46i32;
pub const X64_RegisterCr4: REGISTER_ID = 47i32;
pub const X64_RegisterCr8: REGISTER_ID = 48i32;
pub const X64_RegisterCs: REGISTER_ID = 57i32;
pub const X64_RegisterDr0: REGISTER_ID = 50i32;
pub const X64_RegisterDr1: REGISTER_ID = 51i32;
pub const X64_RegisterDr2: REGISTER_ID = 52i32;
pub const X64_RegisterDr3: REGISTER_ID = 53i32;
pub const X64_RegisterDr6: REGISTER_ID = 54i32;
pub const X64_RegisterDr7: REGISTER_ID = 55i32;
pub const X64_RegisterDs: REGISTER_ID = 59i32;
pub const X64_RegisterEfer: REGISTER_ID = 49i32;
pub const X64_RegisterEs: REGISTER_ID = 56i32;
pub const X64_RegisterFpControlStatus: REGISTER_ID = 42i32;
pub const X64_RegisterFpMmx0: REGISTER_ID = 34i32;
pub const X64_RegisterFpMmx1: REGISTER_ID = 35i32;
pub const X64_RegisterFpMmx2: REGISTER_ID = 36i32;
pub const X64_RegisterFpMmx3: REGISTER_ID = 37i32;
pub const X64_RegisterFpMmx4: REGISTER_ID = 38i32;
pub const X64_RegisterFpMmx5: REGISTER_ID = 39i32;
pub const X64_RegisterFpMmx6: REGISTER_ID = 40i32;
pub const X64_RegisterFpMmx7: REGISTER_ID = 41i32;
pub const X64_RegisterFs: REGISTER_ID = 60i32;
pub const X64_RegisterGdtr: REGISTER_ID = 65i32;
pub const X64_RegisterGs: REGISTER_ID = 61i32;
pub const X64_RegisterIdtr: REGISTER_ID = 64i32;
pub const X64_RegisterLdtr: REGISTER_ID = 62i32;
pub const X64_RegisterMax: REGISTER_ID = 66i32;
pub const X64_RegisterR10: REGISTER_ID = 10i32;
pub const X64_RegisterR11: REGISTER_ID = 11i32;
pub const X64_RegisterR12: REGISTER_ID = 12i32;
pub const X64_RegisterR13: REGISTER_ID = 13i32;
pub const X64_RegisterR14: REGISTER_ID = 14i32;
pub const X64_RegisterR15: REGISTER_ID = 15i32;
pub const X64_RegisterR8: REGISTER_ID = 8i32;
pub const X64_RegisterR9: REGISTER_ID = 9i32;
pub const X64_RegisterRFlags: REGISTER_ID = 17i32;
pub const X64_RegisterRax: REGISTER_ID = 0i32;
pub const X64_RegisterRbp: REGISTER_ID = 5i32;
pub const X64_RegisterRbx: REGISTER_ID = 3i32;
pub const X64_RegisterRcx: REGISTER_ID = 1i32;
pub const X64_RegisterRdi: REGISTER_ID = 7i32;
pub const X64_RegisterRdx: REGISTER_ID = 2i32;
pub const X64_RegisterRip: REGISTER_ID = 16i32;
pub const X64_RegisterRsi: REGISTER_ID = 6i32;
pub const X64_RegisterRsp: REGISTER_ID = 4i32;
pub const X64_RegisterSs: REGISTER_ID = 58i32;
pub const X64_RegisterTr: REGISTER_ID = 63i32;
pub const X64_RegisterXmm0: REGISTER_ID = 18i32;
pub const X64_RegisterXmm1: REGISTER_ID = 19i32;
pub const X64_RegisterXmm10: REGISTER_ID = 28i32;
pub const X64_RegisterXmm11: REGISTER_ID = 29i32;
pub const X64_RegisterXmm12: REGISTER_ID = 30i32;
pub const X64_RegisterXmm13: REGISTER_ID = 31i32;
pub const X64_RegisterXmm14: REGISTER_ID = 32i32;
pub const X64_RegisterXmm15: REGISTER_ID = 33i32;
pub const X64_RegisterXmm2: REGISTER_ID = 20i32;
pub const X64_RegisterXmm3: REGISTER_ID = 21i32;
pub const X64_RegisterXmm4: REGISTER_ID = 22i32;
pub const X64_RegisterXmm5: REGISTER_ID = 23i32;
pub const X64_RegisterXmm6: REGISTER_ID = 24i32;
pub const X64_RegisterXmm7: REGISTER_ID = 25i32;
pub const X64_RegisterXmm8: REGISTER_ID = 26i32;
pub const X64_RegisterXmm9: REGISTER_ID = 27i32;
pub const X64_RegisterXmmControlStatus: REGISTER_ID = 43i32;
pub type GUEST_OS_MICROSOFT_IDS = i32;
pub type GUEST_OS_OPENSOURCE_IDS = i32;
pub type GUEST_OS_VENDOR = i32;
pub type HDV_DEVICE_HOST_FLAGS = i32;
pub type HDV_DEVICE_TYPE = i32;
pub type HDV_DOORBELL_FLAGS = i32;
pub type HDV_MMIO_MAPPING_FLAGS = i32;
pub type HDV_PCI_BAR_SELECTOR = i32;
pub type HDV_PCI_INTERFACE_VERSION = i32;
pub type PAGING_MODE = i32;
pub type REGISTER_ID = i32;
pub type VIRTUAL_PROCESSOR_ARCH = i32;
pub type VIRTUAL_PROCESSOR_VENDOR = i32;
pub type WHV_ADVISE_GPA_RANGE_CODE = i32;
pub type WHV_ALLOCATE_VPCI_RESOURCE_FLAGS = i32;
pub type WHV_CACHE_TYPE = i32;
pub type WHV_CAPABILITY_CODE = i32;
pub type WHV_CREATE_VPCI_DEVICE_FLAGS = i32;
pub type WHV_EXCEPTION_TYPE = i32;
pub type WHV_INTERRUPT_DESTINATION_MODE = i32;
pub type WHV_INTERRUPT_TRIGGER_MODE = i32;
pub type WHV_INTERRUPT_TYPE = i32;
pub type WHV_MAP_GPA_RANGE_FLAGS = i32;
pub type WHV_MEMORY_ACCESS_TYPE = i32;
pub type WHV_MSR_ACTION = i32;
pub type WHV_NOTIFICATION_PORT_PROPERTY_CODE = i32;
pub type WHV_NOTIFICATION_PORT_TYPE = i32;
pub type WHV_PARTITION_COUNTER_SET = i32;
pub type WHV_PARTITION_PROPERTY_CODE = i32;
pub type WHV_PROCESSOR_COUNTER_SET = i32;
pub type WHV_PROCESSOR_VENDOR = i32;
pub type WHV_REGISTER_NAME = i32;
pub type WHV_RUN_VP_CANCEL_REASON = i32;
pub type WHV_RUN_VP_EXIT_REASON = i32;
pub type WHV_TRANSLATE_GVA_FLAGS = i32;
pub type WHV_TRANSLATE_GVA_RESULT_CODE = i32;
pub type WHV_TRIGGER_TYPE = i32;
pub type WHV_VIRTUAL_PROCESSOR_PROPERTY_CODE = i32;
pub type WHV_VIRTUAL_PROCESSOR_STATE_TYPE = i32;
pub type WHV_VPCI_DEVICE_NOTIFICATION_TYPE = i32;
pub type WHV_VPCI_DEVICE_PROPERTY_CODE = i32;
pub type WHV_VPCI_DEVICE_REGISTER_SPACE = i32;
pub type WHV_VPCI_INTERRUPT_TARGET_FLAGS = i32;
pub type WHV_VPCI_MMIO_RANGE_FLAGS = i32;
pub type WHV_X64_APIC_WRITE_TYPE = i32;
pub type WHV_X64_CPUID_RESULT2_FLAGS = i32;
pub type WHV_X64_LOCAL_APIC_EMULATION_MODE = i32;
pub type WHV_X64_PENDING_EVENT_TYPE = i32;
pub type WHV_X64_PENDING_INTERRUPTION_TYPE = i32;
pub type WHV_X64_UNSUPPORTED_FEATURE_CODE = i32;
#[repr(C)]
pub struct DOS_IMAGE_INFO {
    pub PdbName: ::windows_sys::core::PCSTR,
    pub ImageBaseAddress: u64,
    pub ImageSize: u32,
    pub Timestamp: u32,
}
impl ::core::marker::Copy for DOS_IMAGE_INFO {}
impl ::core::clone::Clone for DOS_IMAGE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GPA_MEMORY_CHUNK {
    pub GuestPhysicalStartPageIndex: u64,
    pub PageCount: u64,
}
impl ::core::marker::Copy for GPA_MEMORY_CHUNK {}
impl ::core::clone::Clone for GPA_MEMORY_CHUNK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union GUEST_OS_INFO {
    pub AsUINT64: u64,
    pub ClosedSource: GUEST_OS_INFO_0,
    pub OpenSource: GUEST_OS_INFO_1,
}
impl ::core::marker::Copy for GUEST_OS_INFO {}
impl ::core::clone::Clone for GUEST_OS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GUEST_OS_INFO_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for GUEST_OS_INFO_0 {}
impl ::core::clone::Clone for GUEST_OS_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GUEST_OS_INFO_1 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for GUEST_OS_INFO_1 {}
impl ::core::clone::Clone for GUEST_OS_INFO_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct HDV_PCI_DEVICE_INTERFACE {
    pub Version: HDV_PCI_INTERFACE_VERSION,
    pub Initialize: HDV_PCI_DEVICE_INITIALIZE,
    pub Teardown: HDV_PCI_DEVICE_TEARDOWN,
    pub SetConfiguration: HDV_PCI_DEVICE_SET_CONFIGURATION,
    pub GetDetails: HDV_PCI_DEVICE_GET_DETAILS,
    pub Start: HDV_PCI_DEVICE_START,
    pub Stop: HDV_PCI_DEVICE_STOP,
    pub ReadConfigSpace: HDV_PCI_READ_CONFIG_SPACE,
    pub WriteConfigSpace: HDV_PCI_WRITE_CONFIG_SPACE,
    pub ReadInterceptedMemory: HDV_PCI_READ_INTERCEPTED_MEMORY,
    pub WriteInterceptedMemory: HDV_PCI_WRITE_INTERCEPTED_MEMORY,
}
impl ::core::marker::Copy for HDV_PCI_DEVICE_INTERFACE {}
impl ::core::clone::Clone for HDV_PCI_DEVICE_INTERFACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct HDV_PCI_PNP_ID {
    pub VendorID: u16,
    pub DeviceID: u16,
    pub RevisionID: u8,
    pub ProgIf: u8,
    pub SubClass: u8,
    pub BaseClass: u8,
    pub SubVendorID: u16,
    pub SubSystemID: u16,
}
impl ::core::marker::Copy for HDV_PCI_PNP_ID {}
impl ::core::clone::Clone for HDV_PCI_PNP_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct HVSOCKET_ADDRESS_INFO {
    pub SystemId: ::windows_sys::core::GUID,
    pub VirtualMachineId: ::windows_sys::core::GUID,
    pub SiloId: ::windows_sys::core::GUID,
    pub Flags: u32,
}
impl ::core::marker::Copy for HVSOCKET_ADDRESS_INFO {}
impl ::core::clone::Clone for HVSOCKET_ADDRESS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MODULE_INFO {
    pub ProcessImageName: ::windows_sys::core::PCSTR,
    pub Image: DOS_IMAGE_INFO,
}
impl ::core::marker::Copy for MODULE_INFO {}
impl ::core::clone::Clone for MODULE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct SOCKADDR_HV {
    pub Family: super::super::Networking::WinSock::ADDRESS_FAMILY,
    pub Reserved: u16,
    pub VmId: ::windows_sys::core::GUID,
    pub ServiceId: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for SOCKADDR_HV {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for SOCKADDR_HV {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union VIRTUAL_PROCESSOR_REGISTER {
    pub Reg64: u64,
    pub Reg32: u32,
    pub Reg16: u16,
    pub Reg8: u8,
    pub Reg128: VIRTUAL_PROCESSOR_REGISTER_0,
    pub X64: VIRTUAL_PROCESSOR_REGISTER_1,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VIRTUAL_PROCESSOR_REGISTER_0 {
    pub Low64: u64,
    pub High64: u64,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_0 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union VIRTUAL_PROCESSOR_REGISTER_1 {
    pub Segment: VIRTUAL_PROCESSOR_REGISTER_1_1,
    pub Table: VIRTUAL_PROCESSOR_REGISTER_1_2,
    pub FpControlStatus: VIRTUAL_PROCESSOR_REGISTER_1_0,
    pub XmmControlStatus: VIRTUAL_PROCESSOR_REGISTER_1_3,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VIRTUAL_PROCESSOR_REGISTER_1_0 {
    pub FpControl: u16,
    pub FpStatus: u16,
    pub FpTag: u8,
    pub Reserved: u8,
    pub LastFpOp: u16,
    pub Anonymous: VIRTUAL_PROCESSOR_REGISTER_1_0_0,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_0 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union VIRTUAL_PROCESSOR_REGISTER_1_0_0 {
    pub LastFpRip: u64,
    pub Anonymous: VIRTUAL_PROCESSOR_REGISTER_1_0_0_0,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_0_0 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VIRTUAL_PROCESSOR_REGISTER_1_0_0_0 {
    pub LastFpEip: u32,
    pub LastFpCs: u16,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_0_0_0 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VIRTUAL_PROCESSOR_REGISTER_1_1 {
    pub Base: u64,
    pub Limit: u32,
    pub Selector: u16,
    pub Anonymous: VIRTUAL_PROCESSOR_REGISTER_1_1_0,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_1 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union VIRTUAL_PROCESSOR_REGISTER_1_1_0 {
    pub Attributes: u16,
    pub Anonymous: VIRTUAL_PROCESSOR_REGISTER_1_1_0_0,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_1_0 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VIRTUAL_PROCESSOR_REGISTER_1_1_0_0 {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_1_0_0 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_1_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VIRTUAL_PROCESSOR_REGISTER_1_2 {
    pub Limit: u16,
    pub Base: u64,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_2 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VIRTUAL_PROCESSOR_REGISTER_1_3 {
    pub Anonymous: VIRTUAL_PROCESSOR_REGISTER_1_3_0,
    pub XmmStatusControl: u32,
    pub XmmStatusControlMask: u32,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_3 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union VIRTUAL_PROCESSOR_REGISTER_1_3_0 {
    pub LastFpRdp: u64,
    pub Anonymous: VIRTUAL_PROCESSOR_REGISTER_1_3_0_0,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_3_0 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_3_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VIRTUAL_PROCESSOR_REGISTER_1_3_0_0 {
    pub LastFpDp: u32,
    pub LastFpDs: u16,
}
impl ::core::marker::Copy for VIRTUAL_PROCESSOR_REGISTER_1_3_0_0 {}
impl ::core::clone::Clone for VIRTUAL_PROCESSOR_REGISTER_1_3_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VM_GENCOUNTER {
    pub GenerationCount: u64,
    pub GenerationCountHigh: u64,
}
impl ::core::marker::Copy for VM_GENCOUNTER {}
impl ::core::clone::Clone for VM_GENCOUNTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_ACCESS_GPA_CONTROLS {
    pub AsUINT64: u64,
    pub Anonymous: WHV_ACCESS_GPA_CONTROLS_0,
}
impl ::core::marker::Copy for WHV_ACCESS_GPA_CONTROLS {}
impl ::core::clone::Clone for WHV_ACCESS_GPA_CONTROLS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_ACCESS_GPA_CONTROLS_0 {
    pub CacheType: WHV_CACHE_TYPE,
    pub Reserved: u32,
}
impl ::core::marker::Copy for WHV_ACCESS_GPA_CONTROLS_0 {}
impl ::core::clone::Clone for WHV_ACCESS_GPA_CONTROLS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_ADVISE_GPA_RANGE {
    pub Populate: WHV_ADVISE_GPA_RANGE_POPULATE,
}
impl ::core::marker::Copy for WHV_ADVISE_GPA_RANGE {}
impl ::core::clone::Clone for WHV_ADVISE_GPA_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_ADVISE_GPA_RANGE_POPULATE {
    pub Flags: WHV_ADVISE_GPA_RANGE_POPULATE_FLAGS,
    pub AccessType: WHV_MEMORY_ACCESS_TYPE,
}
impl ::core::marker::Copy for WHV_ADVISE_GPA_RANGE_POPULATE {}
impl ::core::clone::Clone for WHV_ADVISE_GPA_RANGE_POPULATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_ADVISE_GPA_RANGE_POPULATE_FLAGS {
    pub AsUINT32: u32,
    pub Anonymous: WHV_ADVISE_GPA_RANGE_POPULATE_FLAGS_0,
}
impl ::core::marker::Copy for WHV_ADVISE_GPA_RANGE_POPULATE_FLAGS {}
impl ::core::clone::Clone for WHV_ADVISE_GPA_RANGE_POPULATE_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_ADVISE_GPA_RANGE_POPULATE_FLAGS_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for WHV_ADVISE_GPA_RANGE_POPULATE_FLAGS_0 {}
impl ::core::clone::Clone for WHV_ADVISE_GPA_RANGE_POPULATE_FLAGS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union WHV_CAPABILITY {
    pub HypervisorPresent: super::super::Foundation::BOOL,
    pub Features: WHV_CAPABILITY_FEATURES,
    pub ExtendedVmExits: WHV_EXTENDED_VM_EXITS,
    pub ProcessorVendor: WHV_PROCESSOR_VENDOR,
    pub ProcessorFeatures: WHV_PROCESSOR_FEATURES,
    pub SyntheticProcessorFeaturesBanks: WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS,
    pub ProcessorXsaveFeatures: WHV_PROCESSOR_XSAVE_FEATURES,
    pub ProcessorClFlushSize: u8,
    pub ExceptionExitBitmap: u64,
    pub X64MsrExitBitmap: WHV_X64_MSR_EXIT_BITMAP,
    pub ProcessorClockFrequency: u64,
    pub InterruptClockFrequency: u64,
    pub ProcessorFeaturesBanks: WHV_PROCESSOR_FEATURES_BANKS,
    pub GpaRangePopulateFlags: WHV_ADVISE_GPA_RANGE_POPULATE_FLAGS,
    pub ProcessorFrequencyCap: WHV_CAPABILITY_PROCESSOR_FREQUENCY_CAP,
    pub ProcessorPerfmonFeatures: WHV_PROCESSOR_PERFMON_FEATURES,
    pub SchedulerFeatures: WHV_SCHEDULER_FEATURES,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHV_CAPABILITY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHV_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_CAPABILITY_FEATURES {
    pub Anonymous: WHV_CAPABILITY_FEATURES_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_CAPABILITY_FEATURES {}
impl ::core::clone::Clone for WHV_CAPABILITY_FEATURES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_CAPABILITY_FEATURES_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_CAPABILITY_FEATURES_0 {}
impl ::core::clone::Clone for WHV_CAPABILITY_FEATURES_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_CAPABILITY_PROCESSOR_FREQUENCY_CAP {
    pub _bitfield: u32,
    pub HighestFrequencyMhz: u32,
    pub NominalFrequencyMhz: u32,
    pub LowestFrequencyMhz: u32,
    pub FrequencyStepMhz: u32,
}
impl ::core::marker::Copy for WHV_CAPABILITY_PROCESSOR_FREQUENCY_CAP {}
impl ::core::clone::Clone for WHV_CAPABILITY_PROCESSOR_FREQUENCY_CAP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_CPUID_OUTPUT {
    pub Eax: u32,
    pub Ebx: u32,
    pub Ecx: u32,
    pub Edx: u32,
}
impl ::core::marker::Copy for WHV_CPUID_OUTPUT {}
impl ::core::clone::Clone for WHV_CPUID_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_DOORBELL_MATCH_DATA {
    pub GuestAddress: u64,
    pub Value: u64,
    pub Length: u32,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for WHV_DOORBELL_MATCH_DATA {}
impl ::core::clone::Clone for WHV_DOORBELL_MATCH_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_EMULATOR_CALLBACKS {
    pub Size: u32,
    pub Reserved: u32,
    pub WHvEmulatorIoPortCallback: WHV_EMULATOR_IO_PORT_CALLBACK,
    pub WHvEmulatorMemoryCallback: WHV_EMULATOR_MEMORY_CALLBACK,
    pub WHvEmulatorGetVirtualProcessorRegisters: WHV_EMULATOR_GET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK,
    pub WHvEmulatorSetVirtualProcessorRegisters: WHV_EMULATOR_SET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK,
    pub WHvEmulatorTranslateGvaPage: WHV_EMULATOR_TRANSLATE_GVA_PAGE_CALLBACK,
}
impl ::core::marker::Copy for WHV_EMULATOR_CALLBACKS {}
impl ::core::clone::Clone for WHV_EMULATOR_CALLBACKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_EMULATOR_IO_ACCESS_INFO {
    pub Direction: u8,
    pub Port: u16,
    pub AccessSize: u16,
    pub Data: u32,
}
impl ::core::marker::Copy for WHV_EMULATOR_IO_ACCESS_INFO {}
impl ::core::clone::Clone for WHV_EMULATOR_IO_ACCESS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_EMULATOR_MEMORY_ACCESS_INFO {
    pub GpaAddress: u64,
    pub Direction: u8,
    pub AccessSize: u8,
    pub Data: [u8; 8],
}
impl ::core::marker::Copy for WHV_EMULATOR_MEMORY_ACCESS_INFO {}
impl ::core::clone::Clone for WHV_EMULATOR_MEMORY_ACCESS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_EMULATOR_STATUS {
    pub Anonymous: WHV_EMULATOR_STATUS_0,
    pub AsUINT32: u32,
}
impl ::core::marker::Copy for WHV_EMULATOR_STATUS {}
impl ::core::clone::Clone for WHV_EMULATOR_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_EMULATOR_STATUS_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for WHV_EMULATOR_STATUS_0 {}
impl ::core::clone::Clone for WHV_EMULATOR_STATUS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_EXTENDED_VM_EXITS {
    pub Anonymous: WHV_EXTENDED_VM_EXITS_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_EXTENDED_VM_EXITS {}
impl ::core::clone::Clone for WHV_EXTENDED_VM_EXITS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_EXTENDED_VM_EXITS_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_EXTENDED_VM_EXITS_0 {}
impl ::core::clone::Clone for WHV_EXTENDED_VM_EXITS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_HYPERCALL_CONTEXT {
    pub Rax: u64,
    pub Rbx: u64,
    pub Rcx: u64,
    pub Rdx: u64,
    pub R8: u64,
    pub Rsi: u64,
    pub Rdi: u64,
    pub Reserved0: u64,
    pub XmmRegisters: [WHV_UINT128; 6],
    pub Reserved1: [u64; 2],
}
impl ::core::marker::Copy for WHV_HYPERCALL_CONTEXT {}
impl ::core::clone::Clone for WHV_HYPERCALL_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_INTERNAL_ACTIVITY_REGISTER {
    pub Anonymous: WHV_INTERNAL_ACTIVITY_REGISTER_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_INTERNAL_ACTIVITY_REGISTER {}
impl ::core::clone::Clone for WHV_INTERNAL_ACTIVITY_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_INTERNAL_ACTIVITY_REGISTER_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_INTERNAL_ACTIVITY_REGISTER_0 {}
impl ::core::clone::Clone for WHV_INTERNAL_ACTIVITY_REGISTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_INTERRUPT_CONTROL {
    pub _bitfield: u64,
    pub Destination: u32,
    pub Vector: u32,
}
impl ::core::marker::Copy for WHV_INTERRUPT_CONTROL {}
impl ::core::clone::Clone for WHV_INTERRUPT_CONTROL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_MEMORY_ACCESS_CONTEXT {
    pub InstructionByteCount: u8,
    pub Reserved: [u8; 3],
    pub InstructionBytes: [u8; 16],
    pub AccessInfo: WHV_MEMORY_ACCESS_INFO,
    pub Gpa: u64,
    pub Gva: u64,
}
impl ::core::marker::Copy for WHV_MEMORY_ACCESS_CONTEXT {}
impl ::core::clone::Clone for WHV_MEMORY_ACCESS_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_MEMORY_ACCESS_INFO {
    pub Anonymous: WHV_MEMORY_ACCESS_INFO_0,
    pub AsUINT32: u32,
}
impl ::core::marker::Copy for WHV_MEMORY_ACCESS_INFO {}
impl ::core::clone::Clone for WHV_MEMORY_ACCESS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_MEMORY_ACCESS_INFO_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for WHV_MEMORY_ACCESS_INFO_0 {}
impl ::core::clone::Clone for WHV_MEMORY_ACCESS_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_MEMORY_RANGE_ENTRY {
    pub GuestAddress: u64,
    pub SizeInBytes: u64,
}
impl ::core::marker::Copy for WHV_MEMORY_RANGE_ENTRY {}
impl ::core::clone::Clone for WHV_MEMORY_RANGE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_MSR_ACTION_ENTRY {
    pub Index: u32,
    pub ReadAction: u8,
    pub WriteAction: u8,
    pub Reserved: u16,
}
impl ::core::marker::Copy for WHV_MSR_ACTION_ENTRY {}
impl ::core::clone::Clone for WHV_MSR_ACTION_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_NOTIFICATION_PORT_PARAMETERS {
    pub NotificationPortType: WHV_NOTIFICATION_PORT_TYPE,
    pub Reserved: u32,
    pub Anonymous: WHV_NOTIFICATION_PORT_PARAMETERS_0,
}
impl ::core::marker::Copy for WHV_NOTIFICATION_PORT_PARAMETERS {}
impl ::core::clone::Clone for WHV_NOTIFICATION_PORT_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_NOTIFICATION_PORT_PARAMETERS_0 {
    pub Doorbell: WHV_DOORBELL_MATCH_DATA,
    pub Event: WHV_NOTIFICATION_PORT_PARAMETERS_0_0,
}
impl ::core::marker::Copy for WHV_NOTIFICATION_PORT_PARAMETERS_0 {}
impl ::core::clone::Clone for WHV_NOTIFICATION_PORT_PARAMETERS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_NOTIFICATION_PORT_PARAMETERS_0_0 {
    pub ConnectionId: u32,
}
impl ::core::marker::Copy for WHV_NOTIFICATION_PORT_PARAMETERS_0_0 {}
impl ::core::clone::Clone for WHV_NOTIFICATION_PORT_PARAMETERS_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type WHV_PARTITION_HANDLE = isize;
#[repr(C)]
pub struct WHV_PARTITION_MEMORY_COUNTERS {
    pub Mapped4KPageCount: u64,
    pub Mapped2MPageCount: u64,
    pub Mapped1GPageCount: u64,
}
impl ::core::marker::Copy for WHV_PARTITION_MEMORY_COUNTERS {}
impl ::core::clone::Clone for WHV_PARTITION_MEMORY_COUNTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union WHV_PARTITION_PROPERTY {
    pub ExtendedVmExits: WHV_EXTENDED_VM_EXITS,
    pub ProcessorFeatures: WHV_PROCESSOR_FEATURES,
    pub SyntheticProcessorFeaturesBanks: WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS,
    pub ProcessorXsaveFeatures: WHV_PROCESSOR_XSAVE_FEATURES,
    pub ProcessorClFlushSize: u8,
    pub ProcessorCount: u32,
    pub CpuidExitList: [u32; 1],
    pub CpuidResultList: [WHV_X64_CPUID_RESULT; 1],
    pub CpuidResultList2: [WHV_X64_CPUID_RESULT2; 1],
    pub MsrActionList: [WHV_MSR_ACTION_ENTRY; 1],
    pub UnimplementedMsrAction: WHV_MSR_ACTION,
    pub ExceptionExitBitmap: u64,
    pub LocalApicEmulationMode: WHV_X64_LOCAL_APIC_EMULATION_MODE,
    pub SeparateSecurityDomain: super::super::Foundation::BOOL,
    pub NestedVirtualization: super::super::Foundation::BOOL,
    pub X64MsrExitBitmap: WHV_X64_MSR_EXIT_BITMAP,
    pub ProcessorClockFrequency: u64,
    pub InterruptClockFrequency: u64,
    pub ApicRemoteRead: super::super::Foundation::BOOL,
    pub ProcessorFeaturesBanks: WHV_PROCESSOR_FEATURES_BANKS,
    pub ReferenceTime: u64,
    pub PrimaryNumaNode: u16,
    pub CpuReserve: u32,
    pub CpuCap: u32,
    pub CpuWeight: u32,
    pub CpuGroupId: u64,
    pub ProcessorFrequencyCap: u32,
    pub AllowDeviceAssignment: super::super::Foundation::BOOL,
    pub ProcessorPerfmonFeatures: WHV_PROCESSOR_PERFMON_FEATURES,
    pub DisableSmt: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHV_PARTITION_PROPERTY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHV_PARTITION_PROPERTY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_APIC_COUNTERS {
    pub MmioAccessCount: u64,
    pub EoiAccessCount: u64,
    pub TprAccessCount: u64,
    pub SentIpiCount: u64,
    pub SelfIpiCount: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_APIC_COUNTERS {}
impl ::core::clone::Clone for WHV_PROCESSOR_APIC_COUNTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_EVENT_COUNTERS {
    pub PageFaultCount: u64,
    pub ExceptionCount: u64,
    pub InterruptCount: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_EVENT_COUNTERS {}
impl ::core::clone::Clone for WHV_PROCESSOR_EVENT_COUNTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_PROCESSOR_FEATURES {
    pub Anonymous: WHV_PROCESSOR_FEATURES_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_FEATURES {}
impl ::core::clone::Clone for WHV_PROCESSOR_FEATURES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_FEATURES_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_FEATURES_0 {}
impl ::core::clone::Clone for WHV_PROCESSOR_FEATURES_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_PROCESSOR_FEATURES1 {
    pub Anonymous: WHV_PROCESSOR_FEATURES1_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_FEATURES1 {}
impl ::core::clone::Clone for WHV_PROCESSOR_FEATURES1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_FEATURES1_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_FEATURES1_0 {}
impl ::core::clone::Clone for WHV_PROCESSOR_FEATURES1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_FEATURES_BANKS {
    pub BanksCount: u32,
    pub Reserved0: u32,
    pub Anonymous: WHV_PROCESSOR_FEATURES_BANKS_0,
}
impl ::core::marker::Copy for WHV_PROCESSOR_FEATURES_BANKS {}
impl ::core::clone::Clone for WHV_PROCESSOR_FEATURES_BANKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_PROCESSOR_FEATURES_BANKS_0 {
    pub Anonymous: WHV_PROCESSOR_FEATURES_BANKS_0_0,
    pub AsUINT64: [u64; 2],
}
impl ::core::marker::Copy for WHV_PROCESSOR_FEATURES_BANKS_0 {}
impl ::core::clone::Clone for WHV_PROCESSOR_FEATURES_BANKS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_FEATURES_BANKS_0_0 {
    pub Bank0: WHV_PROCESSOR_FEATURES,
    pub Bank1: WHV_PROCESSOR_FEATURES1,
}
impl ::core::marker::Copy for WHV_PROCESSOR_FEATURES_BANKS_0_0 {}
impl ::core::clone::Clone for WHV_PROCESSOR_FEATURES_BANKS_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_INTERCEPT_COUNTER {
    pub Count: u64,
    pub Time100ns: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_INTERCEPT_COUNTER {}
impl ::core::clone::Clone for WHV_PROCESSOR_INTERCEPT_COUNTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_INTERCEPT_COUNTERS {
    pub PageInvalidations: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub ControlRegisterAccesses: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub IoInstructions: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub HaltInstructions: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub CpuidInstructions: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub MsrAccesses: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub OtherIntercepts: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub PendingInterrupts: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub EmulatedInstructions: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub DebugRegisterAccesses: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub PageFaultIntercepts: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub NestedPageFaultIntercepts: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub Hypercalls: WHV_PROCESSOR_INTERCEPT_COUNTER,
    pub RdpmcInstructions: WHV_PROCESSOR_INTERCEPT_COUNTER,
}
impl ::core::marker::Copy for WHV_PROCESSOR_INTERCEPT_COUNTERS {}
impl ::core::clone::Clone for WHV_PROCESSOR_INTERCEPT_COUNTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_PROCESSOR_PERFMON_FEATURES {
    pub Anonymous: WHV_PROCESSOR_PERFMON_FEATURES_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_PERFMON_FEATURES {}
impl ::core::clone::Clone for WHV_PROCESSOR_PERFMON_FEATURES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_PERFMON_FEATURES_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_PERFMON_FEATURES_0 {}
impl ::core::clone::Clone for WHV_PROCESSOR_PERFMON_FEATURES_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_RUNTIME_COUNTERS {
    pub TotalRuntime100ns: u64,
    pub HypervisorRuntime100ns: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_RUNTIME_COUNTERS {}
impl ::core::clone::Clone for WHV_PROCESSOR_RUNTIME_COUNTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_SYNTHETIC_FEATURES_COUNTERS {
    pub SyntheticInterruptsCount: u64,
    pub LongSpinWaitHypercallsCount: u64,
    pub OtherHypercallsCount: u64,
    pub SyntheticInterruptHypercallsCount: u64,
    pub VirtualInterruptHypercallsCount: u64,
    pub VirtualMmuHypercallsCount: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_SYNTHETIC_FEATURES_COUNTERS {}
impl ::core::clone::Clone for WHV_PROCESSOR_SYNTHETIC_FEATURES_COUNTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_PROCESSOR_XSAVE_FEATURES {
    pub Anonymous: WHV_PROCESSOR_XSAVE_FEATURES_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_XSAVE_FEATURES {}
impl ::core::clone::Clone for WHV_PROCESSOR_XSAVE_FEATURES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_PROCESSOR_XSAVE_FEATURES_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_PROCESSOR_XSAVE_FEATURES_0 {}
impl ::core::clone::Clone for WHV_PROCESSOR_XSAVE_FEATURES_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_REGISTER_VALUE {
    pub Reg128: WHV_UINT128,
    pub Reg64: u64,
    pub Reg32: u32,
    pub Reg16: u16,
    pub Reg8: u8,
    pub Fp: WHV_X64_FP_REGISTER,
    pub FpControlStatus: WHV_X64_FP_CONTROL_STATUS_REGISTER,
    pub XmmControlStatus: WHV_X64_XMM_CONTROL_STATUS_REGISTER,
    pub Segment: WHV_X64_SEGMENT_REGISTER,
    pub Table: WHV_X64_TABLE_REGISTER,
    pub InterruptState: WHV_X64_INTERRUPT_STATE_REGISTER,
    pub PendingInterruption: WHV_X64_PENDING_INTERRUPTION_REGISTER,
    pub DeliverabilityNotifications: WHV_X64_DELIVERABILITY_NOTIFICATIONS_REGISTER,
    pub ExceptionEvent: WHV_X64_PENDING_EXCEPTION_EVENT,
    pub ExtIntEvent: WHV_X64_PENDING_EXT_INT_EVENT,
    pub InternalActivity: WHV_INTERNAL_ACTIVITY_REGISTER,
    pub PendingDebugException: WHV_X64_PENDING_DEBUG_EXCEPTION,
}
impl ::core::marker::Copy for WHV_REGISTER_VALUE {}
impl ::core::clone::Clone for WHV_REGISTER_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_RUN_VP_CANCELED_CONTEXT {
    pub CancelReason: WHV_RUN_VP_CANCEL_REASON,
}
impl ::core::marker::Copy for WHV_RUN_VP_CANCELED_CONTEXT {}
impl ::core::clone::Clone for WHV_RUN_VP_CANCELED_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_RUN_VP_EXIT_CONTEXT {
    pub ExitReason: WHV_RUN_VP_EXIT_REASON,
    pub Reserved: u32,
    pub VpContext: WHV_VP_EXIT_CONTEXT,
    pub Anonymous: WHV_RUN_VP_EXIT_CONTEXT_0,
}
impl ::core::marker::Copy for WHV_RUN_VP_EXIT_CONTEXT {}
impl ::core::clone::Clone for WHV_RUN_VP_EXIT_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_RUN_VP_EXIT_CONTEXT_0 {
    pub MemoryAccess: WHV_MEMORY_ACCESS_CONTEXT,
    pub IoPortAccess: WHV_X64_IO_PORT_ACCESS_CONTEXT,
    pub MsrAccess: WHV_X64_MSR_ACCESS_CONTEXT,
    pub CpuidAccess: WHV_X64_CPUID_ACCESS_CONTEXT,
    pub VpException: WHV_VP_EXCEPTION_CONTEXT,
    pub InterruptWindow: WHV_X64_INTERRUPTION_DELIVERABLE_CONTEXT,
    pub UnsupportedFeature: WHV_X64_UNSUPPORTED_FEATURE_CONTEXT,
    pub CancelReason: WHV_RUN_VP_CANCELED_CONTEXT,
    pub ApicEoi: WHV_X64_APIC_EOI_CONTEXT,
    pub ReadTsc: WHV_X64_RDTSC_CONTEXT,
    pub ApicSmi: WHV_X64_APIC_SMI_CONTEXT,
    pub Hypercall: WHV_HYPERCALL_CONTEXT,
    pub ApicInitSipi: WHV_X64_APIC_INIT_SIPI_CONTEXT,
    pub ApicWrite: WHV_X64_APIC_WRITE_CONTEXT,
    pub SynicSintDeliverable: WHV_SYNIC_SINT_DELIVERABLE_CONTEXT,
}
impl ::core::marker::Copy for WHV_RUN_VP_EXIT_CONTEXT_0 {}
impl ::core::clone::Clone for WHV_RUN_VP_EXIT_CONTEXT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_SCHEDULER_FEATURES {
    pub Anonymous: WHV_SCHEDULER_FEATURES_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_SCHEDULER_FEATURES {}
impl ::core::clone::Clone for WHV_SCHEDULER_FEATURES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_SCHEDULER_FEATURES_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_SCHEDULER_FEATURES_0 {}
impl ::core::clone::Clone for WHV_SCHEDULER_FEATURES_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHV_SRIOV_RESOURCE_DESCRIPTOR {
    pub PnpInstanceId: [u16; 200],
    pub VirtualFunctionId: super::super::Foundation::LUID,
    pub VirtualFunctionIndex: u16,
    pub Reserved: u16,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHV_SRIOV_RESOURCE_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHV_SRIOV_RESOURCE_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_SYNIC_EVENT_PARAMETERS {
    pub VpIndex: u32,
    pub TargetSint: u8,
    pub Reserved: u8,
    pub FlagNumber: u16,
}
impl ::core::marker::Copy for WHV_SYNIC_EVENT_PARAMETERS {}
impl ::core::clone::Clone for WHV_SYNIC_EVENT_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_SYNIC_SINT_DELIVERABLE_CONTEXT {
    pub DeliverableSints: u16,
    pub Reserved1: u16,
    pub Reserved2: u32,
}
impl ::core::marker::Copy for WHV_SYNIC_SINT_DELIVERABLE_CONTEXT {}
impl ::core::clone::Clone for WHV_SYNIC_SINT_DELIVERABLE_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_SYNTHETIC_PROCESSOR_FEATURES {
    pub Anonymous: WHV_SYNTHETIC_PROCESSOR_FEATURES_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_SYNTHETIC_PROCESSOR_FEATURES {}
impl ::core::clone::Clone for WHV_SYNTHETIC_PROCESSOR_FEATURES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_SYNTHETIC_PROCESSOR_FEATURES_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_SYNTHETIC_PROCESSOR_FEATURES_0 {}
impl ::core::clone::Clone for WHV_SYNTHETIC_PROCESSOR_FEATURES_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS {
    pub BanksCount: u32,
    pub Reserved0: u32,
    pub Anonymous: WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS_0,
}
impl ::core::marker::Copy for WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS {}
impl ::core::clone::Clone for WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS_0 {
    pub Anonymous: WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS_0_0,
    pub AsUINT64: [u64; 1],
}
impl ::core::marker::Copy for WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS_0 {}
impl ::core::clone::Clone for WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS_0_0 {
    pub Bank0: WHV_SYNTHETIC_PROCESSOR_FEATURES,
}
impl ::core::marker::Copy for WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS_0_0 {}
impl ::core::clone::Clone for WHV_SYNTHETIC_PROCESSOR_FEATURES_BANKS_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_TRANSLATE_GVA_RESULT {
    pub ResultCode: WHV_TRANSLATE_GVA_RESULT_CODE,
    pub Reserved: u32,
}
impl ::core::marker::Copy for WHV_TRANSLATE_GVA_RESULT {}
impl ::core::clone::Clone for WHV_TRANSLATE_GVA_RESULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_TRIGGER_PARAMETERS {
    pub TriggerType: WHV_TRIGGER_TYPE,
    pub Reserved: u32,
    pub Anonymous: WHV_TRIGGER_PARAMETERS_0,
}
impl ::core::marker::Copy for WHV_TRIGGER_PARAMETERS {}
impl ::core::clone::Clone for WHV_TRIGGER_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_TRIGGER_PARAMETERS_0 {
    pub Interrupt: WHV_INTERRUPT_CONTROL,
    pub SynicEvent: WHV_SYNIC_EVENT_PARAMETERS,
    pub DeviceInterrupt: WHV_TRIGGER_PARAMETERS_0_0,
}
impl ::core::marker::Copy for WHV_TRIGGER_PARAMETERS_0 {}
impl ::core::clone::Clone for WHV_TRIGGER_PARAMETERS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_TRIGGER_PARAMETERS_0_0 {
    pub LogicalDeviceId: u64,
    pub MsiAddress: u64,
    pub MsiData: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for WHV_TRIGGER_PARAMETERS_0_0 {}
impl ::core::clone::Clone for WHV_TRIGGER_PARAMETERS_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_UINT128 {
    pub Anonymous: WHV_UINT128_0,
    pub Dword: [u32; 4],
}
impl ::core::marker::Copy for WHV_UINT128 {}
impl ::core::clone::Clone for WHV_UINT128 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_UINT128_0 {
    pub Low64: u64,
    pub High64: u64,
}
impl ::core::marker::Copy for WHV_UINT128_0 {}
impl ::core::clone::Clone for WHV_UINT128_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VIRTUAL_PROCESSOR_PROPERTY {
    pub PropertyCode: WHV_VIRTUAL_PROCESSOR_PROPERTY_CODE,
    pub Reserved: u32,
    pub Anonymous: WHV_VIRTUAL_PROCESSOR_PROPERTY_0,
}
impl ::core::marker::Copy for WHV_VIRTUAL_PROCESSOR_PROPERTY {}
impl ::core::clone::Clone for WHV_VIRTUAL_PROCESSOR_PROPERTY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_VIRTUAL_PROCESSOR_PROPERTY_0 {
    pub NumaNode: u16,
    pub Padding: u64,
}
impl ::core::marker::Copy for WHV_VIRTUAL_PROCESSOR_PROPERTY_0 {}
impl ::core::clone::Clone for WHV_VIRTUAL_PROCESSOR_PROPERTY_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VPCI_DEVICE_NOTIFICATION {
    pub NotificationType: WHV_VPCI_DEVICE_NOTIFICATION_TYPE,
    pub Reserved1: u32,
    pub Anonymous: WHV_VPCI_DEVICE_NOTIFICATION_0,
}
impl ::core::marker::Copy for WHV_VPCI_DEVICE_NOTIFICATION {}
impl ::core::clone::Clone for WHV_VPCI_DEVICE_NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_VPCI_DEVICE_NOTIFICATION_0 {
    pub Reserved2: u64,
}
impl ::core::marker::Copy for WHV_VPCI_DEVICE_NOTIFICATION_0 {}
impl ::core::clone::Clone for WHV_VPCI_DEVICE_NOTIFICATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VPCI_DEVICE_REGISTER {
    pub Location: WHV_VPCI_DEVICE_REGISTER_SPACE,
    pub SizeInBytes: u32,
    pub OffsetInBytes: u64,
}
impl ::core::marker::Copy for WHV_VPCI_DEVICE_REGISTER {}
impl ::core::clone::Clone for WHV_VPCI_DEVICE_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VPCI_HARDWARE_IDS {
    pub VendorID: u16,
    pub DeviceID: u16,
    pub RevisionID: u8,
    pub ProgIf: u8,
    pub SubClass: u8,
    pub BaseClass: u8,
    pub SubVendorID: u16,
    pub SubSystemID: u16,
}
impl ::core::marker::Copy for WHV_VPCI_HARDWARE_IDS {}
impl ::core::clone::Clone for WHV_VPCI_HARDWARE_IDS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VPCI_INTERRUPT_TARGET {
    pub Vector: u32,
    pub Flags: WHV_VPCI_INTERRUPT_TARGET_FLAGS,
    pub ProcessorCount: u32,
    pub Processors: [u32; 1],
}
impl ::core::marker::Copy for WHV_VPCI_INTERRUPT_TARGET {}
impl ::core::clone::Clone for WHV_VPCI_INTERRUPT_TARGET {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VPCI_MMIO_MAPPING {
    pub Location: WHV_VPCI_DEVICE_REGISTER_SPACE,
    pub Flags: WHV_VPCI_MMIO_RANGE_FLAGS,
    pub SizeInBytes: u64,
    pub OffsetInBytes: u64,
    pub VirtualAddress: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for WHV_VPCI_MMIO_MAPPING {}
impl ::core::clone::Clone for WHV_VPCI_MMIO_MAPPING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VPCI_PROBED_BARS {
    pub Value: [u32; 6],
}
impl ::core::marker::Copy for WHV_VPCI_PROBED_BARS {}
impl ::core::clone::Clone for WHV_VPCI_PROBED_BARS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VP_EXCEPTION_CONTEXT {
    pub InstructionByteCount: u8,
    pub Reserved: [u8; 3],
    pub InstructionBytes: [u8; 16],
    pub ExceptionInfo: WHV_VP_EXCEPTION_INFO,
    pub ExceptionType: u8,
    pub Reserved2: [u8; 3],
    pub ErrorCode: u32,
    pub ExceptionParameter: u64,
}
impl ::core::marker::Copy for WHV_VP_EXCEPTION_CONTEXT {}
impl ::core::clone::Clone for WHV_VP_EXCEPTION_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_VP_EXCEPTION_INFO {
    pub Anonymous: WHV_VP_EXCEPTION_INFO_0,
    pub AsUINT32: u32,
}
impl ::core::marker::Copy for WHV_VP_EXCEPTION_INFO {}
impl ::core::clone::Clone for WHV_VP_EXCEPTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VP_EXCEPTION_INFO_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for WHV_VP_EXCEPTION_INFO_0 {}
impl ::core::clone::Clone for WHV_VP_EXCEPTION_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_VP_EXIT_CONTEXT {
    pub ExecutionState: WHV_X64_VP_EXECUTION_STATE,
    pub _bitfield: u8,
    pub Reserved: u8,
    pub Reserved2: u32,
    pub Cs: WHV_X64_SEGMENT_REGISTER,
    pub Rip: u64,
    pub Rflags: u64,
}
impl ::core::marker::Copy for WHV_VP_EXIT_CONTEXT {}
impl ::core::clone::Clone for WHV_VP_EXIT_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_APIC_EOI_CONTEXT {
    pub InterruptVector: u32,
}
impl ::core::marker::Copy for WHV_X64_APIC_EOI_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_APIC_EOI_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_APIC_INIT_SIPI_CONTEXT {
    pub ApicIcr: u64,
}
impl ::core::marker::Copy for WHV_X64_APIC_INIT_SIPI_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_APIC_INIT_SIPI_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_APIC_SMI_CONTEXT {
    pub ApicIcr: u64,
}
impl ::core::marker::Copy for WHV_X64_APIC_SMI_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_APIC_SMI_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_APIC_WRITE_CONTEXT {
    pub Type: WHV_X64_APIC_WRITE_TYPE,
    pub Reserved: u32,
    pub WriteValue: u64,
}
impl ::core::marker::Copy for WHV_X64_APIC_WRITE_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_APIC_WRITE_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_CPUID_ACCESS_CONTEXT {
    pub Rax: u64,
    pub Rcx: u64,
    pub Rdx: u64,
    pub Rbx: u64,
    pub DefaultResultRax: u64,
    pub DefaultResultRcx: u64,
    pub DefaultResultRdx: u64,
    pub DefaultResultRbx: u64,
}
impl ::core::marker::Copy for WHV_X64_CPUID_ACCESS_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_CPUID_ACCESS_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_CPUID_RESULT {
    pub Function: u32,
    pub Reserved: [u32; 3],
    pub Eax: u32,
    pub Ebx: u32,
    pub Ecx: u32,
    pub Edx: u32,
}
impl ::core::marker::Copy for WHV_X64_CPUID_RESULT {}
impl ::core::clone::Clone for WHV_X64_CPUID_RESULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_CPUID_RESULT2 {
    pub Function: u32,
    pub Index: u32,
    pub VpIndex: u32,
    pub Flags: WHV_X64_CPUID_RESULT2_FLAGS,
    pub Output: WHV_CPUID_OUTPUT,
    pub Mask: WHV_CPUID_OUTPUT,
}
impl ::core::marker::Copy for WHV_X64_CPUID_RESULT2 {}
impl ::core::clone::Clone for WHV_X64_CPUID_RESULT2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_DELIVERABILITY_NOTIFICATIONS_REGISTER {
    pub Anonymous: WHV_X64_DELIVERABILITY_NOTIFICATIONS_REGISTER_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_X64_DELIVERABILITY_NOTIFICATIONS_REGISTER {}
impl ::core::clone::Clone for WHV_X64_DELIVERABILITY_NOTIFICATIONS_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_DELIVERABILITY_NOTIFICATIONS_REGISTER_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_X64_DELIVERABILITY_NOTIFICATIONS_REGISTER_0 {}
impl ::core::clone::Clone for WHV_X64_DELIVERABILITY_NOTIFICATIONS_REGISTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_FP_CONTROL_STATUS_REGISTER {
    pub Anonymous: WHV_X64_FP_CONTROL_STATUS_REGISTER_0,
    pub AsUINT128: WHV_UINT128,
}
impl ::core::marker::Copy for WHV_X64_FP_CONTROL_STATUS_REGISTER {}
impl ::core::clone::Clone for WHV_X64_FP_CONTROL_STATUS_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_FP_CONTROL_STATUS_REGISTER_0 {
    pub FpControl: u16,
    pub FpStatus: u16,
    pub FpTag: u8,
    pub Reserved: u8,
    pub LastFpOp: u16,
    pub Anonymous: WHV_X64_FP_CONTROL_STATUS_REGISTER_0_0,
}
impl ::core::marker::Copy for WHV_X64_FP_CONTROL_STATUS_REGISTER_0 {}
impl ::core::clone::Clone for WHV_X64_FP_CONTROL_STATUS_REGISTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_FP_CONTROL_STATUS_REGISTER_0_0 {
    pub LastFpRip: u64,
    pub Anonymous: WHV_X64_FP_CONTROL_STATUS_REGISTER_0_0_0,
}
impl ::core::marker::Copy for WHV_X64_FP_CONTROL_STATUS_REGISTER_0_0 {}
impl ::core::clone::Clone for WHV_X64_FP_CONTROL_STATUS_REGISTER_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_FP_CONTROL_STATUS_REGISTER_0_0_0 {
    pub LastFpEip: u32,
    pub LastFpCs: u16,
    pub Reserved2: u16,
}
impl ::core::marker::Copy for WHV_X64_FP_CONTROL_STATUS_REGISTER_0_0_0 {}
impl ::core::clone::Clone for WHV_X64_FP_CONTROL_STATUS_REGISTER_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_FP_REGISTER {
    pub Anonymous: WHV_X64_FP_REGISTER_0,
    pub AsUINT128: WHV_UINT128,
}
impl ::core::marker::Copy for WHV_X64_FP_REGISTER {}
impl ::core::clone::Clone for WHV_X64_FP_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_FP_REGISTER_0 {
    pub Mantissa: u64,
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_X64_FP_REGISTER_0 {}
impl ::core::clone::Clone for WHV_X64_FP_REGISTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_INTERRUPTION_DELIVERABLE_CONTEXT {
    pub DeliverableType: WHV_X64_PENDING_INTERRUPTION_TYPE,
}
impl ::core::marker::Copy for WHV_X64_INTERRUPTION_DELIVERABLE_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_INTERRUPTION_DELIVERABLE_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_INTERRUPT_STATE_REGISTER {
    pub Anonymous: WHV_X64_INTERRUPT_STATE_REGISTER_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_X64_INTERRUPT_STATE_REGISTER {}
impl ::core::clone::Clone for WHV_X64_INTERRUPT_STATE_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_INTERRUPT_STATE_REGISTER_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_X64_INTERRUPT_STATE_REGISTER_0 {}
impl ::core::clone::Clone for WHV_X64_INTERRUPT_STATE_REGISTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_IO_PORT_ACCESS_CONTEXT {
    pub InstructionByteCount: u8,
    pub Reserved: [u8; 3],
    pub InstructionBytes: [u8; 16],
    pub AccessInfo: WHV_X64_IO_PORT_ACCESS_INFO,
    pub PortNumber: u16,
    pub Reserved2: [u16; 3],
    pub Rax: u64,
    pub Rcx: u64,
    pub Rsi: u64,
    pub Rdi: u64,
    pub Ds: WHV_X64_SEGMENT_REGISTER,
    pub Es: WHV_X64_SEGMENT_REGISTER,
}
impl ::core::marker::Copy for WHV_X64_IO_PORT_ACCESS_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_IO_PORT_ACCESS_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_IO_PORT_ACCESS_INFO {
    pub Anonymous: WHV_X64_IO_PORT_ACCESS_INFO_0,
    pub AsUINT32: u32,
}
impl ::core::marker::Copy for WHV_X64_IO_PORT_ACCESS_INFO {}
impl ::core::clone::Clone for WHV_X64_IO_PORT_ACCESS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_IO_PORT_ACCESS_INFO_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for WHV_X64_IO_PORT_ACCESS_INFO_0 {}
impl ::core::clone::Clone for WHV_X64_IO_PORT_ACCESS_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_MSR_ACCESS_CONTEXT {
    pub AccessInfo: WHV_X64_MSR_ACCESS_INFO,
    pub MsrNumber: u32,
    pub Rax: u64,
    pub Rdx: u64,
}
impl ::core::marker::Copy for WHV_X64_MSR_ACCESS_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_MSR_ACCESS_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_MSR_ACCESS_INFO {
    pub Anonymous: WHV_X64_MSR_ACCESS_INFO_0,
    pub AsUINT32: u32,
}
impl ::core::marker::Copy for WHV_X64_MSR_ACCESS_INFO {}
impl ::core::clone::Clone for WHV_X64_MSR_ACCESS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_MSR_ACCESS_INFO_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for WHV_X64_MSR_ACCESS_INFO_0 {}
impl ::core::clone::Clone for WHV_X64_MSR_ACCESS_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_MSR_EXIT_BITMAP {
    pub AsUINT64: u64,
    pub Anonymous: WHV_X64_MSR_EXIT_BITMAP_0,
}
impl ::core::marker::Copy for WHV_X64_MSR_EXIT_BITMAP {}
impl ::core::clone::Clone for WHV_X64_MSR_EXIT_BITMAP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_MSR_EXIT_BITMAP_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_X64_MSR_EXIT_BITMAP_0 {}
impl ::core::clone::Clone for WHV_X64_MSR_EXIT_BITMAP_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_PENDING_DEBUG_EXCEPTION {
    pub AsUINT64: u64,
    pub Anonymous: WHV_X64_PENDING_DEBUG_EXCEPTION_0,
}
impl ::core::marker::Copy for WHV_X64_PENDING_DEBUG_EXCEPTION {}
impl ::core::clone::Clone for WHV_X64_PENDING_DEBUG_EXCEPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_PENDING_DEBUG_EXCEPTION_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_X64_PENDING_DEBUG_EXCEPTION_0 {}
impl ::core::clone::Clone for WHV_X64_PENDING_DEBUG_EXCEPTION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_PENDING_EXCEPTION_EVENT {
    pub Anonymous: WHV_X64_PENDING_EXCEPTION_EVENT_0,
    pub AsUINT128: WHV_UINT128,
}
impl ::core::marker::Copy for WHV_X64_PENDING_EXCEPTION_EVENT {}
impl ::core::clone::Clone for WHV_X64_PENDING_EXCEPTION_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_PENDING_EXCEPTION_EVENT_0 {
    pub _bitfield: u32,
    pub ErrorCode: u32,
    pub ExceptionParameter: u64,
}
impl ::core::marker::Copy for WHV_X64_PENDING_EXCEPTION_EVENT_0 {}
impl ::core::clone::Clone for WHV_X64_PENDING_EXCEPTION_EVENT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_PENDING_EXT_INT_EVENT {
    pub Anonymous: WHV_X64_PENDING_EXT_INT_EVENT_0,
    pub AsUINT128: WHV_UINT128,
}
impl ::core::marker::Copy for WHV_X64_PENDING_EXT_INT_EVENT {}
impl ::core::clone::Clone for WHV_X64_PENDING_EXT_INT_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_PENDING_EXT_INT_EVENT_0 {
    pub _bitfield: u64,
    pub Reserved2: u64,
}
impl ::core::marker::Copy for WHV_X64_PENDING_EXT_INT_EVENT_0 {}
impl ::core::clone::Clone for WHV_X64_PENDING_EXT_INT_EVENT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_PENDING_INTERRUPTION_REGISTER {
    pub Anonymous: WHV_X64_PENDING_INTERRUPTION_REGISTER_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_X64_PENDING_INTERRUPTION_REGISTER {}
impl ::core::clone::Clone for WHV_X64_PENDING_INTERRUPTION_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_PENDING_INTERRUPTION_REGISTER_0 {
    pub _bitfield: u32,
    pub ErrorCode: u32,
}
impl ::core::marker::Copy for WHV_X64_PENDING_INTERRUPTION_REGISTER_0 {}
impl ::core::clone::Clone for WHV_X64_PENDING_INTERRUPTION_REGISTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_RDTSC_CONTEXT {
    pub TscAux: u64,
    pub VirtualOffset: u64,
    pub Tsc: u64,
    pub ReferenceTime: u64,
    pub RdtscInfo: WHV_X64_RDTSC_INFO,
}
impl ::core::marker::Copy for WHV_X64_RDTSC_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_RDTSC_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_RDTSC_INFO {
    pub Anonymous: WHV_X64_RDTSC_INFO_0,
    pub AsUINT64: u64,
}
impl ::core::marker::Copy for WHV_X64_RDTSC_INFO {}
impl ::core::clone::Clone for WHV_X64_RDTSC_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_RDTSC_INFO_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for WHV_X64_RDTSC_INFO_0 {}
impl ::core::clone::Clone for WHV_X64_RDTSC_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_SEGMENT_REGISTER {
    pub Base: u64,
    pub Limit: u32,
    pub Selector: u16,
    pub Anonymous: WHV_X64_SEGMENT_REGISTER_0,
}
impl ::core::marker::Copy for WHV_X64_SEGMENT_REGISTER {}
impl ::core::clone::Clone for WHV_X64_SEGMENT_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_SEGMENT_REGISTER_0 {
    pub Anonymous: WHV_X64_SEGMENT_REGISTER_0_0,
    pub Attributes: u16,
}
impl ::core::marker::Copy for WHV_X64_SEGMENT_REGISTER_0 {}
impl ::core::clone::Clone for WHV_X64_SEGMENT_REGISTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_SEGMENT_REGISTER_0_0 {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for WHV_X64_SEGMENT_REGISTER_0_0 {}
impl ::core::clone::Clone for WHV_X64_SEGMENT_REGISTER_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_TABLE_REGISTER {
    pub Pad: [u16; 3],
    pub Limit: u16,
    pub Base: u64,
}
impl ::core::marker::Copy for WHV_X64_TABLE_REGISTER {}
impl ::core::clone::Clone for WHV_X64_TABLE_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_UNSUPPORTED_FEATURE_CONTEXT {
    pub FeatureCode: WHV_X64_UNSUPPORTED_FEATURE_CODE,
    pub Reserved: u32,
    pub FeatureParameter: u64,
}
impl ::core::marker::Copy for WHV_X64_UNSUPPORTED_FEATURE_CONTEXT {}
impl ::core::clone::Clone for WHV_X64_UNSUPPORTED_FEATURE_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_VP_EXECUTION_STATE {
    pub Anonymous: WHV_X64_VP_EXECUTION_STATE_0,
    pub AsUINT16: u16,
}
impl ::core::marker::Copy for WHV_X64_VP_EXECUTION_STATE {}
impl ::core::clone::Clone for WHV_X64_VP_EXECUTION_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_VP_EXECUTION_STATE_0 {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for WHV_X64_VP_EXECUTION_STATE_0 {}
impl ::core::clone::Clone for WHV_X64_VP_EXECUTION_STATE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_XMM_CONTROL_STATUS_REGISTER {
    pub Anonymous: WHV_X64_XMM_CONTROL_STATUS_REGISTER_0,
    pub AsUINT128: WHV_UINT128,
}
impl ::core::marker::Copy for WHV_X64_XMM_CONTROL_STATUS_REGISTER {}
impl ::core::clone::Clone for WHV_X64_XMM_CONTROL_STATUS_REGISTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_XMM_CONTROL_STATUS_REGISTER_0 {
    pub Anonymous: WHV_X64_XMM_CONTROL_STATUS_REGISTER_0_0,
    pub XmmStatusControl: u32,
    pub XmmStatusControlMask: u32,
}
impl ::core::marker::Copy for WHV_X64_XMM_CONTROL_STATUS_REGISTER_0 {}
impl ::core::clone::Clone for WHV_X64_XMM_CONTROL_STATUS_REGISTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHV_X64_XMM_CONTROL_STATUS_REGISTER_0_0 {
    pub LastFpRdp: u64,
    pub Anonymous: WHV_X64_XMM_CONTROL_STATUS_REGISTER_0_0_0,
}
impl ::core::marker::Copy for WHV_X64_XMM_CONTROL_STATUS_REGISTER_0_0 {}
impl ::core::clone::Clone for WHV_X64_XMM_CONTROL_STATUS_REGISTER_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHV_X64_XMM_CONTROL_STATUS_REGISTER_0_0_0 {
    pub LastFpDp: u32,
    pub LastFpDs: u16,
    pub Reserved: u16,
}
impl ::core::marker::Copy for WHV_X64_XMM_CONTROL_STATUS_REGISTER_0_0_0 {}
impl ::core::clone::Clone for WHV_X64_XMM_CONTROL_STATUS_REGISTER_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type FOUND_IMAGE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, imageinfo: *const DOS_IMAGE_INFO) -> super::super::Foundation::BOOL>;
pub type GUEST_SYMBOLS_PROVIDER_DEBUG_INFO_CALLBACK = ::core::option::Option<unsafe extern "system" fn(infomessage: ::windows_sys::core::PCSTR) -> ()>;
pub type HDV_PCI_DEVICE_GET_DETAILS = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void, pnpid: *mut HDV_PCI_PNP_ID, probedbarscount: u32, probedbars: *mut u32) -> ::windows_sys::core::HRESULT>;
pub type HDV_PCI_DEVICE_INITIALIZE = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
pub type HDV_PCI_DEVICE_SET_CONFIGURATION = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void, configurationvaluecount: u32, configurationvalues: *const ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT>;
pub type HDV_PCI_DEVICE_START = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
pub type HDV_PCI_DEVICE_STOP = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void) -> ()>;
pub type HDV_PCI_DEVICE_TEARDOWN = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void) -> ()>;
pub type HDV_PCI_READ_CONFIG_SPACE = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void, offset: u32, value: *mut u32) -> ::windows_sys::core::HRESULT>;
pub type HDV_PCI_READ_INTERCEPTED_MEMORY = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void, barindex: HDV_PCI_BAR_SELECTOR, offset: u64, length: u64, value: *mut u8) -> ::windows_sys::core::HRESULT>;
pub type HDV_PCI_WRITE_CONFIG_SPACE = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void, offset: u32, value: u32) -> ::windows_sys::core::HRESULT>;
pub type HDV_PCI_WRITE_INTERCEPTED_MEMORY = ::core::option::Option<unsafe extern "system" fn(devicecontext: *const ::core::ffi::c_void, barindex: HDV_PCI_BAR_SELECTOR, offset: u64, length: u64, value: *const u8) -> ::windows_sys::core::HRESULT>;
pub type WHV_EMULATOR_GET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, registernames: *const WHV_REGISTER_NAME, registercount: u32, registervalues: *mut WHV_REGISTER_VALUE) -> ::windows_sys::core::HRESULT>;
pub type WHV_EMULATOR_IO_PORT_CALLBACK = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, ioaccess: *mut WHV_EMULATOR_IO_ACCESS_INFO) -> ::windows_sys::core::HRESULT>;
pub type WHV_EMULATOR_MEMORY_CALLBACK = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, memoryaccess: *mut WHV_EMULATOR_MEMORY_ACCESS_INFO) -> ::windows_sys::core::HRESULT>;
pub type WHV_EMULATOR_SET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, registernames: *const WHV_REGISTER_NAME, registercount: u32, registervalues: *const WHV_REGISTER_VALUE) -> ::windows_sys::core::HRESULT>;
pub type WHV_EMULATOR_TRANSLATE_GVA_PAGE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, gva: u64, translateflags: WHV_TRANSLATE_GVA_FLAGS, translationresult: *mut WHV_TRANSLATE_GVA_RESULT_CODE, gpa: *mut u64) -> ::windows_sys::core::HRESULT>;
