#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListAllocateBufferAndSerialize(sourcecollection : *const SENSOR_COLLECTION_LIST, ptargetbuffersizeinbytes : *mut u32, ptargetbuffer : *mut *mut u8) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListCopyAndMarshall(target : *mut SENSOR_COLLECTION_LIST, source : *const SENSOR_COLLECTION_LIST) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListDeserializeFromBuffer(sourcebuffersizeinbytes : u32, sourcebuffer : *const u8, targetcollection : *mut SENSOR_COLLECTION_LIST) -> super::super::Foundation:: NTSTATUS);
::windows_targets::link!("sensorsutilsv2.dll" "system" fn CollectionsListGetFillableCount(buffersizebytes : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListGetMarshalledSize(collection : *const SENSOR_COLLECTION_LIST) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListGetMarshalledSizeWithoutSerialization(collection : *const SENSOR_COLLECTION_LIST) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListGetSerializedSize(collection : *const SENSOR_COLLECTION_LIST) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListMarshall(target : *mut SENSOR_COLLECTION_LIST) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListSerializeToBuffer(sourcecollection : *const SENSOR_COLLECTION_LIST, targetbuffersizeinbytes : u32, targetbuffer : *mut u8) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListSortSubscribedActivitiesByConfidence(thresholds : *const SENSOR_COLLECTION_LIST, pcollection : *mut SENSOR_COLLECTION_LIST) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn CollectionsListUpdateMarshalledPointer(collection : *mut SENSOR_COLLECTION_LIST) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn EvaluateActivityThresholds(newsample : *const SENSOR_COLLECTION_LIST, oldsample : *const SENSOR_COLLECTION_LIST, thresholds : *const SENSOR_COLLECTION_LIST) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetPerformanceTime(timems : *mut u32) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromCLSIDArray(members : *const ::windows_sys::core::GUID, size : u32, ppropvar : *mut super::super::System::Com::StructuredStorage:: PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn InitPropVariantFromFloat(fltval : f32, ppropvar : *mut super::super::System::Com::StructuredStorage:: PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn IsCollectionListSame(lista : *const SENSOR_COLLECTION_LIST, listb : *const SENSOR_COLLECTION_LIST) -> super::super::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsGUIDPresentInList(guidarray : *const ::windows_sys::core::GUID, arraylength : u32, guidelem : *const ::windows_sys::core::GUID) -> super::super::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn IsKeyPresentInCollectionList(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY) -> super::super::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn IsKeyPresentInPropertyList(plist : *const SENSOR_PROPERTY_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY) -> super::super::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn IsSensorSubscribed(subscriptionlist : *const SENSOR_COLLECTION_LIST, currenttype : ::windows_sys::core::GUID) -> super::super::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetBool(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pretvalue : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetDouble(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pretvalue : *mut f64) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetFileTime(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pretvalue : *mut super::super::Foundation:: FILETIME) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetFloat(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pretvalue : *mut f32) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetGuid(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pretvalue : *mut ::windows_sys::core::GUID) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetInt32(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pretvalue : *mut i32) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetInt64(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pretvalue : *mut i64) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetNthInt64(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, occurrence : u32, pretvalue : *mut i64) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetNthUlong(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, occurrence : u32, pretvalue : *mut u32) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetNthUshort(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, occurrence : u32, pretvalue : *mut u16) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetPropVariant(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, typecheck : super::super::Foundation:: BOOLEAN, pvalue : *mut super::super::System::Com::StructuredStorage:: PROPVARIANT) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetUlong(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pretvalue : *mut u32) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeyGetUshort(plist : *const SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pretvalue : *mut u16) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropKeyFindKeySetPropVariant(plist : *mut SENSOR_COLLECTION_LIST, pkey : *const super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, typecheck : super::super::Foundation:: BOOLEAN, pvalue : *const super::super::System::Com::StructuredStorage:: PROPVARIANT) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn PropVariantGetInformation(propvariantvalue : *const super::super::System::Com::StructuredStorage:: PROPVARIANT, propvariantoffset : *mut u32, propvariantsize : *mut u32, propvariantpointer : *mut *mut ::core::ffi::c_void, remappedtype : *mut super::Properties:: DEVPROPTYPE) -> super::super::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn PropertiesListCopy(target : *mut SENSOR_PROPERTY_LIST, source : *const SENSOR_PROPERTY_LIST) -> super::super::Foundation:: NTSTATUS);
::windows_targets::link!("sensorsutilsv2.dll" "system" fn PropertiesListGetFillableCount(buffersizebytes : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"] fn SensorCollectionGetAt(index : u32, psensorslist : *const SENSOR_COLLECTION_LIST, pkey : *mut super::super::UI::Shell::PropertiesSystem:: PROPERTYKEY, pvalue : *mut super::super::System::Com::StructuredStorage:: PROPVARIANT) -> super::super::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("sensorsutilsv2.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SerializationBufferAllocate(sizeinbytes : u32, pbuffer : *mut *mut u8) -> super::super::Foundation:: NTSTATUS);
::windows_targets::link!("sensorsutilsv2.dll" "system" fn SerializationBufferFree(buffer : *const u8) -> ());
pub type ILocationPermissions = *mut ::core::ffi::c_void;
pub type ISensor = *mut ::core::ffi::c_void;
pub type ISensorCollection = *mut ::core::ffi::c_void;
pub type ISensorDataReport = *mut ::core::ffi::c_void;
pub type ISensorEvents = *mut ::core::ffi::c_void;
pub type ISensorManager = *mut ::core::ffi::c_void;
pub type ISensorManagerEvents = *mut ::core::ffi::c_void;
pub const AXIS_MAX: AXIS = 3i32;
pub const AXIS_X: AXIS = 0i32;
pub const AXIS_Y: AXIS = 1i32;
pub const AXIS_Z: AXIS = 2i32;
pub const ActivityStateCount: ACTIVITY_STATE_COUNT = 8i32;
pub const ActivityState_Biking: ACTIVITY_STATE = 64i32;
pub const ActivityState_Fidgeting: ACTIVITY_STATE = 4i32;
pub const ActivityState_Force_Dword: ACTIVITY_STATE = -1i32;
pub const ActivityState_Idle: ACTIVITY_STATE = 128i32;
pub const ActivityState_InVehicle: ACTIVITY_STATE = 32i32;
pub const ActivityState_Max: ACTIVITY_STATE = 256i32;
pub const ActivityState_Running: ACTIVITY_STATE = 16i32;
pub const ActivityState_Stationary: ACTIVITY_STATE = 2i32;
pub const ActivityState_Unknown: ACTIVITY_STATE = 1i32;
pub const ActivityState_Walking: ACTIVITY_STATE = 8i32;
pub const ElevationChangeMode_Elevator: ELEVATION_CHANGE_MODE = 1i32;
pub const ElevationChangeMode_Force_Dword: ELEVATION_CHANGE_MODE = -1i32;
pub const ElevationChangeMode_Max: ELEVATION_CHANGE_MODE = 3i32;
pub const ElevationChangeMode_Stepping: ELEVATION_CHANGE_MODE = 2i32;
pub const ElevationChangeMode_Unknown: ELEVATION_CHANGE_MODE = 0i32;
pub const GNSS_CLEAR_ALL_ASSISTANCE_DATA: u32 = 1u32;
pub const GUID_DEVINTERFACE_SENSOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xba1bb692_9b7a_4833_9a1e_525ed134e7e2);
pub const GUID_SensorCategory_All: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc317c286_c468_4288_9975_d4c4587c442c);
pub const GUID_SensorCategory_Biometric: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xca19690f_a2c7_477d_a99e_99ec6e2b5648);
pub const GUID_SensorCategory_Electrical: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfb73fcd8_fc4a_483c_ac58_27b691c6beff);
pub const GUID_SensorCategory_Environmental: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x323439aa_7f66_492b_ba0c_73e9aa0a65d5);
pub const GUID_SensorCategory_Light: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x17a665c0_9063_4216_b202_5c7a255e18ce);
pub const GUID_SensorCategory_Location: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbfa794e4_f964_4fdb_90f6_51056bfe4b44);
pub const GUID_SensorCategory_Mechanical: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8d131d68_8ef7_4656_80b5_cccbd93791c5);
pub const GUID_SensorCategory_Motion: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcd09daf1_3b2e_4c3d_b598_b5e5ff93fd46);
pub const GUID_SensorCategory_Orientation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9e6c04b6_96fe_4954_b726_68682a473f69);
pub const GUID_SensorCategory_Other: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2c90e7a9_f4c9_4fa2_af37_56d471fe5a3d);
pub const GUID_SensorCategory_PersonalActivity: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf1609081_1e12_412b_a14d_cbb0e95bd2e5);
pub const GUID_SensorCategory_Scanner: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb000e77e_f5b5_420f_815d_0270a726f270);
pub const GUID_SensorCategory_Unsupported: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2beae7fa_19b0_48c5_a1f6_b5480dc206b0);
pub const GUID_SensorType_Accelerometer3D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc2fb0f5f_e2d2_4c78_bcd0_352a9582819d);
pub const GUID_SensorType_ActivityDetection: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9d9e0118_1807_4f2e_96e4_2ce57142e196);
pub const GUID_SensorType_AmbientLight: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x97f115c8_599a_4153_8894_d2d12899918a);
pub const GUID_SensorType_Barometer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0e903829_ff8a_4a93_97df_3dcbde402288);
pub const GUID_SensorType_Custom: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe83af229_8640_4d18_a213_e22675ebb2c3);
pub const GUID_SensorType_FloorElevation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xade4987f_7ac4_4dfa_9722_0a027181c747);
pub const GUID_SensorType_GeomagneticOrientation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe77195f8_2d1f_4823_971b_1c4467556c9d);
pub const GUID_SensorType_GravityVector: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x03b52c73_bb76_463f_9524_38de76eb700b);
pub const GUID_SensorType_Gyrometer3D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x09485f5a_759e_42c2_bd4b_a349b75c8643);
pub const GUID_SensorType_HingeAngle: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x82358065_f4c4_4da1_b272_13c23332a207);
pub const GUID_SensorType_Humidity: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5c72bf67_bd7e_4257_990b_98a3ba3b400a);
pub const GUID_SensorType_LinearAccelerometer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x038b0283_97b4_41c8_bc24_5ff1aa48fec7);
pub const GUID_SensorType_Magnetometer3D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x55e5effb_15c7_40df_8698_a84b7c863c53);
pub const GUID_SensorType_Orientation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcdb5d8f7_3cfd_41c8_8542_cce622cf5d6e);
pub const GUID_SensorType_Pedometer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb19f89af_e3eb_444b_8dea_202575a71599);
pub const GUID_SensorType_Proximity: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5220dae9_3179_4430_9f90_06266d2a34de);
pub const GUID_SensorType_RelativeOrientation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x40993b51_4706_44dc_98d5_c920c037ffab);
pub const GUID_SensorType_SimpleDeviceOrientation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x86a19291_0482_402c_bf4c_addac52b1c39);
pub const GUID_SensorType_Temperature: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x04fd0ec4_d5da_45fa_95a9_5db38ee19306);
pub const HumanPresenceDetectionTypeCount: HUMAN_PRESENCE_DETECTION_TYPE_COUNT = 4i32;
pub const HumanPresenceDetectionType_AudioBiometric: HUMAN_PRESENCE_DETECTION_TYPE = 8i32;
pub const HumanPresenceDetectionType_FacialBiometric: HUMAN_PRESENCE_DETECTION_TYPE = 4i32;
pub const HumanPresenceDetectionType_Force_Dword: HUMAN_PRESENCE_DETECTION_TYPE = -1i32;
pub const HumanPresenceDetectionType_VendorDefinedBiometric: HUMAN_PRESENCE_DETECTION_TYPE = 2i32;
pub const HumanPresenceDetectionType_VendorDefinedNonBiometric: HUMAN_PRESENCE_DETECTION_TYPE = 1i32;
pub const LOCATION_DESIRED_ACCURACY_DEFAULT: LOCATION_DESIRED_ACCURACY = 0i32;
pub const LOCATION_DESIRED_ACCURACY_HIGH: LOCATION_DESIRED_ACCURACY = 1i32;
pub const LOCATION_POSITION_SOURCE_CELLULAR: LOCATION_POSITION_SOURCE = 0i32;
pub const LOCATION_POSITION_SOURCE_IPADDRESS: LOCATION_POSITION_SOURCE = 3i32;
pub const LOCATION_POSITION_SOURCE_SATELLITE: LOCATION_POSITION_SOURCE = 1i32;
pub const LOCATION_POSITION_SOURCE_UNKNOWN: LOCATION_POSITION_SOURCE = 4i32;
pub const LOCATION_POSITION_SOURCE_WIFI: LOCATION_POSITION_SOURCE = 2i32;
pub const MAGNETOMETER_ACCURACY_APPROXIMATE: MagnetometerAccuracy = 2i32;
pub const MAGNETOMETER_ACCURACY_HIGH: MagnetometerAccuracy = 3i32;
pub const MAGNETOMETER_ACCURACY_UNKNOWN: MagnetometerAccuracy = 0i32;
pub const MAGNETOMETER_ACCURACY_UNRELIABLE: MagnetometerAccuracy = 1i32;
pub const MagnetometerAccuracy_Approximate: MAGNETOMETER_ACCURACY = 2i32;
pub const MagnetometerAccuracy_High: MAGNETOMETER_ACCURACY = 3i32;
pub const MagnetometerAccuracy_Unknown: MAGNETOMETER_ACCURACY = 0i32;
pub const MagnetometerAccuracy_Unreliable: MAGNETOMETER_ACCURACY = 1i32;
pub const PedometerStepTypeCount: PEDOMETER_STEP_TYPE_COUNT = 3i32;
pub const PedometerStepType_Force_Dword: PEDOMETER_STEP_TYPE = -1i32;
pub const PedometerStepType_Max: PEDOMETER_STEP_TYPE = 8i32;
pub const PedometerStepType_Running: PEDOMETER_STEP_TYPE = 4i32;
pub const PedometerStepType_Unknown: PEDOMETER_STEP_TYPE = 1i32;
pub const PedometerStepType_Walking: PEDOMETER_STEP_TYPE = 2i32;
pub const ProximityType_Force_Dword: PROXIMITY_TYPE = -1i32;
pub const ProximityType_HumanProximity: PROXIMITY_TYPE = 1i32;
pub const ProximityType_ObjectProximity: PROXIMITY_TYPE = 0i32;
pub const Proximity_Sensor_Human_Engagement_Capable: PROXIMITY_SENSOR_CAPABILITIES = 2i32;
pub const Proximity_Sensor_Human_Presence_Capable: PROXIMITY_SENSOR_CAPABILITIES = 1i32;
pub const Proximity_Sensor_Supported_Capabilities: PROXIMITY_SENSOR_CAPABILITIES = 3i32;
pub const SENSOR_CATEGORY_ALL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc317c286_c468_4288_9975_d4c4587c442c);
pub const SENSOR_CATEGORY_BIOMETRIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xca19690f_a2c7_477d_a99e_99ec6e2b5648);
pub const SENSOR_CATEGORY_ELECTRICAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfb73fcd8_fc4a_483c_ac58_27b691c6beff);
pub const SENSOR_CATEGORY_ENVIRONMENTAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x323439aa_7f66_492b_ba0c_73e9aa0a65d5);
pub const SENSOR_CATEGORY_LIGHT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x17a665c0_9063_4216_b202_5c7a255e18ce);
pub const SENSOR_CATEGORY_LOCATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbfa794e4_f964_4fdb_90f6_51056bfe4b44);
pub const SENSOR_CATEGORY_MECHANICAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8d131d68_8ef7_4656_80b5_cccbd93791c5);
pub const SENSOR_CATEGORY_MOTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcd09daf1_3b2e_4c3d_b598_b5e5ff93fd46);
pub const SENSOR_CATEGORY_ORIENTATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9e6c04b6_96fe_4954_b726_68682a473f69);
pub const SENSOR_CATEGORY_OTHER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2c90e7a9_f4c9_4fa2_af37_56d471fe5a3d);
pub const SENSOR_CATEGORY_SCANNER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb000e77e_f5b5_420f_815d_0270a726f270);
pub const SENSOR_CATEGORY_UNSUPPORTED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2beae7fa_19b0_48c5_a1f6_b5480dc206b0);
pub const SENSOR_CONNECTION_TYPE_PC_ATTACHED: SensorConnectionType = 1i32;
pub const SENSOR_CONNECTION_TYPE_PC_EXTERNAL: SensorConnectionType = 2i32;
pub const SENSOR_CONNECTION_TYPE_PC_INTEGRATED: SensorConnectionType = 0i32;
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ABSOLUTE_PRESSURE_PASCAL: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x38564a7c_f2f2_49bb_9b2b_ba60f66a58df), pid: 5 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ACCELERATION_X_G: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ACCELERATION_Y_G: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 3 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ACCELERATION_Z_G: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 4 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ADDRESS1: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 23 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ADDRESS2: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 24 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ALTITUDE_ANTENNA_SEALEVEL_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 36 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ALTITUDE_ELLIPSOID_ERROR_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 29 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ALTITUDE_ELLIPSOID_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 5 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ALTITUDE_SEALEVEL_ERROR_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 30 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ALTITUDE_SEALEVEL_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 4 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ANGULAR_ACCELERATION_X_DEGREES_PER_SECOND_SQUARED: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 5 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ANGULAR_ACCELERATION_Y_DEGREES_PER_SECOND_SQUARED: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 6 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ANGULAR_ACCELERATION_Z_DEGREES_PER_SECOND_SQUARED: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 7 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ANGULAR_VELOCITY_X_DEGREES_PER_SECOND: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 10 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ANGULAR_VELOCITY_Y_DEGREES_PER_SECOND: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 11 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ANGULAR_VELOCITY_Z_DEGREES_PER_SECOND: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 12 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ATMOSPHERIC_PRESSURE_BAR: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x8b0aa2f1_2d57_42ee_8cc0_4d27622b46c4), pid: 4 };
pub const SENSOR_DATA_TYPE_BIOMETRIC_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2299288a_6d9e_4b0b_b7ec_3528f89e40af);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_BOOLEAN_SWITCH_ARRAY_STATES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x38564a7c_f2f2_49bb_9b2b_ba60f66a58df), pid: 10 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_BOOLEAN_SWITCH_STATE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x38564a7c_f2f2_49bb_9b2b_ba60f66a58df), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CAPACITANCE_FARAD: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xbbb246d1_e242_4780_a2d3_cded84f35842), pid: 4 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CITY: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 25 };
pub const SENSOR_DATA_TYPE_COMMON_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdb5e0cf2_cf1f_4c18_b46c_d86011d62150);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_COUNTRY_REGION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 28 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CURRENT_AMPS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xbbb246d1_e242_4780_a2d3_cded84f35842), pid: 3 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_BOOLEAN_ARRAY: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 6 };
pub const SENSOR_DATA_TYPE_CUSTOM_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_USAGE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 5 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE1: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 7 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE10: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 16 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE11: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 17 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE12: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 18 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE13: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 19 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE14: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 20 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE15: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 21 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE16: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 22 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE17: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 23 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE18: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 24 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE19: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 25 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE2: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 8 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE20: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 26 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE21: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 27 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE22: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 28 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE23: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 29 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE24: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 30 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE25: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 31 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE26: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 32 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE27: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 33 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE28: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 34 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE3: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 9 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE4: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 10 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE5: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 11 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE6: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 12 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE7: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 13 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE8: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 14 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_CUSTOM_VALUE9: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xb14c764f_07cf_41e8_9d82_ebe3d0776a6f), pid: 15 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_DGPS_DATA_AGE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 35 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_DIFFERENTIAL_REFERENCE_STATION_ID: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 37 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_DISTANCE_X_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 8 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_DISTANCE_Y_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 9 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_DISTANCE_Z_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 10 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ELECTRICAL_FREQUENCY_HERTZ: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xbbb246d1_e242_4780_a2d3_cded84f35842), pid: 9 };
pub const SENSOR_DATA_TYPE_ELECTRICAL_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbbb246d1_e242_4780_a2d3_cded84f35842);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ELECTRICAL_PERCENT_OF_RANGE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xbbb246d1_e242_4780_a2d3_cded84f35842), pid: 8 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ELECTRICAL_POWER_WATTS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xbbb246d1_e242_4780_a2d3_cded84f35842), pid: 7 };
pub const SENSOR_DATA_TYPE_ENVIRONMENTAL_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8b0aa2f1_2d57_42ee_8cc0_4d27622b46c4);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ERROR_RADIUS_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 22 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_FIX_QUALITY: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 10 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_FIX_TYPE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 11 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_FORCE_NEWTONS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x38564a7c_f2f2_49bb_9b2b_ba60f66a58df), pid: 4 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_GAUGE_PRESSURE_PASCAL: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x38564a7c_f2f2_49bb_9b2b_ba60f66a58df), pid: 6 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_GEOIDAL_SEPARATION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 34 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_GPS_OPERATION_MODE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 32 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_GPS_SELECTION_MODE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 31 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_GPS_STATUS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 33 };
pub const SENSOR_DATA_TYPE_GUID_MECHANICAL_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x38564a7c_f2f2_49bb_9b2b_ba60f66a58df);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_HORIZONAL_DILUTION_OF_PRECISION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 13 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_HUMAN_PRESENCE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x2299288a_6d9e_4b0b_b7ec_3528f89e40af), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_HUMAN_PROXIMITY_METERS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x2299288a_6d9e_4b0b_b7ec_3528f89e40af), pid: 3 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_INDUCTANCE_HENRY: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xbbb246d1_e242_4780_a2d3_cded84f35842), pid: 6 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_LATITUDE_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_LIGHT_CHROMACITY: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xe4c77ce2_dcb7_46e9_8439_4fec548833a6), pid: 4 };
pub const SENSOR_DATA_TYPE_LIGHT_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe4c77ce2_dcb7_46e9_8439_4fec548833a6);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_LIGHT_LEVEL_LUX: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xe4c77ce2_dcb7_46e9_8439_4fec548833a6), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_LIGHT_TEMPERATURE_KELVIN: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xe4c77ce2_dcb7_46e9_8439_4fec548833a6), pid: 3 };
pub const SENSOR_DATA_TYPE_LOCATION_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_LOCATION_SOURCE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 40 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_LONGITUDE_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 3 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_FIELD_STRENGTH_X_MILLIGAUSS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 19 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_FIELD_STRENGTH_Y_MILLIGAUSS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 20 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_FIELD_STRENGTH_Z_MILLIGAUSS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 21 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_HEADING_COMPENSATED_MAGNETIC_NORTH_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 11 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_HEADING_COMPENSATED_TRUE_NORTH_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 12 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_HEADING_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 8 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_HEADING_MAGNETIC_NORTH_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 13 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_HEADING_TRUE_NORTH_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 14 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_HEADING_X_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 5 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_HEADING_Y_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 6 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_HEADING_Z_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 7 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETIC_VARIATION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 9 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MAGNETOMETER_ACCURACY: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 22 };
pub const SENSOR_DATA_TYPE_MOTION_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MOTION_STATE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 9 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_MULTIVALUE_SWITCH_STATE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x38564a7c_f2f2_49bb_9b2b_ba60f66a58df), pid: 3 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_NMEA_SENTENCE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 38 };
pub const SENSOR_DATA_TYPE_ORIENTATION_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_POSITION_DILUTION_OF_PRECISION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 12 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_POSTALCODE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 27 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_QUADRANT_ANGLE_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 15 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_QUATERNION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 17 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_RELATIVE_HUMIDITY_PERCENT: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x8b0aa2f1_2d57_42ee_8cc0_4d27622b46c4), pid: 3 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_RESISTANCE_OHMS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xbbb246d1_e242_4780_a2d3_cded84f35842), pid: 5 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_RFID_TAG_40_BIT: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd7a59a3c_3421_44ab_8d3a_9de8ab6c4cae), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_ROTATION_MATRIX: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 16 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SATELLITES_IN_VIEW: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 17 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SATELLITES_IN_VIEW_AZIMUTH: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 20 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SATELLITES_IN_VIEW_ELEVATION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 19 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SATELLITES_IN_VIEW_ID: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 39 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SATELLITES_IN_VIEW_PRNS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 18 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SATELLITES_IN_VIEW_STN_RATIO: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 21 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SATELLITES_USED_COUNT: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 15 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SATELLITES_USED_PRNS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 16 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SATELLITES_USED_PRNS_AND_CONSTELLATIONS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 41 };
pub const SENSOR_DATA_TYPE_SCANNER_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7a59a3c_3421_44ab_8d3a_9de8ab6c4cae);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SIMPLE_DEVICE_ORIENTATION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 18 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SPEED_KNOTS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 6 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_SPEED_METERS_PER_SECOND: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x3f8a69a2_07c5_4e48_a965_cd797aab56d5), pid: 8 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_STATE_PROVINCE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 26 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_STRAIN: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x38564a7c_f2f2_49bb_9b2b_ba60f66a58df), pid: 7 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_TEMPERATURE_CELSIUS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x8b0aa2f1_2d57_42ee_8cc0_4d27622b46c4), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_TILT_X_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_TILT_Y_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 3 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_TILT_Z_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1637d8a2_4248_4275_865d_558de84aedfd), pid: 4 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_TIMESTAMP: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xdb5e0cf2_cf1f_4c18_b46c_d86011d62150), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_TOUCH_STATE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x2299288a_6d9e_4b0b_b7ec_3528f89e40af), pid: 4 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_TRUE_HEADING_DEGREES: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 7 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_VERTICAL_DILUTION_OF_PRECISION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x055c74d8_ca6f_47d6_95c6_1ed3637a0ff4), pid: 14 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_VOLTAGE_VOLTS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xbbb246d1_e242_4780_a2d3_cded84f35842), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_WEIGHT_KILOGRAMS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x38564a7c_f2f2_49bb_9b2b_ba60f66a58df), pid: 8 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_WIND_DIRECTION_DEGREES_ANTICLOCKWISE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x8b0aa2f1_2d57_42ee_8cc0_4d27622b46c4), pid: 5 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_DATA_TYPE_WIND_SPEED_METERS_PER_SECOND: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x8b0aa2f1_2d57_42ee_8cc0_4d27622b46c4), pid: 6 };
pub const SENSOR_ERROR_PARAMETER_COMMON_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x77112bcd_fce1_4f43_b8b8_a88256adb4b3);
pub const SENSOR_EVENT_ACCELEROMETER_SHAKE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x825f5a94_0f48_4396_9ca0_6ecb5c99d915);
pub const SENSOR_EVENT_DATA_UPDATED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2ed0f2a4_0087_41d3_87db_6773370b3c88);
pub const SENSOR_EVENT_PARAMETER_COMMON_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x64346e30_8728_4b34_bdf6_4f52442c5c28);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_EVENT_PARAMETER_EVENT_ID: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x64346e30_8728_4b34_bdf6_4f52442c5c28), pid: 2 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_EVENT_PARAMETER_STATE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x64346e30_8728_4b34_bdf6_4f52442c5c28), pid: 3 };
pub const SENSOR_EVENT_PROPERTY_CHANGED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2358f099_84c9_4d3d_90df_c2421e2b2045);
pub const SENSOR_EVENT_STATE_CHANGED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbfd96016_6bd7_4560_ad34_f2f6607e8f81);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_ACCURACY: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 17 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_CHANGE_SENSITIVITY: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 14 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_CLEAR_ASSISTANCE_DATA: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xe1e962f4_6e65_45f7_9c36_d487b7b1bd34), pid: 2 };
pub const SENSOR_PROPERTY_COMMON_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_CONNECTION_TYPE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 11 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_CURRENT_REPORT_INTERVAL: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 13 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_DESCRIPTION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 10 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_DEVICE_PATH: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 15 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_FRIENDLY_NAME: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 9 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_HID_USAGE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 22 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_LIGHT_RESPONSE_CURVE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 16 };
pub const SENSOR_PROPERTY_LIST_HEADER_SIZE: u32 = 8u32;
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_LOCATION_DESIRED_ACCURACY: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 19 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_MANUFACTURER: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 6 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_MIN_REPORT_INTERVAL: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 12 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_MODEL: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 7 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_PERSISTENT_UNIQUE_ID: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 5 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_RADIO_STATE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 23 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_RADIO_STATE_PREVIOUS: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 24 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_RANGE_MAXIMUM: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 21 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_RANGE_MINIMUM: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 20 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_RESOLUTION: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 18 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_SERIAL_NUMBER: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 8 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_STATE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 3 };
pub const SENSOR_PROPERTY_TEST_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe1e962f4_6e65_45f7_9c36_d487b7b1bd34);
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_TURN_ON_OFF_NMEA: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xe1e962f4_6e65_45f7_9c36_d487b7b1bd34), pid: 3 };
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub const SENSOR_PROPERTY_TYPE: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY = super::super::UI::Shell::PropertiesSystem::PROPERTYKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x7f8383ec_d3ec_495c_a8cf_b8bbe85c2920), pid: 2 };
pub const SENSOR_STATE_ACCESS_DENIED: SensorState = 4i32;
pub const SENSOR_STATE_ERROR: SensorState = 5i32;
pub const SENSOR_STATE_INITIALIZING: SensorState = 3i32;
pub const SENSOR_STATE_MAX: SensorState = 5i32;
pub const SENSOR_STATE_MIN: SensorState = 0i32;
pub const SENSOR_STATE_NOT_AVAILABLE: SensorState = 1i32;
pub const SENSOR_STATE_NO_DATA: SensorState = 2i32;
pub const SENSOR_STATE_READY: SensorState = 0i32;
pub const SENSOR_TYPE_ACCELEROMETER_1D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc04d2387_7340_4cc2_991e_3b18cb8ef2f4);
pub const SENSOR_TYPE_ACCELEROMETER_2D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb2c517a8_f6b5_4ba6_a423_5df560b4cc07);
pub const SENSOR_TYPE_ACCELEROMETER_3D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc2fb0f5f_e2d2_4c78_bcd0_352a9582819d);
pub const SENSOR_TYPE_AGGREGATED_DEVICE_ORIENTATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcdb5d8f7_3cfd_41c8_8542_cce622cf5d6e);
pub const SENSOR_TYPE_AGGREGATED_QUADRANT_ORIENTATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9f81f1af_c4ab_4307_9904_c828bfb90829);
pub const SENSOR_TYPE_AGGREGATED_SIMPLE_DEVICE_ORIENTATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x86a19291_0482_402c_bf4c_addac52b1c39);
pub const SENSOR_TYPE_AMBIENT_LIGHT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x97f115c8_599a_4153_8894_d2d12899918a);
pub const SENSOR_TYPE_BARCODE_SCANNER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x990b3d8f_85bb_45ff_914d_998c04f372df);
pub const SENSOR_TYPE_BOOLEAN_SWITCH: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9c7e371f_1041_460b_8d5c_71e4752e350c);
pub const SENSOR_TYPE_BOOLEAN_SWITCH_ARRAY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x545c8ba5_b143_4545_868f_ca7fd986b4f6);
pub const SENSOR_TYPE_CAPACITANCE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xca2ffb1c_2317_49c0_a0b4_b63ce63461a0);
pub const SENSOR_TYPE_COMPASS_1D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa415f6c5_cb50_49d0_8e62_a8270bd7a26c);
pub const SENSOR_TYPE_COMPASS_2D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x15655cc0_997a_4d30_84db_57caba3648bb);
pub const SENSOR_TYPE_COMPASS_3D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x76b5ce0d_17dd_414d_93a1_e127f40bdf6e);
pub const SENSOR_TYPE_CURRENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5adc9fce_15a0_4bbe_a1ad_2d38a9ae831c);
pub const SENSOR_TYPE_CUSTOM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe83af229_8640_4d18_a213_e22675ebb2c3);
pub const SENSOR_TYPE_DISTANCE_1D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5f14ab2f_1407_4306_a93f_b1dbabe4f9c0);
pub const SENSOR_TYPE_DISTANCE_2D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5cf9a46c_a9a2_4e55_b6a1_a04aafa95a92);
pub const SENSOR_TYPE_DISTANCE_3D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa20cae31_0e25_4772_9fe5_96608a1354b2);
pub const SENSOR_TYPE_ELECTRICAL_POWER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x212f10f5_14ab_4376_9a43_a7794098c2fe);
pub const SENSOR_TYPE_ENVIRONMENTAL_ATMOSPHERIC_PRESSURE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0e903829_ff8a_4a93_97df_3dcbde402288);
pub const SENSOR_TYPE_ENVIRONMENTAL_HUMIDITY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5c72bf67_bd7e_4257_990b_98a3ba3b400a);
pub const SENSOR_TYPE_ENVIRONMENTAL_TEMPERATURE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x04fd0ec4_d5da_45fa_95a9_5db38ee19306);
pub const SENSOR_TYPE_ENVIRONMENTAL_WIND_DIRECTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9ef57a35_9306_434d_af09_37fa5a9c00bd);
pub const SENSOR_TYPE_ENVIRONMENTAL_WIND_SPEED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdd50607b_a45f_42cd_8efd_ec61761c4226);
pub const SENSOR_TYPE_FORCE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc2ab2b02_1a1c_4778_a81b_954a1788cc75);
pub const SENSOR_TYPE_FREQUENCY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8cd2cbb6_73e6_4640_a709_72ae8fb60d7f);
pub const SENSOR_TYPE_GYROMETER_1D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfa088734_f552_4584_8324_edfaf649652c);
pub const SENSOR_TYPE_GYROMETER_2D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x31ef4f83_919b_48bf_8de0_5d7a9d240556);
pub const SENSOR_TYPE_GYROMETER_3D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x09485f5a_759e_42c2_bd4b_a349b75c8643);
pub const SENSOR_TYPE_HUMAN_PRESENCE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc138c12b_ad52_451c_9375_87f518ff10c6);
pub const SENSOR_TYPE_HUMAN_PROXIMITY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5220dae9_3179_4430_9f90_06266d2a34de);
pub const SENSOR_TYPE_INCLINOMETER_1D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96f98c5_7a75_4ba7_94e9_ac868c966dd8);
pub const SENSOR_TYPE_INCLINOMETER_2D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xab140f6d_83eb_4264_b70b_b16a5b256a01);
pub const SENSOR_TYPE_INCLINOMETER_3D: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb84919fb_ea85_4976_8444_6f6f5c6d31db);
pub const SENSOR_TYPE_INDUCTANCE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdc1d933f_c435_4c7d_a2fe_607192a524d3);
pub const SENSOR_TYPE_LOCATION_BROADCAST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd26988cf_5162_4039_bb17_4c58b698e44a);
pub const SENSOR_TYPE_LOCATION_DEAD_RECKONING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1a37d538_f28b_42da_9fce_a9d0a2a6d829);
pub const SENSOR_TYPE_LOCATION_GPS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xed4ca589_327a_4ff9_a560_91da4b48275e);
pub const SENSOR_TYPE_LOCATION_LOOKUP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3b2eae4a_72ce_436d_96d2_3c5b8570e987);
pub const SENSOR_TYPE_LOCATION_OTHER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9b2d0566_0368_4f71_b88d_533f132031de);
pub const SENSOR_TYPE_LOCATION_STATIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x095f8184_0fa9_4445_8e6e_b70f320b6b4c);
pub const SENSOR_TYPE_LOCATION_TRIANGULATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x691c341a_5406_4fe1_942f_2246cbeb39e0);
pub const SENSOR_TYPE_MOTION_DETECTOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5c7c1a12_30a5_43b9_a4b2_cf09ec5b7be8);
pub const SENSOR_TYPE_MULTIVALUE_SWITCH: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb3ee4d76_37a4_4402_b25e_99c60a775fa1);
pub const SENSOR_TYPE_POTENTIOMETER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2b3681a9_cadc_45aa_a6ff_54957c8bb440);
pub const SENSOR_TYPE_PRESSURE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x26d31f34_6352_41cf_b793_ea0713d53d77);
pub const SENSOR_TYPE_RESISTANCE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9993d2c8_c157_4a52_a7b5_195c76037231);
pub const SENSOR_TYPE_RFID_SCANNER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44328ef5_02dd_4e8d_ad5d_9249832b2eca);
pub const SENSOR_TYPE_SCALE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc06dd92c_7feb_438e_9bf6_82207fff5bb8);
pub const SENSOR_TYPE_SPEEDOMETER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6bd73c1f_0bb4_4310_81b2_dfc18a52bf94);
pub const SENSOR_TYPE_STRAIN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc6d1ec0e_6803_4361_ad3d_85bcc58c6d29);
pub const SENSOR_TYPE_TOUCH: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x17db3018_06c4_4f7d_81af_9274b7599c27);
pub const SENSOR_TYPE_UNKNOWN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x10ba83e3_ef4f_41ed_9885_a87d6435a8e1);
pub const SENSOR_TYPE_VOLTAGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc5484637_4fb7_4953_98b8_a56d8aa1fb1e);
pub const SIMPLE_DEVICE_ORIENTATION_NOT_ROTATED: SimpleDeviceOrientation = 0i32;
pub const SIMPLE_DEVICE_ORIENTATION_ROTATED_180: SimpleDeviceOrientation = 2i32;
pub const SIMPLE_DEVICE_ORIENTATION_ROTATED_270: SimpleDeviceOrientation = 3i32;
pub const SIMPLE_DEVICE_ORIENTATION_ROTATED_90: SimpleDeviceOrientation = 1i32;
pub const SIMPLE_DEVICE_ORIENTATION_ROTATED_FACE_DOWN: SimpleDeviceOrientation = 5i32;
pub const SIMPLE_DEVICE_ORIENTATION_ROTATED_FACE_UP: SimpleDeviceOrientation = 4i32;
pub const Sensor: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe97ced00_523a_4133_bf6f_d3a2dae7f6ba);
pub const SensorCollection: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x79c43adb_a429_469f_aa39_2f2b74b75937);
pub const SensorConnectionType_Attached: SENSOR_CONNECTION_TYPES = 1i32;
pub const SensorConnectionType_External: SENSOR_CONNECTION_TYPES = 2i32;
pub const SensorConnectionType_Integrated: SENSOR_CONNECTION_TYPES = 0i32;
pub const SensorDataReport: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4ea9d6ef_694b_4218_8816_ccda8da74bba);
pub const SensorManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x77a1c827_fcd2_4689_8915_9d613cc5fa3e);
pub const SensorState_Active: SENSOR_STATE = 2i32;
pub const SensorState_Error: SENSOR_STATE = 3i32;
pub const SensorState_Idle: SENSOR_STATE = 1i32;
pub const SensorState_Initializing: SENSOR_STATE = 0i32;
pub const SimpleDeviceOrientation_Facedown: SIMPLE_DEVICE_ORIENTATION = 5i32;
pub const SimpleDeviceOrientation_Faceup: SIMPLE_DEVICE_ORIENTATION = 4i32;
pub const SimpleDeviceOrientation_NotRotated: SIMPLE_DEVICE_ORIENTATION = 0i32;
pub const SimpleDeviceOrientation_Rotated180DegreesCounterclockwise: SIMPLE_DEVICE_ORIENTATION = 2i32;
pub const SimpleDeviceOrientation_Rotated270DegreesCounterclockwise: SIMPLE_DEVICE_ORIENTATION = 3i32;
pub const SimpleDeviceOrientation_Rotated90DegreesCounterclockwise: SIMPLE_DEVICE_ORIENTATION = 1i32;
pub type ACTIVITY_STATE = i32;
pub type ACTIVITY_STATE_COUNT = i32;
pub type AXIS = i32;
pub type ELEVATION_CHANGE_MODE = i32;
pub type HUMAN_PRESENCE_DETECTION_TYPE = i32;
pub type HUMAN_PRESENCE_DETECTION_TYPE_COUNT = i32;
pub type LOCATION_DESIRED_ACCURACY = i32;
pub type LOCATION_POSITION_SOURCE = i32;
pub type MAGNETOMETER_ACCURACY = i32;
pub type MagnetometerAccuracy = i32;
pub type PEDOMETER_STEP_TYPE = i32;
pub type PEDOMETER_STEP_TYPE_COUNT = i32;
pub type PROXIMITY_SENSOR_CAPABILITIES = i32;
pub type PROXIMITY_TYPE = i32;
pub type SENSOR_CONNECTION_TYPES = i32;
pub type SENSOR_STATE = i32;
pub type SIMPLE_DEVICE_ORIENTATION = i32;
pub type SensorConnectionType = i32;
pub type SensorState = i32;
pub type SimpleDeviceOrientation = i32;
#[repr(C)]
pub struct MATRIX3X3 {
    pub Anonymous: MATRIX3X3_0,
}
impl ::core::marker::Copy for MATRIX3X3 {}
impl ::core::clone::Clone for MATRIX3X3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union MATRIX3X3_0 {
    pub Anonymous1: MATRIX3X3_0_0,
    pub Anonymous2: MATRIX3X3_0_1,
    pub M: [f32; 9],
}
impl ::core::marker::Copy for MATRIX3X3_0 {}
impl ::core::clone::Clone for MATRIX3X3_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MATRIX3X3_0_0 {
    pub A11: f32,
    pub A12: f32,
    pub A13: f32,
    pub A21: f32,
    pub A22: f32,
    pub A23: f32,
    pub A31: f32,
    pub A32: f32,
    pub A33: f32,
}
impl ::core::marker::Copy for MATRIX3X3_0_0 {}
impl ::core::clone::Clone for MATRIX3X3_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MATRIX3X3_0_1 {
    pub V1: VEC3D,
    pub V2: VEC3D,
    pub V3: VEC3D,
}
impl ::core::marker::Copy for MATRIX3X3_0_1 {}
impl ::core::clone::Clone for MATRIX3X3_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QUATERNION {
    pub X: f32,
    pub Y: f32,
    pub Z: f32,
    pub W: f32,
}
impl ::core::marker::Copy for QUATERNION {}
impl ::core::clone::Clone for QUATERNION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
pub struct SENSOR_COLLECTION_LIST {
    pub AllocatedSizeInBytes: u32,
    pub Count: u32,
    pub List: [SENSOR_VALUE_PAIR; 1],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
impl ::core::marker::Copy for SENSOR_COLLECTION_LIST {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
impl ::core::clone::Clone for SENSOR_COLLECTION_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
pub struct SENSOR_PROPERTY_LIST {
    pub AllocatedSizeInBytes: u32,
    pub Count: u32,
    pub List: [super::super::UI::Shell::PropertiesSystem::PROPERTYKEY; 1],
}
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
impl ::core::marker::Copy for SENSOR_PROPERTY_LIST {}
#[cfg(feature = "Win32_UI_Shell_PropertiesSystem")]
impl ::core::clone::Clone for SENSOR_PROPERTY_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`, `\"Win32_UI_Shell_PropertiesSystem\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
pub struct SENSOR_VALUE_PAIR {
    pub Key: super::super::UI::Shell::PropertiesSystem::PROPERTYKEY,
    pub Value: super::super::System::Com::StructuredStorage::PROPVARIANT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
impl ::core::marker::Copy for SENSOR_VALUE_PAIR {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant", feature = "Win32_UI_Shell_PropertiesSystem"))]
impl ::core::clone::Clone for SENSOR_VALUE_PAIR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VEC3D {
    pub X: f32,
    pub Y: f32,
    pub Z: f32,
}
impl ::core::marker::Copy for VEC3D {}
impl ::core::clone::Clone for VEC3D {
    fn clone(&self) -> Self {
        *self
    }
}
