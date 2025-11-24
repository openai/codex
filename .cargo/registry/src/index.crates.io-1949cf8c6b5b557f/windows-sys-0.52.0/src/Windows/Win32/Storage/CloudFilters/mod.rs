#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfCloseHandle(filehandle : super::super::Foundation:: HANDLE) -> ());
#[cfg(feature = "Win32_System_CorrelationVector")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_System_CorrelationVector\"`"] fn CfConnectSyncRoot(syncrootpath : ::windows_sys::core::PCWSTR, callbacktable : *const CF_CALLBACK_REGISTRATION, callbackcontext : *const ::core::ffi::c_void, connectflags : CF_CONNECT_FLAGS, connectionkey : *mut CF_CONNECTION_KEY) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn CfConvertToPlaceholder(filehandle : super::super::Foundation:: HANDLE, fileidentity : *const ::core::ffi::c_void, fileidentitylength : u32, convertflags : CF_CONVERT_FLAGS, convertusn : *mut i64, overlapped : *mut super::super::System::IO:: OVERLAPPED) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Storage_FileSystem")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Storage_FileSystem\"`"] fn CfCreatePlaceholders(basedirectorypath : ::windows_sys::core::PCWSTR, placeholderarray : *mut CF_PLACEHOLDER_CREATE_INFO, placeholdercount : u32, createflags : CF_CREATE_FLAGS, entriesprocessed : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn CfDehydratePlaceholder(filehandle : super::super::Foundation:: HANDLE, startingoffset : i64, length : i64, dehydrateflags : CF_DEHYDRATE_FLAGS, overlapped : *mut super::super::System::IO:: OVERLAPPED) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("cldapi.dll" "system" fn CfDisconnectSyncRoot(connectionkey : CF_CONNECTION_KEY) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_CorrelationVector"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_CorrelationVector\"`"] fn CfExecute(opinfo : *const CF_OPERATION_INFO, opparams : *mut CF_OPERATION_PARAMETERS) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_CorrelationVector"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_CorrelationVector\"`"] fn CfGetCorrelationVector(filehandle : super::super::Foundation:: HANDLE, correlationvector : *mut super::super::System::CorrelationVector:: CORRELATION_VECTOR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfGetPlaceholderInfo(filehandle : super::super::Foundation:: HANDLE, infoclass : CF_PLACEHOLDER_INFO_CLASS, infobuffer : *mut ::core::ffi::c_void, infobufferlength : u32, returnedlength : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfGetPlaceholderRangeInfo(filehandle : super::super::Foundation:: HANDLE, infoclass : CF_PLACEHOLDER_RANGE_INFO_CLASS, startingoffset : i64, length : i64, infobuffer : *mut ::core::ffi::c_void, infobufferlength : u32, returnedlength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("cldapi.dll" "system" fn CfGetPlaceholderRangeInfoForHydration(connectionkey : CF_CONNECTION_KEY, transferkey : i64, fileid : i64, infoclass : CF_PLACEHOLDER_RANGE_INFO_CLASS, startingoffset : i64, rangelength : i64, infobuffer : *mut ::core::ffi::c_void, infobuffersize : u32, infobufferwritten : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("cldapi.dll" "system" fn CfGetPlaceholderStateFromAttributeTag(fileattributes : u32, reparsetag : u32) -> CF_PLACEHOLDER_STATE);
#[cfg(feature = "Win32_Storage_FileSystem")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Storage_FileSystem\"`"] fn CfGetPlaceholderStateFromFileInfo(infobuffer : *const ::core::ffi::c_void, infoclass : super::FileSystem:: FILE_INFO_BY_HANDLE_CLASS) -> CF_PLACEHOLDER_STATE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"] fn CfGetPlaceholderStateFromFindData(finddata : *const super::FileSystem:: WIN32_FIND_DATAA) -> CF_PLACEHOLDER_STATE);
::windows_targets::link!("cldapi.dll" "system" fn CfGetPlatformInfo(platformversion : *mut CF_PLATFORM_INFO) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfGetSyncRootInfoByHandle(filehandle : super::super::Foundation:: HANDLE, infoclass : CF_SYNC_ROOT_INFO_CLASS, infobuffer : *mut ::core::ffi::c_void, infobufferlength : u32, returnedlength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("cldapi.dll" "system" fn CfGetSyncRootInfoByPath(filepath : ::windows_sys::core::PCWSTR, infoclass : CF_SYNC_ROOT_INFO_CLASS, infobuffer : *mut ::core::ffi::c_void, infobufferlength : u32, returnedlength : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfGetTransferKey(filehandle : super::super::Foundation:: HANDLE, transferkey : *mut i64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfGetWin32HandleFromProtectedHandle(protectedhandle : super::super::Foundation:: HANDLE) -> super::super::Foundation:: HANDLE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn CfHydratePlaceholder(filehandle : super::super::Foundation:: HANDLE, startingoffset : i64, length : i64, hydrateflags : CF_HYDRATE_FLAGS, overlapped : *mut super::super::System::IO:: OVERLAPPED) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfOpenFileWithOplock(filepath : ::windows_sys::core::PCWSTR, flags : CF_OPEN_FILE_FLAGS, protectedhandle : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("cldapi.dll" "system" fn CfQuerySyncProviderStatus(connectionkey : CF_CONNECTION_KEY, providerstatus : *mut CF_SYNC_PROVIDER_STATUS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfReferenceProtectedHandle(protectedhandle : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOLEAN);
::windows_targets::link!("cldapi.dll" "system" fn CfRegisterSyncRoot(syncrootpath : ::windows_sys::core::PCWSTR, registration : *const CF_SYNC_REGISTRATION, policies : *const CF_SYNC_POLICIES, registerflags : CF_REGISTER_FLAGS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfReleaseProtectedHandle(protectedhandle : super::super::Foundation:: HANDLE) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfReleaseTransferKey(filehandle : super::super::Foundation:: HANDLE, transferkey : *const i64) -> ());
::windows_targets::link!("cldapi.dll" "system" fn CfReportProviderProgress(connectionkey : CF_CONNECTION_KEY, transferkey : i64, providerprogresstotal : i64, providerprogresscompleted : i64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("cldapi.dll" "system" fn CfReportProviderProgress2(connectionkey : CF_CONNECTION_KEY, transferkey : i64, requestkey : i64, providerprogresstotal : i64, providerprogresscompleted : i64, targetsessionid : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("cldapi.dll" "system" fn CfReportSyncStatus(syncrootpath : ::windows_sys::core::PCWSTR, syncstatus : *const CF_SYNC_STATUS) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn CfRevertPlaceholder(filehandle : super::super::Foundation:: HANDLE, revertflags : CF_REVERT_FLAGS, overlapped : *mut super::super::System::IO:: OVERLAPPED) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_CorrelationVector"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_CorrelationVector\"`"] fn CfSetCorrelationVector(filehandle : super::super::Foundation:: HANDLE, correlationvector : *const super::super::System::CorrelationVector:: CORRELATION_VECTOR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CfSetInSyncState(filehandle : super::super::Foundation:: HANDLE, insyncstate : CF_IN_SYNC_STATE, insyncflags : CF_SET_IN_SYNC_FLAGS, insyncusn : *mut i64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn CfSetPinState(filehandle : super::super::Foundation:: HANDLE, pinstate : CF_PIN_STATE, pinflags : CF_SET_PIN_FLAGS, overlapped : *mut super::super::System::IO:: OVERLAPPED) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("cldapi.dll" "system" fn CfUnregisterSyncRoot(syncrootpath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_IO"))]
::windows_targets::link!("cldapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_IO\"`"] fn CfUpdatePlaceholder(filehandle : super::super::Foundation:: HANDLE, fsmetadata : *const CF_FS_METADATA, fileidentity : *const ::core::ffi::c_void, fileidentitylength : u32, dehydraterangearray : *const CF_FILE_RANGE, dehydraterangecount : u32, updateflags : CF_UPDATE_FLAGS, updateusn : *mut i64, overlapped : *mut super::super::System::IO:: OVERLAPPED) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("cldapi.dll" "system" fn CfUpdateSyncProviderStatus(connectionkey : CF_CONNECTION_KEY, providerstatus : CF_SYNC_PROVIDER_STATUS) -> ::windows_sys::core::HRESULT);
pub const CF_CALLBACK_CANCEL_FLAG_IO_ABORTED: CF_CALLBACK_CANCEL_FLAGS = 2i32;
pub const CF_CALLBACK_CANCEL_FLAG_IO_TIMEOUT: CF_CALLBACK_CANCEL_FLAGS = 1i32;
pub const CF_CALLBACK_CANCEL_FLAG_NONE: CF_CALLBACK_CANCEL_FLAGS = 0i32;
pub const CF_CALLBACK_CLOSE_COMPLETION_FLAG_DELETED: CF_CALLBACK_CLOSE_COMPLETION_FLAGS = 1i32;
pub const CF_CALLBACK_CLOSE_COMPLETION_FLAG_NONE: CF_CALLBACK_CLOSE_COMPLETION_FLAGS = 0i32;
pub const CF_CALLBACK_DEHYDRATE_COMPLETION_FLAG_BACKGROUND: CF_CALLBACK_DEHYDRATE_COMPLETION_FLAGS = 1i32;
pub const CF_CALLBACK_DEHYDRATE_COMPLETION_FLAG_DEHYDRATED: CF_CALLBACK_DEHYDRATE_COMPLETION_FLAGS = 2i32;
pub const CF_CALLBACK_DEHYDRATE_COMPLETION_FLAG_NONE: CF_CALLBACK_DEHYDRATE_COMPLETION_FLAGS = 0i32;
pub const CF_CALLBACK_DEHYDRATE_FLAG_BACKGROUND: CF_CALLBACK_DEHYDRATE_FLAGS = 1i32;
pub const CF_CALLBACK_DEHYDRATE_FLAG_NONE: CF_CALLBACK_DEHYDRATE_FLAGS = 0i32;
pub const CF_CALLBACK_DEHYDRATION_REASON_NONE: CF_CALLBACK_DEHYDRATION_REASON = 0i32;
pub const CF_CALLBACK_DEHYDRATION_REASON_SYSTEM_INACTIVITY: CF_CALLBACK_DEHYDRATION_REASON = 3i32;
pub const CF_CALLBACK_DEHYDRATION_REASON_SYSTEM_LOW_SPACE: CF_CALLBACK_DEHYDRATION_REASON = 2i32;
pub const CF_CALLBACK_DEHYDRATION_REASON_SYSTEM_OS_UPGRADE: CF_CALLBACK_DEHYDRATION_REASON = 4i32;
pub const CF_CALLBACK_DEHYDRATION_REASON_USER_MANUAL: CF_CALLBACK_DEHYDRATION_REASON = 1i32;
pub const CF_CALLBACK_DELETE_COMPLETION_FLAG_NONE: CF_CALLBACK_DELETE_COMPLETION_FLAGS = 0i32;
pub const CF_CALLBACK_DELETE_FLAG_IS_DIRECTORY: CF_CALLBACK_DELETE_FLAGS = 1i32;
pub const CF_CALLBACK_DELETE_FLAG_IS_UNDELETE: CF_CALLBACK_DELETE_FLAGS = 2i32;
pub const CF_CALLBACK_DELETE_FLAG_NONE: CF_CALLBACK_DELETE_FLAGS = 0i32;
pub const CF_CALLBACK_FETCH_DATA_FLAG_EXPLICIT_HYDRATION: CF_CALLBACK_FETCH_DATA_FLAGS = 2i32;
pub const CF_CALLBACK_FETCH_DATA_FLAG_NONE: CF_CALLBACK_FETCH_DATA_FLAGS = 0i32;
pub const CF_CALLBACK_FETCH_DATA_FLAG_RECOVERY: CF_CALLBACK_FETCH_DATA_FLAGS = 1i32;
pub const CF_CALLBACK_FETCH_PLACEHOLDERS_FLAG_NONE: CF_CALLBACK_FETCH_PLACEHOLDERS_FLAGS = 0i32;
pub const CF_CALLBACK_OPEN_COMPLETION_FLAG_NONE: CF_CALLBACK_OPEN_COMPLETION_FLAGS = 0i32;
pub const CF_CALLBACK_OPEN_COMPLETION_FLAG_PLACEHOLDER_UNKNOWN: CF_CALLBACK_OPEN_COMPLETION_FLAGS = 1i32;
pub const CF_CALLBACK_OPEN_COMPLETION_FLAG_PLACEHOLDER_UNSUPPORTED: CF_CALLBACK_OPEN_COMPLETION_FLAGS = 2i32;
pub const CF_CALLBACK_RENAME_COMPLETION_FLAG_NONE: CF_CALLBACK_RENAME_COMPLETION_FLAGS = 0i32;
pub const CF_CALLBACK_RENAME_FLAG_IS_DIRECTORY: CF_CALLBACK_RENAME_FLAGS = 1i32;
pub const CF_CALLBACK_RENAME_FLAG_NONE: CF_CALLBACK_RENAME_FLAGS = 0i32;
pub const CF_CALLBACK_RENAME_FLAG_SOURCE_IN_SCOPE: CF_CALLBACK_RENAME_FLAGS = 2i32;
pub const CF_CALLBACK_RENAME_FLAG_TARGET_IN_SCOPE: CF_CALLBACK_RENAME_FLAGS = 4i32;
pub const CF_CALLBACK_TYPE_CANCEL_FETCH_DATA: CF_CALLBACK_TYPE = 2i32;
pub const CF_CALLBACK_TYPE_CANCEL_FETCH_PLACEHOLDERS: CF_CALLBACK_TYPE = 4i32;
pub const CF_CALLBACK_TYPE_FETCH_DATA: CF_CALLBACK_TYPE = 0i32;
pub const CF_CALLBACK_TYPE_FETCH_PLACEHOLDERS: CF_CALLBACK_TYPE = 3i32;
pub const CF_CALLBACK_TYPE_NONE: CF_CALLBACK_TYPE = -1i32;
pub const CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE: CF_CALLBACK_TYPE = 7i32;
pub const CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE_COMPLETION: CF_CALLBACK_TYPE = 8i32;
pub const CF_CALLBACK_TYPE_NOTIFY_DELETE: CF_CALLBACK_TYPE = 9i32;
pub const CF_CALLBACK_TYPE_NOTIFY_DELETE_COMPLETION: CF_CALLBACK_TYPE = 10i32;
pub const CF_CALLBACK_TYPE_NOTIFY_FILE_CLOSE_COMPLETION: CF_CALLBACK_TYPE = 6i32;
pub const CF_CALLBACK_TYPE_NOTIFY_FILE_OPEN_COMPLETION: CF_CALLBACK_TYPE = 5i32;
pub const CF_CALLBACK_TYPE_NOTIFY_RENAME: CF_CALLBACK_TYPE = 11i32;
pub const CF_CALLBACK_TYPE_NOTIFY_RENAME_COMPLETION: CF_CALLBACK_TYPE = 12i32;
pub const CF_CALLBACK_TYPE_VALIDATE_DATA: CF_CALLBACK_TYPE = 1i32;
pub const CF_CALLBACK_VALIDATE_DATA_FLAG_EXPLICIT_HYDRATION: CF_CALLBACK_VALIDATE_DATA_FLAGS = 2i32;
pub const CF_CALLBACK_VALIDATE_DATA_FLAG_NONE: CF_CALLBACK_VALIDATE_DATA_FLAGS = 0i32;
pub const CF_CONNECT_FLAG_BLOCK_SELF_IMPLICIT_HYDRATION: CF_CONNECT_FLAGS = 8i32;
pub const CF_CONNECT_FLAG_NONE: CF_CONNECT_FLAGS = 0i32;
pub const CF_CONNECT_FLAG_REQUIRE_FULL_FILE_PATH: CF_CONNECT_FLAGS = 4i32;
pub const CF_CONNECT_FLAG_REQUIRE_PROCESS_INFO: CF_CONNECT_FLAGS = 2i32;
pub const CF_CONVERT_FLAG_ALWAYS_FULL: CF_CONVERT_FLAGS = 8i32;
pub const CF_CONVERT_FLAG_DEHYDRATE: CF_CONVERT_FLAGS = 2i32;
pub const CF_CONVERT_FLAG_ENABLE_ON_DEMAND_POPULATION: CF_CONVERT_FLAGS = 4i32;
pub const CF_CONVERT_FLAG_FORCE_CONVERT_TO_CLOUD_FILE: CF_CONVERT_FLAGS = 16i32;
pub const CF_CONVERT_FLAG_MARK_IN_SYNC: CF_CONVERT_FLAGS = 1i32;
pub const CF_CONVERT_FLAG_NONE: CF_CONVERT_FLAGS = 0i32;
pub const CF_CREATE_FLAG_NONE: CF_CREATE_FLAGS = 0i32;
pub const CF_CREATE_FLAG_STOP_ON_ERROR: CF_CREATE_FLAGS = 1i32;
pub const CF_DEHYDRATE_FLAG_BACKGROUND: CF_DEHYDRATE_FLAGS = 1i32;
pub const CF_DEHYDRATE_FLAG_NONE: CF_DEHYDRATE_FLAGS = 0i32;
pub const CF_HARDLINK_POLICY_ALLOWED: CF_HARDLINK_POLICY = 1i32;
pub const CF_HARDLINK_POLICY_NONE: CF_HARDLINK_POLICY = 0i32;
pub const CF_HYDRATE_FLAG_NONE: CF_HYDRATE_FLAGS = 0i32;
pub const CF_HYDRATION_POLICY_ALWAYS_FULL: CF_HYDRATION_POLICY_PRIMARY = 3u16;
pub const CF_HYDRATION_POLICY_FULL: CF_HYDRATION_POLICY_PRIMARY = 2u16;
pub const CF_HYDRATION_POLICY_MODIFIER_ALLOW_FULL_RESTART_HYDRATION: CF_HYDRATION_POLICY_MODIFIER = 8u16;
pub const CF_HYDRATION_POLICY_MODIFIER_AUTO_DEHYDRATION_ALLOWED: CF_HYDRATION_POLICY_MODIFIER = 4u16;
pub const CF_HYDRATION_POLICY_MODIFIER_NONE: CF_HYDRATION_POLICY_MODIFIER = 0u16;
pub const CF_HYDRATION_POLICY_MODIFIER_STREAMING_ALLOWED: CF_HYDRATION_POLICY_MODIFIER = 2u16;
pub const CF_HYDRATION_POLICY_MODIFIER_VALIDATION_REQUIRED: CF_HYDRATION_POLICY_MODIFIER = 1u16;
pub const CF_HYDRATION_POLICY_PARTIAL: CF_HYDRATION_POLICY_PRIMARY = 0u16;
pub const CF_HYDRATION_POLICY_PROGRESSIVE: CF_HYDRATION_POLICY_PRIMARY = 1u16;
pub const CF_INSYNC_POLICY_NONE: CF_INSYNC_POLICY = 0u32;
pub const CF_INSYNC_POLICY_PRESERVE_INSYNC_FOR_SYNC_ENGINE: CF_INSYNC_POLICY = 2147483648u32;
pub const CF_INSYNC_POLICY_TRACK_ALL: CF_INSYNC_POLICY = 16777215u32;
pub const CF_INSYNC_POLICY_TRACK_DIRECTORY_ALL: CF_INSYNC_POLICY = 11184880u32;
pub const CF_INSYNC_POLICY_TRACK_DIRECTORY_CREATION_TIME: CF_INSYNC_POLICY = 16u32;
pub const CF_INSYNC_POLICY_TRACK_DIRECTORY_HIDDEN_ATTRIBUTE: CF_INSYNC_POLICY = 64u32;
pub const CF_INSYNC_POLICY_TRACK_DIRECTORY_LAST_WRITE_TIME: CF_INSYNC_POLICY = 512u32;
pub const CF_INSYNC_POLICY_TRACK_DIRECTORY_READONLY_ATTRIBUTE: CF_INSYNC_POLICY = 32u32;
pub const CF_INSYNC_POLICY_TRACK_DIRECTORY_SYSTEM_ATTRIBUTE: CF_INSYNC_POLICY = 128u32;
pub const CF_INSYNC_POLICY_TRACK_FILE_ALL: CF_INSYNC_POLICY = 5592335u32;
pub const CF_INSYNC_POLICY_TRACK_FILE_CREATION_TIME: CF_INSYNC_POLICY = 1u32;
pub const CF_INSYNC_POLICY_TRACK_FILE_HIDDEN_ATTRIBUTE: CF_INSYNC_POLICY = 4u32;
pub const CF_INSYNC_POLICY_TRACK_FILE_LAST_WRITE_TIME: CF_INSYNC_POLICY = 256u32;
pub const CF_INSYNC_POLICY_TRACK_FILE_READONLY_ATTRIBUTE: CF_INSYNC_POLICY = 2u32;
pub const CF_INSYNC_POLICY_TRACK_FILE_SYSTEM_ATTRIBUTE: CF_INSYNC_POLICY = 8u32;
pub const CF_IN_SYNC_STATE_IN_SYNC: CF_IN_SYNC_STATE = 1i32;
pub const CF_IN_SYNC_STATE_NOT_IN_SYNC: CF_IN_SYNC_STATE = 0i32;
pub const CF_MAX_PRIORITY_HINT: u32 = 15u32;
pub const CF_MAX_PROVIDER_NAME_LENGTH: u32 = 255u32;
pub const CF_MAX_PROVIDER_VERSION_LENGTH: u32 = 255u32;
pub const CF_OPEN_FILE_FLAG_DELETE_ACCESS: CF_OPEN_FILE_FLAGS = 4i32;
pub const CF_OPEN_FILE_FLAG_EXCLUSIVE: CF_OPEN_FILE_FLAGS = 1i32;
pub const CF_OPEN_FILE_FLAG_FOREGROUND: CF_OPEN_FILE_FLAGS = 8i32;
pub const CF_OPEN_FILE_FLAG_NONE: CF_OPEN_FILE_FLAGS = 0i32;
pub const CF_OPEN_FILE_FLAG_WRITE_ACCESS: CF_OPEN_FILE_FLAGS = 2i32;
pub const CF_OPERATION_ACK_DATA_FLAG_NONE: CF_OPERATION_ACK_DATA_FLAGS = 0i32;
pub const CF_OPERATION_ACK_DEHYDRATE_FLAG_NONE: CF_OPERATION_ACK_DEHYDRATE_FLAGS = 0i32;
pub const CF_OPERATION_ACK_DELETE_FLAG_NONE: CF_OPERATION_ACK_DELETE_FLAGS = 0i32;
pub const CF_OPERATION_ACK_RENAME_FLAG_NONE: CF_OPERATION_ACK_RENAME_FLAGS = 0i32;
pub const CF_OPERATION_RESTART_HYDRATION_FLAG_MARK_IN_SYNC: CF_OPERATION_RESTART_HYDRATION_FLAGS = 1i32;
pub const CF_OPERATION_RESTART_HYDRATION_FLAG_NONE: CF_OPERATION_RESTART_HYDRATION_FLAGS = 0i32;
pub const CF_OPERATION_RETRIEVE_DATA_FLAG_NONE: CF_OPERATION_RETRIEVE_DATA_FLAGS = 0i32;
pub const CF_OPERATION_TRANSFER_DATA_FLAG_NONE: CF_OPERATION_TRANSFER_DATA_FLAGS = 0i32;
pub const CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAG_DISABLE_ON_DEMAND_POPULATION: CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAGS = 2i32;
pub const CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAG_NONE: CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAGS = 0i32;
pub const CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAG_STOP_ON_ERROR: CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAGS = 1i32;
pub const CF_OPERATION_TYPE_ACK_DATA: CF_OPERATION_TYPE = 2i32;
pub const CF_OPERATION_TYPE_ACK_DEHYDRATE: CF_OPERATION_TYPE = 5i32;
pub const CF_OPERATION_TYPE_ACK_DELETE: CF_OPERATION_TYPE = 6i32;
pub const CF_OPERATION_TYPE_ACK_RENAME: CF_OPERATION_TYPE = 7i32;
pub const CF_OPERATION_TYPE_RESTART_HYDRATION: CF_OPERATION_TYPE = 3i32;
pub const CF_OPERATION_TYPE_RETRIEVE_DATA: CF_OPERATION_TYPE = 1i32;
pub const CF_OPERATION_TYPE_TRANSFER_DATA: CF_OPERATION_TYPE = 0i32;
pub const CF_OPERATION_TYPE_TRANSFER_PLACEHOLDERS: CF_OPERATION_TYPE = 4i32;
pub const CF_PIN_STATE_EXCLUDED: CF_PIN_STATE = 3i32;
pub const CF_PIN_STATE_INHERIT: CF_PIN_STATE = 4i32;
pub const CF_PIN_STATE_PINNED: CF_PIN_STATE = 1i32;
pub const CF_PIN_STATE_UNPINNED: CF_PIN_STATE = 2i32;
pub const CF_PIN_STATE_UNSPECIFIED: CF_PIN_STATE = 0i32;
pub const CF_PLACEHOLDER_CREATE_FLAG_ALWAYS_FULL: CF_PLACEHOLDER_CREATE_FLAGS = 8i32;
pub const CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION: CF_PLACEHOLDER_CREATE_FLAGS = 1i32;
pub const CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC: CF_PLACEHOLDER_CREATE_FLAGS = 2i32;
pub const CF_PLACEHOLDER_CREATE_FLAG_NONE: CF_PLACEHOLDER_CREATE_FLAGS = 0i32;
pub const CF_PLACEHOLDER_CREATE_FLAG_SUPERSEDE: CF_PLACEHOLDER_CREATE_FLAGS = 4i32;
pub const CF_PLACEHOLDER_INFO_BASIC: CF_PLACEHOLDER_INFO_CLASS = 0i32;
pub const CF_PLACEHOLDER_INFO_STANDARD: CF_PLACEHOLDER_INFO_CLASS = 1i32;
pub const CF_PLACEHOLDER_MANAGEMENT_POLICY_CONVERT_TO_UNRESTRICTED: CF_PLACEHOLDER_MANAGEMENT_POLICY = 2i32;
pub const CF_PLACEHOLDER_MANAGEMENT_POLICY_CREATE_UNRESTRICTED: CF_PLACEHOLDER_MANAGEMENT_POLICY = 1i32;
pub const CF_PLACEHOLDER_MANAGEMENT_POLICY_DEFAULT: CF_PLACEHOLDER_MANAGEMENT_POLICY = 0i32;
pub const CF_PLACEHOLDER_MANAGEMENT_POLICY_UPDATE_UNRESTRICTED: CF_PLACEHOLDER_MANAGEMENT_POLICY = 4i32;
pub const CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH: u32 = 4096u32;
pub const CF_PLACEHOLDER_RANGE_INFO_MODIFIED: CF_PLACEHOLDER_RANGE_INFO_CLASS = 3i32;
pub const CF_PLACEHOLDER_RANGE_INFO_ONDISK: CF_PLACEHOLDER_RANGE_INFO_CLASS = 1i32;
pub const CF_PLACEHOLDER_RANGE_INFO_VALIDATED: CF_PLACEHOLDER_RANGE_INFO_CLASS = 2i32;
pub const CF_PLACEHOLDER_STATE_ESSENTIAL_PROP_PRESENT: CF_PLACEHOLDER_STATE = 4u32;
pub const CF_PLACEHOLDER_STATE_INVALID: CF_PLACEHOLDER_STATE = 4294967295u32;
pub const CF_PLACEHOLDER_STATE_IN_SYNC: CF_PLACEHOLDER_STATE = 8u32;
pub const CF_PLACEHOLDER_STATE_NO_STATES: CF_PLACEHOLDER_STATE = 0u32;
pub const CF_PLACEHOLDER_STATE_PARTIAL: CF_PLACEHOLDER_STATE = 16u32;
pub const CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK: CF_PLACEHOLDER_STATE = 32u32;
pub const CF_PLACEHOLDER_STATE_PLACEHOLDER: CF_PLACEHOLDER_STATE = 1u32;
pub const CF_PLACEHOLDER_STATE_SYNC_ROOT: CF_PLACEHOLDER_STATE = 2u32;
pub const CF_POPULATION_POLICY_ALWAYS_FULL: CF_POPULATION_POLICY_PRIMARY = 3u16;
pub const CF_POPULATION_POLICY_FULL: CF_POPULATION_POLICY_PRIMARY = 2u16;
pub const CF_POPULATION_POLICY_MODIFIER_NONE: CF_POPULATION_POLICY_MODIFIER = 0u16;
pub const CF_POPULATION_POLICY_PARTIAL: CF_POPULATION_POLICY_PRIMARY = 0u16;
pub const CF_PROVIDER_STATUS_CLEAR_FLAGS: CF_SYNC_PROVIDER_STATUS = 2147483648u32;
pub const CF_PROVIDER_STATUS_CONNECTIVITY_LOST: CF_SYNC_PROVIDER_STATUS = 64u32;
pub const CF_PROVIDER_STATUS_DISCONNECTED: CF_SYNC_PROVIDER_STATUS = 0u32;
pub const CF_PROVIDER_STATUS_ERROR: CF_SYNC_PROVIDER_STATUS = 3221225474u32;
pub const CF_PROVIDER_STATUS_IDLE: CF_SYNC_PROVIDER_STATUS = 1u32;
pub const CF_PROVIDER_STATUS_POPULATE_CONTENT: CF_SYNC_PROVIDER_STATUS = 8u32;
pub const CF_PROVIDER_STATUS_POPULATE_METADATA: CF_SYNC_PROVIDER_STATUS = 4u32;
pub const CF_PROVIDER_STATUS_POPULATE_NAMESPACE: CF_SYNC_PROVIDER_STATUS = 2u32;
pub const CF_PROVIDER_STATUS_SYNC_FULL: CF_SYNC_PROVIDER_STATUS = 32u32;
pub const CF_PROVIDER_STATUS_SYNC_INCREMENTAL: CF_SYNC_PROVIDER_STATUS = 16u32;
pub const CF_PROVIDER_STATUS_TERMINATED: CF_SYNC_PROVIDER_STATUS = 3221225473u32;
pub const CF_REGISTER_FLAG_DISABLE_ON_DEMAND_POPULATION_ON_ROOT: CF_REGISTER_FLAGS = 2i32;
pub const CF_REGISTER_FLAG_MARK_IN_SYNC_ON_ROOT: CF_REGISTER_FLAGS = 4i32;
pub const CF_REGISTER_FLAG_NONE: CF_REGISTER_FLAGS = 0i32;
pub const CF_REGISTER_FLAG_UPDATE: CF_REGISTER_FLAGS = 1i32;
pub const CF_REQUEST_KEY_DEFAULT: u32 = 0u32;
pub const CF_REVERT_FLAG_NONE: CF_REVERT_FLAGS = 0i32;
pub const CF_SET_IN_SYNC_FLAG_NONE: CF_SET_IN_SYNC_FLAGS = 0i32;
pub const CF_SET_PIN_FLAG_NONE: CF_SET_PIN_FLAGS = 0i32;
pub const CF_SET_PIN_FLAG_RECURSE: CF_SET_PIN_FLAGS = 1i32;
pub const CF_SET_PIN_FLAG_RECURSE_ONLY: CF_SET_PIN_FLAGS = 2i32;
pub const CF_SET_PIN_FLAG_RECURSE_STOP_ON_ERROR: CF_SET_PIN_FLAGS = 4i32;
pub const CF_SYNC_ROOT_INFO_BASIC: CF_SYNC_ROOT_INFO_CLASS = 0i32;
pub const CF_SYNC_ROOT_INFO_PROVIDER: CF_SYNC_ROOT_INFO_CLASS = 2i32;
pub const CF_SYNC_ROOT_INFO_STANDARD: CF_SYNC_ROOT_INFO_CLASS = 1i32;
pub const CF_UPDATE_FLAG_ALLOW_PARTIAL: CF_UPDATE_FLAGS = 1024i32;
pub const CF_UPDATE_FLAG_ALWAYS_FULL: CF_UPDATE_FLAGS = 512i32;
pub const CF_UPDATE_FLAG_CLEAR_IN_SYNC: CF_UPDATE_FLAGS = 64i32;
pub const CF_UPDATE_FLAG_DEHYDRATE: CF_UPDATE_FLAGS = 4i32;
pub const CF_UPDATE_FLAG_DISABLE_ON_DEMAND_POPULATION: CF_UPDATE_FLAGS = 16i32;
pub const CF_UPDATE_FLAG_ENABLE_ON_DEMAND_POPULATION: CF_UPDATE_FLAGS = 8i32;
pub const CF_UPDATE_FLAG_MARK_IN_SYNC: CF_UPDATE_FLAGS = 2i32;
pub const CF_UPDATE_FLAG_NONE: CF_UPDATE_FLAGS = 0i32;
pub const CF_UPDATE_FLAG_PASSTHROUGH_FS_METADATA: CF_UPDATE_FLAGS = 256i32;
pub const CF_UPDATE_FLAG_REMOVE_FILE_IDENTITY: CF_UPDATE_FLAGS = 32i32;
pub const CF_UPDATE_FLAG_REMOVE_PROPERTY: CF_UPDATE_FLAGS = 128i32;
pub const CF_UPDATE_FLAG_VERIFY_IN_SYNC: CF_UPDATE_FLAGS = 1i32;
pub type CF_CALLBACK_CANCEL_FLAGS = i32;
pub type CF_CALLBACK_CLOSE_COMPLETION_FLAGS = i32;
pub type CF_CALLBACK_DEHYDRATE_COMPLETION_FLAGS = i32;
pub type CF_CALLBACK_DEHYDRATE_FLAGS = i32;
pub type CF_CALLBACK_DEHYDRATION_REASON = i32;
pub type CF_CALLBACK_DELETE_COMPLETION_FLAGS = i32;
pub type CF_CALLBACK_DELETE_FLAGS = i32;
pub type CF_CALLBACK_FETCH_DATA_FLAGS = i32;
pub type CF_CALLBACK_FETCH_PLACEHOLDERS_FLAGS = i32;
pub type CF_CALLBACK_OPEN_COMPLETION_FLAGS = i32;
pub type CF_CALLBACK_RENAME_COMPLETION_FLAGS = i32;
pub type CF_CALLBACK_RENAME_FLAGS = i32;
pub type CF_CALLBACK_TYPE = i32;
pub type CF_CALLBACK_VALIDATE_DATA_FLAGS = i32;
pub type CF_CONNECT_FLAGS = i32;
pub type CF_CONVERT_FLAGS = i32;
pub type CF_CREATE_FLAGS = i32;
pub type CF_DEHYDRATE_FLAGS = i32;
pub type CF_HARDLINK_POLICY = i32;
pub type CF_HYDRATE_FLAGS = i32;
pub type CF_HYDRATION_POLICY_MODIFIER = u16;
pub type CF_HYDRATION_POLICY_PRIMARY = u16;
pub type CF_INSYNC_POLICY = u32;
pub type CF_IN_SYNC_STATE = i32;
pub type CF_OPEN_FILE_FLAGS = i32;
pub type CF_OPERATION_ACK_DATA_FLAGS = i32;
pub type CF_OPERATION_ACK_DEHYDRATE_FLAGS = i32;
pub type CF_OPERATION_ACK_DELETE_FLAGS = i32;
pub type CF_OPERATION_ACK_RENAME_FLAGS = i32;
pub type CF_OPERATION_RESTART_HYDRATION_FLAGS = i32;
pub type CF_OPERATION_RETRIEVE_DATA_FLAGS = i32;
pub type CF_OPERATION_TRANSFER_DATA_FLAGS = i32;
pub type CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAGS = i32;
pub type CF_OPERATION_TYPE = i32;
pub type CF_PIN_STATE = i32;
pub type CF_PLACEHOLDER_CREATE_FLAGS = i32;
pub type CF_PLACEHOLDER_INFO_CLASS = i32;
pub type CF_PLACEHOLDER_MANAGEMENT_POLICY = i32;
pub type CF_PLACEHOLDER_RANGE_INFO_CLASS = i32;
pub type CF_PLACEHOLDER_STATE = u32;
pub type CF_POPULATION_POLICY_MODIFIER = u16;
pub type CF_POPULATION_POLICY_PRIMARY = u16;
pub type CF_REGISTER_FLAGS = i32;
pub type CF_REVERT_FLAGS = i32;
pub type CF_SET_IN_SYNC_FLAGS = i32;
pub type CF_SET_PIN_FLAGS = i32;
pub type CF_SYNC_PROVIDER_STATUS = u32;
pub type CF_SYNC_ROOT_INFO_CLASS = i32;
pub type CF_UPDATE_FLAGS = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_System_CorrelationVector\"`"]
#[cfg(feature = "Win32_System_CorrelationVector")]
pub struct CF_CALLBACK_INFO {
    pub StructSize: u32,
    pub ConnectionKey: CF_CONNECTION_KEY,
    pub CallbackContext: *mut ::core::ffi::c_void,
    pub VolumeGuidName: ::windows_sys::core::PCWSTR,
    pub VolumeDosName: ::windows_sys::core::PCWSTR,
    pub VolumeSerialNumber: u32,
    pub SyncRootFileId: i64,
    pub SyncRootIdentity: *const ::core::ffi::c_void,
    pub SyncRootIdentityLength: u32,
    pub FileId: i64,
    pub FileSize: i64,
    pub FileIdentity: *const ::core::ffi::c_void,
    pub FileIdentityLength: u32,
    pub NormalizedPath: ::windows_sys::core::PCWSTR,
    pub TransferKey: i64,
    pub PriorityHint: u8,
    pub CorrelationVector: *mut super::super::System::CorrelationVector::CORRELATION_VECTOR,
    pub ProcessInfo: *mut CF_PROCESS_INFO,
    pub RequestKey: i64,
}
#[cfg(feature = "Win32_System_CorrelationVector")]
impl ::core::marker::Copy for CF_CALLBACK_INFO {}
#[cfg(feature = "Win32_System_CorrelationVector")]
impl ::core::clone::Clone for CF_CALLBACK_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS {
    pub ParamSize: u32,
    pub Anonymous: CF_CALLBACK_PARAMETERS_0,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CF_CALLBACK_PARAMETERS_0 {
    pub Cancel: CF_CALLBACK_PARAMETERS_0_0,
    pub FetchData: CF_CALLBACK_PARAMETERS_0_6,
    pub ValidateData: CF_CALLBACK_PARAMETERS_0_11,
    pub FetchPlaceholders: CF_CALLBACK_PARAMETERS_0_7,
    pub OpenCompletion: CF_CALLBACK_PARAMETERS_0_8,
    pub CloseCompletion: CF_CALLBACK_PARAMETERS_0_1,
    pub Dehydrate: CF_CALLBACK_PARAMETERS_0_3,
    pub DehydrateCompletion: CF_CALLBACK_PARAMETERS_0_2,
    pub Delete: CF_CALLBACK_PARAMETERS_0_5,
    pub DeleteCompletion: CF_CALLBACK_PARAMETERS_0_4,
    pub Rename: CF_CALLBACK_PARAMETERS_0_10,
    pub RenameCompletion: CF_CALLBACK_PARAMETERS_0_9,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_0 {
    pub Flags: CF_CALLBACK_CANCEL_FLAGS,
    pub Anonymous: CF_CALLBACK_PARAMETERS_0_0_0,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_0 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CF_CALLBACK_PARAMETERS_0_0_0 {
    pub FetchData: CF_CALLBACK_PARAMETERS_0_0_0_0,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_0_0 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_0_0_0 {
    pub FileOffset: i64,
    pub Length: i64,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_0_0_0 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_1 {
    pub Flags: CF_CALLBACK_CLOSE_COMPLETION_FLAGS,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_1 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_2 {
    pub Flags: CF_CALLBACK_DEHYDRATE_COMPLETION_FLAGS,
    pub Reason: CF_CALLBACK_DEHYDRATION_REASON,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_2 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_3 {
    pub Flags: CF_CALLBACK_DEHYDRATE_FLAGS,
    pub Reason: CF_CALLBACK_DEHYDRATION_REASON,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_3 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_4 {
    pub Flags: CF_CALLBACK_DELETE_COMPLETION_FLAGS,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_4 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_5 {
    pub Flags: CF_CALLBACK_DELETE_FLAGS,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_5 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_6 {
    pub Flags: CF_CALLBACK_FETCH_DATA_FLAGS,
    pub RequiredFileOffset: i64,
    pub RequiredLength: i64,
    pub OptionalFileOffset: i64,
    pub OptionalLength: i64,
    pub LastDehydrationTime: i64,
    pub LastDehydrationReason: CF_CALLBACK_DEHYDRATION_REASON,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_6 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_7 {
    pub Flags: CF_CALLBACK_FETCH_PLACEHOLDERS_FLAGS,
    pub Pattern: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_7 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_7 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_8 {
    pub Flags: CF_CALLBACK_OPEN_COMPLETION_FLAGS,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_8 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_8 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_9 {
    pub Flags: CF_CALLBACK_RENAME_COMPLETION_FLAGS,
    pub SourcePath: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_9 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_9 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_10 {
    pub Flags: CF_CALLBACK_RENAME_FLAGS,
    pub TargetPath: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_10 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_10 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_CALLBACK_PARAMETERS_0_11 {
    pub Flags: CF_CALLBACK_VALIDATE_DATA_FLAGS,
    pub RequiredFileOffset: i64,
    pub RequiredLength: i64,
}
impl ::core::marker::Copy for CF_CALLBACK_PARAMETERS_0_11 {}
impl ::core::clone::Clone for CF_CALLBACK_PARAMETERS_0_11 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_CorrelationVector\"`"]
#[cfg(feature = "Win32_System_CorrelationVector")]
pub struct CF_CALLBACK_REGISTRATION {
    pub Type: CF_CALLBACK_TYPE,
    pub Callback: CF_CALLBACK,
}
#[cfg(feature = "Win32_System_CorrelationVector")]
impl ::core::marker::Copy for CF_CALLBACK_REGISTRATION {}
#[cfg(feature = "Win32_System_CorrelationVector")]
impl ::core::clone::Clone for CF_CALLBACK_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
pub type CF_CONNECTION_KEY = i64;
#[repr(C)]
pub struct CF_FILE_RANGE {
    pub StartingOffset: i64,
    pub Length: i64,
}
impl ::core::marker::Copy for CF_FILE_RANGE {}
impl ::core::clone::Clone for CF_FILE_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct CF_FS_METADATA {
    pub BasicInfo: super::FileSystem::FILE_BASIC_INFO,
    pub FileSize: i64,
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for CF_FS_METADATA {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for CF_FS_METADATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_HYDRATION_POLICY {
    pub Primary: CF_HYDRATION_POLICY_PRIMARY,
    pub Modifier: CF_HYDRATION_POLICY_MODIFIER,
}
impl ::core::marker::Copy for CF_HYDRATION_POLICY {}
impl ::core::clone::Clone for CF_HYDRATION_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_CorrelationVector\"`"]
#[cfg(feature = "Win32_System_CorrelationVector")]
pub struct CF_OPERATION_INFO {
    pub StructSize: u32,
    pub Type: CF_OPERATION_TYPE,
    pub ConnectionKey: CF_CONNECTION_KEY,
    pub TransferKey: i64,
    pub CorrelationVector: *const super::super::System::CorrelationVector::CORRELATION_VECTOR,
    pub SyncStatus: *const CF_SYNC_STATUS,
    pub RequestKey: i64,
}
#[cfg(feature = "Win32_System_CorrelationVector")]
impl ::core::marker::Copy for CF_OPERATION_INFO {}
#[cfg(feature = "Win32_System_CorrelationVector")]
impl ::core::clone::Clone for CF_OPERATION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub struct CF_OPERATION_PARAMETERS {
    pub ParamSize: u32,
    pub Anonymous: CF_OPERATION_PARAMETERS_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub union CF_OPERATION_PARAMETERS_0 {
    pub TransferData: CF_OPERATION_PARAMETERS_0_6,
    pub RetrieveData: CF_OPERATION_PARAMETERS_0_5,
    pub AckData: CF_OPERATION_PARAMETERS_0_0,
    pub RestartHydration: CF_OPERATION_PARAMETERS_0_4,
    pub TransferPlaceholders: CF_OPERATION_PARAMETERS_0_7,
    pub AckDehydrate: CF_OPERATION_PARAMETERS_0_1,
    pub AckRename: CF_OPERATION_PARAMETERS_0_3,
    pub AckDelete: CF_OPERATION_PARAMETERS_0_2,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub struct CF_OPERATION_PARAMETERS_0_0 {
    pub Flags: CF_OPERATION_ACK_DATA_FLAGS,
    pub CompletionStatus: super::super::Foundation::NTSTATUS,
    pub Offset: i64,
    pub Length: i64,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub struct CF_OPERATION_PARAMETERS_0_1 {
    pub Flags: CF_OPERATION_ACK_DEHYDRATE_FLAGS,
    pub CompletionStatus: super::super::Foundation::NTSTATUS,
    pub FileIdentity: *const ::core::ffi::c_void,
    pub FileIdentityLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS_0_1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub struct CF_OPERATION_PARAMETERS_0_2 {
    pub Flags: CF_OPERATION_ACK_DELETE_FLAGS,
    pub CompletionStatus: super::super::Foundation::NTSTATUS,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS_0_2 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub struct CF_OPERATION_PARAMETERS_0_3 {
    pub Flags: CF_OPERATION_ACK_RENAME_FLAGS,
    pub CompletionStatus: super::super::Foundation::NTSTATUS,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS_0_3 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub struct CF_OPERATION_PARAMETERS_0_4 {
    pub Flags: CF_OPERATION_RESTART_HYDRATION_FLAGS,
    pub FsMetadata: *const CF_FS_METADATA,
    pub FileIdentity: *const ::core::ffi::c_void,
    pub FileIdentityLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS_0_4 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS_0_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub struct CF_OPERATION_PARAMETERS_0_5 {
    pub Flags: CF_OPERATION_RETRIEVE_DATA_FLAGS,
    pub Buffer: *mut ::core::ffi::c_void,
    pub Offset: i64,
    pub Length: i64,
    pub ReturnedLength: i64,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS_0_5 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS_0_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub struct CF_OPERATION_PARAMETERS_0_6 {
    pub Flags: CF_OPERATION_TRANSFER_DATA_FLAGS,
    pub CompletionStatus: super::super::Foundation::NTSTATUS,
    pub Buffer: *const ::core::ffi::c_void,
    pub Offset: i64,
    pub Length: i64,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS_0_6 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS_0_6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
pub struct CF_OPERATION_PARAMETERS_0_7 {
    pub Flags: CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAGS,
    pub CompletionStatus: super::super::Foundation::NTSTATUS,
    pub PlaceholderTotalCount: i64,
    pub PlaceholderArray: *mut CF_PLACEHOLDER_CREATE_INFO,
    pub PlaceholderCount: u32,
    pub EntriesProcessed: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::marker::Copy for CF_OPERATION_PARAMETERS_0_7 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem"))]
impl ::core::clone::Clone for CF_OPERATION_PARAMETERS_0_7 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_PLACEHOLDER_BASIC_INFO {
    pub PinState: CF_PIN_STATE,
    pub InSyncState: CF_IN_SYNC_STATE,
    pub FileId: i64,
    pub SyncRootFileId: i64,
    pub FileIdentityLength: u32,
    pub FileIdentity: [u8; 1],
}
impl ::core::marker::Copy for CF_PLACEHOLDER_BASIC_INFO {}
impl ::core::clone::Clone for CF_PLACEHOLDER_BASIC_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct CF_PLACEHOLDER_CREATE_INFO {
    pub RelativeFileName: ::windows_sys::core::PCWSTR,
    pub FsMetadata: CF_FS_METADATA,
    pub FileIdentity: *const ::core::ffi::c_void,
    pub FileIdentityLength: u32,
    pub Flags: CF_PLACEHOLDER_CREATE_FLAGS,
    pub Result: ::windows_sys::core::HRESULT,
    pub CreateUsn: i64,
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for CF_PLACEHOLDER_CREATE_INFO {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for CF_PLACEHOLDER_CREATE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_PLACEHOLDER_STANDARD_INFO {
    pub OnDiskDataSize: i64,
    pub ValidatedDataSize: i64,
    pub ModifiedDataSize: i64,
    pub PropertiesSize: i64,
    pub PinState: CF_PIN_STATE,
    pub InSyncState: CF_IN_SYNC_STATE,
    pub FileId: i64,
    pub SyncRootFileId: i64,
    pub FileIdentityLength: u32,
    pub FileIdentity: [u8; 1],
}
impl ::core::marker::Copy for CF_PLACEHOLDER_STANDARD_INFO {}
impl ::core::clone::Clone for CF_PLACEHOLDER_STANDARD_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_PLATFORM_INFO {
    pub BuildNumber: u32,
    pub RevisionNumber: u32,
    pub IntegrationNumber: u32,
}
impl ::core::marker::Copy for CF_PLATFORM_INFO {}
impl ::core::clone::Clone for CF_PLATFORM_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_POPULATION_POLICY {
    pub Primary: CF_POPULATION_POLICY_PRIMARY,
    pub Modifier: CF_POPULATION_POLICY_MODIFIER,
}
impl ::core::marker::Copy for CF_POPULATION_POLICY {}
impl ::core::clone::Clone for CF_POPULATION_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_PROCESS_INFO {
    pub StructSize: u32,
    pub ProcessId: u32,
    pub ImagePath: ::windows_sys::core::PCWSTR,
    pub PackageName: ::windows_sys::core::PCWSTR,
    pub ApplicationId: ::windows_sys::core::PCWSTR,
    pub CommandLine: ::windows_sys::core::PCWSTR,
    pub SessionId: u32,
}
impl ::core::marker::Copy for CF_PROCESS_INFO {}
impl ::core::clone::Clone for CF_PROCESS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_SYNC_POLICIES {
    pub StructSize: u32,
    pub Hydration: CF_HYDRATION_POLICY,
    pub Population: CF_POPULATION_POLICY,
    pub InSync: CF_INSYNC_POLICY,
    pub HardLink: CF_HARDLINK_POLICY,
    pub PlaceholderManagement: CF_PLACEHOLDER_MANAGEMENT_POLICY,
}
impl ::core::marker::Copy for CF_SYNC_POLICIES {}
impl ::core::clone::Clone for CF_SYNC_POLICIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_SYNC_REGISTRATION {
    pub StructSize: u32,
    pub ProviderName: ::windows_sys::core::PCWSTR,
    pub ProviderVersion: ::windows_sys::core::PCWSTR,
    pub SyncRootIdentity: *const ::core::ffi::c_void,
    pub SyncRootIdentityLength: u32,
    pub FileIdentity: *const ::core::ffi::c_void,
    pub FileIdentityLength: u32,
    pub ProviderId: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for CF_SYNC_REGISTRATION {}
impl ::core::clone::Clone for CF_SYNC_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_SYNC_ROOT_BASIC_INFO {
    pub SyncRootFileId: i64,
}
impl ::core::marker::Copy for CF_SYNC_ROOT_BASIC_INFO {}
impl ::core::clone::Clone for CF_SYNC_ROOT_BASIC_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_SYNC_ROOT_PROVIDER_INFO {
    pub ProviderStatus: CF_SYNC_PROVIDER_STATUS,
    pub ProviderName: [u16; 256],
    pub ProviderVersion: [u16; 256],
}
impl ::core::marker::Copy for CF_SYNC_ROOT_PROVIDER_INFO {}
impl ::core::clone::Clone for CF_SYNC_ROOT_PROVIDER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_SYNC_ROOT_STANDARD_INFO {
    pub SyncRootFileId: i64,
    pub HydrationPolicy: CF_HYDRATION_POLICY,
    pub PopulationPolicy: CF_POPULATION_POLICY,
    pub InSyncPolicy: CF_INSYNC_POLICY,
    pub HardLinkPolicy: CF_HARDLINK_POLICY,
    pub ProviderStatus: CF_SYNC_PROVIDER_STATUS,
    pub ProviderName: [u16; 256],
    pub ProviderVersion: [u16; 256],
    pub SyncRootIdentityLength: u32,
    pub SyncRootIdentity: [u8; 1],
}
impl ::core::marker::Copy for CF_SYNC_ROOT_STANDARD_INFO {}
impl ::core::clone::Clone for CF_SYNC_ROOT_STANDARD_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CF_SYNC_STATUS {
    pub StructSize: u32,
    pub Code: u32,
    pub DescriptionOffset: u32,
    pub DescriptionLength: u32,
    pub DeviceIdOffset: u32,
    pub DeviceIdLength: u32,
}
impl ::core::marker::Copy for CF_SYNC_STATUS {}
impl ::core::clone::Clone for CF_SYNC_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_System_CorrelationVector\"`"]
#[cfg(feature = "Win32_System_CorrelationVector")]
pub type CF_CALLBACK = ::core::option::Option<unsafe extern "system" fn(callbackinfo: *const CF_CALLBACK_INFO, callbackparameters: *const CF_CALLBACK_PARAMETERS) -> ()>;
