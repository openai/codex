#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CallNtPowerInformation(informationlevel : POWER_INFORMATION_LEVEL, inputbuffer : *const ::core::ffi::c_void, inputbufferlength : u32, outputbuffer : *mut ::core::ffi::c_void, outputbufferlength : u32) -> super::super::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CanUserWritePwrScheme() -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DeletePwrScheme(uiid : u32) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DevicePowerClose() -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DevicePowerEnumDevices(queryindex : u32, queryinterpretationflags : u32, queryflags : u32, preturnbuffer : *mut u8, pbuffersize : *mut u32) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DevicePowerOpen(debugmask : u32) -> super::super::Foundation:: BOOLEAN);
::windows_targets::link!("powrprof.dll" "system" fn DevicePowerSetDeviceState(devicedescription : ::windows_sys::core::PCWSTR, setflags : u32, setdata : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumPwrSchemes(lpfn : PWRSCHEMESENUMPROC, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetActivePwrScheme(puiid : *mut u32) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCurrentPowerPolicies(pglobalpowerpolicy : *mut GLOBAL_POWER_POLICY, ppowerpolicy : *mut POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDevicePowerState(hdevice : super::super::Foundation:: HANDLE, pfon : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetPwrCapabilities(lpspc : *mut SYSTEM_POWER_CAPABILITIES) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetPwrDiskSpindownRange(puimax : *mut u32, puimin : *mut u32) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetSystemPowerStatus(lpsystempowerstatus : *mut SYSTEM_POWER_STATUS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsAdminOverrideActive(papp : *const ADMINISTRATOR_POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsPwrHibernateAllowed() -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsPwrShutdownAllowed() -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsPwrSuspendAllowed() -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsSystemResumeAutomatic() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerCanRestoreIndividualDefaultPowerScheme(schemeguid : *const ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerClearRequest(powerrequest : super::super::Foundation:: HANDLE, requesttype : POWER_REQUEST_TYPE) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerCreatePossibleSetting(rootsystempowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, possiblesettingindex : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Threading\"`"] fn PowerCreateRequest(context : *const super::Threading:: REASON_CONTEXT) -> super::super::Foundation:: HANDLE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerCreateSetting(rootsystempowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerDeleteScheme(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
::windows_targets::link!("powrprof.dll" "system" fn PowerDeterminePlatformRole() -> POWER_PLATFORM_ROLE);
::windows_targets::link!("powrprof.dll" "system" fn PowerDeterminePlatformRoleEx(version : POWER_PLATFORM_ROLE_VERSION) -> POWER_PLATFORM_ROLE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerDuplicateScheme(rootpowerkey : super::Registry:: HKEY, sourceschemeguid : *const ::windows_sys::core::GUID, destinationschemeguid : *mut *mut ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerEnumerate(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, accessflags : POWER_DATA_ACCESSOR, index : u32, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerGetActiveScheme(userrootpowerkey : super::Registry:: HKEY, activepolicyguid : *mut *mut ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerImportPowerScheme(rootpowerkey : super::Registry:: HKEY, importfilenamepath : ::windows_sys::core::PCWSTR, destinationschemeguid : *mut *mut ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerIsSettingRangeDefined(subkeyguid : *const ::windows_sys::core::GUID, settingguid : *const ::windows_sys::core::GUID) -> super::super::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerOpenSystemPowerKey(phsystempowerkey : *mut super::Registry:: HKEY, access : u32, openexisting : super::super::Foundation:: BOOL) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerOpenUserPowerKey(phuserpowerkey : *mut super::Registry:: HKEY, access : u32, openexisting : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn PowerReadACDefaultIndex(rootpowerkey : super::Registry:: HKEY, schemepersonalityguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, acdefaultindex : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadACValue(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, r#type : *mut u32, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn PowerReadACValueIndex(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, acvalueindex : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn PowerReadDCDefaultIndex(rootpowerkey : super::Registry:: HKEY, schemepersonalityguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, dcdefaultindex : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadDCValue(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, r#type : *mut u32, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn PowerReadDCValueIndex(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, dcvalueindex : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadDescription(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadFriendlyName(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadIconResourceSpecifier(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadPossibleDescription(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, possiblesettingindex : u32, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadPossibleFriendlyName(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, possiblesettingindex : u32, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadPossibleValue(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, r#type : *mut u32, possiblesettingindex : u32, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
::windows_targets::link!("powrprof.dll" "system" fn PowerReadSettingAttributes(subgroupguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadValueIncrement(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, valueincrement : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadValueMax(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, valuemaximum : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadValueMin(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, valueminimum : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerReadValueUnitsSpecifier(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, buffer : *mut u8, buffersize : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
::windows_targets::link!("powrprof.dll" "system" fn PowerRegisterForEffectivePowerModeNotifications(version : u32, callback : EFFECTIVE_POWER_MODE_CALLBACK, context : *const ::core::ffi::c_void, registrationhandle : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn PowerRegisterSuspendResumeNotification(flags : super::super::UI::WindowsAndMessaging:: REGISTER_NOTIFICATION_FLAGS, recipient : super::super::Foundation:: HANDLE, registrationhandle : *mut *mut ::core::ffi::c_void) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerRemovePowerSetting(powersettingsubkeyguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
::windows_targets::link!("powrprof.dll" "system" fn PowerReplaceDefaultPowerSchemes() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerReportThermalEvent(event : *const THERMAL_EVENT) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerRestoreDefaultPowerSchemes() -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerRestoreIndividualDefaultPowerScheme(schemeguid : *const ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerSetActiveScheme(userrootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerSetRequest(powerrequest : super::super::Foundation:: HANDLE, requesttype : POWER_REQUEST_TYPE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerSettingAccessCheck(accessflags : POWER_DATA_ACCESSOR, powerguid : *const ::windows_sys::core::GUID) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerSettingAccessCheckEx(accessflags : POWER_DATA_ACCESSOR, powerguid : *const ::windows_sys::core::GUID, accesstype : super::Registry:: REG_SAM_FLAGS) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn PowerSettingRegisterNotification(settingguid : *const ::windows_sys::core::GUID, flags : super::super::UI::WindowsAndMessaging:: REGISTER_NOTIFICATION_FLAGS, recipient : super::super::Foundation:: HANDLE, registrationhandle : *mut *mut ::core::ffi::c_void) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerSettingUnregisterNotification(registrationhandle : HPOWERNOTIFY) -> super::super::Foundation:: WIN32_ERROR);
::windows_targets::link!("powrprof.dll" "system" fn PowerUnregisterFromEffectivePowerModeNotifications(registrationhandle : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerUnregisterSuspendResumeNotification(registrationhandle : HPOWERNOTIFY) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn PowerWriteACDefaultIndex(rootsystempowerkey : super::Registry:: HKEY, schemepersonalityguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, defaultacindex : u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn PowerWriteACValueIndex(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, acvalueindex : u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn PowerWriteDCDefaultIndex(rootsystempowerkey : super::Registry:: HKEY, schemepersonalityguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, defaultdcindex : u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn PowerWriteDCValueIndex(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, dcvalueindex : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWriteDescription(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, buffer : *const u8, buffersize : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWriteFriendlyName(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, buffer : *const u8, buffersize : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWriteIconResourceSpecifier(rootpowerkey : super::Registry:: HKEY, schemeguid : *const ::windows_sys::core::GUID, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, buffer : *const u8, buffersize : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWritePossibleDescription(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, possiblesettingindex : u32, buffer : *const u8, buffersize : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWritePossibleFriendlyName(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, possiblesettingindex : u32, buffer : *const u8, buffersize : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWritePossibleValue(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, r#type : u32, possiblesettingindex : u32, buffer : *const u8, buffersize : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PowerWriteSettingAttributes(subgroupguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, attributes : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWriteValueIncrement(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, valueincrement : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWriteValueMax(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, valuemaximum : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWriteValueMin(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, valueminimum : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PowerWriteValueUnitsSpecifier(rootpowerkey : super::Registry:: HKEY, subgroupofpowersettingsguid : *const ::windows_sys::core::GUID, powersettingguid : *const ::windows_sys::core::GUID, buffer : *const u8, buffersize : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReadGlobalPwrPolicy(pglobalpowerpolicy : *const GLOBAL_POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReadProcessorPwrScheme(uiid : u32, pmachineprocessorpowerpolicy : *mut MACHINE_PROCESSOR_POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReadPwrScheme(uiid : u32, ppowerpolicy : *mut POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterPowerSettingNotification(hrecipient : super::super::Foundation:: HANDLE, powersettingguid : *const ::windows_sys::core::GUID, flags : u32) -> HPOWERNOTIFY);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn RegisterSuspendResumeNotification(hrecipient : super::super::Foundation:: HANDLE, flags : super::super::UI::WindowsAndMessaging:: REGISTER_NOTIFICATION_FLAGS) -> HPOWERNOTIFY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RequestWakeupLatency(latency : LATENCY_TIME) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetActivePwrScheme(uiid : u32, pglobalpowerpolicy : *const GLOBAL_POWER_POLICY, ppowerpolicy : *const POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetSuspendState(bhibernate : super::super::Foundation:: BOOLEAN, bforce : super::super::Foundation:: BOOLEAN, bwakeupeventsdisabled : super::super::Foundation:: BOOLEAN) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetSystemPowerState(fsuspend : super::super::Foundation:: BOOL, fforce : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn SetThreadExecutionState(esflags : EXECUTION_STATE) -> EXECUTION_STATE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnregisterPowerSettingNotification(handle : HPOWERNOTIFY) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnregisterSuspendResumeNotification(handle : HPOWERNOTIFY) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ValidatePowerPolicies(pglobalpowerpolicy : *mut GLOBAL_POWER_POLICY, ppowerpolicy : *mut POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WriteGlobalPwrPolicy(pglobalpowerpolicy : *const GLOBAL_POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WriteProcessorPwrScheme(uiid : u32, pmachineprocessorpowerpolicy : *const MACHINE_PROCESSOR_POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("powrprof.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WritePwrScheme(puiid : *const u32, lpszschemename : ::windows_sys::core::PCWSTR, lpszdescription : ::windows_sys::core::PCWSTR, lpscheme : *const POWER_POLICY) -> super::super::Foundation:: BOOLEAN);
pub const ACCESS_ACTIVE_OVERLAY_SCHEME: POWER_DATA_ACCESSOR = 27i32;
pub const ACCESS_ACTIVE_SCHEME: POWER_DATA_ACCESSOR = 19i32;
pub const ACCESS_AC_POWER_SETTING_INDEX: POWER_DATA_ACCESSOR = 0i32;
pub const ACCESS_AC_POWER_SETTING_MAX: POWER_DATA_ACCESSOR = 21i32;
pub const ACCESS_AC_POWER_SETTING_MIN: POWER_DATA_ACCESSOR = 23i32;
pub const ACCESS_ATTRIBUTES: POWER_DATA_ACCESSOR = 15i32;
pub const ACCESS_CREATE_SCHEME: POWER_DATA_ACCESSOR = 20i32;
pub const ACCESS_DC_POWER_SETTING_INDEX: POWER_DATA_ACCESSOR = 1i32;
pub const ACCESS_DC_POWER_SETTING_MAX: POWER_DATA_ACCESSOR = 22i32;
pub const ACCESS_DC_POWER_SETTING_MIN: POWER_DATA_ACCESSOR = 24i32;
pub const ACCESS_DEFAULT_AC_POWER_SETTING: POWER_DATA_ACCESSOR = 7i32;
pub const ACCESS_DEFAULT_DC_POWER_SETTING: POWER_DATA_ACCESSOR = 8i32;
pub const ACCESS_DEFAULT_SECURITY_DESCRIPTOR: POWER_DATA_ACCESSOR = 14i32;
pub const ACCESS_DESCRIPTION: POWER_DATA_ACCESSOR = 3i32;
pub const ACCESS_FRIENDLY_NAME: POWER_DATA_ACCESSOR = 2i32;
pub const ACCESS_ICON_RESOURCE: POWER_DATA_ACCESSOR = 13i32;
pub const ACCESS_INDIVIDUAL_SETTING: POWER_DATA_ACCESSOR = 18i32;
pub const ACCESS_OVERLAY_SCHEME: POWER_DATA_ACCESSOR = 26i32;
pub const ACCESS_POSSIBLE_POWER_SETTING: POWER_DATA_ACCESSOR = 4i32;
pub const ACCESS_POSSIBLE_POWER_SETTING_DESCRIPTION: POWER_DATA_ACCESSOR = 6i32;
pub const ACCESS_POSSIBLE_POWER_SETTING_FRIENDLY_NAME: POWER_DATA_ACCESSOR = 5i32;
pub const ACCESS_POSSIBLE_VALUE_INCREMENT: POWER_DATA_ACCESSOR = 11i32;
pub const ACCESS_POSSIBLE_VALUE_MAX: POWER_DATA_ACCESSOR = 10i32;
pub const ACCESS_POSSIBLE_VALUE_MIN: POWER_DATA_ACCESSOR = 9i32;
pub const ACCESS_POSSIBLE_VALUE_UNITS: POWER_DATA_ACCESSOR = 12i32;
pub const ACCESS_PROFILE: POWER_DATA_ACCESSOR = 25i32;
pub const ACCESS_SCHEME: POWER_DATA_ACCESSOR = 16i32;
pub const ACCESS_SUBGROUP: POWER_DATA_ACCESSOR = 17i32;
pub const ACPI_TIME_ADJUST_DAYLIGHT: u32 = 1u32;
pub const ACPI_TIME_IN_DAYLIGHT: u32 = 2u32;
pub const ACPI_TIME_ZONE_UNKNOWN: u32 = 2047u32;
pub const ACTIVE_COOLING: u32 = 0u32;
pub const ALTITUDE_GROUP_POLICY: POWER_SETTING_ALTITUDE = 0i32;
pub const ALTITUDE_INTERNAL_OVERRIDE: POWER_SETTING_ALTITUDE = 5i32;
pub const ALTITUDE_OEM_CUSTOMIZATION: POWER_SETTING_ALTITUDE = 4i32;
pub const ALTITUDE_OS_DEFAULT: POWER_SETTING_ALTITUDE = 6i32;
pub const ALTITUDE_PROVISIONING: POWER_SETTING_ALTITUDE = 3i32;
pub const ALTITUDE_RUNTIME_OVERRIDE: POWER_SETTING_ALTITUDE = 2i32;
pub const ALTITUDE_USER: POWER_SETTING_ALTITUDE = 1i32;
pub const AcpiTimeResolutionMax: ACPI_TIME_RESOLUTION = 2i32;
pub const AcpiTimeResolutionMilliseconds: ACPI_TIME_RESOLUTION = 0i32;
pub const AcpiTimeResolutionSeconds: ACPI_TIME_RESOLUTION = 1i32;
pub const AdministratorPowerPolicy: POWER_INFORMATION_LEVEL = 9i32;
pub const BATTERY_CAPACITY_RELATIVE: u32 = 1073741824u32;
pub const BATTERY_CHARGING: u32 = 4u32;
pub const BATTERY_CLASS_MAJOR_VERSION: u32 = 1u32;
pub const BATTERY_CLASS_MINOR_VERSION: u32 = 0u32;
pub const BATTERY_CLASS_MINOR_VERSION_1: u32 = 1u32;
pub const BATTERY_CRITICAL: u32 = 8u32;
pub const BATTERY_CYCLE_COUNT_WMI_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xef98db24_0014_4c25_a50b_c724ae5cd371);
pub const BATTERY_DISCHARGING: u32 = 2u32;
pub const BATTERY_FULL_CHARGED_CAPACITY_WMI_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x40b40565_96f7_4435_8694_97e0e4395905);
pub const BATTERY_IS_SHORT_TERM: u32 = 536870912u32;
pub const BATTERY_MINIPORT_UPDATE_DATA_VER_1: u32 = 1u32;
pub const BATTERY_MINIPORT_UPDATE_DATA_VER_2: u32 = 2u32;
pub const BATTERY_POWER_ON_LINE: u32 = 1u32;
pub const BATTERY_RUNTIME_WMI_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x535a3767_1ac2_49bc_a077_3f7a02e40aec);
pub const BATTERY_SEALED: u32 = 268435456u32;
pub const BATTERY_SET_CHARGER_ID_SUPPORTED: u32 = 8u32;
pub const BATTERY_SET_CHARGE_SUPPORTED: u32 = 1u32;
pub const BATTERY_SET_CHARGINGSOURCE_SUPPORTED: u32 = 4u32;
pub const BATTERY_SET_DISCHARGE_SUPPORTED: u32 = 2u32;
pub const BATTERY_STATIC_DATA_WMI_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x05e1e463_e4e2_4ea9_80cb_9bd4b3ca0655);
pub const BATTERY_STATUS_CHANGE_WMI_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcddfa0c3_7c5b_4e43_a034_059fa5b84364);
pub const BATTERY_STATUS_WMI_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfc4670d1_ebbf_416e_87ce_374a4ebc111a);
pub const BATTERY_SYSTEM_BATTERY: u32 = 2147483648u32;
pub const BATTERY_TAG_CHANGE_WMI_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5e1f6e19_8786_4d23_94fc_9e746bd5d888);
pub const BATTERY_TAG_INVALID: u32 = 0u32;
pub const BATTERY_TEMPERATURE_WMI_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1a52a14d_adce_4a44_9a3e_c8d8f15ff2c2);
pub const BATTERY_UNKNOWN_CAPACITY: u32 = 4294967295u32;
pub const BATTERY_UNKNOWN_CURRENT: u32 = 4294967295u32;
pub const BATTERY_UNKNOWN_RATE: u32 = 2147483648u32;
pub const BATTERY_UNKNOWN_TIME: u32 = 4294967295u32;
pub const BATTERY_UNKNOWN_VOLTAGE: u32 = 4294967295u32;
pub const BATTERY_USB_CHARGER_STATUS_FN_DEFAULT_USB: u32 = 1u32;
pub const BATTERY_USB_CHARGER_STATUS_UCM_PD: u32 = 2u32;
pub const BatteryCharge: BATTERY_SET_INFORMATION_LEVEL = 1i32;
pub const BatteryChargerId: BATTERY_SET_INFORMATION_LEVEL = 4i32;
pub const BatteryChargerStatus: BATTERY_SET_INFORMATION_LEVEL = 5i32;
pub const BatteryChargingSource: BATTERY_SET_INFORMATION_LEVEL = 3i32;
pub const BatteryChargingSourceType_AC: BATTERY_CHARGING_SOURCE_TYPE = 1i32;
pub const BatteryChargingSourceType_Max: BATTERY_CHARGING_SOURCE_TYPE = 4i32;
pub const BatteryChargingSourceType_USB: BATTERY_CHARGING_SOURCE_TYPE = 2i32;
pub const BatteryChargingSourceType_Wireless: BATTERY_CHARGING_SOURCE_TYPE = 3i32;
pub const BatteryCriticalBias: BATTERY_SET_INFORMATION_LEVEL = 0i32;
pub const BatteryDeviceName: BATTERY_QUERY_INFORMATION_LEVEL = 4i32;
pub const BatteryDeviceState: POWER_INFORMATION_LEVEL = 86i32;
pub const BatteryDischarge: BATTERY_SET_INFORMATION_LEVEL = 2i32;
pub const BatteryEstimatedTime: BATTERY_QUERY_INFORMATION_LEVEL = 3i32;
pub const BatteryGranularityInformation: BATTERY_QUERY_INFORMATION_LEVEL = 1i32;
pub const BatteryInformation: BATTERY_QUERY_INFORMATION_LEVEL = 0i32;
pub const BatteryManufactureDate: BATTERY_QUERY_INFORMATION_LEVEL = 5i32;
pub const BatteryManufactureName: BATTERY_QUERY_INFORMATION_LEVEL = 6i32;
pub const BatterySerialNumber: BATTERY_QUERY_INFORMATION_LEVEL = 8i32;
pub const BatteryTemperature: BATTERY_QUERY_INFORMATION_LEVEL = 2i32;
pub const BatteryUniqueID: BATTERY_QUERY_INFORMATION_LEVEL = 7i32;
pub const BlackBoxRecorderDirectAccessBuffer: POWER_INFORMATION_LEVEL = 97i32;
pub const CsDeviceNotification: POWER_INFORMATION_LEVEL = 74i32;
pub const DEVICEPOWER_AND_OPERATION: u32 = 1073741824u32;
pub const DEVICEPOWER_CLEAR_WAKEENABLED: u32 = 2u32;
pub const DEVICEPOWER_FILTER_DEVICES_PRESENT: u32 = 536870912u32;
pub const DEVICEPOWER_FILTER_HARDWARE: u32 = 268435456u32;
pub const DEVICEPOWER_FILTER_ON_NAME: u32 = 33554432u32;
pub const DEVICEPOWER_FILTER_WAKEENABLED: u32 = 134217728u32;
pub const DEVICEPOWER_FILTER_WAKEPROGRAMMABLE: u32 = 67108864u32;
pub const DEVICEPOWER_HARDWAREID: u32 = 2147483648u32;
pub const DEVICEPOWER_SET_WAKEENABLED: u32 = 1u32;
pub const DisplayBurst: POWER_INFORMATION_LEVEL = 77i32;
pub const EFFECTIVE_POWER_MODE_V1: u32 = 1u32;
pub const EFFECTIVE_POWER_MODE_V2: u32 = 2u32;
pub const EMI_NAME_MAX: u32 = 16u32;
pub const EMI_VERSION_V1: u32 = 1u32;
pub const EMI_VERSION_V2: u32 = 2u32;
pub const ES_AWAYMODE_REQUIRED: EXECUTION_STATE = 64u32;
pub const ES_CONTINUOUS: EXECUTION_STATE = 2147483648u32;
pub const ES_DISPLAY_REQUIRED: EXECUTION_STATE = 2u32;
pub const ES_SYSTEM_REQUIRED: EXECUTION_STATE = 1u32;
pub const ES_USER_PRESENT: EXECUTION_STATE = 4u32;
pub const EffectivePowerModeBalanced: EFFECTIVE_POWER_MODE = 2i32;
pub const EffectivePowerModeBatterySaver: EFFECTIVE_POWER_MODE = 0i32;
pub const EffectivePowerModeBetterBattery: EFFECTIVE_POWER_MODE = 1i32;
pub const EffectivePowerModeGameMode: EFFECTIVE_POWER_MODE = 5i32;
pub const EffectivePowerModeHighPerformance: EFFECTIVE_POWER_MODE = 3i32;
pub const EffectivePowerModeMaxPerformance: EFFECTIVE_POWER_MODE = 4i32;
pub const EffectivePowerModeMixedReality: EFFECTIVE_POWER_MODE = 6i32;
pub const EmiMeasurementUnitPicowattHours: EMI_MEASUREMENT_UNIT = 0i32;
pub const EnableMultiBatteryDisplay: u32 = 2u32;
pub const EnablePasswordLogon: u32 = 4u32;
pub const EnableSysTrayBatteryMeter: u32 = 1u32;
pub const EnableVideoDimDisplay: u32 = 16u32;
pub const EnableWakeOnRing: u32 = 8u32;
pub const EnergyTrackerCreate: POWER_INFORMATION_LEVEL = 92i32;
pub const EnergyTrackerQuery: POWER_INFORMATION_LEVEL = 93i32;
pub const ExitLatencySamplingPercentage: POWER_INFORMATION_LEVEL = 78i32;
pub const FirmwareTableInformationRegistered: POWER_INFORMATION_LEVEL = 69i32;
pub const GUID_CLASS_INPUT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030);
pub const GUID_DEVICE_ACPI_TIME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x97f99bf6_4497_4f18_bb22_4b9fb2fbef9c);
pub const GUID_DEVICE_APPLICATIONLAUNCH_BUTTON: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x629758ee_986e_4d9e_8e47_de27f8ab054d);
pub const GUID_DEVICE_BATTERY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x72631e54_78a4_11d0_bcf7_00aa00b7b32a);
pub const GUID_DEVICE_ENERGY_METER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x45bd8344_7ed6_49cf_a440_c276c933b053);
pub const GUID_DEVICE_FAN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x05ecd13d_81da_4a2a_8a4c_524f23dd4dc9);
pub const GUID_DEVICE_LID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4afa3d52_74a7_11d0_be5e_00a0c9062857);
pub const GUID_DEVICE_MEMORY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3fd0f03d_92e0_45fb_b75c_5ed8ffb01021);
pub const GUID_DEVICE_MESSAGE_INDICATOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcd48a365_fa94_4ce2_a232_a1b764e5d8b4);
pub const GUID_DEVICE_PROCESSOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x97fadb10_4e33_40ae_359c_8bef029dbdd0);
pub const GUID_DEVICE_SYS_BUTTON: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4afa3d53_74a7_11d0_be5e_00a0c9062857);
pub const GUID_DEVICE_THERMAL_ZONE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4afa3d51_74a7_11d0_be5e_00a0c9062857);
pub const GUID_DEVINTERFACE_THERMAL_COOLING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdbe4373d_3c81_40cb_ace4_e0e5d05f0c9f);
pub const GUID_DEVINTERFACE_THERMAL_MANAGER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x927ec093_69a4_4bc0_bd02_711664714463);
pub const GetPowerRequestList: POWER_INFORMATION_LEVEL = 45i32;
pub const GetPowerSettingValue: POWER_INFORMATION_LEVEL = 59i32;
pub const GroupPark: POWER_INFORMATION_LEVEL = 48i32;
pub const IOCTL_ACPI_GET_REAL_TIME: u32 = 2703888u32;
pub const IOCTL_ACPI_SET_REAL_TIME: u32 = 2720276u32;
pub const IOCTL_BATTERY_CHARGING_SOURCE_CHANGE: u32 = 2703440u32;
pub const IOCTL_BATTERY_QUERY_INFORMATION: u32 = 2703428u32;
pub const IOCTL_BATTERY_QUERY_STATUS: u32 = 2703436u32;
pub const IOCTL_BATTERY_QUERY_TAG: u32 = 2703424u32;
pub const IOCTL_BATTERY_SET_INFORMATION: u32 = 2719816u32;
pub const IOCTL_EMI_GET_MEASUREMENT: u32 = 2244620u32;
pub const IOCTL_EMI_GET_METADATA: u32 = 2244616u32;
pub const IOCTL_EMI_GET_METADATA_SIZE: u32 = 2244612u32;
pub const IOCTL_EMI_GET_VERSION: u32 = 2244608u32;
pub const IOCTL_GET_ACPI_TIME_AND_ALARM_CAPABILITIES: u32 = 2703900u32;
pub const IOCTL_GET_PROCESSOR_OBJ_INFO: u32 = 2703744u32;
pub const IOCTL_GET_SYS_BUTTON_CAPS: u32 = 2703680u32;
pub const IOCTL_GET_SYS_BUTTON_EVENT: u32 = 2703684u32;
pub const IOCTL_GET_WAKE_ALARM_POLICY: u32 = 2736652u32;
pub const IOCTL_GET_WAKE_ALARM_SYSTEM_POWERSTATE: u32 = 2703896u32;
pub const IOCTL_GET_WAKE_ALARM_VALUE: u32 = 2736648u32;
pub const IOCTL_NOTIFY_SWITCH_EVENT: u32 = 2703616u32;
pub const IOCTL_QUERY_LID: u32 = 2703552u32;
pub const IOCTL_RUN_ACTIVE_COOLING_METHOD: u32 = 2719880u32;
pub const IOCTL_SET_SYS_MESSAGE_INDICATOR: u32 = 2720192u32;
pub const IOCTL_SET_WAKE_ALARM_POLICY: u32 = 2720260u32;
pub const IOCTL_SET_WAKE_ALARM_VALUE: u32 = 2720256u32;
pub const IOCTL_THERMAL_QUERY_INFORMATION: u32 = 2703488u32;
pub const IOCTL_THERMAL_READ_POLICY: u32 = 2703508u32;
pub const IOCTL_THERMAL_READ_TEMPERATURE: u32 = 2703504u32;
pub const IOCTL_THERMAL_SET_COOLING_POLICY: u32 = 2719876u32;
pub const IOCTL_THERMAL_SET_PASSIVE_LIMIT: u32 = 2719884u32;
pub const IdleResiliency: POWER_INFORMATION_LEVEL = 60i32;
pub const LT_DONT_CARE: LATENCY_TIME = 0i32;
pub const LT_LOWEST_LATENCY: LATENCY_TIME = 1i32;
pub const LastResumePerformance: POWER_INFORMATION_LEVEL = 76i32;
pub const LastSleepTime: POWER_INFORMATION_LEVEL = 15i32;
pub const LastWakeTime: POWER_INFORMATION_LEVEL = 14i32;
pub const LogicalProcessorIdling: POWER_INFORMATION_LEVEL = 56i32;
pub const MAX_ACTIVE_COOLING_LEVELS: u32 = 10u32;
pub const MAX_BATTERY_STRING_SIZE: u32 = 128u32;
pub const MonitorCapabilities: POWER_INFORMATION_LEVEL = 40i32;
pub const MonitorInvocation: POWER_INFORMATION_LEVEL = 68i32;
pub const MonitorRequestReasonAcDcDisplayBurst: POWER_MONITOR_REQUEST_REASON = 5i32;
pub const MonitorRequestReasonAcDcDisplayBurstSuppressed: POWER_MONITOR_REQUEST_REASON = 28i32;
pub const MonitorRequestReasonBatteryCountChange: POWER_MONITOR_REQUEST_REASON = 16i32;
pub const MonitorRequestReasonBatteryCountChangeSuppressed: POWER_MONITOR_REQUEST_REASON = 49i32;
pub const MonitorRequestReasonBatteryPreCritical: POWER_MONITOR_REQUEST_REASON = 53i32;
pub const MonitorRequestReasonBuiltinPanel: POWER_MONITOR_REQUEST_REASON = 47i32;
pub const MonitorRequestReasonDP: POWER_MONITOR_REQUEST_REASON = 19i32;
pub const MonitorRequestReasonDim: POWER_MONITOR_REQUEST_REASON = 46i32;
pub const MonitorRequestReasonDirectedDrips: POWER_MONITOR_REQUEST_REASON = 45i32;
pub const MonitorRequestReasonDisplayRequiredUnDim: POWER_MONITOR_REQUEST_REASON = 48i32;
pub const MonitorRequestReasonFullWake: POWER_MONITOR_REQUEST_REASON = 9i32;
pub const MonitorRequestReasonGracePeriod: POWER_MONITOR_REQUEST_REASON = 17i32;
pub const MonitorRequestReasonIdleTimeout: POWER_MONITOR_REQUEST_REASON = 12i32;
pub const MonitorRequestReasonLid: POWER_MONITOR_REQUEST_REASON = 15i32;
pub const MonitorRequestReasonMax: POWER_MONITOR_REQUEST_REASON = 55i32;
pub const MonitorRequestReasonNearProximity: POWER_MONITOR_REQUEST_REASON = 22i32;
pub const MonitorRequestReasonPdcSignal: POWER_MONITOR_REQUEST_REASON = 27i32;
pub const MonitorRequestReasonPdcSignalFingerprint: POWER_MONITOR_REQUEST_REASON = 44i32;
pub const MonitorRequestReasonPdcSignalHeyCortana: POWER_MONITOR_REQUEST_REASON = 42i32;
pub const MonitorRequestReasonPdcSignalHolographicShell: POWER_MONITOR_REQUEST_REASON = 43i32;
pub const MonitorRequestReasonPdcSignalSensorsHumanPresence: POWER_MONITOR_REQUEST_REASON = 52i32;
pub const MonitorRequestReasonPdcSignalWindowsMobilePwrNotif: POWER_MONITOR_REQUEST_REASON = 40i32;
pub const MonitorRequestReasonPdcSignalWindowsMobileShell: POWER_MONITOR_REQUEST_REASON = 41i32;
pub const MonitorRequestReasonPnP: POWER_MONITOR_REQUEST_REASON = 18i32;
pub const MonitorRequestReasonPoSetSystemState: POWER_MONITOR_REQUEST_REASON = 7i32;
pub const MonitorRequestReasonPolicyChange: POWER_MONITOR_REQUEST_REASON = 13i32;
pub const MonitorRequestReasonPowerButton: POWER_MONITOR_REQUEST_REASON = 1i32;
pub const MonitorRequestReasonRemoteConnection: POWER_MONITOR_REQUEST_REASON = 2i32;
pub const MonitorRequestReasonResumeModernStandby: POWER_MONITOR_REQUEST_REASON = 50i32;
pub const MonitorRequestReasonResumePdc: POWER_MONITOR_REQUEST_REASON = 24i32;
pub const MonitorRequestReasonResumeS4: POWER_MONITOR_REQUEST_REASON = 25i32;
pub const MonitorRequestReasonScMonitorpower: POWER_MONITOR_REQUEST_REASON = 3i32;
pub const MonitorRequestReasonScreenOffRequest: POWER_MONITOR_REQUEST_REASON = 11i32;
pub const MonitorRequestReasonSessionUnlock: POWER_MONITOR_REQUEST_REASON = 10i32;
pub const MonitorRequestReasonSetThreadExecutionState: POWER_MONITOR_REQUEST_REASON = 8i32;
pub const MonitorRequestReasonSleepButton: POWER_MONITOR_REQUEST_REASON = 14i32;
pub const MonitorRequestReasonSxTransition: POWER_MONITOR_REQUEST_REASON = 20i32;
pub const MonitorRequestReasonSystemIdle: POWER_MONITOR_REQUEST_REASON = 21i32;
pub const MonitorRequestReasonSystemStateEntered: POWER_MONITOR_REQUEST_REASON = 29i32;
pub const MonitorRequestReasonTerminal: POWER_MONITOR_REQUEST_REASON = 26i32;
pub const MonitorRequestReasonTerminalInit: POWER_MONITOR_REQUEST_REASON = 51i32;
pub const MonitorRequestReasonThermalStandby: POWER_MONITOR_REQUEST_REASON = 23i32;
pub const MonitorRequestReasonUnknown: POWER_MONITOR_REQUEST_REASON = 0i32;
pub const MonitorRequestReasonUserDisplayBurst: POWER_MONITOR_REQUEST_REASON = 6i32;
pub const MonitorRequestReasonUserInput: POWER_MONITOR_REQUEST_REASON = 4i32;
pub const MonitorRequestReasonUserInputAccelerometer: POWER_MONITOR_REQUEST_REASON = 35i32;
pub const MonitorRequestReasonUserInputHid: POWER_MONITOR_REQUEST_REASON = 36i32;
pub const MonitorRequestReasonUserInputInitialization: POWER_MONITOR_REQUEST_REASON = 39i32;
pub const MonitorRequestReasonUserInputKeyboard: POWER_MONITOR_REQUEST_REASON = 31i32;
pub const MonitorRequestReasonUserInputMouse: POWER_MONITOR_REQUEST_REASON = 32i32;
pub const MonitorRequestReasonUserInputPen: POWER_MONITOR_REQUEST_REASON = 34i32;
pub const MonitorRequestReasonUserInputPoUserPresent: POWER_MONITOR_REQUEST_REASON = 37i32;
pub const MonitorRequestReasonUserInputSessionSwitch: POWER_MONITOR_REQUEST_REASON = 38i32;
pub const MonitorRequestReasonUserInputTouch: POWER_MONITOR_REQUEST_REASON = 54i32;
pub const MonitorRequestReasonUserInputTouchpad: POWER_MONITOR_REQUEST_REASON = 33i32;
pub const MonitorRequestReasonWinrt: POWER_MONITOR_REQUEST_REASON = 30i32;
pub const MonitorRequestTypeOff: POWER_MONITOR_REQUEST_TYPE = 0i32;
pub const MonitorRequestTypeOnAndPresent: POWER_MONITOR_REQUEST_TYPE = 1i32;
pub const MonitorRequestTypeToggleOn: POWER_MONITOR_REQUEST_TYPE = 2i32;
pub const NotifyUserModeLegacyPowerEvent: POWER_INFORMATION_LEVEL = 47i32;
pub const NotifyUserPowerSetting: POWER_INFORMATION_LEVEL = 26i32;
pub const PASSIVE_COOLING: u32 = 1u32;
pub const PDCAP_S0_SUPPORTED: u32 = 65536u32;
pub const PDCAP_S1_SUPPORTED: u32 = 131072u32;
pub const PDCAP_S2_SUPPORTED: u32 = 262144u32;
pub const PDCAP_S3_SUPPORTED: u32 = 524288u32;
pub const PDCAP_S4_SUPPORTED: u32 = 16777216u32;
pub const PDCAP_S5_SUPPORTED: u32 = 33554432u32;
pub const PDCAP_WAKE_FROM_S0_SUPPORTED: u32 = 1048576u32;
pub const PDCAP_WAKE_FROM_S1_SUPPORTED: u32 = 2097152u32;
pub const PDCAP_WAKE_FROM_S2_SUPPORTED: u32 = 4194304u32;
pub const PDCAP_WAKE_FROM_S3_SUPPORTED: u32 = 8388608u32;
pub const POWER_ATTRIBUTE_HIDE: u32 = 1u32;
pub const POWER_ATTRIBUTE_SHOW_AOAC: u32 = 2u32;
pub const POWER_FORCE_TRIGGER_RESET: POWER_ACTION_POLICY_EVENT_CODE = 2147483648u32;
pub const POWER_LEVEL_USER_NOTIFY_EXEC: POWER_ACTION_POLICY_EVENT_CODE = 4u32;
pub const POWER_LEVEL_USER_NOTIFY_SOUND: POWER_ACTION_POLICY_EVENT_CODE = 2u32;
pub const POWER_LEVEL_USER_NOTIFY_TEXT: POWER_ACTION_POLICY_EVENT_CODE = 1u32;
pub const POWER_PLATFORM_ROLE_V1: POWER_PLATFORM_ROLE_VERSION = 1u32;
pub const POWER_PLATFORM_ROLE_V2: POWER_PLATFORM_ROLE_VERSION = 2u32;
pub const POWER_USER_NOTIFY_BUTTON: POWER_ACTION_POLICY_EVENT_CODE = 8u32;
pub const POWER_USER_NOTIFY_SHUTDOWN: POWER_ACTION_POLICY_EVENT_CODE = 16u32;
pub const PO_TZ_ACTIVE: POWER_COOLING_MODE = 0u16;
pub const PO_TZ_INVALID_MODE: POWER_COOLING_MODE = 2u16;
pub const PO_TZ_PASSIVE: POWER_COOLING_MODE = 1u16;
pub const PPM_FIRMWARE_ACPI1C2: u32 = 1u32;
pub const PPM_FIRMWARE_ACPI1C3: u32 = 2u32;
pub const PPM_FIRMWARE_ACPI1TSTATES: u32 = 4u32;
pub const PPM_FIRMWARE_CPC: u32 = 262144u32;
pub const PPM_FIRMWARE_CSD: u32 = 16u32;
pub const PPM_FIRMWARE_CST: u32 = 8u32;
pub const PPM_FIRMWARE_LPI: u32 = 524288u32;
pub const PPM_FIRMWARE_OSC: u32 = 65536u32;
pub const PPM_FIRMWARE_PCCH: u32 = 16384u32;
pub const PPM_FIRMWARE_PCCP: u32 = 32768u32;
pub const PPM_FIRMWARE_PCT: u32 = 32u32;
pub const PPM_FIRMWARE_PDC: u32 = 131072u32;
pub const PPM_FIRMWARE_PPC: u32 = 256u32;
pub const PPM_FIRMWARE_PSD: u32 = 512u32;
pub const PPM_FIRMWARE_PSS: u32 = 64u32;
pub const PPM_FIRMWARE_PTC: u32 = 1024u32;
pub const PPM_FIRMWARE_TPC: u32 = 4096u32;
pub const PPM_FIRMWARE_TSD: u32 = 8192u32;
pub const PPM_FIRMWARE_TSS: u32 = 2048u32;
pub const PPM_FIRMWARE_XPSS: u32 = 128u32;
pub const PPM_IDLESTATES_DATA_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xba138e10_e250_4ad7_8616_cf1a7ad410e7);
pub const PPM_IDLESTATE_CHANGE_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4838fe4f_f71c_4e51_9ecc_8430a7ac4c6c);
pub const PPM_IDLE_ACCOUNTING_EX_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd67abd39_81f8_4a5e_8152_72e31ec912ee);
pub const PPM_IDLE_ACCOUNTING_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe2a26f78_ae07_4ee0_a30f_ce54f55a94cd);
pub const PPM_IDLE_IMPLEMENTATION_CSTATES: u32 = 1u32;
pub const PPM_IDLE_IMPLEMENTATION_LPISTATES: u32 = 4u32;
pub const PPM_IDLE_IMPLEMENTATION_MICROPEP: u32 = 3u32;
pub const PPM_IDLE_IMPLEMENTATION_NONE: u32 = 0u32;
pub const PPM_IDLE_IMPLEMENTATION_PEP: u32 = 2u32;
pub const PPM_PERFMON_PERFSTATE_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7fd18652_0cfe_40d2_b0a1_0b066a87759e);
pub const PPM_PERFORMANCE_IMPLEMENTATION_CPPC: u32 = 3u32;
pub const PPM_PERFORMANCE_IMPLEMENTATION_NONE: u32 = 0u32;
pub const PPM_PERFORMANCE_IMPLEMENTATION_PCCV1: u32 = 2u32;
pub const PPM_PERFORMANCE_IMPLEMENTATION_PEP: u32 = 4u32;
pub const PPM_PERFORMANCE_IMPLEMENTATION_PSTATES: u32 = 1u32;
pub const PPM_PERFSTATES_DATA_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5708cc20_7d40_4bf4_b4aa_2b01338d0126);
pub const PPM_PERFSTATE_CHANGE_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa5b32ddd_7f39_4abc_b892_900e43b59ebb);
pub const PPM_PERFSTATE_DOMAIN_CHANGE_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x995e6b7f_d653_497a_b978_36a30c29bf01);
pub const PPM_THERMALCONSTRAINT_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa852c2c8_1a4c_423b_8c2c_f30d82931a88);
pub const PPM_THERMAL_POLICY_CHANGE_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x48f377b8_6880_4c7b_8bdc_380176c6654d);
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const PROCESSOR_NUMBER_PKEY: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x5724c81d_d5af_4c1f_a103_a06e28f204c6), pid: 1 };
pub const PdcInvocation: POWER_INFORMATION_LEVEL = 67i32;
pub const PhysicalPowerButtonPress: POWER_INFORMATION_LEVEL = 90i32;
pub const PlatformIdleStates: POWER_INFORMATION_LEVEL = 80i32;
pub const PlatformIdleVeto: POWER_INFORMATION_LEVEL = 82i32;
pub const PlatformInformation: POWER_INFORMATION_LEVEL = 66i32;
pub const PlatformRole: POWER_INFORMATION_LEVEL = 75i32;
pub const PlatformRoleAppliancePC: POWER_PLATFORM_ROLE = 6i32;
pub const PlatformRoleDesktop: POWER_PLATFORM_ROLE = 1i32;
pub const PlatformRoleEnterpriseServer: POWER_PLATFORM_ROLE = 4i32;
pub const PlatformRoleMaximum: POWER_PLATFORM_ROLE = 9i32;
pub const PlatformRoleMobile: POWER_PLATFORM_ROLE = 2i32;
pub const PlatformRolePerformanceServer: POWER_PLATFORM_ROLE = 7i32;
pub const PlatformRoleSOHOServer: POWER_PLATFORM_ROLE = 5i32;
pub const PlatformRoleSlate: POWER_PLATFORM_ROLE = 8i32;
pub const PlatformRoleUnspecified: POWER_PLATFORM_ROLE = 0i32;
pub const PlatformRoleWorkstation: POWER_PLATFORM_ROLE = 3i32;
pub const PlmPowerRequestCreate: POWER_INFORMATION_LEVEL = 72i32;
pub const PoAc: SYSTEM_POWER_CONDITION = 0i32;
pub const PoConditionMaximum: SYSTEM_POWER_CONDITION = 3i32;
pub const PoDc: SYSTEM_POWER_CONDITION = 1i32;
pub const PoHot: SYSTEM_POWER_CONDITION = 2i32;
pub const PowerActionDisplayOff: POWER_ACTION = 8i32;
pub const PowerActionHibernate: POWER_ACTION = 3i32;
pub const PowerActionNone: POWER_ACTION = 0i32;
pub const PowerActionReserved: POWER_ACTION = 1i32;
pub const PowerActionShutdown: POWER_ACTION = 4i32;
pub const PowerActionShutdownOff: POWER_ACTION = 6i32;
pub const PowerActionShutdownReset: POWER_ACTION = 5i32;
pub const PowerActionSleep: POWER_ACTION = 2i32;
pub const PowerActionWarmEject: POWER_ACTION = 7i32;
pub const PowerDeviceD0: DEVICE_POWER_STATE = 1i32;
pub const PowerDeviceD1: DEVICE_POWER_STATE = 2i32;
pub const PowerDeviceD2: DEVICE_POWER_STATE = 3i32;
pub const PowerDeviceD3: DEVICE_POWER_STATE = 4i32;
pub const PowerDeviceMaximum: DEVICE_POWER_STATE = 5i32;
pub const PowerDeviceUnspecified: DEVICE_POWER_STATE = 0i32;
pub const PowerInformationInternal: POWER_INFORMATION_LEVEL = 87i32;
pub const PowerInformationLevelMaximum: POWER_INFORMATION_LEVEL = 98i32;
pub const PowerInformationLevelUnused0: POWER_INFORMATION_LEVEL = 27i32;
pub const PowerRequestAction: POWER_INFORMATION_LEVEL = 44i32;
pub const PowerRequestActionInternal: POWER_INFORMATION_LEVEL = 85i32;
pub const PowerRequestAwayModeRequired: POWER_REQUEST_TYPE = 2i32;
pub const PowerRequestCreate: POWER_INFORMATION_LEVEL = 43i32;
pub const PowerRequestDisplayRequired: POWER_REQUEST_TYPE = 0i32;
pub const PowerRequestExecutionRequired: POWER_REQUEST_TYPE = 3i32;
pub const PowerRequestSystemRequired: POWER_REQUEST_TYPE = 1i32;
pub const PowerSettingNotificationName: POWER_INFORMATION_LEVEL = 58i32;
pub const PowerShutdownNotification: POWER_INFORMATION_LEVEL = 39i32;
pub const PowerSystemHibernate: SYSTEM_POWER_STATE = 5i32;
pub const PowerSystemMaximum: SYSTEM_POWER_STATE = 7i32;
pub const PowerSystemShutdown: SYSTEM_POWER_STATE = 6i32;
pub const PowerSystemSleeping1: SYSTEM_POWER_STATE = 2i32;
pub const PowerSystemSleeping2: SYSTEM_POWER_STATE = 3i32;
pub const PowerSystemSleeping3: SYSTEM_POWER_STATE = 4i32;
pub const PowerSystemUnspecified: SYSTEM_POWER_STATE = 0i32;
pub const PowerSystemWorking: SYSTEM_POWER_STATE = 1i32;
pub const PowerUserInactive: USER_ACTIVITY_PRESENCE = 2i32;
pub const PowerUserInvalid: USER_ACTIVITY_PRESENCE = 3i32;
pub const PowerUserMaximum: USER_ACTIVITY_PRESENCE = 3i32;
pub const PowerUserNotPresent: USER_ACTIVITY_PRESENCE = 1i32;
pub const PowerUserPresent: USER_ACTIVITY_PRESENCE = 0i32;
pub const ProcessorCap: POWER_INFORMATION_LEVEL = 34i32;
pub const ProcessorIdleDomains: POWER_INFORMATION_LEVEL = 49i32;
pub const ProcessorIdleStates: POWER_INFORMATION_LEVEL = 33i32;
pub const ProcessorIdleStatesHv: POWER_INFORMATION_LEVEL = 52i32;
pub const ProcessorIdleVeto: POWER_INFORMATION_LEVEL = 81i32;
pub const ProcessorInformation: POWER_INFORMATION_LEVEL = 11i32;
pub const ProcessorInformationEx: POWER_INFORMATION_LEVEL = 46i32;
pub const ProcessorLoad: POWER_INFORMATION_LEVEL = 38i32;
pub const ProcessorPerfCapHv: POWER_INFORMATION_LEVEL = 54i32;
pub const ProcessorPerfStates: POWER_INFORMATION_LEVEL = 32i32;
pub const ProcessorPerfStatesHv: POWER_INFORMATION_LEVEL = 53i32;
pub const ProcessorPowerPolicyAc: POWER_INFORMATION_LEVEL = 18i32;
pub const ProcessorPowerPolicyCurrent: POWER_INFORMATION_LEVEL = 22i32;
pub const ProcessorPowerPolicyDc: POWER_INFORMATION_LEVEL = 19i32;
pub const ProcessorSetIdle: POWER_INFORMATION_LEVEL = 55i32;
pub const ProcessorStateHandler: POWER_INFORMATION_LEVEL = 7i32;
pub const ProcessorStateHandler2: POWER_INFORMATION_LEVEL = 13i32;
pub const QueryPotentialDripsConstraint: POWER_INFORMATION_LEVEL = 91i32;
pub const RegisterSpmPowerSettings: POWER_INFORMATION_LEVEL = 79i32;
pub const SYS_BUTTON_LID: u32 = 4u32;
pub const SYS_BUTTON_LID_CHANGED: u32 = 524288u32;
pub const SYS_BUTTON_LID_CLOSED: u32 = 131072u32;
pub const SYS_BUTTON_LID_INITIAL: u32 = 262144u32;
pub const SYS_BUTTON_LID_OPEN: u32 = 65536u32;
pub const SYS_BUTTON_LID_STATE_MASK: u32 = 196608u32;
pub const SYS_BUTTON_POWER: u32 = 1u32;
pub const SYS_BUTTON_SLEEP: u32 = 2u32;
pub const SYS_BUTTON_WAKE: u32 = 2147483648u32;
pub const ScreenOff: POWER_INFORMATION_LEVEL = 73i32;
pub const SendSuspendResumeNotification: POWER_INFORMATION_LEVEL = 96i32;
pub const SessionAllowExternalDmaDevices: POWER_INFORMATION_LEVEL = 95i32;
pub const SessionConnectNotification: POWER_INFORMATION_LEVEL = 62i32;
pub const SessionDisplayState: POWER_INFORMATION_LEVEL = 42i32;
pub const SessionLockState: POWER_INFORMATION_LEVEL = 64i32;
pub const SessionPowerCleanup: POWER_INFORMATION_LEVEL = 63i32;
pub const SessionPowerInit: POWER_INFORMATION_LEVEL = 41i32;
pub const SessionRITState: POWER_INFORMATION_LEVEL = 61i32;
pub const SetPowerSettingValue: POWER_INFORMATION_LEVEL = 25i32;
pub const SetShutdownSelectedTime: POWER_INFORMATION_LEVEL = 70i32;
pub const SuspendResumeInvocation: POWER_INFORMATION_LEVEL = 71i32;
pub const SystemBatteryState: POWER_INFORMATION_LEVEL = 5i32;
pub const SystemBatteryStatePrecise: POWER_INFORMATION_LEVEL = 83i32;
pub const SystemExecutionState: POWER_INFORMATION_LEVEL = 16i32;
pub const SystemHiberFileInformation: POWER_INFORMATION_LEVEL = 36i32;
pub const SystemHiberFileSize: POWER_INFORMATION_LEVEL = 51i32;
pub const SystemHiberFileType: POWER_INFORMATION_LEVEL = 89i32;
pub const SystemHiberbootState: POWER_INFORMATION_LEVEL = 65i32;
pub const SystemMonitorHiberBootPowerOff: POWER_INFORMATION_LEVEL = 28i32;
pub const SystemPowerCapabilities: POWER_INFORMATION_LEVEL = 4i32;
pub const SystemPowerInformation: POWER_INFORMATION_LEVEL = 12i32;
pub const SystemPowerLoggingEntry: POWER_INFORMATION_LEVEL = 24i32;
pub const SystemPowerPolicyAc: POWER_INFORMATION_LEVEL = 0i32;
pub const SystemPowerPolicyCurrent: POWER_INFORMATION_LEVEL = 8i32;
pub const SystemPowerPolicyDc: POWER_INFORMATION_LEVEL = 1i32;
pub const SystemPowerStateHandler: POWER_INFORMATION_LEVEL = 6i32;
pub const SystemPowerStateLogging: POWER_INFORMATION_LEVEL = 23i32;
pub const SystemPowerStateNotifyHandler: POWER_INFORMATION_LEVEL = 17i32;
pub const SystemReserveHiberFile: POWER_INFORMATION_LEVEL = 10i32;
pub const SystemVideoState: POWER_INFORMATION_LEVEL = 29i32;
pub const SystemWakeSource: POWER_INFORMATION_LEVEL = 35i32;
pub const THERMAL_COOLING_INTERFACE_VERSION: u32 = 1u32;
pub const THERMAL_DEVICE_INTERFACE_VERSION: u32 = 1u32;
pub const THERMAL_EVENT_VERSION: u32 = 1u32;
pub const THERMAL_POLICY_VERSION_1: u32 = 1u32;
pub const THERMAL_POLICY_VERSION_2: u32 = 2u32;
pub const TZ_ACTIVATION_REASON_CURRENT: u32 = 2u32;
pub const TZ_ACTIVATION_REASON_THERMAL: u32 = 1u32;
pub const ThermalEvent: POWER_INFORMATION_LEVEL = 84i32;
pub const ThermalStandby: POWER_INFORMATION_LEVEL = 88i32;
pub const TraceApplicationPowerMessage: POWER_INFORMATION_LEVEL = 30i32;
pub const TraceApplicationPowerMessageEnd: POWER_INFORMATION_LEVEL = 31i32;
pub const TraceServicePowerMessage: POWER_INFORMATION_LEVEL = 37i32;
pub const UNKNOWN_CAPACITY: u32 = 4294967295u32;
pub const UNKNOWN_CURRENT: u32 = 4294967295u32;
pub const UNKNOWN_RATE: u32 = 2147483648u32;
pub const UNKNOWN_VOLTAGE: u32 = 4294967295u32;
pub const UpdateBlackBoxRecorder: POWER_INFORMATION_LEVEL = 94i32;
pub const UsbChargerPort_Legacy: USB_CHARGER_PORT = 0i32;
pub const UsbChargerPort_Max: USB_CHARGER_PORT = 2i32;
pub const UsbChargerPort_TypeC: USB_CHARGER_PORT = 1i32;
pub const UserNotPresent: POWER_USER_PRESENCE_TYPE = 0i32;
pub const UserPresence: POWER_INFORMATION_LEVEL = 57i32;
pub const UserPresent: POWER_USER_PRESENCE_TYPE = 1i32;
pub const UserUnknown: POWER_USER_PRESENCE_TYPE = 255i32;
pub const VerifyProcessorPowerPolicyAc: POWER_INFORMATION_LEVEL = 20i32;
pub const VerifyProcessorPowerPolicyDc: POWER_INFORMATION_LEVEL = 21i32;
pub const VerifySystemPolicyAc: POWER_INFORMATION_LEVEL = 2i32;
pub const VerifySystemPolicyDc: POWER_INFORMATION_LEVEL = 3i32;
pub const WakeTimerList: POWER_INFORMATION_LEVEL = 50i32;
pub type ACPI_TIME_RESOLUTION = i32;
pub type BATTERY_CHARGING_SOURCE_TYPE = i32;
pub type BATTERY_QUERY_INFORMATION_LEVEL = i32;
pub type BATTERY_SET_INFORMATION_LEVEL = i32;
pub type DEVICE_POWER_STATE = i32;
pub type EFFECTIVE_POWER_MODE = i32;
pub type EMI_MEASUREMENT_UNIT = i32;
pub type EXECUTION_STATE = u32;
pub type LATENCY_TIME = i32;
pub type POWER_ACTION = i32;
pub type POWER_ACTION_POLICY_EVENT_CODE = u32;
pub type POWER_COOLING_MODE = u16;
pub type POWER_DATA_ACCESSOR = i32;
pub type POWER_INFORMATION_LEVEL = i32;
pub type POWER_MONITOR_REQUEST_REASON = i32;
pub type POWER_MONITOR_REQUEST_TYPE = i32;
pub type POWER_PLATFORM_ROLE = i32;
pub type POWER_PLATFORM_ROLE_VERSION = u32;
pub type POWER_REQUEST_TYPE = i32;
pub type POWER_SETTING_ALTITUDE = i32;
pub type POWER_USER_PRESENCE_TYPE = i32;
pub type SYSTEM_POWER_CONDITION = i32;
pub type SYSTEM_POWER_STATE = i32;
pub type USB_CHARGER_PORT = i32;
pub type USER_ACTIVITY_PRESENCE = i32;
#[repr(C)]
pub struct ACPI_REAL_TIME {
    pub Year: u16,
    pub Month: u8,
    pub Day: u8,
    pub Hour: u8,
    pub Minute: u8,
    pub Second: u8,
    pub Valid: u8,
    pub Milliseconds: u16,
    pub TimeZone: i16,
    pub DayLight: u8,
    pub Reserved1: [u8; 3],
}
impl ::core::marker::Copy for ACPI_REAL_TIME {}
impl ::core::clone::Clone for ACPI_REAL_TIME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ACPI_TIME_AND_ALARM_CAPABILITIES {
    pub AcWakeSupported: super::super::Foundation::BOOLEAN,
    pub DcWakeSupported: super::super::Foundation::BOOLEAN,
    pub S4AcWakeSupported: super::super::Foundation::BOOLEAN,
    pub S4DcWakeSupported: super::super::Foundation::BOOLEAN,
    pub S5AcWakeSupported: super::super::Foundation::BOOLEAN,
    pub S5DcWakeSupported: super::super::Foundation::BOOLEAN,
    pub S4S5WakeStatusSupported: super::super::Foundation::BOOLEAN,
    pub DeepestWakeSystemState: u32,
    pub RealTimeFeaturesSupported: super::super::Foundation::BOOLEAN,
    pub RealTimeResolution: ACPI_TIME_RESOLUTION,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ACPI_TIME_AND_ALARM_CAPABILITIES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ACPI_TIME_AND_ALARM_CAPABILITIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADMINISTRATOR_POWER_POLICY {
    pub MinSleep: SYSTEM_POWER_STATE,
    pub MaxSleep: SYSTEM_POWER_STATE,
    pub MinVideoTimeout: u32,
    pub MaxVideoTimeout: u32,
    pub MinSpindownTimeout: u32,
    pub MaxSpindownTimeout: u32,
}
impl ::core::marker::Copy for ADMINISTRATOR_POWER_POLICY {}
impl ::core::clone::Clone for ADMINISTRATOR_POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_CHARGER_STATUS {
    pub Type: BATTERY_CHARGING_SOURCE_TYPE,
    pub VaData: [u32; 1],
}
impl ::core::marker::Copy for BATTERY_CHARGER_STATUS {}
impl ::core::clone::Clone for BATTERY_CHARGER_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_CHARGING_SOURCE {
    pub Type: BATTERY_CHARGING_SOURCE_TYPE,
    pub MaxCurrent: u32,
}
impl ::core::marker::Copy for BATTERY_CHARGING_SOURCE {}
impl ::core::clone::Clone for BATTERY_CHARGING_SOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BATTERY_CHARGING_SOURCE_INFORMATION {
    pub Type: BATTERY_CHARGING_SOURCE_TYPE,
    pub SourceOnline: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BATTERY_CHARGING_SOURCE_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BATTERY_CHARGING_SOURCE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_INFORMATION {
    pub Capabilities: u32,
    pub Technology: u8,
    pub Reserved: [u8; 3],
    pub Chemistry: [u8; 4],
    pub DesignedCapacity: u32,
    pub FullChargedCapacity: u32,
    pub DefaultAlert1: u32,
    pub DefaultAlert2: u32,
    pub CriticalBias: u32,
    pub CycleCount: u32,
}
impl ::core::marker::Copy for BATTERY_INFORMATION {}
impl ::core::clone::Clone for BATTERY_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_MANUFACTURE_DATE {
    pub Day: u8,
    pub Month: u8,
    pub Year: u16,
}
impl ::core::marker::Copy for BATTERY_MANUFACTURE_DATE {}
impl ::core::clone::Clone for BATTERY_MANUFACTURE_DATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_QUERY_INFORMATION {
    pub BatteryTag: u32,
    pub InformationLevel: BATTERY_QUERY_INFORMATION_LEVEL,
    pub AtRate: u32,
}
impl ::core::marker::Copy for BATTERY_QUERY_INFORMATION {}
impl ::core::clone::Clone for BATTERY_QUERY_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_REPORTING_SCALE {
    pub Granularity: u32,
    pub Capacity: u32,
}
impl ::core::marker::Copy for BATTERY_REPORTING_SCALE {}
impl ::core::clone::Clone for BATTERY_REPORTING_SCALE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_SET_INFORMATION {
    pub BatteryTag: u32,
    pub InformationLevel: BATTERY_SET_INFORMATION_LEVEL,
    pub Buffer: [u8; 1],
}
impl ::core::marker::Copy for BATTERY_SET_INFORMATION {}
impl ::core::clone::Clone for BATTERY_SET_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_STATUS {
    pub PowerState: u32,
    pub Capacity: u32,
    pub Voltage: u32,
    pub Rate: i32,
}
impl ::core::marker::Copy for BATTERY_STATUS {}
impl ::core::clone::Clone for BATTERY_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_USB_CHARGER_STATUS {
    pub Type: BATTERY_CHARGING_SOURCE_TYPE,
    pub Reserved: u32,
    pub Flags: u32,
    pub MaxCurrent: u32,
    pub Voltage: u32,
    pub PortType: USB_CHARGER_PORT,
    pub PortId: u64,
    pub PowerSourceInformation: *mut ::core::ffi::c_void,
    pub OemCharger: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for BATTERY_USB_CHARGER_STATUS {}
impl ::core::clone::Clone for BATTERY_USB_CHARGER_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BATTERY_WAIT_STATUS {
    pub BatteryTag: u32,
    pub Timeout: u32,
    pub PowerState: u32,
    pub LowCapacity: u32,
    pub HighCapacity: u32,
}
impl ::core::marker::Copy for BATTERY_WAIT_STATUS {}
impl ::core::clone::Clone for BATTERY_WAIT_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CM_POWER_DATA {
    pub PD_Size: u32,
    pub PD_MostRecentPowerState: DEVICE_POWER_STATE,
    pub PD_Capabilities: u32,
    pub PD_D1Latency: u32,
    pub PD_D2Latency: u32,
    pub PD_D3Latency: u32,
    pub PD_PowerStateMapping: [DEVICE_POWER_STATE; 7],
    pub PD_DeepestSystemWake: SYSTEM_POWER_STATE,
}
impl ::core::marker::Copy for CM_POWER_DATA {}
impl ::core::clone::Clone for CM_POWER_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
    pub Callback: PDEVICE_NOTIFY_CALLBACK_ROUTINE,
    pub Context: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {}
impl ::core::clone::Clone for DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EMI_CHANNEL_MEASUREMENT_DATA {
    pub AbsoluteEnergy: u64,
    pub AbsoluteTime: u64,
}
impl ::core::marker::Copy for EMI_CHANNEL_MEASUREMENT_DATA {}
impl ::core::clone::Clone for EMI_CHANNEL_MEASUREMENT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EMI_CHANNEL_V2 {
    pub MeasurementUnit: EMI_MEASUREMENT_UNIT,
    pub ChannelNameSize: u16,
    pub ChannelName: [u16; 1],
}
impl ::core::marker::Copy for EMI_CHANNEL_V2 {}
impl ::core::clone::Clone for EMI_CHANNEL_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EMI_MEASUREMENT_DATA_V2 {
    pub ChannelData: [EMI_CHANNEL_MEASUREMENT_DATA; 1],
}
impl ::core::marker::Copy for EMI_MEASUREMENT_DATA_V2 {}
impl ::core::clone::Clone for EMI_MEASUREMENT_DATA_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EMI_METADATA_SIZE {
    pub MetadataSize: u32,
}
impl ::core::marker::Copy for EMI_METADATA_SIZE {}
impl ::core::clone::Clone for EMI_METADATA_SIZE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EMI_METADATA_V1 {
    pub MeasurementUnit: EMI_MEASUREMENT_UNIT,
    pub HardwareOEM: [u16; 16],
    pub HardwareModel: [u16; 16],
    pub HardwareRevision: u16,
    pub MeteredHardwareNameSize: u16,
    pub MeteredHardwareName: [u16; 1],
}
impl ::core::marker::Copy for EMI_METADATA_V1 {}
impl ::core::clone::Clone for EMI_METADATA_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EMI_METADATA_V2 {
    pub HardwareOEM: [u16; 16],
    pub HardwareModel: [u16; 16],
    pub HardwareRevision: u16,
    pub ChannelCount: u16,
    pub Channels: [EMI_CHANNEL_V2; 1],
}
impl ::core::marker::Copy for EMI_METADATA_V2 {}
impl ::core::clone::Clone for EMI_METADATA_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EMI_VERSION {
    pub EmiVersion: u16,
}
impl ::core::marker::Copy for EMI_VERSION {}
impl ::core::clone::Clone for EMI_VERSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GLOBAL_MACHINE_POWER_POLICY {
    pub Revision: u32,
    pub LidOpenWakeAc: SYSTEM_POWER_STATE,
    pub LidOpenWakeDc: SYSTEM_POWER_STATE,
    pub BroadcastCapacityResolution: u32,
}
impl ::core::marker::Copy for GLOBAL_MACHINE_POWER_POLICY {}
impl ::core::clone::Clone for GLOBAL_MACHINE_POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct GLOBAL_POWER_POLICY {
    pub user: GLOBAL_USER_POWER_POLICY,
    pub mach: GLOBAL_MACHINE_POWER_POLICY,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for GLOBAL_POWER_POLICY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for GLOBAL_POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct GLOBAL_USER_POWER_POLICY {
    pub Revision: u32,
    pub PowerButtonAc: POWER_ACTION_POLICY,
    pub PowerButtonDc: POWER_ACTION_POLICY,
    pub SleepButtonAc: POWER_ACTION_POLICY,
    pub SleepButtonDc: POWER_ACTION_POLICY,
    pub LidCloseAc: POWER_ACTION_POLICY,
    pub LidCloseDc: POWER_ACTION_POLICY,
    pub DischargePolicy: [SYSTEM_POWER_LEVEL; 4],
    pub GlobalFlags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for GLOBAL_USER_POWER_POLICY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for GLOBAL_USER_POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HPOWERNOTIFY = isize;
#[repr(C)]
pub struct MACHINE_POWER_POLICY {
    pub Revision: u32,
    pub MinSleepAc: SYSTEM_POWER_STATE,
    pub MinSleepDc: SYSTEM_POWER_STATE,
    pub ReducedLatencySleepAc: SYSTEM_POWER_STATE,
    pub ReducedLatencySleepDc: SYSTEM_POWER_STATE,
    pub DozeTimeoutAc: u32,
    pub DozeTimeoutDc: u32,
    pub DozeS4TimeoutAc: u32,
    pub DozeS4TimeoutDc: u32,
    pub MinThrottleAc: u8,
    pub MinThrottleDc: u8,
    pub pad1: [u8; 2],
    pub OverThrottledAc: POWER_ACTION_POLICY,
    pub OverThrottledDc: POWER_ACTION_POLICY,
}
impl ::core::marker::Copy for MACHINE_POWER_POLICY {}
impl ::core::clone::Clone for MACHINE_POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MACHINE_PROCESSOR_POWER_POLICY {
    pub Revision: u32,
    pub ProcessorPolicyAc: PROCESSOR_POWER_POLICY,
    pub ProcessorPolicyDc: PROCESSOR_POWER_POLICY,
}
impl ::core::marker::Copy for MACHINE_PROCESSOR_POWER_POLICY {}
impl ::core::clone::Clone for MACHINE_PROCESSOR_POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POWERBROADCAST_SETTING {
    pub PowerSetting: ::windows_sys::core::GUID,
    pub DataLength: u32,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for POWERBROADCAST_SETTING {}
impl ::core::clone::Clone for POWERBROADCAST_SETTING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POWER_ACTION_POLICY {
    pub Action: POWER_ACTION,
    pub Flags: u32,
    pub EventCode: POWER_ACTION_POLICY_EVENT_CODE,
}
impl ::core::marker::Copy for POWER_ACTION_POLICY {}
impl ::core::clone::Clone for POWER_ACTION_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POWER_IDLE_RESILIENCY {
    pub CoalescingTimeout: u32,
    pub IdleResiliencyPeriod: u32,
}
impl ::core::marker::Copy for POWER_IDLE_RESILIENCY {}
impl ::core::clone::Clone for POWER_IDLE_RESILIENCY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct POWER_MONITOR_INVOCATION {
    pub Console: super::super::Foundation::BOOLEAN,
    pub RequestReason: POWER_MONITOR_REQUEST_REASON,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for POWER_MONITOR_INVOCATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for POWER_MONITOR_INVOCATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct POWER_PLATFORM_INFORMATION {
    pub AoAc: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for POWER_PLATFORM_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for POWER_PLATFORM_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct POWER_POLICY {
    pub user: USER_POWER_POLICY,
    pub mach: MACHINE_POWER_POLICY,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for POWER_POLICY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct POWER_SESSION_ALLOW_EXTERNAL_DMA_DEVICES {
    pub IsAllowed: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for POWER_SESSION_ALLOW_EXTERNAL_DMA_DEVICES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for POWER_SESSION_ALLOW_EXTERNAL_DMA_DEVICES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct POWER_SESSION_CONNECT {
    pub Connected: super::super::Foundation::BOOLEAN,
    pub Console: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for POWER_SESSION_CONNECT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for POWER_SESSION_CONNECT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct POWER_SESSION_RIT_STATE {
    pub Active: super::super::Foundation::BOOLEAN,
    pub LastInputTime: u64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for POWER_SESSION_RIT_STATE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for POWER_SESSION_RIT_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POWER_SESSION_TIMEOUTS {
    pub InputTimeout: u32,
    pub DisplayTimeout: u32,
}
impl ::core::marker::Copy for POWER_SESSION_TIMEOUTS {}
impl ::core::clone::Clone for POWER_SESSION_TIMEOUTS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct POWER_SESSION_WINLOGON {
    pub SessionId: u32,
    pub Console: super::super::Foundation::BOOLEAN,
    pub Locked: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for POWER_SESSION_WINLOGON {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for POWER_SESSION_WINLOGON {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POWER_USER_PRESENCE {
    pub UserPresence: POWER_USER_PRESENCE_TYPE,
}
impl ::core::marker::Copy for POWER_USER_PRESENCE {}
impl ::core::clone::Clone for POWER_USER_PRESENCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_IDLESTATE_EVENT {
    pub NewState: u32,
    pub OldState: u32,
    pub Processors: u64,
}
impl ::core::marker::Copy for PPM_IDLESTATE_EVENT {}
impl ::core::clone::Clone for PPM_IDLESTATE_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_IDLE_ACCOUNTING {
    pub StateCount: u32,
    pub TotalTransitions: u32,
    pub ResetCount: u32,
    pub StartTime: u64,
    pub State: [PPM_IDLE_STATE_ACCOUNTING; 1],
}
impl ::core::marker::Copy for PPM_IDLE_ACCOUNTING {}
impl ::core::clone::Clone for PPM_IDLE_ACCOUNTING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_IDLE_ACCOUNTING_EX {
    pub StateCount: u32,
    pub TotalTransitions: u32,
    pub ResetCount: u32,
    pub AbortCount: u32,
    pub StartTime: u64,
    pub State: [PPM_IDLE_STATE_ACCOUNTING_EX; 1],
}
impl ::core::marker::Copy for PPM_IDLE_ACCOUNTING_EX {}
impl ::core::clone::Clone for PPM_IDLE_ACCOUNTING_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_IDLE_STATE_ACCOUNTING {
    pub IdleTransitions: u32,
    pub FailedTransitions: u32,
    pub InvalidBucketIndex: u32,
    pub TotalTime: u64,
    pub IdleTimeBuckets: [u32; 6],
}
impl ::core::marker::Copy for PPM_IDLE_STATE_ACCOUNTING {}
impl ::core::clone::Clone for PPM_IDLE_STATE_ACCOUNTING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_IDLE_STATE_ACCOUNTING_EX {
    pub TotalTime: u64,
    pub IdleTransitions: u32,
    pub FailedTransitions: u32,
    pub InvalidBucketIndex: u32,
    pub MinTimeUs: u32,
    pub MaxTimeUs: u32,
    pub CancelledTransitions: u32,
    pub IdleTimeBuckets: [PPM_IDLE_STATE_BUCKET_EX; 16],
}
impl ::core::marker::Copy for PPM_IDLE_STATE_ACCOUNTING_EX {}
impl ::core::clone::Clone for PPM_IDLE_STATE_ACCOUNTING_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_IDLE_STATE_BUCKET_EX {
    pub TotalTimeUs: u64,
    pub MinTimeUs: u32,
    pub MaxTimeUs: u32,
    pub Count: u32,
}
impl ::core::marker::Copy for PPM_IDLE_STATE_BUCKET_EX {}
impl ::core::clone::Clone for PPM_IDLE_STATE_BUCKET_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_PERFSTATE_DOMAIN_EVENT {
    pub State: u32,
    pub Latency: u32,
    pub Speed: u32,
    pub Processors: u64,
}
impl ::core::marker::Copy for PPM_PERFSTATE_DOMAIN_EVENT {}
impl ::core::clone::Clone for PPM_PERFSTATE_DOMAIN_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_PERFSTATE_EVENT {
    pub State: u32,
    pub Status: u32,
    pub Latency: u32,
    pub Speed: u32,
    pub Processor: u32,
}
impl ::core::marker::Copy for PPM_PERFSTATE_EVENT {}
impl ::core::clone::Clone for PPM_PERFSTATE_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_THERMALCHANGE_EVENT {
    pub ThermalConstraint: u32,
    pub Processors: u64,
}
impl ::core::marker::Copy for PPM_THERMALCHANGE_EVENT {}
impl ::core::clone::Clone for PPM_THERMALCHANGE_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_THERMAL_POLICY_EVENT {
    pub Mode: u8,
    pub Processors: u64,
}
impl ::core::marker::Copy for PPM_THERMAL_POLICY_EVENT {}
impl ::core::clone::Clone for PPM_THERMAL_POLICY_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_WMI_IDLE_STATE {
    pub Latency: u32,
    pub Power: u32,
    pub TimeCheck: u32,
    pub PromotePercent: u8,
    pub DemotePercent: u8,
    pub StateType: u8,
    pub Reserved: u8,
    pub StateFlags: u32,
    pub Context: u32,
    pub IdleHandler: u32,
    pub Reserved1: u32,
}
impl ::core::marker::Copy for PPM_WMI_IDLE_STATE {}
impl ::core::clone::Clone for PPM_WMI_IDLE_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_WMI_IDLE_STATES {
    pub Type: u32,
    pub Count: u32,
    pub TargetState: u32,
    pub OldState: u32,
    pub TargetProcessors: u64,
    pub State: [PPM_WMI_IDLE_STATE; 1],
}
impl ::core::marker::Copy for PPM_WMI_IDLE_STATES {}
impl ::core::clone::Clone for PPM_WMI_IDLE_STATES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_WMI_IDLE_STATES_EX {
    pub Type: u32,
    pub Count: u32,
    pub TargetState: u32,
    pub OldState: u32,
    pub TargetProcessors: *mut ::core::ffi::c_void,
    pub State: [PPM_WMI_IDLE_STATE; 1],
}
impl ::core::marker::Copy for PPM_WMI_IDLE_STATES_EX {}
impl ::core::clone::Clone for PPM_WMI_IDLE_STATES_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_WMI_LEGACY_PERFSTATE {
    pub Frequency: u32,
    pub Flags: u32,
    pub PercentFrequency: u32,
}
impl ::core::marker::Copy for PPM_WMI_LEGACY_PERFSTATE {}
impl ::core::clone::Clone for PPM_WMI_LEGACY_PERFSTATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_WMI_PERF_STATE {
    pub Frequency: u32,
    pub Power: u32,
    pub PercentFrequency: u8,
    pub IncreaseLevel: u8,
    pub DecreaseLevel: u8,
    pub Type: u8,
    pub IncreaseTime: u32,
    pub DecreaseTime: u32,
    pub Control: u64,
    pub Status: u64,
    pub HitCount: u32,
    pub Reserved1: u32,
    pub Reserved2: u64,
    pub Reserved3: u64,
}
impl ::core::marker::Copy for PPM_WMI_PERF_STATE {}
impl ::core::clone::Clone for PPM_WMI_PERF_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_WMI_PERF_STATES {
    pub Count: u32,
    pub MaxFrequency: u32,
    pub CurrentState: u32,
    pub MaxPerfState: u32,
    pub MinPerfState: u32,
    pub LowestPerfState: u32,
    pub ThermalConstraint: u32,
    pub BusyAdjThreshold: u8,
    pub PolicyType: u8,
    pub Type: u8,
    pub Reserved: u8,
    pub TimerInterval: u32,
    pub TargetProcessors: u64,
    pub PStateHandler: u32,
    pub PStateContext: u32,
    pub TStateHandler: u32,
    pub TStateContext: u32,
    pub FeedbackHandler: u32,
    pub Reserved1: u32,
    pub Reserved2: u64,
    pub State: [PPM_WMI_PERF_STATE; 1],
}
impl ::core::marker::Copy for PPM_WMI_PERF_STATES {}
impl ::core::clone::Clone for PPM_WMI_PERF_STATES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPM_WMI_PERF_STATES_EX {
    pub Count: u32,
    pub MaxFrequency: u32,
    pub CurrentState: u32,
    pub MaxPerfState: u32,
    pub MinPerfState: u32,
    pub LowestPerfState: u32,
    pub ThermalConstraint: u32,
    pub BusyAdjThreshold: u8,
    pub PolicyType: u8,
    pub Type: u8,
    pub Reserved: u8,
    pub TimerInterval: u32,
    pub TargetProcessors: *mut ::core::ffi::c_void,
    pub PStateHandler: u32,
    pub PStateContext: u32,
    pub TStateHandler: u32,
    pub TStateContext: u32,
    pub FeedbackHandler: u32,
    pub Reserved1: u32,
    pub Reserved2: u64,
    pub State: [PPM_WMI_PERF_STATE; 1],
}
impl ::core::marker::Copy for PPM_WMI_PERF_STATES_EX {}
impl ::core::clone::Clone for PPM_WMI_PERF_STATES_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROCESSOR_OBJECT_INFO {
    pub PhysicalID: u32,
    pub PBlkAddress: u32,
    pub PBlkLength: u8,
}
impl ::core::marker::Copy for PROCESSOR_OBJECT_INFO {}
impl ::core::clone::Clone for PROCESSOR_OBJECT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROCESSOR_OBJECT_INFO_EX {
    pub PhysicalID: u32,
    pub PBlkAddress: u32,
    pub PBlkLength: u8,
    pub InitialApicId: u32,
}
impl ::core::marker::Copy for PROCESSOR_OBJECT_INFO_EX {}
impl ::core::clone::Clone for PROCESSOR_OBJECT_INFO_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROCESSOR_POWER_INFORMATION {
    pub Number: u32,
    pub MaxMhz: u32,
    pub CurrentMhz: u32,
    pub MhzLimit: u32,
    pub MaxIdleState: u32,
    pub CurrentIdleState: u32,
}
impl ::core::marker::Copy for PROCESSOR_POWER_INFORMATION {}
impl ::core::clone::Clone for PROCESSOR_POWER_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROCESSOR_POWER_POLICY {
    pub Revision: u32,
    pub DynamicThrottle: u8,
    pub Spare: [u8; 3],
    pub _bitfield: u32,
    pub PolicyCount: u32,
    pub Policy: [PROCESSOR_POWER_POLICY_INFO; 3],
}
impl ::core::marker::Copy for PROCESSOR_POWER_POLICY {}
impl ::core::clone::Clone for PROCESSOR_POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROCESSOR_POWER_POLICY_INFO {
    pub TimeCheck: u32,
    pub DemoteLimit: u32,
    pub PromoteLimit: u32,
    pub DemotePercent: u8,
    pub PromotePercent: u8,
    pub Spare: [u8; 2],
    pub _bitfield: u32,
}
impl ::core::marker::Copy for PROCESSOR_POWER_POLICY_INFO {}
impl ::core::clone::Clone for PROCESSOR_POWER_POLICY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RESUME_PERFORMANCE {
    pub PostTimeMs: u32,
    pub TotalResumeTimeMs: u64,
    pub ResumeCompleteTimestamp: u64,
}
impl ::core::marker::Copy for RESUME_PERFORMANCE {}
impl ::core::clone::Clone for RESUME_PERFORMANCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SET_POWER_SETTING_VALUE {
    pub Version: u32,
    pub Guid: ::windows_sys::core::GUID,
    pub PowerCondition: SYSTEM_POWER_CONDITION,
    pub DataLength: u32,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for SET_POWER_SETTING_VALUE {}
impl ::core::clone::Clone for SET_POWER_SETTING_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SYSTEM_BATTERY_STATE {
    pub AcOnLine: super::super::Foundation::BOOLEAN,
    pub BatteryPresent: super::super::Foundation::BOOLEAN,
    pub Charging: super::super::Foundation::BOOLEAN,
    pub Discharging: super::super::Foundation::BOOLEAN,
    pub Spare1: [super::super::Foundation::BOOLEAN; 3],
    pub Tag: u8,
    pub MaxCapacity: u32,
    pub RemainingCapacity: u32,
    pub Rate: u32,
    pub EstimatedTime: u32,
    pub DefaultAlert1: u32,
    pub DefaultAlert2: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SYSTEM_BATTERY_STATE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SYSTEM_BATTERY_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SYSTEM_POWER_CAPABILITIES {
    pub PowerButtonPresent: super::super::Foundation::BOOLEAN,
    pub SleepButtonPresent: super::super::Foundation::BOOLEAN,
    pub LidPresent: super::super::Foundation::BOOLEAN,
    pub SystemS1: super::super::Foundation::BOOLEAN,
    pub SystemS2: super::super::Foundation::BOOLEAN,
    pub SystemS3: super::super::Foundation::BOOLEAN,
    pub SystemS4: super::super::Foundation::BOOLEAN,
    pub SystemS5: super::super::Foundation::BOOLEAN,
    pub HiberFilePresent: super::super::Foundation::BOOLEAN,
    pub FullWake: super::super::Foundation::BOOLEAN,
    pub VideoDimPresent: super::super::Foundation::BOOLEAN,
    pub ApmPresent: super::super::Foundation::BOOLEAN,
    pub UpsPresent: super::super::Foundation::BOOLEAN,
    pub ThermalControl: super::super::Foundation::BOOLEAN,
    pub ProcessorThrottle: super::super::Foundation::BOOLEAN,
    pub ProcessorMinThrottle: u8,
    pub ProcessorMaxThrottle: u8,
    pub FastSystemS4: super::super::Foundation::BOOLEAN,
    pub Hiberboot: super::super::Foundation::BOOLEAN,
    pub WakeAlarmPresent: super::super::Foundation::BOOLEAN,
    pub AoAc: super::super::Foundation::BOOLEAN,
    pub DiskSpinDown: super::super::Foundation::BOOLEAN,
    pub HiberFileType: u8,
    pub AoAcConnectivitySupported: super::super::Foundation::BOOLEAN,
    pub spare3: [u8; 6],
    pub SystemBatteriesPresent: super::super::Foundation::BOOLEAN,
    pub BatteriesAreShortTerm: super::super::Foundation::BOOLEAN,
    pub BatteryScale: [BATTERY_REPORTING_SCALE; 3],
    pub AcOnLineWake: SYSTEM_POWER_STATE,
    pub SoftLidWake: SYSTEM_POWER_STATE,
    pub RtcWake: SYSTEM_POWER_STATE,
    pub MinDeviceWakeState: SYSTEM_POWER_STATE,
    pub DefaultLowLatencyWake: SYSTEM_POWER_STATE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SYSTEM_POWER_CAPABILITIES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SYSTEM_POWER_CAPABILITIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYSTEM_POWER_INFORMATION {
    pub MaxIdlenessAllowed: u32,
    pub Idleness: u32,
    pub TimeRemaining: u32,
    pub CoolingMode: POWER_COOLING_MODE,
}
impl ::core::marker::Copy for SYSTEM_POWER_INFORMATION {}
impl ::core::clone::Clone for SYSTEM_POWER_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SYSTEM_POWER_LEVEL {
    pub Enable: super::super::Foundation::BOOLEAN,
    pub Spare: [u8; 3],
    pub BatteryLevel: u32,
    pub PowerPolicy: POWER_ACTION_POLICY,
    pub MinSystemState: SYSTEM_POWER_STATE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SYSTEM_POWER_LEVEL {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SYSTEM_POWER_LEVEL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SYSTEM_POWER_POLICY {
    pub Revision: u32,
    pub PowerButton: POWER_ACTION_POLICY,
    pub SleepButton: POWER_ACTION_POLICY,
    pub LidClose: POWER_ACTION_POLICY,
    pub LidOpenWake: SYSTEM_POWER_STATE,
    pub Reserved: u32,
    pub Idle: POWER_ACTION_POLICY,
    pub IdleTimeout: u32,
    pub IdleSensitivity: u8,
    pub DynamicThrottle: u8,
    pub Spare2: [u8; 2],
    pub MinSleep: SYSTEM_POWER_STATE,
    pub MaxSleep: SYSTEM_POWER_STATE,
    pub ReducedLatencySleep: SYSTEM_POWER_STATE,
    pub WinLogonFlags: u32,
    pub Spare3: u32,
    pub DozeS4Timeout: u32,
    pub BroadcastCapacityResolution: u32,
    pub DischargePolicy: [SYSTEM_POWER_LEVEL; 4],
    pub VideoTimeout: u32,
    pub VideoDimDisplay: super::super::Foundation::BOOLEAN,
    pub VideoReserved: [u32; 3],
    pub SpindownTimeout: u32,
    pub OptimizeForPower: super::super::Foundation::BOOLEAN,
    pub FanThrottleTolerance: u8,
    pub ForcedThrottle: u8,
    pub MinThrottle: u8,
    pub OverThrottled: POWER_ACTION_POLICY,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SYSTEM_POWER_POLICY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SYSTEM_POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYSTEM_POWER_STATUS {
    pub ACLineStatus: u8,
    pub BatteryFlag: u8,
    pub BatteryLifePercent: u8,
    pub SystemStatusFlag: u8,
    pub BatteryLifeTime: u32,
    pub BatteryFullLifeTime: u32,
}
impl ::core::marker::Copy for SYSTEM_POWER_STATUS {}
impl ::core::clone::Clone for SYSTEM_POWER_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct THERMAL_EVENT {
    pub Version: u32,
    pub Size: u32,
    pub Type: u32,
    pub Temperature: u32,
    pub TripPointTemperature: u32,
    pub Initiator: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for THERMAL_EVENT {}
impl ::core::clone::Clone for THERMAL_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct THERMAL_INFORMATION {
    pub ThermalStamp: u32,
    pub ThermalConstant1: u32,
    pub ThermalConstant2: u32,
    pub Processors: usize,
    pub SamplingPeriod: u32,
    pub CurrentTemperature: u32,
    pub PassiveTripPoint: u32,
    pub CriticalTripPoint: u32,
    pub ActiveTripPointCount: u8,
    pub ActiveTripPoint: [u32; 10],
}
impl ::core::marker::Copy for THERMAL_INFORMATION {}
impl ::core::clone::Clone for THERMAL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct THERMAL_POLICY {
    pub Version: u32,
    pub WaitForUpdate: super::super::Foundation::BOOLEAN,
    pub Hibernate: super::super::Foundation::BOOLEAN,
    pub Critical: super::super::Foundation::BOOLEAN,
    pub ThermalStandby: super::super::Foundation::BOOLEAN,
    pub ActivationReasons: u32,
    pub PassiveLimit: u32,
    pub ActiveLevel: u32,
    pub OverThrottled: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for THERMAL_POLICY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for THERMAL_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct THERMAL_WAIT_READ {
    pub Timeout: u32,
    pub LowTemperature: u32,
    pub HighTemperature: u32,
}
impl ::core::marker::Copy for THERMAL_WAIT_READ {}
impl ::core::clone::Clone for THERMAL_WAIT_READ {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct USER_POWER_POLICY {
    pub Revision: u32,
    pub IdleAc: POWER_ACTION_POLICY,
    pub IdleDc: POWER_ACTION_POLICY,
    pub IdleTimeoutAc: u32,
    pub IdleTimeoutDc: u32,
    pub IdleSensitivityAc: u8,
    pub IdleSensitivityDc: u8,
    pub ThrottlePolicyAc: u8,
    pub ThrottlePolicyDc: u8,
    pub MaxSleepAc: SYSTEM_POWER_STATE,
    pub MaxSleepDc: SYSTEM_POWER_STATE,
    pub Reserved: [u32; 2],
    pub VideoTimeoutAc: u32,
    pub VideoTimeoutDc: u32,
    pub SpindownTimeoutAc: u32,
    pub SpindownTimeoutDc: u32,
    pub OptimizeForPowerAc: super::super::Foundation::BOOLEAN,
    pub OptimizeForPowerDc: super::super::Foundation::BOOLEAN,
    pub FanThrottleToleranceAc: u8,
    pub FanThrottleToleranceDc: u8,
    pub ForcedThrottleAc: u8,
    pub ForcedThrottleDc: u8,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for USER_POWER_POLICY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for USER_POWER_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WAKE_ALARM_INFORMATION {
    pub TimerIdentifier: u32,
    pub Timeout: u32,
}
impl ::core::marker::Copy for WAKE_ALARM_INFORMATION {}
impl ::core::clone::Clone for WAKE_ALARM_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
pub type EFFECTIVE_POWER_MODE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(mode: EFFECTIVE_POWER_MODE, context: *const ::core::ffi::c_void) -> ()>;
pub type PDEVICE_NOTIFY_CALLBACK_ROUTINE = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, r#type: u32, setting: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PWRSCHEMESENUMPROC = ::core::option::Option<unsafe extern "system" fn(index: u32, namesize: u32, name: ::windows_sys::core::PCWSTR, descriptionsize: u32, description: ::windows_sys::core::PCWSTR, policy: *const POWER_POLICY, context: super::super::Foundation::LPARAM) -> super::super::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PWRSCHEMESENUMPROC_V1 = ::core::option::Option<unsafe extern "system" fn(index: u32, namesize: u32, name: *const i8, descriptionsize: u32, description: *const i8, policy: *const POWER_POLICY, context: super::super::Foundation::LPARAM) -> super::super::Foundation::BOOLEAN>;
