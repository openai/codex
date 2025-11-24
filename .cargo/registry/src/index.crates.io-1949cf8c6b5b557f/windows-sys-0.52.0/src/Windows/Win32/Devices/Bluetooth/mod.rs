#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bthprops.cpl" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothAuthenticateDevice(hwndparent : super::super::Foundation:: HWND, hradio : super::super::Foundation:: HANDLE, pbtbi : *mut BLUETOOTH_DEVICE_INFO, pszpasskey : ::windows_sys::core::PCWSTR, ulpasskeylength : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bthprops.cpl" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothAuthenticateDeviceEx(hwndparentin : super::super::Foundation:: HWND, hradioin : super::super::Foundation:: HANDLE, pbtdiinout : *mut BLUETOOTH_DEVICE_INFO, pbtoobdata : *const BLUETOOTH_OOB_DATA_INFO, authenticationrequirement : AUTHENTICATION_REQUIREMENTS) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bthprops.cpl" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothAuthenticateMultipleDevices(hwndparent : super::super::Foundation:: HWND, hradio : super::super::Foundation:: HANDLE, cdevices : u32, rgbtdi : *mut BLUETOOTH_DEVICE_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bthprops.cpl" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothDisplayDeviceProperties(hwndparent : super::super::Foundation:: HWND, pbtdi : *mut BLUETOOTH_DEVICE_INFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothEnableDiscovery(hradio : super::super::Foundation:: HANDLE, fenabled : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothEnableIncomingConnections(hradio : super::super::Foundation:: HANDLE, fenabled : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothEnumerateInstalledServices(hradio : super::super::Foundation:: HANDLE, pbtdi : *const BLUETOOTH_DEVICE_INFO, pcserviceinout : *mut u32, pguidservices : *mut ::windows_sys::core::GUID) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothFindDeviceClose(hfind : HBLUETOOTH_DEVICE_FIND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothFindFirstDevice(pbtsp : *const BLUETOOTH_DEVICE_SEARCH_PARAMS, pbtdi : *mut BLUETOOTH_DEVICE_INFO) -> HBLUETOOTH_DEVICE_FIND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothFindFirstRadio(pbtfrp : *const BLUETOOTH_FIND_RADIO_PARAMS, phradio : *mut super::super::Foundation:: HANDLE) -> HBLUETOOTH_RADIO_FIND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothFindNextDevice(hfind : HBLUETOOTH_DEVICE_FIND, pbtdi : *mut BLUETOOTH_DEVICE_INFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothFindNextRadio(hfind : HBLUETOOTH_RADIO_FIND, phradio : *mut super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothFindRadioClose(hfind : HBLUETOOTH_RADIO_FIND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTAbortReliableWrite(hdevice : super::super::Foundation:: HANDLE, reliablewritecontext : u64, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTBeginReliableWrite(hdevice : super::super::Foundation:: HANDLE, reliablewritecontext : *mut u64, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTEndReliableWrite(hdevice : super::super::Foundation:: HANDLE, reliablewritecontext : u64, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTGetCharacteristicValue(hdevice : super::super::Foundation:: HANDLE, characteristic : *const BTH_LE_GATT_CHARACTERISTIC, characteristicvaluedatasize : u32, characteristicvalue : *mut BTH_LE_GATT_CHARACTERISTIC_VALUE, characteristicvaluesizerequired : *mut u16, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTGetCharacteristics(hdevice : super::super::Foundation:: HANDLE, service : *const BTH_LE_GATT_SERVICE, characteristicsbuffercount : u16, characteristicsbuffer : *mut BTH_LE_GATT_CHARACTERISTIC, characteristicsbufferactual : *mut u16, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTGetDescriptorValue(hdevice : super::super::Foundation:: HANDLE, descriptor : *const BTH_LE_GATT_DESCRIPTOR, descriptorvaluedatasize : u32, descriptorvalue : *mut BTH_LE_GATT_DESCRIPTOR_VALUE, descriptorvaluesizerequired : *mut u16, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTGetDescriptors(hdevice : super::super::Foundation:: HANDLE, characteristic : *const BTH_LE_GATT_CHARACTERISTIC, descriptorsbuffercount : u16, descriptorsbuffer : *mut BTH_LE_GATT_DESCRIPTOR, descriptorsbufferactual : *mut u16, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTGetIncludedServices(hdevice : super::super::Foundation:: HANDLE, parentservice : *const BTH_LE_GATT_SERVICE, includedservicesbuffercount : u16, includedservicesbuffer : *mut BTH_LE_GATT_SERVICE, includedservicesbufferactual : *mut u16, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTGetServices(hdevice : super::super::Foundation:: HANDLE, servicesbuffercount : u16, servicesbuffer : *mut BTH_LE_GATT_SERVICE, servicesbufferactual : *mut u16, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTRegisterEvent(hservice : super::super::Foundation:: HANDLE, eventtype : BTH_LE_GATT_EVENT_TYPE, eventparameterin : *const ::core::ffi::c_void, callback : PFNBLUETOOTH_GATT_EVENT_CALLBACK, callbackcontext : *const ::core::ffi::c_void, peventhandle : *mut isize, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTSetCharacteristicValue(hdevice : super::super::Foundation:: HANDLE, characteristic : *const BTH_LE_GATT_CHARACTERISTIC, characteristicvalue : *const BTH_LE_GATT_CHARACTERISTIC_VALUE, reliablewritecontext : u64, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGATTSetDescriptorValue(hdevice : super::super::Foundation:: HANDLE, descriptor : *const BTH_LE_GATT_DESCRIPTOR, descriptorvalue : *const BTH_LE_GATT_DESCRIPTOR_VALUE, flags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("bluetoothapis.dll" "system" fn BluetoothGATTUnregisterEvent(eventhandle : isize, flags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGetDeviceInfo(hradio : super::super::Foundation:: HANDLE, pbtdi : *mut BLUETOOTH_DEVICE_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothGetRadioInfo(hradio : super::super::Foundation:: HANDLE, pradioinfo : *mut BLUETOOTH_RADIO_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothIsConnectable(hradio : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothIsDiscoverable(hradio : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothIsVersionAvailable(majorversion : u8, minorversion : u8) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothRegisterForAuthentication(pbtdi : *const BLUETOOTH_DEVICE_INFO, phreghandle : *mut isize, pfncallback : PFN_AUTHENTICATION_CALLBACK, pvparam : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothRegisterForAuthenticationEx(pbtdiin : *const BLUETOOTH_DEVICE_INFO, phreghandleout : *mut isize, pfncallbackin : PFN_AUTHENTICATION_CALLBACK_EX, pvparam : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("bluetoothapis.dll" "system" fn BluetoothRemoveDevice(paddress : *const BLUETOOTH_ADDRESS) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothSdpEnumAttributes(psdpstream : *const u8, cbstreamsize : u32, pfncallback : PFN_BLUETOOTH_ENUM_ATTRIBUTES_CALLBACK, pvparam : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
::windows_targets::link!("bluetoothapis.dll" "system" fn BluetoothSdpGetAttributeValue(precordstream : *const u8, cbrecordlength : u32, usattributeid : u16, pattributedata : *mut SDP_ELEMENT_DATA) -> u32);
::windows_targets::link!("bluetoothapis.dll" "system" fn BluetoothSdpGetContainerElementData(pcontainerstream : *const u8, cbcontainerlength : u32, pelement : *mut isize, pdata : *mut SDP_ELEMENT_DATA) -> u32);
::windows_targets::link!("bluetoothapis.dll" "system" fn BluetoothSdpGetElementData(psdpstream : *const u8, cbsdpstreamlength : u32, pdata : *mut SDP_ELEMENT_DATA) -> u32);
::windows_targets::link!("bluetoothapis.dll" "system" fn BluetoothSdpGetString(precordstream : *const u8, cbrecordlength : u32, pstringdata : *const SDP_STRING_TYPE_DATA, usstringoffset : u16, pszstring : ::windows_sys::core::PWSTR, pcchstringlength : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bthprops.cpl" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothSelectDevices(pbtsdp : *mut BLUETOOTH_SELECT_DEVICE_PARAMS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bthprops.cpl" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothSelectDevicesFree(pbtsdp : *mut BLUETOOTH_SELECT_DEVICE_PARAMS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothSendAuthenticationResponse(hradio : super::super::Foundation:: HANDLE, pbtdi : *const BLUETOOTH_DEVICE_INFO, pszpasskey : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothSendAuthenticationResponseEx(hradioin : super::super::Foundation:: HANDLE, pauthresponse : *const BLUETOOTH_AUTHENTICATE_RESPONSE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothSetLocalServiceInfo(hradioin : super::super::Foundation:: HANDLE, pclassguid : *const ::windows_sys::core::GUID, ulinstance : u32, pserviceinfoin : *const BLUETOOTH_LOCAL_SERVICE_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothSetServiceState(hradio : super::super::Foundation:: HANDLE, pbtdi : *const BLUETOOTH_DEVICE_INFO, pguidservice : *const ::windows_sys::core::GUID, dwserviceflags : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothUnregisterAuthentication(hreghandle : isize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("bluetoothapis.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BluetoothUpdateDeviceRecord(pbtdi : *const BLUETOOTH_DEVICE_INFO) -> u32);
pub const A2DP_SINK_SUPPORTED_FEATURES_AMPLIFIER: u32 = 8u32;
pub const A2DP_SINK_SUPPORTED_FEATURES_HEADPHONE: u32 = 1u32;
pub const A2DP_SINK_SUPPORTED_FEATURES_RECORDER: u32 = 4u32;
pub const A2DP_SINK_SUPPORTED_FEATURES_SPEAKER: u32 = 2u32;
pub const A2DP_SOURCE_SUPPORTED_FEATURES_MICROPHONE: u32 = 2u32;
pub const A2DP_SOURCE_SUPPORTED_FEATURES_MIXER: u32 = 8u32;
pub const A2DP_SOURCE_SUPPORTED_FEATURES_PLAYER: u32 = 1u32;
pub const A2DP_SOURCE_SUPPORTED_FEATURES_TUNER: u32 = 4u32;
pub const AF_BTH: u16 = 32u16;
pub const ATT_PROTOCOL_UUID16: u32 = 7u32;
pub const AVCTP_PROTOCOL_UUID16: u32 = 23u32;
pub const AVDTP_PROTOCOL_UUID16: u32 = 25u32;
pub const AVRCP_SUPPORTED_FEATURES_CATEGORY_1: u32 = 1u32;
pub const AVRCP_SUPPORTED_FEATURES_CATEGORY_2: u32 = 2u32;
pub const AVRCP_SUPPORTED_FEATURES_CATEGORY_3: u32 = 4u32;
pub const AVRCP_SUPPORTED_FEATURES_CATEGORY_4: u32 = 8u32;
pub const AVRCP_SUPPORTED_FEATURES_CT_BROWSING: u32 = 64u32;
pub const AVRCP_SUPPORTED_FEATURES_CT_COVER_ART_IMAGE: u32 = 256u32;
pub const AVRCP_SUPPORTED_FEATURES_CT_COVER_ART_IMAGE_PROPERTIES: u32 = 128u32;
pub const AVRCP_SUPPORTED_FEATURES_CT_COVER_ART_LINKED_THUMBNAIL: u32 = 512u32;
pub const AVRCP_SUPPORTED_FEATURES_TG_BROWSING: u32 = 64u32;
pub const AVRCP_SUPPORTED_FEATURES_TG_COVER_ART: u32 = 256u32;
pub const AVRCP_SUPPORTED_FEATURES_TG_GROUP_NAVIGATION: u32 = 32u32;
pub const AVRCP_SUPPORTED_FEATURES_TG_MULTIPLE_PLAYER_APPLICATIONS: u32 = 128u32;
pub const AVRCP_SUPPORTED_FEATURES_TG_PLAYER_APPLICATION_SETTINGS: u32 = 16u32;
pub const AVRemoteControlControllerServiceClass_UUID16: u32 = 4367u32;
pub const AVRemoteControlServiceClassID_UUID16: u32 = 4366u32;
pub const AVRemoteControlTargetServiceClassID_UUID16: u32 = 4364u32;
pub const AdvancedAudioDistributionProfileID_UUID16: u32 = 4365u32;
pub const AdvancedAudioDistributionServiceClassID_UUID16: u32 = 4365u32;
pub const AudioSinkServiceClassID_UUID16: u32 = 4363u32;
pub const AudioSinkSourceServiceClassID_UUID16: u32 = 4363u32;
pub const AudioSourceServiceClassID_UUID16: u32 = 4362u32;
pub const AudioVideoServiceClassID_UUID16: u32 = 4396u32;
pub const AudioVideoServiceClass_UUID16: u32 = 4396u32;
pub const BDIF_ADDRESS: u32 = 1u32;
pub const BDIF_BR: u32 = 16384u32;
pub const BDIF_BR_SECURE_CONNECTION_PAIRED: u32 = 134217728u32;
pub const BDIF_COD: u32 = 2u32;
pub const BDIF_CONNECTED: u32 = 32u32;
pub const BDIF_CONNECTION_INBOUND: u32 = 67108864u32;
pub const BDIF_DEBUGKEY: u32 = 536870912u32;
pub const BDIF_EIR: u32 = 8192u32;
pub const BDIF_LE: u32 = 32768u32;
pub const BDIF_LE_CONNECTABLE: u32 = 33554432u32;
pub const BDIF_LE_CONNECTED: u32 = 16777216u32;
pub const BDIF_LE_DEBUGKEY: u32 = 1073741824u32;
pub const BDIF_LE_DISCOVERABLE: u32 = 2097152u32;
pub const BDIF_LE_MITM_PROTECTED: u32 = 262144u32;
pub const BDIF_LE_NAME: u32 = 4194304u32;
pub const BDIF_LE_PAIRED: u32 = 65536u32;
pub const BDIF_LE_PERSONAL: u32 = 131072u32;
pub const BDIF_LE_PRIVACY_ENABLED: u32 = 524288u32;
pub const BDIF_LE_RANDOM_ADDRESS_TYPE: u32 = 1048576u32;
pub const BDIF_LE_SECURE_CONNECTION_PAIRED: u32 = 268435456u32;
pub const BDIF_LE_VISIBLE: u32 = 8388608u32;
pub const BDIF_NAME: u32 = 4u32;
pub const BDIF_PAIRED: u32 = 8u32;
pub const BDIF_PERSONAL: u32 = 16u32;
pub const BDIF_RSSI: u32 = 4096u32;
pub const BDIF_SHORT_NAME: u32 = 64u32;
pub const BDIF_SSP_MITM_PROTECTED: u32 = 1024u32;
pub const BDIF_SSP_PAIRED: u32 = 512u32;
pub const BDIF_SSP_SUPPORTED: u32 = 256u32;
pub const BDIF_TX_POWER: u32 = 2147483648u32;
pub const BDIF_VISIBLE: u32 = 128u32;
pub const BLUETOOTH_AUTHENTICATION_METHOD_LEGACY: BLUETOOTH_AUTHENTICATION_METHOD = 1i32;
pub const BLUETOOTH_AUTHENTICATION_METHOD_NUMERIC_COMPARISON: BLUETOOTH_AUTHENTICATION_METHOD = 3i32;
pub const BLUETOOTH_AUTHENTICATION_METHOD_OOB: BLUETOOTH_AUTHENTICATION_METHOD = 2i32;
pub const BLUETOOTH_AUTHENTICATION_METHOD_PASSKEY: BLUETOOTH_AUTHENTICATION_METHOD = 5i32;
pub const BLUETOOTH_AUTHENTICATION_METHOD_PASSKEY_NOTIFICATION: BLUETOOTH_AUTHENTICATION_METHOD = 4i32;
pub const BLUETOOTH_DEVICE_NAME_SIZE: u32 = 256u32;
pub const BLUETOOTH_GATT_FLAG_CONNECTION_AUTHENTICATED: u32 = 2u32;
pub const BLUETOOTH_GATT_FLAG_CONNECTION_ENCRYPTED: u32 = 1u32;
pub const BLUETOOTH_GATT_FLAG_FORCE_READ_FROM_CACHE: u32 = 8u32;
pub const BLUETOOTH_GATT_FLAG_FORCE_READ_FROM_DEVICE: u32 = 4u32;
pub const BLUETOOTH_GATT_FLAG_NONE: u32 = 0u32;
pub const BLUETOOTH_GATT_FLAG_RETURN_ALL: u32 = 64u32;
pub const BLUETOOTH_GATT_FLAG_SIGNED_WRITE: u32 = 16u32;
pub const BLUETOOTH_GATT_FLAG_WRITE_WITHOUT_RESPONSE: u32 = 32u32;
pub const BLUETOOTH_IO_CAPABILITY_DISPLAYONLY: BLUETOOTH_IO_CAPABILITY = 0i32;
pub const BLUETOOTH_IO_CAPABILITY_DISPLAYYESNO: BLUETOOTH_IO_CAPABILITY = 1i32;
pub const BLUETOOTH_IO_CAPABILITY_KEYBOARDONLY: BLUETOOTH_IO_CAPABILITY = 2i32;
pub const BLUETOOTH_IO_CAPABILITY_NOINPUTNOOUTPUT: BLUETOOTH_IO_CAPABILITY = 3i32;
pub const BLUETOOTH_IO_CAPABILITY_UNDEFINED: BLUETOOTH_IO_CAPABILITY = 255i32;
pub const BLUETOOTH_MAX_NAME_SIZE: u32 = 248u32;
pub const BLUETOOTH_MAX_PASSKEY_BUFFER_SIZE: u32 = 17u32;
pub const BLUETOOTH_MAX_PASSKEY_SIZE: u32 = 16u32;
pub const BLUETOOTH_MAX_SERVICE_NAME_SIZE: u32 = 256u32;
pub const BLUETOOTH_MITM_ProtectionNotDefined: BLUETOOTH_AUTHENTICATION_REQUIREMENTS = 255i32;
pub const BLUETOOTH_MITM_ProtectionNotRequired: BLUETOOTH_AUTHENTICATION_REQUIREMENTS = 0i32;
pub const BLUETOOTH_MITM_ProtectionNotRequiredBonding: BLUETOOTH_AUTHENTICATION_REQUIREMENTS = 2i32;
pub const BLUETOOTH_MITM_ProtectionNotRequiredGeneralBonding: BLUETOOTH_AUTHENTICATION_REQUIREMENTS = 4i32;
pub const BLUETOOTH_MITM_ProtectionRequired: BLUETOOTH_AUTHENTICATION_REQUIREMENTS = 1i32;
pub const BLUETOOTH_MITM_ProtectionRequiredBonding: BLUETOOTH_AUTHENTICATION_REQUIREMENTS = 3i32;
pub const BLUETOOTH_MITM_ProtectionRequiredGeneralBonding: BLUETOOTH_AUTHENTICATION_REQUIREMENTS = 5i32;
pub const BLUETOOTH_SERVICE_DISABLE: u32 = 0u32;
pub const BLUETOOTH_SERVICE_ENABLE: u32 = 1u32;
pub const BNEP_PROTOCOL_UUID16: u32 = 15u32;
pub const BTHLEENUM_ATT_MTU_DEFAULT: u32 = 23u32;
pub const BTHLEENUM_ATT_MTU_INITIAL_NEGOTIATION: u32 = 525u32;
pub const BTHLEENUM_ATT_MTU_MAX: u32 = 65535u32;
pub const BTHLEENUM_ATT_MTU_MIN: u32 = 23u32;
pub const BTHNS_RESULT_DEVICE_AUTHENTICATED: u32 = 262144u32;
pub const BTHNS_RESULT_DEVICE_CONNECTED: u32 = 65536u32;
pub const BTHNS_RESULT_DEVICE_REMEMBERED: u32 = 131072u32;
pub const BTHPROTO_L2CAP: u32 = 256u32;
pub const BTHPROTO_RFCOMM: u32 = 3u32;
pub const BTH_ADDR_GIAC: u32 = 10390323u32;
pub const BTH_ADDR_IAC_FIRST: u32 = 10390272u32;
pub const BTH_ADDR_IAC_LAST: u32 = 10390335u32;
pub const BTH_ADDR_LIAC: u32 = 10390272u32;
pub const BTH_ADDR_STRING_SIZE: u32 = 12u32;
pub const BTH_EIR_128_UUIDS_COMPLETE_ID: u32 = 7u32;
pub const BTH_EIR_128_UUIDS_PARTIAL_ID: u32 = 6u32;
pub const BTH_EIR_16_UUIDS_COMPLETE_ID: u32 = 3u32;
pub const BTH_EIR_16_UUIDS_PARTIAL_ID: u32 = 2u32;
pub const BTH_EIR_32_UUIDS_COMPLETE_ID: u32 = 5u32;
pub const BTH_EIR_32_UUIDS_PARTIAL_ID: u32 = 4u32;
pub const BTH_EIR_FLAGS_ID: u32 = 1u32;
pub const BTH_EIR_LOCAL_NAME_COMPLETE_ID: u32 = 9u32;
pub const BTH_EIR_LOCAL_NAME_PARTIAL_ID: u32 = 8u32;
pub const BTH_EIR_MANUFACTURER_ID: u32 = 255u32;
pub const BTH_EIR_OOB_BD_ADDR_ID: u32 = 12u32;
pub const BTH_EIR_OOB_COD_ID: u32 = 13u32;
pub const BTH_EIR_OOB_OPT_DATA_LEN_ID: u32 = 11u32;
pub const BTH_EIR_OOB_SP_HASH_ID: u32 = 14u32;
pub const BTH_EIR_OOB_SP_RANDOMIZER_ID: u32 = 15u32;
pub const BTH_EIR_SIZE: u32 = 240u32;
pub const BTH_EIR_TX_POWER_LEVEL_ID: u32 = 10u32;
pub const BTH_ERROR_ACL_CONNECTION_ALREADY_EXISTS: u32 = 11u32;
pub const BTH_ERROR_AUTHENTICATION_FAILURE: u32 = 5u32;
pub const BTH_ERROR_CHANNEL_CLASSIFICATION_NOT_SUPPORTED: u32 = 46u32;
pub const BTH_ERROR_COARSE_CLOCK_ADJUSTMENT_REJECTED: u32 = 64u32;
pub const BTH_ERROR_COMMAND_DISALLOWED: u32 = 12u32;
pub const BTH_ERROR_CONNECTION_FAILED_TO_BE_ESTABLISHED: u32 = 62u32;
pub const BTH_ERROR_CONNECTION_REJECTED_DUE_TO_NO_SUITABLE_CHANNEL_FOUND: u32 = 57u32;
pub const BTH_ERROR_CONNECTION_TERMINATED_DUE_TO_MIC_FAILURE: u32 = 61u32;
pub const BTH_ERROR_CONNECTION_TIMEOUT: u32 = 8u32;
pub const BTH_ERROR_CONTROLLER_BUSY: u32 = 58u32;
pub const BTH_ERROR_DIFFERENT_TRANSACTION_COLLISION: u32 = 42u32;
pub const BTH_ERROR_DIRECTED_ADVERTISING_TIMEOUT: u32 = 60u32;
pub const BTH_ERROR_ENCRYPTION_MODE_NOT_ACCEPTABLE: u32 = 37u32;
pub const BTH_ERROR_EXTENDED_INQUIRY_RESPONSE_TOO_LARGE: u32 = 54u32;
pub const BTH_ERROR_HARDWARE_FAILURE: u32 = 3u32;
pub const BTH_ERROR_HOST_BUSY_PAIRING: u32 = 56u32;
pub const BTH_ERROR_HOST_REJECTED_LIMITED_RESOURCES: u32 = 13u32;
pub const BTH_ERROR_HOST_REJECTED_PERSONAL_DEVICE: u32 = 15u32;
pub const BTH_ERROR_HOST_REJECTED_SECURITY_REASONS: u32 = 14u32;
pub const BTH_ERROR_HOST_TIMEOUT: u32 = 16u32;
pub const BTH_ERROR_INSTANT_PASSED: u32 = 40u32;
pub const BTH_ERROR_INSUFFICIENT_SECURITY: u32 = 47u32;
pub const BTH_ERROR_INVALID_HCI_PARAMETER: u32 = 18u32;
pub const BTH_ERROR_INVALID_LMP_PARAMETERS: u32 = 30u32;
pub const BTH_ERROR_KEY_MISSING: u32 = 6u32;
pub const BTH_ERROR_LIMIT_REACHED: u32 = 67u32;
pub const BTH_ERROR_LMP_PDU_NOT_ALLOWED: u32 = 36u32;
pub const BTH_ERROR_LMP_RESPONSE_TIMEOUT: u32 = 34u32;
pub const BTH_ERROR_LMP_TRANSACTION_COLLISION: u32 = 35u32;
pub const BTH_ERROR_LOCAL_HOST_TERMINATED_CONNECTION: u32 = 22u32;
pub const BTH_ERROR_MAC_CONNECTION_FAILED: u32 = 63u32;
pub const BTH_ERROR_MAX_NUMBER_OF_CONNECTIONS: u32 = 9u32;
pub const BTH_ERROR_MAX_NUMBER_OF_SCO_CONNECTIONS: u32 = 10u32;
pub const BTH_ERROR_MEMORY_FULL: u32 = 7u32;
pub const BTH_ERROR_NO_CONNECTION: u32 = 2u32;
pub const BTH_ERROR_OPERATION_CANCELLED_BY_HOST: u32 = 68u32;
pub const BTH_ERROR_PACKET_TOO_LONG: u32 = 69u32;
pub const BTH_ERROR_PAGE_TIMEOUT: u32 = 4u32;
pub const BTH_ERROR_PAIRING_NOT_ALLOWED: u32 = 24u32;
pub const BTH_ERROR_PAIRING_WITH_UNIT_KEY_NOT_SUPPORTED: u32 = 41u32;
pub const BTH_ERROR_PARAMETER_OUT_OF_MANDATORY_RANGE: u32 = 48u32;
pub const BTH_ERROR_QOS_IS_NOT_SUPPORTED: u32 = 39u32;
pub const BTH_ERROR_QOS_REJECTED: u32 = 45u32;
pub const BTH_ERROR_QOS_UNACCEPTABLE_PARAMETER: u32 = 44u32;
pub const BTH_ERROR_REMOTE_LOW_RESOURCES: u32 = 20u32;
pub const BTH_ERROR_REMOTE_POWERING_OFF: u32 = 21u32;
pub const BTH_ERROR_REMOTE_USER_ENDED_CONNECTION: u32 = 19u32;
pub const BTH_ERROR_REPEATED_ATTEMPTS: u32 = 23u32;
pub const BTH_ERROR_RESERVED_SLOT_VIOLATION: u32 = 52u32;
pub const BTH_ERROR_ROLE_CHANGE_NOT_ALLOWED: u32 = 33u32;
pub const BTH_ERROR_ROLE_SWITCH_FAILED: u32 = 53u32;
pub const BTH_ERROR_ROLE_SWITCH_PENDING: u32 = 50u32;
pub const BTH_ERROR_SCO_AIRMODE_REJECTED: u32 = 29u32;
pub const BTH_ERROR_SCO_INTERVAL_REJECTED: u32 = 28u32;
pub const BTH_ERROR_SCO_OFFSET_REJECTED: u32 = 27u32;
pub const BTH_ERROR_SECURE_SIMPLE_PAIRING_NOT_SUPPORTED_BY_HOST: u32 = 55u32;
pub const BTH_ERROR_SUCCESS: u32 = 0u32;
pub const BTH_ERROR_TYPE_0_SUBMAP_NOT_DEFINED: u32 = 65u32;
pub const BTH_ERROR_UKNOWN_LMP_PDU: u32 = 25u32;
pub const BTH_ERROR_UNACCEPTABLE_CONNECTION_INTERVAL: u32 = 59u32;
pub const BTH_ERROR_UNIT_KEY_NOT_USED: u32 = 38u32;
pub const BTH_ERROR_UNKNOWN_ADVERTISING_IDENTIFIER: u32 = 66u32;
pub const BTH_ERROR_UNKNOWN_HCI_COMMAND: u32 = 1u32;
pub const BTH_ERROR_UNSPECIFIED: u32 = 255u32;
pub const BTH_ERROR_UNSPECIFIED_ERROR: u32 = 31u32;
pub const BTH_ERROR_UNSUPPORTED_FEATURE_OR_PARAMETER: u32 = 17u32;
pub const BTH_ERROR_UNSUPPORTED_LMP_PARM_VALUE: u32 = 32u32;
pub const BTH_ERROR_UNSUPPORTED_REMOTE_FEATURE: u32 = 26u32;
pub const BTH_HOST_FEATURE_ENHANCED_RETRANSMISSION_MODE: u64 = 1u64;
pub const BTH_HOST_FEATURE_LOW_ENERGY: u64 = 4u64;
pub const BTH_HOST_FEATURE_SCO_HCI: u64 = 8u64;
pub const BTH_HOST_FEATURE_SCO_HCIBYPASS: u64 = 16u64;
pub const BTH_HOST_FEATURE_STREAMING_MODE: u64 = 2u64;
pub const BTH_IOCTL_BASE: u32 = 0u32;
pub const BTH_LE_ATT_BLUETOOTH_BASE_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000000_0000_1000_8000_00805f9b34fb);
pub const BTH_LE_ATT_CID: u32 = 4u32;
pub const BTH_LE_ATT_MAX_VALUE_SIZE: u32 = 512u32;
pub const BTH_LE_ATT_TRANSACTION_TIMEOUT: u32 = 30u32;
pub const BTH_LE_ERROR_ATTRIBUTE_NOT_FOUND: u32 = 10u32;
pub const BTH_LE_ERROR_ATTRIBUTE_NOT_LONG: u32 = 11u32;
pub const BTH_LE_ERROR_INSUFFICIENT_AUTHENTICATION: u32 = 5u32;
pub const BTH_LE_ERROR_INSUFFICIENT_AUTHORIZATION: u32 = 8u32;
pub const BTH_LE_ERROR_INSUFFICIENT_ENCRYPTION: u32 = 15u32;
pub const BTH_LE_ERROR_INSUFFICIENT_ENCRYPTION_KEY_SIZE: u32 = 12u32;
pub const BTH_LE_ERROR_INSUFFICIENT_RESOURCES: u32 = 17u32;
pub const BTH_LE_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH: u32 = 13u32;
pub const BTH_LE_ERROR_INVALID_HANDLE: u32 = 1u32;
pub const BTH_LE_ERROR_INVALID_OFFSET: u32 = 7u32;
pub const BTH_LE_ERROR_INVALID_PDU: u32 = 4u32;
pub const BTH_LE_ERROR_PREPARE_QUEUE_FULL: u32 = 9u32;
pub const BTH_LE_ERROR_READ_NOT_PERMITTED: u32 = 2u32;
pub const BTH_LE_ERROR_REQUEST_NOT_SUPPORTED: u32 = 6u32;
pub const BTH_LE_ERROR_UNKNOWN: u32 = 4096u32;
pub const BTH_LE_ERROR_UNLIKELY: u32 = 14u32;
pub const BTH_LE_ERROR_UNSUPPORTED_GROUP_TYPE: u32 = 16u32;
pub const BTH_LE_ERROR_WRITE_NOT_PERMITTED: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SINK_SUBCATEGORY_BOOKSHELF_SPEAKER: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SINK_SUBCATEGORY_SOUNDBAR: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SINK_SUBCATEGORY_SPEAKERPHONE: u32 = 5u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SINK_SUBCATEGORY_STANDALONE_SPEAKER: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SINK_SUBCATEGORY_STANDMOUNTED_SPEAKER: u32 = 4u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SOURCE_SUBCATEGORY_ALARM: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SOURCE_SUBCATEGORY_AUDITORIUM: u32 = 9u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SOURCE_SUBCATEGORY_BELL: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SOURCE_SUBCATEGORY_BROADCASTING_DEVICE: u32 = 5u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SOURCE_SUBCATEGORY_BROADCASTING_ROOM: u32 = 8u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SOURCE_SUBCATEGORY_HORN: u32 = 4u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SOURCE_SUBCATEGORY_KIOSK: u32 = 7u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SOURCE_SUBCATEGORY_MICROPHONE: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_AUDIO_SOURCE_SUBCATEGORY_SERVICE_DESK: u32 = 6u32;
pub const BTH_LE_GAP_APPEARANCE_BLOOD_PRESSURE_SUBCATEGORY_ARM: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_BLOOD_PRESSURE_SUBCATEGORY_WRIST: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_ACCESS_CONTROL: u32 = 28u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_AIRCRAFT: u32 = 38u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_AIR_CONDITIONING: u32 = 25u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_AUDIO_SINK: u32 = 33u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_AUDIO_SOURCE: u32 = 34u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_AV_EQUIPMENT: u32 = 39u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_BARCODE_SCANNER: u32 = 11u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_BLOOD_PRESSURE: u32 = 14u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_CLOCK: u32 = 4u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_COMPUTER: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_CONTINUOUS_GLUCOSE_MONITOR: u32 = 52u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_CONTROL_DEVICE: u32 = 19u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_CYCLING: u32 = 18u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_DISPLAY: u32 = 5u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_DISPLAY_EQUIPMENT: u32 = 40u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_DOMESTIC_APPLIANCE: u32 = 36u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_EYE_GLASSES: u32 = 7u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_FAN: u32 = 23u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_GAMING: u32 = 42u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_GLUCOSE_METER: u32 = 16u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_HEARING_AID: u32 = 41u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_HEART_RATE: u32 = 13u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_HEATING: u32 = 27u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_HID: u32 = 15u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_HUMIDIFIER: u32 = 26u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_HVAC: u32 = 24u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_INSULIN_PUMP: u32 = 53u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_KEYRING: u32 = 9u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_LIGHT_FIXTURES: u32 = 22u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_LIGHT_SOURCE: u32 = 31u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_MASK: u32 = 1023u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_MEDIA_PLAYER: u32 = 10u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_MEDICATION_DELIVERY: u32 = 54u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_MOTORIZED_DEVICE: u32 = 29u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_MOTORIZED_VEHICLE: u32 = 35u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_NETWORK_DEVICE: u32 = 20u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_OFFSET: u32 = 6u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_OUTDOOR_SPORTS_ACTIVITY: u32 = 81u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_PERSONAL_MOBILITY_DEVICE: u32 = 51u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_PHONE: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_PLUSE_OXIMETER: u32 = 49u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_POWER_DEVICE: u32 = 30u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_REMOTE_CONTROL: u32 = 6u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_RUNNING_WALKING_SENSOR: u32 = 17u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_SENSOR: u32 = 21u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_SIGNAGE: u32 = 43u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_TAG: u32 = 8u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_THERMOMETER: u32 = 12u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_UNCATEGORIZED: u32 = 0u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_WATCH: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_WEARABLE_AUDIO_DEVICE: u32 = 37u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_WEIGHT_SCALE: u32 = 50u32;
pub const BTH_LE_GAP_APPEARANCE_CATEGORY_WINDOW_COVERING: u32 = 32u32;
pub const BTH_LE_GAP_APPEARANCE_CYCLING_SUBCATEGORY_CADENCE_SENSOR: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_CYCLING_SUBCATEGORY_CYCLING_COMPUTER: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_CYCLING_SUBCATEGORY_POWER_SENSOR: u32 = 4u32;
pub const BTH_LE_GAP_APPEARANCE_CYCLING_SUBCATEGORY_SPEED_AND_CADENCE_SENSOR: u32 = 5u32;
pub const BTH_LE_GAP_APPEARANCE_CYCLING_SUBCATEGORY_SPEED_SENSOR: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_HEARING_AID_SUBCATEGORY_BEHIND_EAR_HEARING_AID: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_HEARING_AID_SUBCATEGORY_COCHLEAR_IMPLANT: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_HEARING_AID_SUBCATEGORY_IN_EAR_HEARING_AID: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_HEART_RATE_SUBCATEGORY_HEART_RATE_BELT: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_HID_SUBCATEGORY_BARCODE_SCANNER: u32 = 8u32;
pub const BTH_LE_GAP_APPEARANCE_HID_SUBCATEGORY_CARD_READER: u32 = 6u32;
pub const BTH_LE_GAP_APPEARANCE_HID_SUBCATEGORY_DIGITAL_PEN: u32 = 7u32;
pub const BTH_LE_GAP_APPEARANCE_HID_SUBCATEGORY_DIGITIZER_TABLET: u32 = 5u32;
pub const BTH_LE_GAP_APPEARANCE_HID_SUBCATEGORY_GAMEPAD: u32 = 4u32;
pub const BTH_LE_GAP_APPEARANCE_HID_SUBCATEGORY_JOYSTICK: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_HID_SUBCATEGORY_KEYBOARD: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_HID_SUBCATEGORY_MOUSE: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_OUTDOOR_SPORTS_ACTIVITY_SUBCATEGORY_LOCATION_DISPLAY_DEVICE: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_OUTDOOR_SPORTS_ACTIVITY_SUBCATEGORY_LOCATION_NAVIGATION_DISPLAY_DEVICE: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_OUTDOOR_SPORTS_ACTIVITY_SUBCATEGORY_LOCATION_NAVIGATION_POD: u32 = 4u32;
pub const BTH_LE_GAP_APPEARANCE_OUTDOOR_SPORTS_ACTIVITY_SUBCATEGORY_LOCATION_POD: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_PULSE_OXIMETER_SUBCATEGORY_FINGERTIP: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_PULSE_OXIMETER_SUBCATEGORY_WRIST_WORN: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_RUNNING_WALKING_SENSOR_SUBCATEGORY_IN_SHOE: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_RUNNING_WALKING_SENSOR_SUBCATEGORY_ON_HIP: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_RUNNING_WALKING_SENSOR_SUBCATEGORY_ON_SHOE: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_SUBCATEGORY_GENERIC: u32 = 0u32;
pub const BTH_LE_GAP_APPEARANCE_SUB_CATEGORY_MASK: u32 = 63u32;
pub const BTH_LE_GAP_APPEARANCE_THERMOMETER_SUBCATEGORY_EAR: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_WATCH_SUBCATEGORY_SPORTS_WATCH: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_WEARABLE_AUDIO_DEVICE_SUBCATEGORY_EARBUD: u32 = 1u32;
pub const BTH_LE_GAP_APPEARANCE_WEARABLE_AUDIO_DEVICE_SUBCATEGORY_HEADPHONES: u32 = 3u32;
pub const BTH_LE_GAP_APPEARANCE_WEARABLE_AUDIO_DEVICE_SUBCATEGORY_HEADSET: u32 = 2u32;
pub const BTH_LE_GAP_APPEARANCE_WEARABLE_AUDIO_DEVICE_SUBCATEGORY_NECKBAND: u32 = 4u32;
pub const BTH_LE_GATT_ATTRIBUTE_TYPE_CHARACTERISTIC: u32 = 10243u32;
pub const BTH_LE_GATT_ATTRIBUTE_TYPE_INCLUDE: u32 = 10242u32;
pub const BTH_LE_GATT_ATTRIBUTE_TYPE_PRIMARY_SERVICE: u32 = 10240u32;
pub const BTH_LE_GATT_ATTRIBUTE_TYPE_SECONDARY_SERVICE: u32 = 10241u32;
pub const BTH_LE_GATT_CHARACTERISTIC_DESCRIPTOR_AGGREGATE_FORMAT: u32 = 10501u32;
pub const BTH_LE_GATT_CHARACTERISTIC_DESCRIPTOR_CLIENT_CONFIGURATION: u32 = 10498u32;
pub const BTH_LE_GATT_CHARACTERISTIC_DESCRIPTOR_EXTENDED_PROPERTIES: u32 = 10496u32;
pub const BTH_LE_GATT_CHARACTERISTIC_DESCRIPTOR_FORMAT: u32 = 10500u32;
pub const BTH_LE_GATT_CHARACTERISTIC_DESCRIPTOR_SERVER_CONFIGURATION: u32 = 10499u32;
pub const BTH_LE_GATT_CHARACTERISTIC_DESCRIPTOR_USER_DESCRIPTION: u32 = 10497u32;
pub const BTH_LE_GATT_CHARACTERISTIC_TYPE_APPEARANCE: u32 = 10753u32;
pub const BTH_LE_GATT_CHARACTERISTIC_TYPE_DEVICE_NAME: u32 = 10752u32;
pub const BTH_LE_GATT_CHARACTERISTIC_TYPE_PERIPHERAL_PREFERED_CONNECTION_PARAMETER: u32 = 10756u32;
pub const BTH_LE_GATT_CHARACTERISTIC_TYPE_PERIPHERAL_PRIVACY_FLAG: u32 = 10754u32;
pub const BTH_LE_GATT_CHARACTERISTIC_TYPE_RECONNECTION_ADDRESS: u32 = 10755u32;
pub const BTH_LE_GATT_CHARACTERISTIC_TYPE_SERVICE_CHANGED: u32 = 10757u32;
pub const BTH_LE_GATT_DEFAULT_MAX_INCLUDED_SERVICES_DEPTH: u32 = 3u32;
pub const BTH_LE_SERVICE_GAP: u32 = 6144u32;
pub const BTH_LE_SERVICE_GATT: u32 = 6145u32;
pub const BTH_LINK_KEY_LENGTH: u32 = 16u32;
pub const BTH_MAJORVERSION: u32 = 2u32;
pub const BTH_MAX_NAME_SIZE: u32 = 248u32;
pub const BTH_MAX_PIN_SIZE: u32 = 16u32;
pub const BTH_MAX_SERVICE_NAME_SIZE: u32 = 256u32;
pub const BTH_MFG_3COM: u32 = 5u32;
pub const BTH_MFG_ALCATEL: u32 = 36u32;
pub const BTH_MFG_APPLE: u32 = 76u32;
pub const BTH_MFG_ARUBA_NETWORKS: u32 = 283u32;
pub const BTH_MFG_ATMEL: u32 = 19u32;
pub const BTH_MFG_AVM_BERLIN: u32 = 31u32;
pub const BTH_MFG_BANDSPEED: u32 = 32u32;
pub const BTH_MFG_BROADCOM: u32 = 15u32;
pub const BTH_MFG_CONEXANT: u32 = 28u32;
pub const BTH_MFG_CSR: u32 = 10u32;
pub const BTH_MFG_C_TECHNOLOGIES: u32 = 38u32;
pub const BTH_MFG_DIGIANSWER: u32 = 12u32;
pub const BTH_MFG_ERICSSON: u32 = 0u32;
pub const BTH_MFG_HITACHI: u32 = 41u32;
pub const BTH_MFG_IBM: u32 = 3u32;
pub const BTH_MFG_INFINEON: u32 = 9u32;
pub const BTH_MFG_INTEL: u32 = 2u32;
pub const BTH_MFG_INTERNAL_USE: u32 = 65535u32;
pub const BTH_MFG_INVENTEL: u32 = 30u32;
pub const BTH_MFG_KC_TECHNOLOGY: u32 = 22u32;
pub const BTH_MFG_LUCENT: u32 = 7u32;
pub const BTH_MFG_MACRONIX_INTERNATIONAL: u32 = 44u32;
pub const BTH_MFG_MANSELLA: u32 = 33u32;
pub const BTH_MFG_MARVELL: u32 = 72u32;
pub const BTH_MFG_MICROSOFT: u32 = 6u32;
pub const BTH_MFG_MITEL: u32 = 16u32;
pub const BTH_MFG_MITSIBUSHI: u32 = 20u32;
pub const BTH_MFG_MOTOROLA: u32 = 8u32;
pub const BTH_MFG_NEC: u32 = 34u32;
pub const BTH_MFG_NEWLOGIC: u32 = 23u32;
pub const BTH_MFG_NOKIA: u32 = 1u32;
pub const BTH_MFG_NORDIC_SEMICONDUCTORS_ASA: u32 = 89u32;
pub const BTH_MFG_OPEN_INTERFACE: u32 = 39u32;
pub const BTH_MFG_PARTHUS: u32 = 14u32;
pub const BTH_MFG_PHILIPS_SEMICONDUCTOR: u32 = 37u32;
pub const BTH_MFG_QUALCOMM: u32 = 29u32;
pub const BTH_MFG_RF_MICRO_DEVICES: u32 = 40u32;
pub const BTH_MFG_ROHDE_SCHWARZ: u32 = 25u32;
pub const BTH_MFG_RTX_TELECOM: u32 = 21u32;
pub const BTH_MFG_SIGNIA: u32 = 27u32;
pub const BTH_MFG_SILICONWAVE: u32 = 11u32;
pub const BTH_MFG_SYMBOL_TECHNOLOGIES: u32 = 42u32;
pub const BTH_MFG_TENOVIS: u32 = 43u32;
pub const BTH_MFG_TI: u32 = 13u32;
pub const BTH_MFG_TOSHIBA: u32 = 4u32;
pub const BTH_MFG_TRANSILICA: u32 = 24u32;
pub const BTH_MFG_TTPCOM: u32 = 26u32;
pub const BTH_MFG_WAVEPLUS_TECHNOLOGY_CO: u32 = 35u32;
pub const BTH_MFG_WIDCOMM: u32 = 17u32;
pub const BTH_MFG_ZEEVO: u32 = 18u32;
pub const BTH_MINORVERSION: u32 = 1u32;
pub const BTH_SDP_VERSION: u32 = 1u32;
pub const BTH_VID_DEFAULT_VALUE: u32 = 65535u32;
pub const BT_PORT_DYN_FIRST: u32 = 4097u32;
pub const BT_PORT_MAX: u32 = 65535u32;
pub const BT_PORT_MIN: u32 = 1u32;
pub const BasicPrintingProfileID_UUID16: u32 = 4386u32;
pub const BasicPrintingServiceClassID_UUID16: u32 = 4386u32;
pub const Bluetooth_Base_UUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000000_0000_1000_8000_00805f9b34fb);
pub const BrowseGroupDescriptorServiceClassID_UUID16: u32 = 4097u32;
pub const CMPT_PROTOCOL_UUID16: u32 = 27u32;
pub const COD_AUDIO_MINOR_CAMCORDER: u32 = 13u32;
pub const COD_AUDIO_MINOR_CAR_AUDIO: u32 = 8u32;
pub const COD_AUDIO_MINOR_GAMING_TOY: u32 = 18u32;
pub const COD_AUDIO_MINOR_HANDS_FREE: u32 = 2u32;
pub const COD_AUDIO_MINOR_HEADPHONES: u32 = 6u32;
pub const COD_AUDIO_MINOR_HEADSET: u32 = 1u32;
pub const COD_AUDIO_MINOR_HEADSET_HANDS_FREE: u32 = 3u32;
pub const COD_AUDIO_MINOR_HIFI_AUDIO: u32 = 10u32;
pub const COD_AUDIO_MINOR_LOUDSPEAKER: u32 = 5u32;
pub const COD_AUDIO_MINOR_MICROPHONE: u32 = 4u32;
pub const COD_AUDIO_MINOR_PORTABLE_AUDIO: u32 = 7u32;
pub const COD_AUDIO_MINOR_SET_TOP_BOX: u32 = 9u32;
pub const COD_AUDIO_MINOR_UNCLASSIFIED: u32 = 0u32;
pub const COD_AUDIO_MINOR_VCR: u32 = 11u32;
pub const COD_AUDIO_MINOR_VIDEO_CAMERA: u32 = 12u32;
pub const COD_AUDIO_MINOR_VIDEO_DISPLAY_CONFERENCING: u32 = 16u32;
pub const COD_AUDIO_MINOR_VIDEO_DISPLAY_LOUDSPEAKER: u32 = 15u32;
pub const COD_AUDIO_MINOR_VIDEO_MONITOR: u32 = 14u32;
pub const COD_COMPUTER_MINOR_DESKTOP: u32 = 1u32;
pub const COD_COMPUTER_MINOR_HANDHELD: u32 = 4u32;
pub const COD_COMPUTER_MINOR_LAPTOP: u32 = 3u32;
pub const COD_COMPUTER_MINOR_PALM: u32 = 5u32;
pub const COD_COMPUTER_MINOR_SERVER: u32 = 2u32;
pub const COD_COMPUTER_MINOR_UNCLASSIFIED: u32 = 0u32;
pub const COD_COMPUTER_MINOR_WEARABLE: u32 = 6u32;
pub const COD_FORMAT_BIT_OFFSET: u32 = 0u32;
pub const COD_FORMAT_MASK: u32 = 3u32;
pub const COD_HEALTH_MINOR_BLOOD_PRESSURE_MONITOR: u32 = 1u32;
pub const COD_HEALTH_MINOR_GLUCOSE_METER: u32 = 4u32;
pub const COD_HEALTH_MINOR_HEALTH_DATA_DISPLAY: u32 = 7u32;
pub const COD_HEALTH_MINOR_HEART_PULSE_MONITOR: u32 = 6u32;
pub const COD_HEALTH_MINOR_PULSE_OXIMETER: u32 = 5u32;
pub const COD_HEALTH_MINOR_STEP_COUNTER: u32 = 8u32;
pub const COD_HEALTH_MINOR_THERMOMETER: u32 = 2u32;
pub const COD_HEALTH_MINOR_WEIGHING_SCALE: u32 = 3u32;
pub const COD_IMAGING_MINOR_CAMERA_MASK: u32 = 8u32;
pub const COD_IMAGING_MINOR_DISPLAY_MASK: u32 = 4u32;
pub const COD_IMAGING_MINOR_PRINTER_MASK: u32 = 32u32;
pub const COD_IMAGING_MINOR_SCANNER_MASK: u32 = 16u32;
pub const COD_LAN_ACCESS_0_USED: u32 = 0u32;
pub const COD_LAN_ACCESS_17_USED: u32 = 1u32;
pub const COD_LAN_ACCESS_33_USED: u32 = 2u32;
pub const COD_LAN_ACCESS_50_USED: u32 = 3u32;
pub const COD_LAN_ACCESS_67_USED: u32 = 4u32;
pub const COD_LAN_ACCESS_83_USED: u32 = 5u32;
pub const COD_LAN_ACCESS_99_USED: u32 = 6u32;
pub const COD_LAN_ACCESS_BIT_OFFSET: u32 = 5u32;
pub const COD_LAN_ACCESS_FULL: u32 = 7u32;
pub const COD_LAN_ACCESS_MASK: u32 = 224u32;
pub const COD_LAN_MINOR_MASK: u32 = 28u32;
pub const COD_LAN_MINOR_UNCLASSIFIED: u32 = 0u32;
pub const COD_MAJOR_AUDIO: u32 = 4u32;
pub const COD_MAJOR_COMPUTER: u32 = 1u32;
pub const COD_MAJOR_HEALTH: u32 = 9u32;
pub const COD_MAJOR_IMAGING: u32 = 6u32;
pub const COD_MAJOR_LAN_ACCESS: u32 = 3u32;
pub const COD_MAJOR_MASK: u32 = 7936u32;
pub const COD_MAJOR_MISCELLANEOUS: u32 = 0u32;
pub const COD_MAJOR_PERIPHERAL: u32 = 5u32;
pub const COD_MAJOR_PHONE: u32 = 2u32;
pub const COD_MAJOR_TOY: u32 = 8u32;
pub const COD_MAJOR_UNCLASSIFIED: u32 = 31u32;
pub const COD_MAJOR_WEARABLE: u32 = 7u32;
pub const COD_MINOR_BIT_OFFSET: u32 = 2u32;
pub const COD_MINOR_MASK: u32 = 252u32;
pub const COD_PERIPHERAL_MINOR_GAMEPAD: u32 = 2u32;
pub const COD_PERIPHERAL_MINOR_JOYSTICK: u32 = 1u32;
pub const COD_PERIPHERAL_MINOR_KEYBOARD_MASK: u32 = 16u32;
pub const COD_PERIPHERAL_MINOR_NO_CATEGORY: u32 = 0u32;
pub const COD_PERIPHERAL_MINOR_POINTER_MASK: u32 = 32u32;
pub const COD_PERIPHERAL_MINOR_REMOTE_CONTROL: u32 = 3u32;
pub const COD_PERIPHERAL_MINOR_SENSING: u32 = 4u32;
pub const COD_PHONE_MINOR_CELLULAR: u32 = 1u32;
pub const COD_PHONE_MINOR_CORDLESS: u32 = 2u32;
pub const COD_PHONE_MINOR_SMART: u32 = 3u32;
pub const COD_PHONE_MINOR_UNCLASSIFIED: u32 = 0u32;
pub const COD_PHONE_MINOR_WIRED_MODEM: u32 = 4u32;
pub const COD_SERVICE_AUDIO: u32 = 256u32;
pub const COD_SERVICE_CAPTURING: u32 = 64u32;
pub const COD_SERVICE_INFORMATION: u32 = 1024u32;
pub const COD_SERVICE_LE_AUDIO: u32 = 2u32;
pub const COD_SERVICE_LIMITED: u32 = 1u32;
pub const COD_SERVICE_MASK: u32 = 16769024u32;
pub const COD_SERVICE_MAX_COUNT: u32 = 10u32;
pub const COD_SERVICE_NETWORKING: u32 = 16u32;
pub const COD_SERVICE_OBJECT_XFER: u32 = 128u32;
pub const COD_SERVICE_POSITIONING: u32 = 8u32;
pub const COD_SERVICE_RENDERING: u32 = 32u32;
pub const COD_SERVICE_TELEPHONY: u32 = 512u32;
pub const COD_TOY_MINOR_CONTROLLER: u32 = 4u32;
pub const COD_TOY_MINOR_DOLL_ACTION_FIGURE: u32 = 3u32;
pub const COD_TOY_MINOR_GAME: u32 = 5u32;
pub const COD_TOY_MINOR_ROBOT: u32 = 1u32;
pub const COD_TOY_MINOR_VEHICLE: u32 = 2u32;
pub const COD_VERSION: u32 = 0u32;
pub const COD_WEARABLE_MINOR_GLASSES: u32 = 5u32;
pub const COD_WEARABLE_MINOR_HELMET: u32 = 4u32;
pub const COD_WEARABLE_MINOR_JACKET: u32 = 3u32;
pub const COD_WEARABLE_MINOR_PAGER: u32 = 2u32;
pub const COD_WEARABLE_MINOR_WRIST_WATCH: u32 = 1u32;
pub const CORDLESS_EXTERNAL_NETWORK_ANALOG_CELLULAR: u32 = 5u32;
pub const CORDLESS_EXTERNAL_NETWORK_CDMA: u32 = 4u32;
pub const CORDLESS_EXTERNAL_NETWORK_GSM: u32 = 3u32;
pub const CORDLESS_EXTERNAL_NETWORK_ISDN: u32 = 2u32;
pub const CORDLESS_EXTERNAL_NETWORK_OTHER: u32 = 7u32;
pub const CORDLESS_EXTERNAL_NETWORK_PACKET_SWITCHED: u32 = 6u32;
pub const CORDLESS_EXTERNAL_NETWORK_PSTN: u32 = 1u32;
pub const CTNAccessServiceClassID_UUID16: u32 = 4412u32;
pub const CTNNotificationServiceClassID_UUID16: u32 = 4413u32;
pub const CTNProfileID_UUID16: u32 = 4414u32;
pub const CharacteristicAggregateFormat: BTH_LE_GATT_DESCRIPTOR_TYPE = 5i32;
pub const CharacteristicExtendedProperties: BTH_LE_GATT_DESCRIPTOR_TYPE = 0i32;
pub const CharacteristicFormat: BTH_LE_GATT_DESCRIPTOR_TYPE = 4i32;
pub const CharacteristicUserDescription: BTH_LE_GATT_DESCRIPTOR_TYPE = 1i32;
pub const CharacteristicValueChangedEvent: BTH_LE_GATT_EVENT_TYPE = 0i32;
pub const ClientCharacteristicConfiguration: BTH_LE_GATT_DESCRIPTOR_TYPE = 2i32;
pub const CommonISDNAccessServiceClassID_UUID16: u32 = 4392u32;
pub const CommonISDNAccessServiceClass_UUID16: u32 = 4392u32;
pub const CordlessServiceClassID_UUID16: u32 = 4361u32;
pub const CordlessTelephonyServiceClassID_UUID16: u32 = 4361u32;
pub const CustomDescriptor: BTH_LE_GATT_DESCRIPTOR_TYPE = 6i32;
pub const DI_VENDOR_ID_SOURCE_BLUETOOTH_SIG: u32 = 1u32;
pub const DI_VENDOR_ID_SOURCE_USB_IF: u32 = 2u32;
pub const DialupNetworkingServiceClassID_UUID16: u32 = 4355u32;
pub const DirectPrintingReferenceObjectsServiceClassID_UUID16: u32 = 4384u32;
pub const DirectPrintingServiceClassID_UUID16: u32 = 4376u32;
pub const ENCODING_UTF_8: u32 = 106u32;
pub const ESdpUpnpIpLapServiceClassID_UUID16: u32 = 4865u32;
pub const ESdpUpnpIpPanServiceClassID_UUID16: u32 = 4864u32;
pub const ESdpUpnpL2capServiceClassID_UUID16: u32 = 4866u32;
pub const FTP_PROTOCOL_UUID16: u32 = 10u32;
pub const FaxServiceClassID_UUID16: u32 = 4369u32;
pub const GNSSProfileID_UUID16: u32 = 4405u32;
pub const GNSSServerServiceClassID_UUID16: u32 = 4406u32;
pub const GNServiceClassID_UUID16: u32 = 4375u32;
pub const GUID_BLUETOOTHLE_DEVICE_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x781aee18_7733_4ce4_add0_91f41c67b592);
pub const GUID_BLUETOOTH_AUTHENTICATION_REQUEST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5dc9136d_996c_46db_84f5_32c0a3f47352);
pub const GUID_BLUETOOTH_GATT_SERVICE_DEVICE_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6e3bb679_4372_40c8_9eaa_4509df260cd8);
pub const GUID_BLUETOOTH_HCI_EVENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfc240062_1541_49be_b463_84c4dcd7bf7f);
pub const GUID_BLUETOOTH_HCI_VENDOR_EVENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x547247e6_45bb_4c33_af8c_c00efe15a71d);
pub const GUID_BLUETOOTH_KEYPRESS_EVENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd668dfcd_0f4e_4efc_bfe0_392eeec5109c);
pub const GUID_BLUETOOTH_L2CAP_EVENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7eae4030_b709_4aa8_ac55_e953829c9daa);
pub const GUID_BLUETOOTH_RADIO_IN_RANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xea3b5b82_26ee_450e_b0d8_d26fe30a3869);
pub const GUID_BLUETOOTH_RADIO_OUT_OF_RANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe28867c9_c2aa_4ced_b969_4570866037c4);
pub const GUID_BTHPORT_DEVICE_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0850302a_b344_4fda_9be9_90576b8d46f0);
pub const GUID_BTH_RFCOMM_SERVICE_DEVICE_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb142fc3e_fa4e_460b_8abc_072b628b3c70);
pub const GenericAudioServiceClassID_UUID16: u32 = 4611u32;
pub const GenericFileTransferServiceClassID_UUID16: u32 = 4610u32;
pub const GenericNetworkingServiceClassID_UUID16: u32 = 4609u32;
pub const GenericTelephonyServiceClassID_UUID16: u32 = 4612u32;
pub const HCCC_PROTOCOL_UUID16: u32 = 18u32;
pub const HCDC_PROTOCOL_UUID16: u32 = 20u32;
pub const HCI_CONNECTION_TYPE_ACL: u32 = 1u32;
pub const HCI_CONNECTION_TYPE_LE: u32 = 3u32;
pub const HCI_CONNECTION_TYPE_SCO: u32 = 2u32;
pub const HCI_CONNNECTION_TYPE_ACL: u32 = 1u32;
pub const HCI_CONNNECTION_TYPE_SCO: u32 = 2u32;
pub const HCN_PROTOCOL_UUID16: u32 = 22u32;
pub const HCRPrintServiceClassID_UUID16: u32 = 4390u32;
pub const HCRScanServiceClassID_UUID16: u32 = 4391u32;
pub const HID_PROTOCOL_UUID16: u32 = 17u32;
pub const HTTP_PROTOCOL_UUID16: u32 = 12u32;
pub const HandsfreeAudioGatewayServiceClassID_UUID16: u32 = 4383u32;
pub const HandsfreeServiceClassID_UUID16: u32 = 4382u32;
pub const HardcopyCableReplacementProfileID_UUID16: u32 = 4389u32;
pub const HardcopyCableReplacementServiceClassID_UUID16: u32 = 4389u32;
pub const HeadsetAudioGatewayServiceClassID_UUID16: u32 = 4370u32;
pub const HeadsetHSServiceClassID_UUID16: u32 = 4401u32;
pub const HeadsetServiceClassID_UUID16: u32 = 4360u32;
pub const HealthDeviceProfileID_UUID16: u32 = 5120u32;
pub const HealthDeviceProfileSinkServiceClassID_UUID16: u32 = 5122u32;
pub const HealthDeviceProfileSourceServiceClassID_UUID16: u32 = 5121u32;
pub const HumanInterfaceDeviceServiceClassID_UUID16: u32 = 4388u32;
pub const IP_PROTOCOL_UUID16: u32 = 9u32;
pub const ImagingAutomaticArchiveServiceClassID_UUID16: u32 = 4380u32;
pub const ImagingReferenceObjectsServiceClassID_UUID16: u32 = 4381u32;
pub const ImagingResponderServiceClassID_UUID16: u32 = 4379u32;
pub const ImagingServiceClassID_UUID16: u32 = 4378u32;
pub const ImagingServiceProfileID_UUID16: u32 = 4378u32;
pub const IntercomServiceClassID_UUID16: u32 = 4368u32;
pub const IoCaps_DisplayOnly: IO_CAPABILITY = 0i32;
pub const IoCaps_DisplayYesNo: IO_CAPABILITY = 1i32;
pub const IoCaps_KeyboardOnly: IO_CAPABILITY = 2i32;
pub const IoCaps_NoInputNoOutput: IO_CAPABILITY = 3i32;
pub const IoCaps_Undefined: IO_CAPABILITY = 255i32;
pub const IrMCSyncServiceClassID_UUID16: u32 = 4356u32;
pub const IrMcSyncCommandServiceClassID_UUID16: u32 = 4359u32;
pub const L2CAP_DEFAULT_MTU: u32 = 672u32;
pub const L2CAP_MAX_MTU: u32 = 65535u32;
pub const L2CAP_MIN_MTU: u32 = 48u32;
pub const L2CAP_PROTOCOL_UUID16: u32 = 256u32;
pub const LANAccessUsingPPPServiceClassID_UUID16: u32 = 4354u32;
pub const LANGUAGE_EN_US: u32 = 25966u32;
pub const LANG_BASE_ENCODING_INDEX: u32 = 1u32;
pub const LANG_BASE_LANGUAGE_INDEX: u32 = 0u32;
pub const LANG_BASE_OFFSET_INDEX: u32 = 2u32;
pub const LANG_DEFAULT_ID: u32 = 256u32;
pub const LAP_GIAC_VALUE: u32 = 10390323u32;
pub const LAP_LIAC_VALUE: u32 = 10390272u32;
pub const MAX_L2CAP_INFO_DATA_LENGTH: u32 = 44u32;
pub const MAX_L2CAP_PING_DATA_LENGTH: u32 = 44u32;
pub const MAX_UUIDS_IN_QUERY: u32 = 12u32;
pub const MITMProtectionNotDefined: AUTHENTICATION_REQUIREMENTS = 255i32;
pub const MITMProtectionNotRequired: AUTHENTICATION_REQUIREMENTS = 0i32;
pub const MITMProtectionNotRequiredBonding: AUTHENTICATION_REQUIREMENTS = 2i32;
pub const MITMProtectionNotRequiredGeneralBonding: AUTHENTICATION_REQUIREMENTS = 4i32;
pub const MITMProtectionRequired: AUTHENTICATION_REQUIREMENTS = 1i32;
pub const MITMProtectionRequiredBonding: AUTHENTICATION_REQUIREMENTS = 3i32;
pub const MITMProtectionRequiredGeneralBonding: AUTHENTICATION_REQUIREMENTS = 5i32;
pub const MPSProfileID_UUID16: u32 = 4410u32;
pub const MPSServiceClassID_UUID16: u32 = 4411u32;
pub const MessageAccessProfileID_UUID16: u32 = 4404u32;
pub const MessageAccessServerServiceClassID_UUID16: u32 = 4402u32;
pub const MessageNotificationServerServiceClassID_UUID16: u32 = 4403u32;
pub const NAPServiceClassID_UUID16: u32 = 4374u32;
pub const NS_BTH: u32 = 16u32;
pub const NodeContainerTypeAlternative: NodeContainerType = 1i32;
pub const NodeContainerTypeSequence: NodeContainerType = 0i32;
pub const OBEXFileTransferServiceClassID_UUID16: u32 = 4358u32;
pub const OBEXObjectPushServiceClassID_UUID16: u32 = 4357u32;
pub const OBEX_PROTOCOL_UUID16: u32 = 8u32;
pub const OBJECT_PUSH_FORMAT_ANY: u32 = 255u32;
pub const OBJECT_PUSH_FORMAT_ICAL_2_0: u32 = 4u32;
pub const OBJECT_PUSH_FORMAT_VCAL_1_0: u32 = 3u32;
pub const OBJECT_PUSH_FORMAT_VCARD_2_1: u32 = 1u32;
pub const OBJECT_PUSH_FORMAT_VCARD_3_0: u32 = 2u32;
pub const OBJECT_PUSH_FORMAT_VMESSAGE: u32 = 6u32;
pub const OBJECT_PUSH_FORMAT_VNOTE: u32 = 5u32;
pub const PANUServiceClassID_UUID16: u32 = 4373u32;
pub const PF_BTH: u16 = 32u16;
pub const PSM_3DSP: u32 = 33u32;
pub const PSM_ATT: u32 = 31u32;
pub const PSM_AVCTP: u32 = 23u32;
pub const PSM_AVCTP_BROWSE: u32 = 27u32;
pub const PSM_AVDTP: u32 = 25u32;
pub const PSM_BNEP: u32 = 15u32;
pub const PSM_HID_CONTROL: u32 = 17u32;
pub const PSM_HID_INTERRUPT: u32 = 19u32;
pub const PSM_LE_IPSP: u32 = 35u32;
pub const PSM_RFCOMM: u32 = 3u32;
pub const PSM_SDP: u32 = 1u32;
pub const PSM_TCS_BIN: u32 = 5u32;
pub const PSM_TCS_BIN_CORDLESS: u32 = 7u32;
pub const PSM_UDI_C_PLANE: u32 = 29u32;
pub const PSM_UPNP: u32 = 21u32;
pub const PhonebookAccessPceServiceClassID_UUID16: u32 = 4398u32;
pub const PhonebookAccessProfileID_UUID16: u32 = 4400u32;
pub const PhonebookAccessPseServiceClassID_UUID16: u32 = 4399u32;
pub const PnPInformationServiceClassID_UUID16: u32 = 4608u32;
pub const PrintingStatusServiceClassID_UUID16: u32 = 4387u32;
pub const PublicBrowseGroupServiceClassID_UUID16: u32 = 4098u32;
pub const RFCOMM_CMD_MSC: u32 = 1u32;
pub const RFCOMM_CMD_NONE: u32 = 0u32;
pub const RFCOMM_CMD_RLS: u32 = 2u32;
pub const RFCOMM_CMD_RPN: u32 = 3u32;
pub const RFCOMM_CMD_RPN_REQUEST: u32 = 4u32;
pub const RFCOMM_CMD_RPN_RESPONSE: u32 = 5u32;
pub const RFCOMM_MAX_MTU: u32 = 1011u32;
pub const RFCOMM_MIN_MTU: u32 = 23u32;
pub const RFCOMM_PROTOCOL_UUID16: u32 = 3u32;
pub const RLS_ERROR: u32 = 1u32;
pub const RLS_FRAMING: u32 = 8u32;
pub const RLS_OVERRUN: u32 = 2u32;
pub const RLS_PARITY: u32 = 4u32;
pub const RPN_BAUD_115200: u32 = 7u32;
pub const RPN_BAUD_19200: u32 = 4u32;
pub const RPN_BAUD_230400: u32 = 8u32;
pub const RPN_BAUD_2400: u32 = 0u32;
pub const RPN_BAUD_38400: u32 = 5u32;
pub const RPN_BAUD_4800: u32 = 1u32;
pub const RPN_BAUD_57600: u32 = 6u32;
pub const RPN_BAUD_7200: u32 = 2u32;
pub const RPN_BAUD_9600: u32 = 3u32;
pub const RPN_DATA_5: u32 = 0u32;
pub const RPN_DATA_6: u32 = 1u32;
pub const RPN_DATA_7: u32 = 2u32;
pub const RPN_DATA_8: u32 = 3u32;
pub const RPN_FLOW_RTC_IN: u32 = 16u32;
pub const RPN_FLOW_RTC_OUT: u32 = 32u32;
pub const RPN_FLOW_RTR_IN: u32 = 4u32;
pub const RPN_FLOW_RTR_OUT: u32 = 8u32;
pub const RPN_FLOW_X_IN: u32 = 1u32;
pub const RPN_FLOW_X_OUT: u32 = 2u32;
pub const RPN_PARAM_BAUD: u32 = 1u32;
pub const RPN_PARAM_DATA: u32 = 2u32;
pub const RPN_PARAM_PARITY: u32 = 8u32;
pub const RPN_PARAM_P_TYPE: u32 = 16u32;
pub const RPN_PARAM_RTC_IN: u32 = 16u32;
pub const RPN_PARAM_RTC_OUT: u32 = 32u32;
pub const RPN_PARAM_RTR_IN: u32 = 4u32;
pub const RPN_PARAM_RTR_OUT: u32 = 8u32;
pub const RPN_PARAM_STOP: u32 = 4u32;
pub const RPN_PARAM_XOFF: u32 = 64u32;
pub const RPN_PARAM_XON: u32 = 32u32;
pub const RPN_PARAM_X_IN: u32 = 1u32;
pub const RPN_PARAM_X_OUT: u32 = 2u32;
pub const RPN_PARITY_EVEN: u32 = 24u32;
pub const RPN_PARITY_MARK: u32 = 40u32;
pub const RPN_PARITY_NONE: u32 = 0u32;
pub const RPN_PARITY_ODD: u32 = 8u32;
pub const RPN_PARITY_SPACE: u32 = 56u32;
pub const RPN_STOP_1: u32 = 0u32;
pub const RPN_STOP_1_5: u32 = 4u32;
pub const ReferencePrintingServiceClassID_UUID16: u32 = 4377u32;
pub const ReflectsUIServiceClassID_UUID16: u32 = 4385u32;
pub const SAP_BIT_OFFSET: u32 = 0u32;
pub const SDP_ATTRIB_A2DP_SUPPORTED_FEATURES: u32 = 785u32;
pub const SDP_ATTRIB_ADDITIONAL_PROTOCOL_DESCRIPTOR_LIST: u32 = 13u32;
pub const SDP_ATTRIB_AVAILABILITY: u32 = 8u32;
pub const SDP_ATTRIB_AVRCP_SUPPORTED_FEATURES: u32 = 785u32;
pub const SDP_ATTRIB_BROWSE_GROUP_ID: u32 = 512u32;
pub const SDP_ATTRIB_BROWSE_GROUP_LIST: u32 = 5u32;
pub const SDP_ATTRIB_CLASS_ID_LIST: u32 = 1u32;
pub const SDP_ATTRIB_CLIENT_EXECUTABLE_URL: u32 = 11u32;
pub const SDP_ATTRIB_CORDLESS_EXTERNAL_NETWORK: u32 = 769u32;
pub const SDP_ATTRIB_DI_PRIMARY_RECORD: u32 = 516u32;
pub const SDP_ATTRIB_DI_PRODUCT_ID: u32 = 514u32;
pub const SDP_ATTRIB_DI_SPECIFICATION_ID: u32 = 512u32;
pub const SDP_ATTRIB_DI_VENDOR_ID: u32 = 513u32;
pub const SDP_ATTRIB_DI_VENDOR_ID_SOURCE: u32 = 517u32;
pub const SDP_ATTRIB_DI_VERSION: u32 = 515u32;
pub const SDP_ATTRIB_DOCUMENTATION_URL: u32 = 10u32;
pub const SDP_ATTRIB_FAX_AUDIO_FEEDBACK_SUPPORT: u32 = 773u32;
pub const SDP_ATTRIB_FAX_CLASS_1_SUPPORT: u32 = 770u32;
pub const SDP_ATTRIB_FAX_CLASS_2_0_SUPPORT: u32 = 771u32;
pub const SDP_ATTRIB_FAX_CLASS_2_SUPPORT: u32 = 772u32;
pub const SDP_ATTRIB_HEADSET_REMOTE_AUDIO_VOLUME_CONTROL: u32 = 770u32;
pub const SDP_ATTRIB_HFP_SUPPORTED_FEATURES: u32 = 785u32;
pub const SDP_ATTRIB_HID_BATTERY_POWER: u32 = 521u32;
pub const SDP_ATTRIB_HID_BOOT_DEVICE: u32 = 526u32;
pub const SDP_ATTRIB_HID_COUNTRY_CODE: u32 = 515u32;
pub const SDP_ATTRIB_HID_DESCRIPTOR_LIST: u32 = 518u32;
pub const SDP_ATTRIB_HID_DEVICE_RELEASE_NUMBER: u32 = 512u32;
pub const SDP_ATTRIB_HID_DEVICE_SUBCLASS: u32 = 514u32;
pub const SDP_ATTRIB_HID_LANG_ID_BASE_LIST: u32 = 519u32;
pub const SDP_ATTRIB_HID_NORMALLY_CONNECTABLE: u32 = 525u32;
pub const SDP_ATTRIB_HID_PARSER_VERSION: u32 = 513u32;
pub const SDP_ATTRIB_HID_PROFILE_VERSION: u32 = 523u32;
pub const SDP_ATTRIB_HID_RECONNECT_INITIATE: u32 = 517u32;
pub const SDP_ATTRIB_HID_REMOTE_WAKE: u32 = 522u32;
pub const SDP_ATTRIB_HID_SDP_DISABLE: u32 = 520u32;
pub const SDP_ATTRIB_HID_SSR_HOST_MAX_LATENCY: u32 = 527u32;
pub const SDP_ATTRIB_HID_SSR_HOST_MIN_TIMEOUT: u32 = 528u32;
pub const SDP_ATTRIB_HID_SUPERVISION_TIMEOUT: u32 = 524u32;
pub const SDP_ATTRIB_HID_VIRTUAL_CABLE: u32 = 516u32;
pub const SDP_ATTRIB_ICON_URL: u32 = 12u32;
pub const SDP_ATTRIB_IMAGING_SUPPORTED_CAPABILITIES: u32 = 784u32;
pub const SDP_ATTRIB_IMAGING_SUPPORTED_FEATURES: u32 = 785u32;
pub const SDP_ATTRIB_IMAGING_SUPPORTED_FUNCTIONS: u32 = 786u32;
pub const SDP_ATTRIB_IMAGING_TOTAL_DATA_CAPACITY: u32 = 787u32;
pub const SDP_ATTRIB_INFO_TIME_TO_LIVE: u32 = 7u32;
pub const SDP_ATTRIB_LANG_BASE_ATTRIB_ID_LIST: u32 = 6u32;
pub const SDP_ATTRIB_LAN_LPSUBNET: u32 = 512u32;
pub const SDP_ATTRIB_OBJECT_PUSH_SUPPORTED_FORMATS_LIST: u32 = 771u32;
pub const SDP_ATTRIB_PAN_HOME_PAGE_URL: u32 = 776u32;
pub const SDP_ATTRIB_PAN_MAX_NET_ACCESS_RATE: u32 = 780u32;
pub const SDP_ATTRIB_PAN_NETWORK_ADDRESS: u32 = 774u32;
pub const SDP_ATTRIB_PAN_NET_ACCESS_TYPE: u32 = 779u32;
pub const SDP_ATTRIB_PAN_SECURITY_DESCRIPTION: u32 = 778u32;
pub const SDP_ATTRIB_PAN_WAP_GATEWAY: u32 = 775u32;
pub const SDP_ATTRIB_PAN_WAP_STACK_TYPE: u32 = 777u32;
pub const SDP_ATTRIB_PROFILE_DESCRIPTOR_LIST: u32 = 9u32;
pub const SDP_ATTRIB_PROFILE_SPECIFIC: u32 = 512u32;
pub const SDP_ATTRIB_PROTOCOL_DESCRIPTOR_LIST: u32 = 4u32;
pub const SDP_ATTRIB_RECORD_HANDLE: u32 = 0u32;
pub const SDP_ATTRIB_RECORD_STATE: u32 = 2u32;
pub const SDP_ATTRIB_SDP_DATABASE_STATE: u32 = 513u32;
pub const SDP_ATTRIB_SDP_VERSION_NUMBER_LIST: u32 = 512u32;
pub const SDP_ATTRIB_SERVICE_ID: u32 = 3u32;
pub const SDP_ATTRIB_SERVICE_VERSION: u32 = 768u32;
pub const SDP_ATTRIB_SYNCH_SUPPORTED_DATA_STORES_LIST: u32 = 769u32;
pub const SDP_CONNECT_ALLOW_PIN: u32 = 2u32;
pub const SDP_CONNECT_CACHE: u32 = 1u32;
pub const SDP_DEFAULT_INQUIRY_MAX_RESPONSES: u32 = 255u32;
pub const SDP_DEFAULT_INQUIRY_SECONDS: u32 = 6u32;
pub const SDP_ERROR_INSUFFICIENT_RESOURCES: u32 = 6u32;
pub const SDP_ERROR_INVALID_CONTINUATION_STATE: u32 = 5u32;
pub const SDP_ERROR_INVALID_PDU_SIZE: u32 = 4u32;
pub const SDP_ERROR_INVALID_RECORD_HANDLE: u32 = 2u32;
pub const SDP_ERROR_INVALID_REQUEST_SYNTAX: u32 = 3u32;
pub const SDP_ERROR_INVALID_SDP_VERSION: u32 = 1u32;
pub const SDP_MAX_INQUIRY_SECONDS: u32 = 60u32;
pub const SDP_PROTOCOL_UUID16: u32 = 1u32;
pub const SDP_REQUEST_TO_DEFAULT: u32 = 0u32;
pub const SDP_REQUEST_TO_MAX: u32 = 45u32;
pub const SDP_REQUEST_TO_MIN: u32 = 10u32;
pub const SDP_SEARCH_NO_FORMAT_CHECK: u32 = 2u32;
pub const SDP_SEARCH_NO_PARSE_CHECK: u32 = 1u32;
pub const SDP_SERVICE_ATTRIBUTE_REQUEST: u32 = 2u32;
pub const SDP_SERVICE_SEARCH_ATTRIBUTE_REQUEST: u32 = 3u32;
pub const SDP_SERVICE_SEARCH_REQUEST: u32 = 1u32;
pub const SDP_ST_INT128: SDP_SPECIFICTYPE = 1056i32;
pub const SDP_ST_INT16: SDP_SPECIFICTYPE = 288i32;
pub const SDP_ST_INT32: SDP_SPECIFICTYPE = 544i32;
pub const SDP_ST_INT64: SDP_SPECIFICTYPE = 800i32;
pub const SDP_ST_INT8: SDP_SPECIFICTYPE = 32i32;
pub const SDP_ST_NONE: SDP_SPECIFICTYPE = 0i32;
pub const SDP_ST_UINT128: SDP_SPECIFICTYPE = 1040i32;
pub const SDP_ST_UINT16: SDP_SPECIFICTYPE = 272i32;
pub const SDP_ST_UINT32: SDP_SPECIFICTYPE = 528i32;
pub const SDP_ST_UINT64: SDP_SPECIFICTYPE = 784i32;
pub const SDP_ST_UINT8: SDP_SPECIFICTYPE = 16i32;
pub const SDP_ST_UUID128: SDP_SPECIFICTYPE = 1072i32;
pub const SDP_ST_UUID16: SDP_SPECIFICTYPE = 304i32;
pub const SDP_ST_UUID32: SDP_SPECIFICTYPE = 544i32;
pub const SDP_TYPE_ALTERNATIVE: SDP_TYPE = 7i32;
pub const SDP_TYPE_BOOLEAN: SDP_TYPE = 5i32;
pub const SDP_TYPE_CONTAINER: SDP_TYPE = 32i32;
pub const SDP_TYPE_INT: SDP_TYPE = 2i32;
pub const SDP_TYPE_NIL: SDP_TYPE = 0i32;
pub const SDP_TYPE_SEQUENCE: SDP_TYPE = 6i32;
pub const SDP_TYPE_STRING: SDP_TYPE = 4i32;
pub const SDP_TYPE_UINT: SDP_TYPE = 1i32;
pub const SDP_TYPE_URL: SDP_TYPE = 8i32;
pub const SDP_TYPE_UUID: SDP_TYPE = 3i32;
pub const SERVICE_OPTION_DO_NOT_PUBLISH: u32 = 2u32;
pub const SERVICE_OPTION_DO_NOT_PUBLISH_EIR: u32 = 8u32;
pub const SERVICE_OPTION_NO_PUBLIC_BROWSE: u32 = 4u32;
pub const SERVICE_SECURITY_AUTHENTICATE: u32 = 4u32;
pub const SERVICE_SECURITY_AUTHORIZE: u32 = 2u32;
pub const SERVICE_SECURITY_DISABLED: u32 = 268435456u32;
pub const SERVICE_SECURITY_ENCRYPT_OPTIONAL: u32 = 32u32;
pub const SERVICE_SECURITY_ENCRYPT_REQUIRED: u32 = 16u32;
pub const SERVICE_SECURITY_NONE: u32 = 1u32;
pub const SERVICE_SECURITY_NO_ASK: u32 = 536870912u32;
pub const SERVICE_SECURITY_USE_DEFAULTS: u32 = 0u32;
pub const SOL_L2CAP: u32 = 256u32;
pub const SOL_RFCOMM: u32 = 3u32;
pub const SOL_SDP: u32 = 257u32;
pub const SO_BTH_AUTHENTICATE: u32 = 2147483649u32;
pub const SO_BTH_ENCRYPT: u32 = 2u32;
pub const SO_BTH_MTU: u32 = 2147483655u32;
pub const SO_BTH_MTU_MAX: u32 = 2147483656u32;
pub const SO_BTH_MTU_MIN: u32 = 2147483658u32;
pub const STRING_DESCRIPTION_OFFSET: u32 = 1u32;
pub const STRING_NAME_OFFSET: u32 = 0u32;
pub const STRING_PROVIDER_NAME_OFFSET: u32 = 2u32;
pub const STR_ADDR_FMT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("(%02x:%02x:%02x:%02x:%02x:%02x)");
pub const STR_ADDR_FMTA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("(%02x:%02x:%02x:%02x:%02x:%02x)");
pub const STR_ADDR_FMTW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("(%02x:%02x:%02x:%02x:%02x:%02x)");
pub const STR_ADDR_SHORT_FMT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("%04x%08x");
pub const STR_ADDR_SHORT_FMTA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("%04x%08x");
pub const STR_ADDR_SHORT_FMTW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("%04x%08x");
pub const STR_USBHCI_CLASS_HARDWAREID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("USB\\Class_E0&SubClass_01&Prot_01");
pub const STR_USBHCI_CLASS_HARDWAREIDA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("USB\\Class_E0&SubClass_01&Prot_01");
pub const STR_USBHCI_CLASS_HARDWAREIDW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("USB\\Class_E0&SubClass_01&Prot_01");
pub const SVCID_BTH_PROVIDER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x06aa63e0_7d60_41ff_afb2_3ee6d2d9392d);
pub const SYNCH_DATA_STORE_CALENDAR: u32 = 3u32;
pub const SYNCH_DATA_STORE_MESSAGES: u32 = 6u32;
pub const SYNCH_DATA_STORE_NOTES: u32 = 5u32;
pub const SYNCH_DATA_STORE_PHONEBOOK: u32 = 1u32;
pub const SerialPortServiceClassID_UUID16: u32 = 4353u32;
pub const ServerCharacteristicConfiguration: BTH_LE_GATT_DESCRIPTOR_TYPE = 3i32;
pub const ServiceDiscoveryServerServiceClassID_UUID16: u32 = 4096u32;
pub const SimAccessServiceClassID_UUID16: u32 = 4397u32;
pub const TCP_PROTOCOL_UUID16: u32 = 4u32;
pub const TCSAT_PROTOCOL_UUID16: u32 = 6u32;
pub const TCSBIN_PROTOCOL_UUID16: u32 = 5u32;
pub const ThreeDimensionalDisplayServiceClassID_UUID16: u32 = 4407u32;
pub const ThreeDimensionalGlassesServiceClassID_UUID16: u32 = 4408u32;
pub const ThreeDimensionalSynchronizationProfileID_UUID16: u32 = 4409u32;
pub const UDIMTServiceClassID_UUID16: u32 = 4394u32;
pub const UDIMTServiceClass_UUID16: u32 = 4394u32;
pub const UDITAServiceClassID_UUID16: u32 = 4395u32;
pub const UDITAServiceClass_UUID16: u32 = 4395u32;
pub const UDI_C_PLANE_PROTOCOL_UUID16: u32 = 29u32;
pub const UDP_PROTOCOL_UUID16: u32 = 2u32;
pub const UPNP_PROTOCOL_UUID16: u32 = 16u32;
pub const UPnpIpServiceClassID_UUID16: u32 = 4614u32;
pub const UPnpServiceClassID_UUID16: u32 = 4613u32;
pub const VideoConferencingGWServiceClassID_UUID16: u32 = 4393u32;
pub const VideoConferencingGWServiceClass_UUID16: u32 = 4393u32;
pub const VideoConferencingServiceClassID_UUID16: u32 = 4367u32;
pub const VideoDistributionProfileID_UUID16: u32 = 4869u32;
pub const VideoSinkServiceClassID_UUID16: u32 = 4868u32;
pub const VideoSourceServiceClassID_UUID16: u32 = 4867u32;
pub const WAPClientServiceClassID_UUID16: u32 = 4372u32;
pub const WAPServiceClassID_UUID16: u32 = 4371u32;
pub const WSP_PROTOCOL_UUID16: u32 = 14u32;
pub type AUTHENTICATION_REQUIREMENTS = i32;
pub type BLUETOOTH_AUTHENTICATION_METHOD = i32;
pub type BLUETOOTH_AUTHENTICATION_REQUIREMENTS = i32;
pub type BLUETOOTH_IO_CAPABILITY = i32;
pub type BTH_LE_GATT_DESCRIPTOR_TYPE = i32;
pub type BTH_LE_GATT_EVENT_TYPE = i32;
pub type IO_CAPABILITY = i32;
pub type NodeContainerType = i32;
pub type SDP_SPECIFICTYPE = i32;
pub type SDP_TYPE = i32;
#[repr(C)]
pub struct BLUETOOTH_ADDRESS {
    pub Anonymous: BLUETOOTH_ADDRESS_0,
}
impl ::core::marker::Copy for BLUETOOTH_ADDRESS {}
impl ::core::clone::Clone for BLUETOOTH_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union BLUETOOTH_ADDRESS_0 {
    pub ullLong: u64,
    pub rgBytes: [u8; 6],
}
impl ::core::marker::Copy for BLUETOOTH_ADDRESS_0 {}
impl ::core::clone::Clone for BLUETOOTH_ADDRESS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BLUETOOTH_AUTHENTICATE_RESPONSE {
    pub bthAddressRemote: BLUETOOTH_ADDRESS,
    pub authMethod: BLUETOOTH_AUTHENTICATION_METHOD,
    pub Anonymous: BLUETOOTH_AUTHENTICATE_RESPONSE_0,
    pub negativeResponse: u8,
}
impl ::core::marker::Copy for BLUETOOTH_AUTHENTICATE_RESPONSE {}
impl ::core::clone::Clone for BLUETOOTH_AUTHENTICATE_RESPONSE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union BLUETOOTH_AUTHENTICATE_RESPONSE_0 {
    pub pinInfo: BLUETOOTH_PIN_INFO,
    pub oobInfo: BLUETOOTH_OOB_DATA_INFO,
    pub numericCompInfo: BLUETOOTH_NUMERIC_COMPARISON_INFO,
    pub passkeyInfo: BLUETOOTH_PASSKEY_INFO,
}
impl ::core::marker::Copy for BLUETOOTH_AUTHENTICATE_RESPONSE_0 {}
impl ::core::clone::Clone for BLUETOOTH_AUTHENTICATE_RESPONSE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BLUETOOTH_AUTHENTICATION_CALLBACK_PARAMS {
    pub deviceInfo: BLUETOOTH_DEVICE_INFO,
    pub authenticationMethod: BLUETOOTH_AUTHENTICATION_METHOD,
    pub ioCapability: BLUETOOTH_IO_CAPABILITY,
    pub authenticationRequirements: BLUETOOTH_AUTHENTICATION_REQUIREMENTS,
    pub Anonymous: BLUETOOTH_AUTHENTICATION_CALLBACK_PARAMS_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BLUETOOTH_AUTHENTICATION_CALLBACK_PARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BLUETOOTH_AUTHENTICATION_CALLBACK_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union BLUETOOTH_AUTHENTICATION_CALLBACK_PARAMS_0 {
    pub Numeric_Value: u32,
    pub Passkey: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BLUETOOTH_AUTHENTICATION_CALLBACK_PARAMS_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BLUETOOTH_AUTHENTICATION_CALLBACK_PARAMS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BLUETOOTH_COD_PAIRS {
    pub ulCODMask: u32,
    pub pcszDescription: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for BLUETOOTH_COD_PAIRS {}
impl ::core::clone::Clone for BLUETOOTH_COD_PAIRS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BLUETOOTH_DEVICE_INFO {
    pub dwSize: u32,
    pub Address: BLUETOOTH_ADDRESS,
    pub ulClassofDevice: u32,
    pub fConnected: super::super::Foundation::BOOL,
    pub fRemembered: super::super::Foundation::BOOL,
    pub fAuthenticated: super::super::Foundation::BOOL,
    pub stLastSeen: super::super::Foundation::SYSTEMTIME,
    pub stLastUsed: super::super::Foundation::SYSTEMTIME,
    pub szName: [u16; 248],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BLUETOOTH_DEVICE_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BLUETOOTH_DEVICE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BLUETOOTH_DEVICE_SEARCH_PARAMS {
    pub dwSize: u32,
    pub fReturnAuthenticated: super::super::Foundation::BOOL,
    pub fReturnRemembered: super::super::Foundation::BOOL,
    pub fReturnUnknown: super::super::Foundation::BOOL,
    pub fReturnConnected: super::super::Foundation::BOOL,
    pub fIssueInquiry: super::super::Foundation::BOOL,
    pub cTimeoutMultiplier: u8,
    pub hRadio: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BLUETOOTH_DEVICE_SEARCH_PARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BLUETOOTH_DEVICE_SEARCH_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BLUETOOTH_FIND_RADIO_PARAMS {
    pub dwSize: u32,
}
impl ::core::marker::Copy for BLUETOOTH_FIND_RADIO_PARAMS {}
impl ::core::clone::Clone for BLUETOOTH_FIND_RADIO_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BLUETOOTH_GATT_VALUE_CHANGED_EVENT {
    pub ChangedAttributeHandle: u16,
    pub CharacteristicValueDataSize: usize,
    pub CharacteristicValue: *mut BTH_LE_GATT_CHARACTERISTIC_VALUE,
}
impl ::core::marker::Copy for BLUETOOTH_GATT_VALUE_CHANGED_EVENT {}
impl ::core::clone::Clone for BLUETOOTH_GATT_VALUE_CHANGED_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION {
    pub NumCharacteristics: u16,
    pub Characteristics: [BTH_LE_GATT_CHARACTERISTIC; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BLUETOOTH_LOCAL_SERVICE_INFO {
    pub Enabled: super::super::Foundation::BOOL,
    pub btAddr: BLUETOOTH_ADDRESS,
    pub szName: [u16; 256],
    pub szDeviceString: [u16; 256],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BLUETOOTH_LOCAL_SERVICE_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BLUETOOTH_LOCAL_SERVICE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BLUETOOTH_NUMERIC_COMPARISON_INFO {
    pub NumericValue: u32,
}
impl ::core::marker::Copy for BLUETOOTH_NUMERIC_COMPARISON_INFO {}
impl ::core::clone::Clone for BLUETOOTH_NUMERIC_COMPARISON_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BLUETOOTH_OOB_DATA_INFO {
    pub C: [u8; 16],
    pub R: [u8; 16],
}
impl ::core::marker::Copy for BLUETOOTH_OOB_DATA_INFO {}
impl ::core::clone::Clone for BLUETOOTH_OOB_DATA_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BLUETOOTH_PASSKEY_INFO {
    pub passkey: u32,
}
impl ::core::marker::Copy for BLUETOOTH_PASSKEY_INFO {}
impl ::core::clone::Clone for BLUETOOTH_PASSKEY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BLUETOOTH_PIN_INFO {
    pub pin: [u8; 16],
    pub pinLength: u8,
}
impl ::core::marker::Copy for BLUETOOTH_PIN_INFO {}
impl ::core::clone::Clone for BLUETOOTH_PIN_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BLUETOOTH_RADIO_INFO {
    pub dwSize: u32,
    pub address: BLUETOOTH_ADDRESS,
    pub szName: [u16; 248],
    pub ulClassofDevice: u32,
    pub lmpSubversion: u16,
    pub manufacturer: u16,
}
impl ::core::marker::Copy for BLUETOOTH_RADIO_INFO {}
impl ::core::clone::Clone for BLUETOOTH_RADIO_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BLUETOOTH_SELECT_DEVICE_PARAMS {
    pub dwSize: u32,
    pub cNumOfClasses: u32,
    pub prgClassOfDevices: *mut BLUETOOTH_COD_PAIRS,
    pub pszInfo: ::windows_sys::core::PWSTR,
    pub hwndParent: super::super::Foundation::HWND,
    pub fForceAuthentication: super::super::Foundation::BOOL,
    pub fShowAuthenticated: super::super::Foundation::BOOL,
    pub fShowRemembered: super::super::Foundation::BOOL,
    pub fShowUnknown: super::super::Foundation::BOOL,
    pub fAddNewDeviceWizard: super::super::Foundation::BOOL,
    pub fSkipServicesPage: super::super::Foundation::BOOL,
    pub pfnDeviceCallback: PFN_DEVICE_CALLBACK,
    pub pvParam: *mut ::core::ffi::c_void,
    pub cNumDevices: u32,
    pub pDevices: *mut BLUETOOTH_DEVICE_INFO,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BLUETOOTH_SELECT_DEVICE_PARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BLUETOOTH_SELECT_DEVICE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BTH_DEVICE_INFO {
    pub flags: u32,
    pub address: u64,
    pub classOfDevice: u32,
    pub name: [u8; 248],
}
impl ::core::marker::Copy for BTH_DEVICE_INFO {}
impl ::core::clone::Clone for BTH_DEVICE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BTH_HCI_EVENT_INFO {
    pub bthAddress: u64,
    pub connectionType: u8,
    pub connected: u8,
}
impl ::core::marker::Copy for BTH_HCI_EVENT_INFO {}
impl ::core::clone::Clone for BTH_HCI_EVENT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct BTH_INFO_REQ {
    pub btAddr: u64,
    pub infoType: u16,
}
impl ::core::marker::Copy for BTH_INFO_REQ {}
impl ::core::clone::Clone for BTH_INFO_REQ {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct BTH_INFO_RSP {
    pub result: u16,
    pub dataLen: u8,
    pub Anonymous: BTH_INFO_RSP_0,
}
impl ::core::marker::Copy for BTH_INFO_RSP {}
impl ::core::clone::Clone for BTH_INFO_RSP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union BTH_INFO_RSP_0 {
    pub connectionlessMTU: u16,
    pub data: [u8; 44],
}
impl ::core::marker::Copy for BTH_INFO_RSP_0 {}
impl ::core::clone::Clone for BTH_INFO_RSP_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BTH_L2CAP_EVENT_INFO {
    pub bthAddress: u64,
    pub psm: u16,
    pub connected: u8,
    pub initiated: u8,
}
impl ::core::marker::Copy for BTH_L2CAP_EVENT_INFO {}
impl ::core::clone::Clone for BTH_L2CAP_EVENT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_LE_GATT_CHARACTERISTIC {
    pub ServiceHandle: u16,
    pub CharacteristicUuid: BTH_LE_UUID,
    pub AttributeHandle: u16,
    pub CharacteristicValueHandle: u16,
    pub IsBroadcastable: super::super::Foundation::BOOLEAN,
    pub IsReadable: super::super::Foundation::BOOLEAN,
    pub IsWritable: super::super::Foundation::BOOLEAN,
    pub IsWritableWithoutResponse: super::super::Foundation::BOOLEAN,
    pub IsSignedWritable: super::super::Foundation::BOOLEAN,
    pub IsNotifiable: super::super::Foundation::BOOLEAN,
    pub IsIndicatable: super::super::Foundation::BOOLEAN,
    pub HasExtendedProperties: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_GATT_CHARACTERISTIC {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_GATT_CHARACTERISTIC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BTH_LE_GATT_CHARACTERISTIC_VALUE {
    pub DataSize: u32,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for BTH_LE_GATT_CHARACTERISTIC_VALUE {}
impl ::core::clone::Clone for BTH_LE_GATT_CHARACTERISTIC_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_LE_GATT_DESCRIPTOR {
    pub ServiceHandle: u16,
    pub CharacteristicHandle: u16,
    pub DescriptorType: BTH_LE_GATT_DESCRIPTOR_TYPE,
    pub DescriptorUuid: BTH_LE_UUID,
    pub AttributeHandle: u16,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_GATT_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_GATT_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_LE_GATT_DESCRIPTOR_VALUE {
    pub DescriptorType: BTH_LE_GATT_DESCRIPTOR_TYPE,
    pub DescriptorUuid: BTH_LE_UUID,
    pub Anonymous: BTH_LE_GATT_DESCRIPTOR_VALUE_0,
    pub DataSize: u32,
    pub Data: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_GATT_DESCRIPTOR_VALUE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_GATT_DESCRIPTOR_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union BTH_LE_GATT_DESCRIPTOR_VALUE_0 {
    pub CharacteristicExtendedProperties: BTH_LE_GATT_DESCRIPTOR_VALUE_0_0,
    pub ClientCharacteristicConfiguration: BTH_LE_GATT_DESCRIPTOR_VALUE_0_2,
    pub ServerCharacteristicConfiguration: BTH_LE_GATT_DESCRIPTOR_VALUE_0_3,
    pub CharacteristicFormat: BTH_LE_GATT_DESCRIPTOR_VALUE_0_1,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_GATT_DESCRIPTOR_VALUE_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_GATT_DESCRIPTOR_VALUE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_LE_GATT_DESCRIPTOR_VALUE_0_0 {
    pub IsReliableWriteEnabled: super::super::Foundation::BOOLEAN,
    pub IsAuxiliariesWritable: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_GATT_DESCRIPTOR_VALUE_0_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_GATT_DESCRIPTOR_VALUE_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_LE_GATT_DESCRIPTOR_VALUE_0_1 {
    pub Format: u8,
    pub Exponent: u8,
    pub Unit: BTH_LE_UUID,
    pub NameSpace: u8,
    pub Description: BTH_LE_UUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_GATT_DESCRIPTOR_VALUE_0_1 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_GATT_DESCRIPTOR_VALUE_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_LE_GATT_DESCRIPTOR_VALUE_0_2 {
    pub IsSubscribeToNotification: super::super::Foundation::BOOLEAN,
    pub IsSubscribeToIndication: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_GATT_DESCRIPTOR_VALUE_0_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_GATT_DESCRIPTOR_VALUE_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_LE_GATT_DESCRIPTOR_VALUE_0_3 {
    pub IsBroadcast: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_GATT_DESCRIPTOR_VALUE_0_3 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_GATT_DESCRIPTOR_VALUE_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_LE_GATT_SERVICE {
    pub ServiceUuid: BTH_LE_UUID,
    pub AttributeHandle: u16,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_GATT_SERVICE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_GATT_SERVICE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_LE_UUID {
    pub IsShortUuid: super::super::Foundation::BOOLEAN,
    pub Value: BTH_LE_UUID_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_UUID {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_UUID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union BTH_LE_UUID_0 {
    pub ShortUuid: u16,
    pub LongUuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_LE_UUID_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_LE_UUID_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct BTH_PING_REQ {
    pub btAddr: u64,
    pub dataLen: u8,
    pub data: [u8; 44],
}
impl ::core::marker::Copy for BTH_PING_REQ {}
impl ::core::clone::Clone for BTH_PING_REQ {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BTH_PING_RSP {
    pub dataLen: u8,
    pub data: [u8; 44],
}
impl ::core::marker::Copy for BTH_PING_RSP {}
impl ::core::clone::Clone for BTH_PING_RSP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct BTH_QUERY_DEVICE {
    pub LAP: u32,
    pub length: u8,
}
impl ::core::marker::Copy for BTH_QUERY_DEVICE {}
impl ::core::clone::Clone for BTH_QUERY_DEVICE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct BTH_QUERY_SERVICE {
    pub r#type: u32,
    pub serviceHandle: u32,
    pub uuids: [SdpQueryUuid; 12],
    pub numRange: u32,
    pub pRange: [SdpAttributeRange; 1],
}
impl ::core::marker::Copy for BTH_QUERY_SERVICE {}
impl ::core::clone::Clone for BTH_QUERY_SERVICE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BTH_RADIO_IN_RANGE {
    pub deviceInfo: BTH_DEVICE_INFO,
    pub previousDeviceFlags: u32,
}
impl ::core::marker::Copy for BTH_RADIO_IN_RANGE {}
impl ::core::clone::Clone for BTH_RADIO_IN_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BTH_SET_SERVICE {
    pub pSdpVersion: *mut u32,
    pub pRecordHandle: *mut super::super::Foundation::HANDLE,
    pub fCodService: u32,
    pub Reserved: [u32; 5],
    pub ulRecordLength: u32,
    pub pRecord: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BTH_SET_SERVICE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BTH_SET_SERVICE {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HANDLE_SDP_TYPE = u64;
pub type HBLUETOOTH_DEVICE_FIND = isize;
pub type HBLUETOOTH_RADIO_FIND = isize;
#[repr(C, packed(1))]
pub struct RFCOMM_COMMAND {
    pub CmdType: u32,
    pub Data: RFCOMM_COMMAND_0,
}
impl ::core::marker::Copy for RFCOMM_COMMAND {}
impl ::core::clone::Clone for RFCOMM_COMMAND {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union RFCOMM_COMMAND_0 {
    pub MSC: RFCOMM_MSC_DATA,
    pub RLS: RFCOMM_RLS_DATA,
    pub RPN: RFCOMM_RPN_DATA,
}
impl ::core::marker::Copy for RFCOMM_COMMAND_0 {}
impl ::core::clone::Clone for RFCOMM_COMMAND_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RFCOMM_MSC_DATA {
    pub Signals: u8,
    pub Break: u8,
}
impl ::core::marker::Copy for RFCOMM_MSC_DATA {}
impl ::core::clone::Clone for RFCOMM_MSC_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RFCOMM_RLS_DATA {
    pub LineStatus: u8,
}
impl ::core::marker::Copy for RFCOMM_RLS_DATA {}
impl ::core::clone::Clone for RFCOMM_RLS_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RFCOMM_RPN_DATA {
    pub Baud: u8,
    pub Data: u8,
    pub FlowControl: u8,
    pub XonChar: u8,
    pub XoffChar: u8,
    pub ParameterMask1: u8,
    pub ParameterMask2: u8,
}
impl ::core::marker::Copy for RFCOMM_RPN_DATA {}
impl ::core::clone::Clone for RFCOMM_RPN_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SDP_ELEMENT_DATA {
    pub r#type: SDP_TYPE,
    pub specificType: SDP_SPECIFICTYPE,
    pub data: SDP_ELEMENT_DATA_0,
}
impl ::core::marker::Copy for SDP_ELEMENT_DATA {}
impl ::core::clone::Clone for SDP_ELEMENT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union SDP_ELEMENT_DATA_0 {
    pub int128: SDP_LARGE_INTEGER_16,
    pub int64: i64,
    pub int32: i32,
    pub int16: i16,
    pub int8: u8,
    pub uint128: SDP_ULARGE_INTEGER_16,
    pub uint64: u64,
    pub uint32: u32,
    pub uint16: u16,
    pub uint8: u8,
    pub booleanVal: u8,
    pub uuid128: ::windows_sys::core::GUID,
    pub uuid32: u32,
    pub uuid16: u16,
    pub string: SDP_ELEMENT_DATA_0_2,
    pub url: SDP_ELEMENT_DATA_0_3,
    pub sequence: SDP_ELEMENT_DATA_0_1,
    pub alternative: SDP_ELEMENT_DATA_0_0,
}
impl ::core::marker::Copy for SDP_ELEMENT_DATA_0 {}
impl ::core::clone::Clone for SDP_ELEMENT_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SDP_ELEMENT_DATA_0_0 {
    pub value: *mut u8,
    pub length: u32,
}
impl ::core::marker::Copy for SDP_ELEMENT_DATA_0_0 {}
impl ::core::clone::Clone for SDP_ELEMENT_DATA_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SDP_ELEMENT_DATA_0_1 {
    pub value: *mut u8,
    pub length: u32,
}
impl ::core::marker::Copy for SDP_ELEMENT_DATA_0_1 {}
impl ::core::clone::Clone for SDP_ELEMENT_DATA_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SDP_ELEMENT_DATA_0_2 {
    pub value: *mut u8,
    pub length: u32,
}
impl ::core::marker::Copy for SDP_ELEMENT_DATA_0_2 {}
impl ::core::clone::Clone for SDP_ELEMENT_DATA_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SDP_ELEMENT_DATA_0_3 {
    pub value: *mut u8,
    pub length: u32,
}
impl ::core::marker::Copy for SDP_ELEMENT_DATA_0_3 {}
impl ::core::clone::Clone for SDP_ELEMENT_DATA_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SDP_LARGE_INTEGER_16 {
    pub LowPart: u64,
    pub HighPart: i64,
}
impl ::core::marker::Copy for SDP_LARGE_INTEGER_16 {}
impl ::core::clone::Clone for SDP_LARGE_INTEGER_16 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SDP_STRING_TYPE_DATA {
    pub encoding: u16,
    pub mibeNum: u16,
    pub attributeId: u16,
}
impl ::core::marker::Copy for SDP_STRING_TYPE_DATA {}
impl ::core::clone::Clone for SDP_STRING_TYPE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SDP_ULARGE_INTEGER_16 {
    pub LowPart: u64,
    pub HighPart: u64,
}
impl ::core::marker::Copy for SDP_ULARGE_INTEGER_16 {}
impl ::core::clone::Clone for SDP_ULARGE_INTEGER_16 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct SOCKADDR_BTH {
    pub addressFamily: u16,
    pub btAddr: u64,
    pub serviceClassId: ::windows_sys::core::GUID,
    pub port: u32,
}
impl ::core::marker::Copy for SOCKADDR_BTH {}
impl ::core::clone::Clone for SOCKADDR_BTH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SdpAttributeRange {
    pub minAttribute: u16,
    pub maxAttribute: u16,
}
impl ::core::marker::Copy for SdpAttributeRange {}
impl ::core::clone::Clone for SdpAttributeRange {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SdpQueryUuid {
    pub u: SdpQueryUuidUnion,
    pub uuidType: u16,
}
impl ::core::marker::Copy for SdpQueryUuid {}
impl ::core::clone::Clone for SdpQueryUuid {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union SdpQueryUuidUnion {
    pub uuid128: ::windows_sys::core::GUID,
    pub uuid32: u32,
    pub uuid16: u16,
}
impl ::core::marker::Copy for SdpQueryUuidUnion {}
impl ::core::clone::Clone for SdpQueryUuidUnion {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PFNBLUETOOTH_GATT_EVENT_CALLBACK = ::core::option::Option<unsafe extern "system" fn(eventtype: BTH_LE_GATT_EVENT_TYPE, eventoutparameter: *const ::core::ffi::c_void, context: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_AUTHENTICATION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(pvparam: *mut ::core::ffi::c_void, pdevice: *mut BLUETOOTH_DEVICE_INFO) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_AUTHENTICATION_CALLBACK_EX = ::core::option::Option<unsafe extern "system" fn(pvparam: *const ::core::ffi::c_void, pauthcallbackparams: *const BLUETOOTH_AUTHENTICATION_CALLBACK_PARAMS) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_BLUETOOTH_ENUM_ATTRIBUTES_CALLBACK = ::core::option::Option<unsafe extern "system" fn(uattribid: u32, pvaluestream: *const u8, cbstreamsize: u32, pvparam: *const ::core::ffi::c_void) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_DEVICE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(pvparam: *mut ::core::ffi::c_void, pdevice: *const BLUETOOTH_DEVICE_INFO) -> super::super::Foundation::BOOL>;
