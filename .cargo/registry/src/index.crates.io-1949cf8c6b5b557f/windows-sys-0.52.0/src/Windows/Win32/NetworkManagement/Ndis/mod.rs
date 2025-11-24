pub const AUTHENTICATE: OFFLOAD_OPERATION_E = 1i32;
pub const CLOCK_NETWORK_DERIVED: u32 = 2u32;
pub const CLOCK_PRECISION: u32 = 4u32;
pub const DD_NDIS_DEVICE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("\\Device\\NDIS");
pub const DOT11_RSN_KCK_LENGTH: u32 = 16u32;
pub const DOT11_RSN_KEK_LENGTH: u32 = 16u32;
pub const DOT11_RSN_MAX_CIPHER_KEY_LENGTH: u32 = 32u32;
pub const EAPOL_REQUEST_ID_WOL_FLAG_MUST_ENCRYPT: u32 = 1u32;
pub const ENCRYPT: OFFLOAD_OPERATION_E = 2i32;
pub const ETHERNET_LENGTH_OF_ADDRESS: u32 = 6u32;
pub const GUID_DEVINTERFACE_NET: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcac88484_7515_4c03_82e6_71a87abac361);
pub const GUID_DEVINTERFACE_NETUIO: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x08336f60_0679_4c6c_85d2_ae7ced65fff7);
pub const GUID_NDIS_802_11_ADD_KEY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xab8b5a62_1d51_49d8_ba5c_fa980be03a1d);
pub const GUID_NDIS_802_11_ADD_WEP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4307bff0_2129_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_ASSOCIATION_INFORMATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa08d4dd0_960e_40bd_8cf6_c538af98f2e3);
pub const GUID_NDIS_802_11_AUTHENTICATION_MODE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x43920a24_2129_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_BSSID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2504b6c2_1fa5_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_BSSID_LIST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x69526f9a_2062_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_BSSID_LIST_SCAN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0d9e01e1_ba70_11d4_b675_002048570337);
pub const GUID_NDIS_802_11_CONFIGURATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4a4df982_2068_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_DESIRED_RATES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x452ee08e_2536_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_DISASSOCIATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x43671f40_2129_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_FRAGMENTATION_THRESHOLD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x69aaa7c4_2062_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_INFRASTRUCTURE_MODE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x697d5a7e_2062_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_MEDIA_STREAM_MODE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0a56af66_d84b_49eb_a28d_5282cbb6d0cd);
pub const GUID_NDIS_802_11_NETWORK_TYPES_SUPPORTED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8531d6e6_2041_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_NETWORK_TYPE_IN_USE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x857e2326_2041_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_NUMBER_OF_ANTENNAS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x01779336_2064_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_POWER_MODE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x85be837c_2041_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_PRIVACY_FILTER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6733c4e9_4792_11d4_97f1_00c04f79c403);
pub const GUID_NDIS_802_11_RELOAD_DEFAULTS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x748b14e8_32ee_4425_b91b_c9848c58b55a);
pub const GUID_NDIS_802_11_REMOVE_KEY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x73cb28e9_3188_42d5_b553_b21237e6088c);
pub const GUID_NDIS_802_11_REMOVE_WEP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x433c345c_2129_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_RSSI: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1507db16_2053_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_RSSI_TRIGGER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x155689b8_2053_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_RTS_THRESHOLD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0134d07e_2064_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_RX_ANTENNA_SELECTED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x01ac07a2_2064_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_SSID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7d2a90ea_2041_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_STATISTICS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x42bb73b0_2129_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_SUPPORTED_RATES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x49db8722_2068_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_TEST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4b9ca16a_6a60_4e9d_920c_6335953fa0b5);
pub const GUID_NDIS_802_11_TX_ANTENNA_SELECTED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x01dbb74a_2064_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_TX_POWER_LEVEL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x11e6ba76_2053_11d4_97eb_00c04f79c403);
pub const GUID_NDIS_802_11_WEP_STATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb027a21f_3cfa_4125_800b_3f7a18fddcdc);
pub const GUID_NDIS_802_3_CURRENT_ADDRESS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795700_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_3_MAC_OPTIONS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795703_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_3_MAXIMUM_LIST_SIZE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795702_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_3_MULTICAST_LIST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795701_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_3_PERMANENT_ADDRESS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x447956ff_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_3_RCV_ERROR_ALIGNMENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795704_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_3_XMIT_MORE_COLLISIONS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795706_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_3_XMIT_ONE_COLLISION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795705_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_5_CURRENT_ADDRESS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795708_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_5_CURRENT_FUNCTIONAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795709_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_5_CURRENT_GROUP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4479570a_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_5_CURRENT_RING_STATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xacf14032_a61c_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_5_CURRENT_RING_STATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x890a36ec_a61c_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_5_LAST_OPEN_STATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4479570b_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_5_LINE_ERRORS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xacf14033_a61c_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_5_LOST_FRAMES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xacf14034_a61c_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_802_5_PERMANENT_ADDRESS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44795707_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_ENUMERATE_ADAPTER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d7f_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_ENUMERATE_ADAPTERS_EX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x16716917_4306_4be4_9b5a_3809ae44b125);
pub const GUID_NDIS_ENUMERATE_VC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d82_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_DRIVER_VERSION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad198_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_HARDWARE_STATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad192_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_LINK_SPEED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad195_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_MAC_OPTIONS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad19a_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_MEDIA_CONNECT_STATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad19b_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_MEDIA_IN_USE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad194_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_MEDIA_SUPPORTED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad193_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_MINIMUM_LINK_SPEED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad19d_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_RCV_PDUS_ERROR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0a214808_e35f_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_RCV_PDUS_NO_BUFFER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0a214809_e35f_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_RCV_PDUS_OK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0a214806_e35f_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_VENDOR_DESCRIPTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad197_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_VENDOR_DRIVER_VERSION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad19c_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_VENDOR_ID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x791ad196_e35c_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_XMIT_PDUS_ERROR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0a214807_e35f_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CO_XMIT_PDUS_OK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0a214805_e35f_11d0_9692_00c04fc3358c);
pub const GUID_NDIS_GEN_CURRENT_LOOKAHEAD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10361_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_CURRENT_PACKET_FILTER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10360_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_DRIVER_VERSION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10362_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_ENUMERATE_PORTS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf1d6abe8_15e4_4407_81b7_6b830c777cd9);
pub const GUID_NDIS_GEN_HARDWARE_STATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10354_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_INTERRUPT_MODERATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd9c8eea5_f16e_467c_84d5_6345a22ce213);
pub const GUID_NDIS_GEN_INTERRUPT_MODERATION_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd789adfa_9c56_433b_ad01_7574f3cedbe9);
pub const GUID_NDIS_GEN_LINK_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8c7d3579_252b_4614_82c5_a650daa15049);
pub const GUID_NDIS_GEN_LINK_SPEED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10359_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_LINK_STATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xba1f4c14_a945_4762_b916_0b5515b6f43a);
pub const GUID_NDIS_GEN_MAC_OPTIONS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10365_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_MAXIMUM_FRAME_SIZE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10358_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_MAXIMUM_LOOKAHEAD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10357_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_MAXIMUM_SEND_PACKETS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10367_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_MAXIMUM_TOTAL_SIZE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10363_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_MEDIA_CONNECT_STATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10366_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_MEDIA_IN_USE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10356_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_MEDIA_SUPPORTED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec10355_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_PCI_DEVICE_CUSTOM_PROPERTIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaa39f5ab_e260_4d01_82b0_b737c880ea05);
pub const GUID_NDIS_GEN_PHYSICAL_MEDIUM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x418ca16d_3937_4208_940a_ec6196278085);
pub const GUID_NDIS_GEN_PHYSICAL_MEDIUM_EX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x899e7782_035b_43f9_8bb6_2b58971612e5);
pub const GUID_NDIS_GEN_PORT_AUTHENTICATION_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaab6ac31_86fb_48fb_8b48_63db235ace16);
pub const GUID_NDIS_GEN_PORT_STATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6fbf2a5f_8b8f_4920_8143_e6c460f52524);
pub const GUID_NDIS_GEN_RCV_ERROR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x447956fd_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_RCV_NO_BUFFER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x447956fe_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_RCV_OK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x447956fb_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_RECEIVE_BLOCK_SIZE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec1035d_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_RECEIVE_BUFFER_SPACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec1035b_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_STATISTICS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x368c45b5_c129_43c1_939e_7edc2d7fe621);
pub const GUID_NDIS_GEN_TRANSMIT_BLOCK_SIZE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec1035c_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_TRANSMIT_BUFFER_SPACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec1035a_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_VENDOR_DESCRIPTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec1035f_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_VENDOR_DRIVER_VERSION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x447956f9_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_VENDOR_ID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5ec1035e_a61a_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_VLAN_ID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x765dc702_c5e8_4b67_843b_3f5a4ff2648b);
pub const GUID_NDIS_GEN_XMIT_ERROR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x447956fc_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_GEN_XMIT_OK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x447956fa_a61b_11d0_8dd4_00c04fc3358c);
pub const GUID_NDIS_HD_SPLIT_CURRENT_CONFIG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x81d1303c_ab00_4e49_80b1_5e6e0bf9be53);
pub const GUID_NDIS_HD_SPLIT_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8c048bea_2913_4458_b68e_17f6c1e5c60e);
pub const GUID_NDIS_LAN_CLASS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xad498944_762f_11d0_8dcb_00c04fc3358c);
pub const GUID_NDIS_NDK_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7969ba4d_dd80_4bc7_b3e6_68043997e519);
pub const GUID_NDIS_NDK_STATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x530c69c9_2f51_49de_a1af_088d54ffa474);
pub const GUID_NDIS_NOTIFY_ADAPTER_ARRIVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d81_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_NOTIFY_ADAPTER_REMOVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d80_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_NOTIFY_BIND: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5413531c_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_NOTIFY_DEVICE_POWER_OFF: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x81bc8189_b026_46ab_b964_f182e342934e);
pub const GUID_NDIS_NOTIFY_DEVICE_POWER_OFF_EX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4159353c_5cd7_42ce_8fe4_a45a2380cc4f);
pub const GUID_NDIS_NOTIFY_DEVICE_POWER_ON: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5f81cfd0_f046_4342_af61_895acedaefd9);
pub const GUID_NDIS_NOTIFY_DEVICE_POWER_ON_EX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2b440188_92ac_4f60_9b2d_20a30cbb6bbe);
pub const GUID_NDIS_NOTIFY_FILTER_ARRIVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0b6d3c89_5917_43ca_b578_d01a7967c41c);
pub const GUID_NDIS_NOTIFY_FILTER_REMOVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1f177cd9_5955_4721_9f6a_78ebdfaef889);
pub const GUID_NDIS_NOTIFY_UNBIND: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6e3ce1ec_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_NOTIFY_VC_ARRIVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x182f9e0c_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_NOTIFY_VC_REMOVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d79_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_PM_ACTIVE_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb2cf76e3_b3ae_4394_a01f_338c9870e939);
pub const GUID_NDIS_PM_ADMIN_CONFIG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1528d111_708a_4ca4_9215_c05771161cda);
pub const GUID_NDIS_RECEIVE_FILTER_ENUM_FILTERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3f2c141d_83bc_11dd_94b8_001d09162bc3);
pub const GUID_NDIS_RECEIVE_FILTER_ENUM_QUEUES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3f2c141b_83bc_11dd_94b8_001d09162bc3);
pub const GUID_NDIS_RECEIVE_FILTER_GLOBAL_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3f2c141a_83bc_11dd_94b8_001d09162bc3);
pub const GUID_NDIS_RECEIVE_FILTER_HARDWARE_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3f2c1419_83bc_11dd_94b8_001d09162bc3);
pub const GUID_NDIS_RECEIVE_FILTER_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3f2c141e_83bc_11dd_94b8_001d09162bc3);
pub const GUID_NDIS_RECEIVE_FILTER_QUEUE_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3f2c141c_83bc_11dd_94b8_001d09162bc3);
pub const GUID_NDIS_RECEIVE_SCALE_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x26c28774_4252_48fe_a610_a58a398c0eb1);
pub const GUID_NDIS_RSS_ENABLED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9565cd55_3402_4e32_a5b6_2f143f2f2c30);
pub const GUID_NDIS_STATUS_DOT11_ASSOCIATION_COMPLETION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x458bbea7_45a4_4ae2_b176_e51f96fc0568);
pub const GUID_NDIS_STATUS_DOT11_ASSOCIATION_START: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3927843b_6980_4b48_b15b_4de50977ac40);
pub const GUID_NDIS_STATUS_DOT11_CONNECTION_COMPLETION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x96efd9c9_7f1b_4a89_bc04_3e9e271765f1);
pub const GUID_NDIS_STATUS_DOT11_CONNECTION_START: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7b74299d_998f_4454_ad08_c5af28576d1b);
pub const GUID_NDIS_STATUS_DOT11_DISASSOCIATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3fbeb6fc_0fe2_43fd_b2ad_bd99b5f93e13);
pub const GUID_NDIS_STATUS_DOT11_LINK_QUALITY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa3285184_ea99_48ed_825e_a426b11c2754);
pub const GUID_NDIS_STATUS_DOT11_MPDU_MAX_LENGTH_CHANGED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1d6560ec_8e48_4a3e_9fd5_a01b698db6c5);
pub const GUID_NDIS_STATUS_DOT11_PHY_STATE_CHANGED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdeb45316_71b5_4736_bdef_0a9e9f4e62dc);
pub const GUID_NDIS_STATUS_DOT11_PMKID_CANDIDATE_LIST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x26d8b8f6_db82_49eb_8bf3_4c130ef06950);
pub const GUID_NDIS_STATUS_DOT11_ROAMING_COMPLETION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdd9d47d1_282b_41e4_b924_66368817fcd3);
pub const GUID_NDIS_STATUS_DOT11_ROAMING_START: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb2412d0d_26c8_4f4e_93df_f7b705a0b433);
pub const GUID_NDIS_STATUS_DOT11_SCAN_CONFIRM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8500591e_a0c7_4efb_9342_b674b002cbe6);
pub const GUID_NDIS_STATUS_DOT11_TKIPMIC_FAILURE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x442c2ae4_9bc5_4b90_a889_455ef220f4ee);
pub const GUID_NDIS_STATUS_EXTERNAL_CONNECTIVITY_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfd306974_c420_4433_b0fe_4cf6a613f59f);
pub const GUID_NDIS_STATUS_HD_SPLIT_CURRENT_CONFIG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6c744b0e_ee9c_4205_90a2_015f6d65f403);
pub const GUID_NDIS_STATUS_LINK_SPEED_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d85_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_STATUS_LINK_STATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x64c6f797_878c_4311_9246_65dba89c3a61);
pub const GUID_NDIS_STATUS_MEDIA_CONNECT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d7d_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_STATUS_MEDIA_DISCONNECT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d7e_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_STATUS_MEDIA_SPECIFIC_INDICATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d84_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_STATUS_NETWORK_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xca8a56f9_ce81_40e6_a70f_a067a476e9e9);
pub const GUID_NDIS_STATUS_OPER_STATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf917b663_845e_4d3d_b6d4_15eb27af81c5);
pub const GUID_NDIS_STATUS_PACKET_FILTER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd47c5407_2e75_46dd_8146_1d7ed2d6ab1d);
pub const GUID_NDIS_STATUS_PM_OFFLOAD_REJECTED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xadd1d481_711e_4d1a_92ca_a62db9329712);
pub const GUID_NDIS_STATUS_PM_WAKE_REASON: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0933fd58_ca62_438f_83da_dfc1cccb8145);
pub const GUID_NDIS_STATUS_PM_WOL_PATTERN_REJECTED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf72cf68e_18d4_4d63_9a19_e69b13916b1a);
pub const GUID_NDIS_STATUS_PORT_STATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1dac0dfe_43e5_44b7_b759_7bf46de32e81);
pub const GUID_NDIS_STATUS_RESET_END: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d77_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_STATUS_RESET_START: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x981f2d76_b1f3_11d0_8dd7_00c04fc3358c);
pub const GUID_NDIS_STATUS_TASK_OFFLOAD_CURRENT_CONFIG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x45049fc6_54d8_40c8_9c3d_b011c4e715bc);
pub const GUID_NDIS_STATUS_TASK_OFFLOAD_HARDWARE_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb6b8158b_217c_4b2a_be86_6a04beea65b8);
pub const GUID_NDIS_STATUS_TCP_CONNECTION_OFFLOAD_CURRENT_CONFIG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf8edaeff_24e4_4ae6_a413_0b27f76b243d);
pub const GUID_NDIS_STATUS_TCP_CONNECTION_OFFLOAD_HARDWARE_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x391969b6_402c_43bf_8922_39eae0da1bb5);
pub const GUID_NDIS_SWITCH_MICROSOFT_VENDOR_ID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x202547fe_1c9c_40b9_bba1_08ada1f98b3c);
pub const GUID_NDIS_SWITCH_PORT_PROPERTY_PROFILE_ID_DEFAULT_EXTERNAL_NIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0b347846_0a0c_470a_9b7a_0d965850698f);
pub const GUID_NDIS_TCP_CONNECTION_OFFLOAD_CURRENT_CONFIG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2ee6aef1_0851_458b_bf0d_792343d1cde1);
pub const GUID_NDIS_TCP_CONNECTION_OFFLOAD_HARDWARE_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8ce71f2c_d63a_4390_a487_18fa47262ceb);
pub const GUID_NDIS_TCP_OFFLOAD_CURRENT_CONFIG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x68542fed_5c74_461e_8934_91c6f9c60960);
pub const GUID_NDIS_TCP_OFFLOAD_HARDWARE_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcd5f1102_590f_4ada_ab65_5b31b1dc0172);
pub const GUID_NDIS_TCP_OFFLOAD_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8ead9a22_7f69_4bc6_949a_c8187b074e61);
pub const GUID_NDIS_TCP_RSC_STATISTICS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x83104445_9b5d_4ee6_a2a5_2bd3fb3c36af);
pub const GUID_NDIS_WAKE_ON_MAGIC_PACKET_ONLY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa14f1c97_8839_4f8a_9996_a28996ebbf1d);
pub const GUID_NIC_SWITCH_CURRENT_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe76fdaf3_0be7_4d95_87e9_5aead4b590e9);
pub const GUID_NIC_SWITCH_HARDWARE_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x37cab40c_d1e8_4301_8c1d_58465e0c4c0f);
pub const GUID_PM_ADD_PROTOCOL_OFFLOAD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0c06c112_0d93_439b_9e6d_26be130c9784);
pub const GUID_PM_ADD_WOL_PATTERN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6fc83ba7_52bc_4faa_ac51_7d2ffe63ba90);
pub const GUID_PM_CURRENT_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3abdbd14_d44a_4a3f_9a63_a0a42a51b131);
pub const GUID_PM_GET_PROTOCOL_OFFLOAD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa6435cd9_149f_498e_951b_2d94bea3e3a3);
pub const GUID_PM_HARDWARE_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xece5360d_3291_4a6e_8044_00511fed27ee);
pub const GUID_PM_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x560245d2_e251_409c_a280_311935be3b28);
pub const GUID_PM_PROTOCOL_OFFLOAD_LIST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x736ec5ab_ca8f_4043_bb58_da402a48d9cc);
pub const GUID_PM_REMOVE_PROTOCOL_OFFLOAD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdecd7be2_a6b0_43ca_ae45_d000d20e5265);
pub const GUID_PM_REMOVE_WOL_PATTERN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa037a915_c6ca_4322_b3e3_ef754ec498dc);
pub const GUID_PM_WOL_PATTERN_LIST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4022be37_7ee2_47be_a5a5_050fc79afc75);
pub const GUID_RECEIVE_FILTER_CURRENT_CAPABILITIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4054e80f_2bc1_4ccc_b033_4abc0c4a1e8c);
pub const GUID_STATUS_MEDIA_SPECIFIC_INDICATION_EX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaaacfca7_954a_4632_a16e_a8a63793a9e5);
pub const IF_ADMINISTRATIVE_DEMANDDIAL: IF_ADMINISTRATIVE_STATE = 2i32;
pub const IF_ADMINISTRATIVE_DISABLED: IF_ADMINISTRATIVE_STATE = 0i32;
pub const IF_ADMINISTRATIVE_ENABLED: IF_ADMINISTRATIVE_STATE = 1i32;
pub const IF_MAX_PHYS_ADDRESS_LENGTH: u32 = 32u32;
pub const IF_MAX_STRING_SIZE: u32 = 256u32;
pub const IOCTL_NDIS_RESERVED5: u32 = 1507380u32;
pub const IOCTL_NDIS_RESERVED6: u32 = 1540152u32;
pub const IPSEC_OFFLOAD_V2_AND_TCP_CHECKSUM_COEXISTENCE: u32 = 2u32;
pub const IPSEC_OFFLOAD_V2_AND_UDP_CHECKSUM_COEXISTENCE: u32 = 4u32;
pub const IPSEC_OFFLOAD_V2_AUTHENTICATION_AES_GCM_128: u32 = 8u32;
pub const IPSEC_OFFLOAD_V2_AUTHENTICATION_AES_GCM_192: u32 = 16u32;
pub const IPSEC_OFFLOAD_V2_AUTHENTICATION_AES_GCM_256: u32 = 32u32;
pub const IPSEC_OFFLOAD_V2_AUTHENTICATION_MD5: u32 = 1u32;
pub const IPSEC_OFFLOAD_V2_AUTHENTICATION_SHA_1: u32 = 2u32;
pub const IPSEC_OFFLOAD_V2_AUTHENTICATION_SHA_256: u32 = 4u32;
pub const IPSEC_OFFLOAD_V2_ENCRYPTION_3_DES_CBC: u32 = 4u32;
pub const IPSEC_OFFLOAD_V2_ENCRYPTION_AES_CBC_128: u32 = 64u32;
pub const IPSEC_OFFLOAD_V2_ENCRYPTION_AES_CBC_192: u32 = 128u32;
pub const IPSEC_OFFLOAD_V2_ENCRYPTION_AES_CBC_256: u32 = 256u32;
pub const IPSEC_OFFLOAD_V2_ENCRYPTION_AES_GCM_128: u32 = 8u32;
pub const IPSEC_OFFLOAD_V2_ENCRYPTION_AES_GCM_192: u32 = 16u32;
pub const IPSEC_OFFLOAD_V2_ENCRYPTION_AES_GCM_256: u32 = 32u32;
pub const IPSEC_OFFLOAD_V2_ENCRYPTION_DES_CBC: u32 = 2u32;
pub const IPSEC_OFFLOAD_V2_ENCRYPTION_NONE: u32 = 1u32;
pub const IfOperStatusDormant: IF_OPER_STATUS = 5i32;
pub const IfOperStatusDown: IF_OPER_STATUS = 2i32;
pub const IfOperStatusLowerLayerDown: IF_OPER_STATUS = 7i32;
pub const IfOperStatusNotPresent: IF_OPER_STATUS = 6i32;
pub const IfOperStatusTesting: IF_OPER_STATUS = 3i32;
pub const IfOperStatusUnknown: IF_OPER_STATUS = 4i32;
pub const IfOperStatusUp: IF_OPER_STATUS = 1i32;
pub const MAXIMUM_IP_OPER_STATUS_ADDRESS_FAMILIES_SUPPORTED: u32 = 32u32;
pub const MediaConnectStateConnected: NET_IF_MEDIA_CONNECT_STATE = 1i32;
pub const MediaConnectStateDisconnected: NET_IF_MEDIA_CONNECT_STATE = 2i32;
pub const MediaConnectStateUnknown: NET_IF_MEDIA_CONNECT_STATE = 0i32;
pub const MediaDuplexStateFull: NET_IF_MEDIA_DUPLEX_STATE = 2i32;
pub const MediaDuplexStateHalf: NET_IF_MEDIA_DUPLEX_STATE = 1i32;
pub const MediaDuplexStateUnknown: NET_IF_MEDIA_DUPLEX_STATE = 0i32;
pub const NDIS_802_11_AI_REQFI_CAPABILITIES: u32 = 1u32;
pub const NDIS_802_11_AI_REQFI_CURRENTAPADDRESS: u32 = 4u32;
pub const NDIS_802_11_AI_REQFI_LISTENINTERVAL: u32 = 2u32;
pub const NDIS_802_11_AI_RESFI_ASSOCIATIONID: u32 = 4u32;
pub const NDIS_802_11_AI_RESFI_CAPABILITIES: u32 = 1u32;
pub const NDIS_802_11_AI_RESFI_STATUSCODE: u32 = 2u32;
pub const NDIS_802_11_AUTH_REQUEST_AUTH_FIELDS: u32 = 15u32;
pub const NDIS_802_11_AUTH_REQUEST_GROUP_ERROR: u32 = 14u32;
pub const NDIS_802_11_AUTH_REQUEST_KEYUPDATE: u32 = 2u32;
pub const NDIS_802_11_AUTH_REQUEST_PAIRWISE_ERROR: u32 = 6u32;
pub const NDIS_802_11_AUTH_REQUEST_REAUTH: u32 = 1u32;
pub const NDIS_802_11_LENGTH_RATES: u32 = 8u32;
pub const NDIS_802_11_LENGTH_RATES_EX: u32 = 16u32;
pub const NDIS_802_11_LENGTH_SSID: u32 = 32u32;
pub const NDIS_802_11_PMKID_CANDIDATE_PREAUTH_ENABLED: u32 = 1u32;
pub const NDIS_802_3_MAC_OPTION_PRIORITY: u32 = 1u32;
pub const NDIS_CO_MAC_OPTION_DYNAMIC_LINK_SPEED: u32 = 1u32;
pub const NDIS_DEFAULT_RECEIVE_FILTER_ID: u32 = 0u32;
pub const NDIS_DEFAULT_RECEIVE_QUEUE_GROUP_ID: u32 = 0u32;
pub const NDIS_DEFAULT_RECEIVE_QUEUE_ID: u32 = 0u32;
pub const NDIS_DEFAULT_SWITCH_ID: u32 = 0u32;
pub const NDIS_DEFAULT_VPORT_ID: u32 = 0u32;
pub const NDIS_DEVICE_TYPE_ENDPOINT: u32 = 1u32;
pub const NDIS_DEVICE_WAKE_ON_MAGIC_PACKET_ENABLE: u32 = 4u32;
pub const NDIS_DEVICE_WAKE_ON_PATTERN_MATCH_ENABLE: u32 = 2u32;
pub const NDIS_DEVICE_WAKE_UP_ENABLE: u32 = 1u32;
pub const NDIS_ENCAPSULATED_PACKET_TASK_OFFLOAD_INNER_IPV4: u32 = 1u32;
pub const NDIS_ENCAPSULATED_PACKET_TASK_OFFLOAD_INNER_IPV6: u32 = 4u32;
pub const NDIS_ENCAPSULATED_PACKET_TASK_OFFLOAD_NOT_SUPPORTED: u32 = 0u32;
pub const NDIS_ENCAPSULATED_PACKET_TASK_OFFLOAD_OUTER_IPV4: u32 = 2u32;
pub const NDIS_ENCAPSULATED_PACKET_TASK_OFFLOAD_OUTER_IPV6: u32 = 8u32;
pub const NDIS_ENCAPSULATION_IEEE_802_3: u32 = 2u32;
pub const NDIS_ENCAPSULATION_IEEE_802_3_P_AND_Q: u32 = 4u32;
pub const NDIS_ENCAPSULATION_IEEE_802_3_P_AND_Q_IN_OOB: u32 = 8u32;
pub const NDIS_ENCAPSULATION_IEEE_LLC_SNAP_ROUTED: u32 = 16u32;
pub const NDIS_ENCAPSULATION_NOT_SUPPORTED: u32 = 0u32;
pub const NDIS_ENCAPSULATION_NULL: u32 = 1u32;
pub const NDIS_ENCAPSULATION_TYPE_GRE_MAC: u32 = 1u32;
pub const NDIS_ENCAPSULATION_TYPE_VXLAN: u32 = 2u32;
pub const NDIS_ETH_TYPE_802_1Q: u32 = 33024u32;
pub const NDIS_ETH_TYPE_802_1X: u32 = 34958u32;
pub const NDIS_ETH_TYPE_ARP: u32 = 2054u32;
pub const NDIS_ETH_TYPE_IPV4: u32 = 2048u32;
pub const NDIS_ETH_TYPE_IPV6: u32 = 34525u32;
pub const NDIS_ETH_TYPE_SLOW_PROTOCOL: u32 = 34825u32;
pub const NDIS_GFP_ENCAPSULATION_TYPE_IP_IN_GRE: u32 = 4u32;
pub const NDIS_GFP_ENCAPSULATION_TYPE_IP_IN_IP: u32 = 2u32;
pub const NDIS_GFP_ENCAPSULATION_TYPE_NOT_ENCAPSULATED: u32 = 1u32;
pub const NDIS_GFP_ENCAPSULATION_TYPE_NVGRE: u32 = 8u32;
pub const NDIS_GFP_ENCAPSULATION_TYPE_VXLAN: u32 = 16u32;
pub const NDIS_GFP_EXACT_MATCH_PROFILE_RDMA_FLOW: u32 = 1u32;
pub const NDIS_GFP_EXACT_MATCH_PROFILE_REVISION_1: u32 = 1u32;
pub const NDIS_GFP_HEADER_GROUP_EXACT_MATCH_IS_TTL_ONE: u32 = 1u32;
pub const NDIS_GFP_HEADER_GROUP_EXACT_MATCH_PROFILE_IS_TTL_ONE: u32 = 1u32;
pub const NDIS_GFP_HEADER_GROUP_EXACT_MATCH_PROFILE_REVISION_1: u32 = 1u32;
pub const NDIS_GFP_HEADER_GROUP_EXACT_MATCH_REVISION_1: u32 = 1u32;
pub const NDIS_GFP_HEADER_GROUP_WILDCARD_MATCH_IS_TTL_ONE: u32 = 1u32;
pub const NDIS_GFP_HEADER_GROUP_WILDCARD_MATCH_PROFILE_IS_TTL_ONE: u32 = 1u32;
pub const NDIS_GFP_HEADER_GROUP_WILDCARD_MATCH_PROFILE_REVISION_1: u32 = 1u32;
pub const NDIS_GFP_HEADER_GROUP_WILDCARD_MATCH_REVISION_1: u32 = 1u32;
pub const NDIS_GFP_HEADER_PRESENT_ESP: u32 = 2048u32;
pub const NDIS_GFP_HEADER_PRESENT_ETHERNET: u32 = 1u32;
pub const NDIS_GFP_HEADER_PRESENT_ICMP: u32 = 32u32;
pub const NDIS_GFP_HEADER_PRESENT_IPV4: u32 = 2u32;
pub const NDIS_GFP_HEADER_PRESENT_IPV6: u32 = 4u32;
pub const NDIS_GFP_HEADER_PRESENT_IP_IN_GRE_ENCAP: u32 = 256u32;
pub const NDIS_GFP_HEADER_PRESENT_IP_IN_IP_ENCAP: u32 = 128u32;
pub const NDIS_GFP_HEADER_PRESENT_NO_ENCAP: u32 = 64u32;
pub const NDIS_GFP_HEADER_PRESENT_NVGRE_ENCAP: u32 = 512u32;
pub const NDIS_GFP_HEADER_PRESENT_TCP: u32 = 8u32;
pub const NDIS_GFP_HEADER_PRESENT_UDP: u32 = 16u32;
pub const NDIS_GFP_HEADER_PRESENT_VXLAN_ENCAP: u32 = 1024u32;
pub const NDIS_GFP_UNDEFINED_PROFILE_ID: u32 = 0u32;
pub const NDIS_GFP_WILDCARD_MATCH_PROFILE_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_COUNTER_INFO_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_COUNTER_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_COUNTER_PARAMETERS_CLIENT_SPECIFIED_ADDRESS: u32 = 1u32;
pub const NDIS_GFT_COUNTER_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_COUNTER_VALUE_ARRAY_GET_VALUES: u32 = 2u32;
pub const NDIS_GFT_COUNTER_VALUE_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_COUNTER_VALUE_ARRAY_UPDATE_MEMORY_MAPPED_COUNTERS: u32 = 1u32;
pub const NDIS_GFT_CUSTOM_ACTION_LAST_ACTION: u32 = 1u32;
pub const NDIS_GFT_CUSTOM_ACTION_PROFILE_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_CUSTOM_ACTION_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_DELETE_PROFILE_ALL_PROFILES: u32 = 1u32;
pub const NDIS_GFT_DELETE_PROFILE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_DELETE_TABLE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_EMFE_ADD_IN_ACTIVATED_STATE: u32 = 1u32;
pub const NDIS_GFT_EMFE_ALL_VPORT_FLOW_ENTRIES: u32 = 33554432u32;
pub const NDIS_GFT_EMFE_COPY_AFTER_TCP_FIN_FLAG_SET: u32 = 2097152u32;
pub const NDIS_GFT_EMFE_COPY_AFTER_TCP_RST_FLAG_SET: u32 = 4194304u32;
pub const NDIS_GFT_EMFE_COPY_ALL_PACKETS: u32 = 65536u32;
pub const NDIS_GFT_EMFE_COPY_CONDITION_CHANGED: u32 = 16777216u32;
pub const NDIS_GFT_EMFE_COPY_FIRST_PACKET: u32 = 131072u32;
pub const NDIS_GFT_EMFE_COPY_WHEN_TCP_FLAG_SET: u32 = 262144u32;
pub const NDIS_GFT_EMFE_COUNTER_ALLOCATE: u32 = 1u32;
pub const NDIS_GFT_EMFE_COUNTER_CLIENT_SPECIFIED_ADDRESS: u32 = 4u32;
pub const NDIS_GFT_EMFE_COUNTER_MEMORY_MAPPED: u32 = 2u32;
pub const NDIS_GFT_EMFE_COUNTER_TRACK_TCP_FLOW: u32 = 8u32;
pub const NDIS_GFT_EMFE_CUSTOM_ACTION_PRESENT: u32 = 524288u32;
pub const NDIS_GFT_EMFE_MATCH_AND_ACTION_MUST_BE_SUPPORTED: u32 = 2u32;
pub const NDIS_GFT_EMFE_META_ACTION_BEFORE_HEADER_TRANSPOSITION: u32 = 1048576u32;
pub const NDIS_GFT_EMFE_RDMA_FLOW: u32 = 4u32;
pub const NDIS_GFT_EMFE_REDIRECT_TO_EGRESS_QUEUE_OF_VPORT: u32 = 8192u32;
pub const NDIS_GFT_EMFE_REDIRECT_TO_EGRESS_QUEUE_OF_VPORT_IF_TTL_IS_ONE: u32 = 32768u32;
pub const NDIS_GFT_EMFE_REDIRECT_TO_INGRESS_QUEUE_OF_VPORT: u32 = 4096u32;
pub const NDIS_GFT_EMFE_REDIRECT_TO_INGRESS_QUEUE_OF_VPORT_IF_TTL_IS_ONE: u32 = 16384u32;
pub const NDIS_GFT_EXACT_MATCH_FLOW_ENTRY_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_FLOW_ENTRY_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_FLOW_ENTRY_ID_ALL_NIC_SWITCH_FLOW_ENTRIES: u32 = 1u32;
pub const NDIS_GFT_FLOW_ENTRY_ID_ALL_TABLE_FLOW_ENTRIES: u32 = 2u32;
pub const NDIS_GFT_FLOW_ENTRY_ID_ALL_VPORT_FLOW_ENTRIES: u32 = 4u32;
pub const NDIS_GFT_FLOW_ENTRY_ID_ARRAY_COUNTER_VALUES: u32 = 65536u32;
pub const NDIS_GFT_FLOW_ENTRY_ID_ARRAY_DEFINED: u32 = 16u32;
pub const NDIS_GFT_FLOW_ENTRY_ID_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_FLOW_ENTRY_ID_RANGE_DEFINED: u32 = 8u32;
pub const NDIS_GFT_FLOW_ENTRY_INFO_ALL_FLOW_ENTRIES: u32 = 1u32;
pub const NDIS_GFT_FLOW_ENTRY_INFO_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_FREE_COUNTER_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_HEADER_GROUP_TRANSPOSITION_DECREMENT_TTL_IF_NOT_ONE: u32 = 1u32;
pub const NDIS_GFT_HEADER_GROUP_TRANSPOSITION_PROFILE_DECREMENT_TTL_IF_NOT_ONE: u32 = 1u32;
pub const NDIS_GFT_HEADER_GROUP_TRANSPOSITION_PROFILE_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_HEADER_GROUP_TRANSPOSITION_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_HEADER_TRANSPOSITION_PROFILE_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_HTP_COPY_ALL_PACKETS: u32 = 16u32;
pub const NDIS_GFT_HTP_COPY_FIRST_PACKET: u32 = 32u32;
pub const NDIS_GFT_HTP_COPY_WHEN_TCP_FLAG_SET: u32 = 64u32;
pub const NDIS_GFT_HTP_CUSTOM_ACTION_PRESENT: u32 = 128u32;
pub const NDIS_GFT_HTP_META_ACTION_BEFORE_HEADER_TRANSPOSITION: u32 = 256u32;
pub const NDIS_GFT_HTP_REDIRECT_TO_EGRESS_QUEUE_OF_VPORT: u32 = 2u32;
pub const NDIS_GFT_HTP_REDIRECT_TO_EGRESS_QUEUE_OF_VPORT_IF_TTL_IS_ONE: u32 = 8u32;
pub const NDIS_GFT_HTP_REDIRECT_TO_INGRESS_QUEUE_OF_VPORT: u32 = 1u32;
pub const NDIS_GFT_HTP_REDIRECT_TO_INGRESS_QUEUE_OF_VPORT_IF_TTL_IS_ONE: u32 = 4u32;
pub const NDIS_GFT_MAX_COUNTER_OBJECTS_PER_FLOW_ENTRY: u32 = 8u32;
pub const NDIS_GFT_OFFLOAD_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_OFFLOAD_CAPS_8021P_PRIORITY_MASK: u32 = 131072u32;
pub const NDIS_GFT_OFFLOAD_CAPS_ADD_FLOW_ENTRY_DEACTIVATED_PREFERRED: u32 = 1u32;
pub const NDIS_GFT_OFFLOAD_CAPS_ALLOW: u32 = 262144u32;
pub const NDIS_GFT_OFFLOAD_CAPS_CLIENT_SPECIFIED_MEMORY_MAPPED_COUNTERS: u32 = 16u32;
pub const NDIS_GFT_OFFLOAD_CAPS_COMBINED_COUNTER_AND_STATE: u32 = 256u32;
pub const NDIS_GFT_OFFLOAD_CAPS_COPY_ALL: u32 = 256u32;
pub const NDIS_GFT_OFFLOAD_CAPS_COPY_FIRST: u32 = 512u32;
pub const NDIS_GFT_OFFLOAD_CAPS_COPY_WHEN_TCP_FLAG_SET: u32 = 1024u32;
pub const NDIS_GFT_OFFLOAD_CAPS_DESIGNATED_EXCEPTION_VPORT: u32 = 32768u32;
pub const NDIS_GFT_OFFLOAD_CAPS_DROP: u32 = 524288u32;
pub const NDIS_GFT_OFFLOAD_CAPS_DSCP_MASK: u32 = 65536u32;
pub const NDIS_GFT_OFFLOAD_CAPS_EGRESS_AGGREGATE_COUNTERS: u32 = 64u32;
pub const NDIS_GFT_OFFLOAD_CAPS_EGRESS_EXACT_MATCH: u32 = 8u32;
pub const NDIS_GFT_OFFLOAD_CAPS_EGRESS_WILDCARD_MATCH: u32 = 2u32;
pub const NDIS_GFT_OFFLOAD_CAPS_EXT_VPORT_EGRESS_EXACT_MATCH: u32 = 128u32;
pub const NDIS_GFT_OFFLOAD_CAPS_EXT_VPORT_EGRESS_WILDCARD_MATCH: u32 = 32u32;
pub const NDIS_GFT_OFFLOAD_CAPS_EXT_VPORT_INGRESS_EXACT_MATCH: u32 = 64u32;
pub const NDIS_GFT_OFFLOAD_CAPS_EXT_VPORT_INGRESS_WILDCARD_MATCH: u32 = 16u32;
pub const NDIS_GFT_OFFLOAD_CAPS_IGNORE_ACTION_SUPPORTED: u32 = 8u32;
pub const NDIS_GFT_OFFLOAD_CAPS_INGRESS_AGGREGATE_COUNTERS: u32 = 32u32;
pub const NDIS_GFT_OFFLOAD_CAPS_INGRESS_EXACT_MATCH: u32 = 4u32;
pub const NDIS_GFT_OFFLOAD_CAPS_INGRESS_WILDCARD_MATCH: u32 = 1u32;
pub const NDIS_GFT_OFFLOAD_CAPS_MEMORY_MAPPED_COUNTERS: u32 = 1u32;
pub const NDIS_GFT_OFFLOAD_CAPS_MEMORY_MAPPED_PAKCET_AND_BYTE_COUNTERS: u32 = 2u32;
pub const NDIS_GFT_OFFLOAD_CAPS_META_ACTION_AFTER_HEADER_TRANSPOSITION: u32 = 8192u32;
pub const NDIS_GFT_OFFLOAD_CAPS_META_ACTION_BEFORE_HEADER_TRANSPOSITION: u32 = 4096u32;
pub const NDIS_GFT_OFFLOAD_CAPS_MODIFY: u32 = 4u32;
pub const NDIS_GFT_OFFLOAD_CAPS_PER_FLOW_ENTRY_COUNTERS: u32 = 4u32;
pub const NDIS_GFT_OFFLOAD_CAPS_PER_PACKET_COUNTER_UPDATE: u32 = 8u32;
pub const NDIS_GFT_OFFLOAD_CAPS_PER_VPORT_EXCEPTION_VPORT: u32 = 16384u32;
pub const NDIS_GFT_OFFLOAD_CAPS_POP: u32 = 1u32;
pub const NDIS_GFT_OFFLOAD_CAPS_PUSH: u32 = 2u32;
pub const NDIS_GFT_OFFLOAD_CAPS_RATE_LIMITING_QUEUE_SUPPORTED: u32 = 2u32;
pub const NDIS_GFT_OFFLOAD_CAPS_REDIRECT_TO_EGRESS_QUEUE_OF_VPORT: u32 = 32u32;
pub const NDIS_GFT_OFFLOAD_CAPS_REDIRECT_TO_EGRESS_QUEUE_OF_VPORT_IF_TTL_IS_ONE: u32 = 128u32;
pub const NDIS_GFT_OFFLOAD_CAPS_REDIRECT_TO_INGRESS_QUEUE_OF_VPORT: u32 = 16u32;
pub const NDIS_GFT_OFFLOAD_CAPS_REDIRECT_TO_INGRESS_QUEUE_OF_VPORT_IF_TTL_IS_ONE: u32 = 64u32;
pub const NDIS_GFT_OFFLOAD_CAPS_SAMPLE: u32 = 2048u32;
pub const NDIS_GFT_OFFLOAD_CAPS_TRACK_TCP_FLOW_STATE: u32 = 128u32;
pub const NDIS_GFT_OFFLOAD_PARAMETERS_CUSTOM_PROVIDER_RESERVED: u32 = 4278190080u32;
pub const NDIS_GFT_OFFLOAD_PARAMETERS_ENABLE_OFFLOAD: u32 = 1u32;
pub const NDIS_GFT_OFFLOAD_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_PROFILE_INFO_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_PROFILE_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_RESERVED_CUSTOM_ACTIONS: u32 = 256u32;
pub const NDIS_GFT_STATISTICS_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_TABLE_INCLUDE_EXTERNAL_VPPORT: u32 = 1u32;
pub const NDIS_GFT_TABLE_INFO_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_TABLE_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_TABLE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_UNDEFINED_COUNTER_ID: u32 = 0u32;
pub const NDIS_GFT_UNDEFINED_CUSTOM_ACTION: u32 = 0u32;
pub const NDIS_GFT_UNDEFINED_FLOW_ENTRY_ID: u32 = 0u32;
pub const NDIS_GFT_UNDEFINED_TABLE_ID: u32 = 0u32;
pub const NDIS_GFT_VPORT_DSCP_FLAGS_CHANGED: u32 = 67108864u32;
pub const NDIS_GFT_VPORT_DSCP_GUARD_ENABLE_RX: u32 = 1u32;
pub const NDIS_GFT_VPORT_DSCP_GUARD_ENABLE_TX: u32 = 2u32;
pub const NDIS_GFT_VPORT_DSCP_MASK_CHANGED: u32 = 8388608u32;
pub const NDIS_GFT_VPORT_DSCP_MASK_ENABLE_RX: u32 = 4u32;
pub const NDIS_GFT_VPORT_DSCP_MASK_ENABLE_TX: u32 = 8u32;
pub const NDIS_GFT_VPORT_ENABLE: u32 = 1u32;
pub const NDIS_GFT_VPORT_ENABLE_STATE_CHANGED: u32 = 1048576u32;
pub const NDIS_GFT_VPORT_EXCEPTION_VPORT_CHANGED: u32 = 2097152u32;
pub const NDIS_GFT_VPORT_MAX_DSCP_MASK_COUNTER_OBJECTS: u32 = 64u32;
pub const NDIS_GFT_VPORT_MAX_PRIORITY_MASK_COUNTER_OBJECTS: u32 = 8u32;
pub const NDIS_GFT_VPORT_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_GFT_VPORT_PARAMS_CHANGE_MASK: u32 = 4293918720u32;
pub const NDIS_GFT_VPORT_PARAMS_CUSTOM_PROVIDER_RESERVED: u32 = 1044480u32;
pub const NDIS_GFT_VPORT_PARSE_VXLAN: u32 = 2u32;
pub const NDIS_GFT_VPORT_PARSE_VXLAN_NOT_IN_SRC_PORT_RANGE: u32 = 4u32;
pub const NDIS_GFT_VPORT_PRIORITY_MASK_CHANGED: u32 = 16777216u32;
pub const NDIS_GFT_VPORT_SAMPLING_RATE_CHANGED: u32 = 4194304u32;
pub const NDIS_GFT_VPORT_VXLAN_SETTINGS_CHANGED: u32 = 33554432u32;
pub const NDIS_GFT_WCFE_ADD_IN_ACTIVATED_STATE: u32 = 1u32;
pub const NDIS_GFT_WCFE_COPY_ALL_PACKETS: u32 = 32u32;
pub const NDIS_GFT_WCFE_COUNTER_ALLOCATE: u32 = 1u32;
pub const NDIS_GFT_WCFE_COUNTER_CLIENT_SPECIFIED_ADDRESS: u32 = 4u32;
pub const NDIS_GFT_WCFE_COUNTER_MEMORY_MAPPED: u32 = 2u32;
pub const NDIS_GFT_WCFE_CUSTOM_ACTION_PRESENT: u32 = 64u32;
pub const NDIS_GFT_WCFE_REDIRECT_TO_EGRESS_QUEUE_OF_VPORT: u32 = 4u32;
pub const NDIS_GFT_WCFE_REDIRECT_TO_EGRESS_QUEUE_OF_VPORT_IF_TTL_IS_ONE: u32 = 16u32;
pub const NDIS_GFT_WCFE_REDIRECT_TO_INGRESS_QUEUE_OF_VPORT: u32 = 2u32;
pub const NDIS_GFT_WCFE_REDIRECT_TO_INGRESS_QUEUE_OF_VPORT_IF_TTL_IS_ONE: u32 = 8u32;
pub const NDIS_GFT_WILDCARD_MATCH_FLOW_ENTRY_REVISION_1: u32 = 1u32;
pub const NDIS_HARDWARE_CROSSTIMESTAMP_REVISION_1: u32 = 1u32;
pub const NDIS_HASH_FUNCTION_MASK: u32 = 255u32;
pub const NDIS_HASH_IPV4: u32 = 256u32;
pub const NDIS_HASH_IPV6: u32 = 1024u32;
pub const NDIS_HASH_IPV6_EX: u32 = 2048u32;
pub const NDIS_HASH_TCP_IPV4: u32 = 512u32;
pub const NDIS_HASH_TCP_IPV6: u32 = 4096u32;
pub const NDIS_HASH_TCP_IPV6_EX: u32 = 8192u32;
pub const NDIS_HASH_TYPE_MASK: u32 = 16776960u32;
pub const NDIS_HASH_UDP_IPV4: u32 = 16384u32;
pub const NDIS_HASH_UDP_IPV6: u32 = 32768u32;
pub const NDIS_HASH_UDP_IPV6_EX: u32 = 65536u32;
pub const NDIS_HD_SPLIT_CAPS_SUPPORTS_HEADER_DATA_SPLIT: u32 = 1u32;
pub const NDIS_HD_SPLIT_CAPS_SUPPORTS_IPV4_OPTIONS: u32 = 2u32;
pub const NDIS_HD_SPLIT_CAPS_SUPPORTS_IPV6_EXTENSION_HEADERS: u32 = 4u32;
pub const NDIS_HD_SPLIT_CAPS_SUPPORTS_TCP_OPTIONS: u32 = 8u32;
pub const NDIS_HD_SPLIT_COMBINE_ALL_HEADERS: u32 = 1u32;
pub const NDIS_HD_SPLIT_CURRENT_CONFIG_REVISION_1: u32 = 1u32;
pub const NDIS_HD_SPLIT_ENABLE_HEADER_DATA_SPLIT: u32 = 1u32;
pub const NDIS_HD_SPLIT_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_HYPERVISOR_INFO_FLAG_HYPERVISOR_PRESENT: u32 = 1u32;
pub const NDIS_HYPERVISOR_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_INTERRUPT_MODERATION_CHANGE_NEEDS_REINITIALIZE: u32 = 2u32;
pub const NDIS_INTERRUPT_MODERATION_CHANGE_NEEDS_RESET: u32 = 1u32;
pub const NDIS_INTERRUPT_MODERATION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_IP_OPER_STATE_REVISION_1: u32 = 1u32;
pub const NDIS_IP_OPER_STATUS_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_ISOLATION_NAME_MAX_STRING_SIZE: u32 = 127u32;
pub const NDIS_ISOLATION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_LINK_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_LINK_STATE_DUPLEX_AUTO_NEGOTIATED: u32 = 4u32;
pub const NDIS_LINK_STATE_PAUSE_FUNCTIONS_AUTO_NEGOTIATED: u32 = 8u32;
pub const NDIS_LINK_STATE_RCV_LINK_SPEED_AUTO_NEGOTIATED: u32 = 2u32;
pub const NDIS_LINK_STATE_REVISION_1: u32 = 1u32;
pub const NDIS_LINK_STATE_XMIT_LINK_SPEED_AUTO_NEGOTIATED: u32 = 1u32;
pub const NDIS_MAC_OPTION_8021P_PRIORITY: u32 = 64u32;
pub const NDIS_MAC_OPTION_8021Q_VLAN: u32 = 512u32;
pub const NDIS_MAC_OPTION_COPY_LOOKAHEAD_DATA: u32 = 1u32;
pub const NDIS_MAC_OPTION_EOTX_INDICATION: u32 = 32u32;
pub const NDIS_MAC_OPTION_FULL_DUPLEX: u32 = 16u32;
pub const NDIS_MAC_OPTION_NO_LOOPBACK: u32 = 8u32;
pub const NDIS_MAC_OPTION_RECEIVE_AT_DPC: u32 = 256u32;
pub const NDIS_MAC_OPTION_RECEIVE_SERIALIZED: u32 = 2u32;
pub const NDIS_MAC_OPTION_RESERVED: u32 = 2147483648u32;
pub const NDIS_MAC_OPTION_SUPPORTS_MAC_ADDRESS_OVERWRITE: u32 = 128u32;
pub const NDIS_MAC_OPTION_TRANSFERS_NOT_PEND: u32 = 4u32;
pub const NDIS_MAXIMUM_PORTS: u32 = 16777216u32;
pub const NDIS_MEDIA_CAP_RECEIVE: u32 = 2u32;
pub const NDIS_MEDIA_CAP_TRANSMIT: u32 = 1u32;
pub const NDIS_NDK_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_NDK_CONNECTIONS_REVISION_1: u32 = 1u32;
pub const NDIS_NDK_LOCAL_ENDPOINTS_REVISION_1: u32 = 1u32;
pub const NDIS_NDK_STATISTICS_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_CAPABILITIES_REVISION_2: u32 = 2u32;
pub const NDIS_NIC_SWITCH_CAPABILITIES_REVISION_3: u32 = 3u32;
pub const NDIS_NIC_SWITCH_CAPS_ASYMMETRIC_QUEUE_PAIRS_FOR_NONDEFAULT_VPORT_SUPPORTED: u32 = 4u32;
pub const NDIS_NIC_SWITCH_CAPS_NIC_SWITCH_WITHOUT_IOV_SUPPORTED: u32 = 64u32;
pub const NDIS_NIC_SWITCH_CAPS_PER_VPORT_INTERRUPT_MODERATION_SUPPORTED: u32 = 2u32;
pub const NDIS_NIC_SWITCH_CAPS_RSS_ON_PF_VPORTS_SUPPORTED: u32 = 128u32;
pub const NDIS_NIC_SWITCH_CAPS_RSS_PARAMETERS_PER_PF_VPORT_SUPPORTED: u32 = 32u32;
pub const NDIS_NIC_SWITCH_CAPS_RSS_PER_PF_VPORT_HASH_FUNCTION_SUPPORTED: u32 = 512u32;
pub const NDIS_NIC_SWITCH_CAPS_RSS_PER_PF_VPORT_HASH_KEY_SUPPORTED: u32 = 2048u32;
pub const NDIS_NIC_SWITCH_CAPS_RSS_PER_PF_VPORT_HASH_TYPE_SUPPORTED: u32 = 1024u32;
pub const NDIS_NIC_SWITCH_CAPS_RSS_PER_PF_VPORT_INDIRECTION_TABLE_SIZE_RESTRICTED: u32 = 4096u32;
pub const NDIS_NIC_SWITCH_CAPS_RSS_PER_PF_VPORT_INDIRECTION_TABLE_SUPPORTED: u32 = 256u32;
pub const NDIS_NIC_SWITCH_CAPS_SINGLE_VPORT_POOL: u32 = 16u32;
pub const NDIS_NIC_SWITCH_CAPS_VF_RSS_SUPPORTED: u32 = 8u32;
pub const NDIS_NIC_SWITCH_CAPS_VLAN_SUPPORTED: u32 = 1u32;
pub const NDIS_NIC_SWITCH_DELETE_SWITCH_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_DELETE_VPORT_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_FREE_VF_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_INFO_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_PARAMETERS_CHANGE_MASK: u32 = 4294901760u32;
pub const NDIS_NIC_SWITCH_PARAMETERS_DEFAULT_NUMBER_OF_QUEUE_PAIRS_FOR_DEFAULT_VPORT: u32 = 1u32;
pub const NDIS_NIC_SWITCH_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_NIC_SWITCH_PARAMETERS_SWITCH_NAME_CHANGED: u32 = 65536u32;
pub const NDIS_NIC_SWITCH_VF_INFO_ARRAY_ENUM_ON_SPECIFIC_SWITCH: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VF_INFO_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VF_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VF_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VPORT_INFO_ARRAY_ENUM_ON_SPECIFIC_FUNCTION: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VPORT_INFO_ARRAY_ENUM_ON_SPECIFIC_SWITCH: u32 = 2u32;
pub const NDIS_NIC_SWITCH_VPORT_INFO_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VPORT_INFO_GFT_ENABLED: u32 = 4u32;
pub const NDIS_NIC_SWITCH_VPORT_INFO_LOOKAHEAD_SPLIT_ENABLED: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VPORT_INFO_PACKET_DIRECT_RX_ONLY: u32 = 2u32;
pub const NDIS_NIC_SWITCH_VPORT_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_CHANGE_MASK: u32 = 4294901760u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_ENFORCE_MAX_SG_LIST: u32 = 32768u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_FLAGS_CHANGED: u32 = 65536u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_INT_MOD_CHANGED: u32 = 262144u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_LOOKAHEAD_SPLIT_ENABLED: u32 = 1u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_NAME_CHANGED: u32 = 131072u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_NDK_PARAMS_CHANGED: u32 = 2097152u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_NUM_QUEUE_PAIRS_CHANGED: u32 = 8388608u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_PACKET_DIRECT_RX_ONLY: u32 = 2u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_PROCESSOR_AFFINITY_CHANGED: u32 = 1048576u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_QOS_SQ_ID_CHANGED: u32 = 4194304u32;
pub const NDIS_NIC_SWITCH_VPORT_PARAMS_STATE_CHANGED: u32 = 524288u32;
pub const NDIS_OBJECT_REVISION_1: u32 = 1u32;
pub const NDIS_OBJECT_TYPE_BIND_PARAMETERS: u32 = 134u32;
pub const NDIS_OBJECT_TYPE_CLIENT_CHIMNEY_OFFLOAD_CHARACTERISTICS: u32 = 147u32;
pub const NDIS_OBJECT_TYPE_CLIENT_CHIMNEY_OFFLOAD_GENERIC_CHARACTERISTICS: u32 = 142u32;
pub const NDIS_OBJECT_TYPE_CONFIGURATION_OBJECT: u32 = 169u32;
pub const NDIS_OBJECT_TYPE_CO_CALL_MANAGER_OPTIONAL_HANDLERS: u32 = 165u32;
pub const NDIS_OBJECT_TYPE_CO_CLIENT_OPTIONAL_HANDLERS: u32 = 166u32;
pub const NDIS_OBJECT_TYPE_CO_MINIPORT_CHARACTERISTICS: u32 = 145u32;
pub const NDIS_OBJECT_TYPE_CO_PROTOCOL_CHARACTERISTICS: u32 = 144u32;
pub const NDIS_OBJECT_TYPE_DEFAULT: u32 = 128u32;
pub const NDIS_OBJECT_TYPE_DEVICE_OBJECT_ATTRIBUTES: u32 = 133u32;
pub const NDIS_OBJECT_TYPE_DRIVER_WRAPPER_OBJECT: u32 = 170u32;
pub const NDIS_OBJECT_TYPE_FILTER_ATTACH_PARAMETERS: u32 = 153u32;
pub const NDIS_OBJECT_TYPE_FILTER_ATTRIBUTES: u32 = 141u32;
pub const NDIS_OBJECT_TYPE_FILTER_DRIVER_CHARACTERISTICS: u32 = 139u32;
pub const NDIS_OBJECT_TYPE_FILTER_PARTIAL_CHARACTERISTICS: u32 = 140u32;
pub const NDIS_OBJECT_TYPE_FILTER_PAUSE_PARAMETERS: u32 = 154u32;
pub const NDIS_OBJECT_TYPE_FILTER_RESTART_PARAMETERS: u32 = 155u32;
pub const NDIS_OBJECT_TYPE_HD_SPLIT_ATTRIBUTES: u32 = 171u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_ADAPTER_GENERAL_ATTRIBUTES: u32 = 159u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_ADAPTER_HARDWARE_ASSIST_ATTRIBUTES: u32 = 175u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_ADAPTER_NATIVE_802_11_ATTRIBUTES: u32 = 161u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_ADAPTER_NDK_ATTRIBUTES: u32 = 179u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_ADAPTER_OFFLOAD_ATTRIBUTES: u32 = 160u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_ADAPTER_PACKET_DIRECT_ATTRIBUTES: u32 = 197u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_ADAPTER_REGISTRATION_ATTRIBUTES: u32 = 158u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_ADD_DEVICE_REGISTRATION_ATTRIBUTES: u32 = 164u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_DEVICE_POWER_NOTIFICATION: u32 = 198u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_DRIVER_CHARACTERISTICS: u32 = 138u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_INIT_PARAMETERS: u32 = 129u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_INTERRUPT: u32 = 132u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_PNP_CHARACTERISTICS: u32 = 146u32;
pub const NDIS_OBJECT_TYPE_MINIPORT_SS_CHARACTERISTICS: u32 = 180u32;
pub const NDIS_OBJECT_TYPE_NDK_PROVIDER_CHARACTERISTICS: u32 = 178u32;
pub const NDIS_OBJECT_TYPE_NSI_COMPARTMENT_RW_STRUCT: u32 = 173u32;
pub const NDIS_OBJECT_TYPE_NSI_INTERFACE_PERSIST_RW_STRUCT: u32 = 174u32;
pub const NDIS_OBJECT_TYPE_NSI_NETWORK_RW_STRUCT: u32 = 172u32;
pub const NDIS_OBJECT_TYPE_OFFLOAD: u32 = 167u32;
pub const NDIS_OBJECT_TYPE_OFFLOAD_ENCAPSULATION: u32 = 168u32;
pub const NDIS_OBJECT_TYPE_OID_REQUEST: u32 = 150u32;
pub const NDIS_OBJECT_TYPE_OPEN_PARAMETERS: u32 = 135u32;
pub const NDIS_OBJECT_TYPE_PCI_DEVICE_CUSTOM_PROPERTIES_REVISION_1: u32 = 1u32;
pub const NDIS_OBJECT_TYPE_PCI_DEVICE_CUSTOM_PROPERTIES_REVISION_2: u32 = 2u32;
pub const NDIS_OBJECT_TYPE_PD_RECEIVE_QUEUE: u32 = 191u32;
pub const NDIS_OBJECT_TYPE_PD_TRANSMIT_QUEUE: u32 = 190u32;
pub const NDIS_OBJECT_TYPE_PORT_CHARACTERISTICS: u32 = 156u32;
pub const NDIS_OBJECT_TYPE_PORT_STATE: u32 = 157u32;
pub const NDIS_OBJECT_TYPE_PROTOCOL_DRIVER_CHARACTERISTICS: u32 = 149u32;
pub const NDIS_OBJECT_TYPE_PROTOCOL_RESTART_PARAMETERS: u32 = 163u32;
pub const NDIS_OBJECT_TYPE_PROVIDER_CHIMNEY_OFFLOAD_CHARACTERISTICS: u32 = 148u32;
pub const NDIS_OBJECT_TYPE_PROVIDER_CHIMNEY_OFFLOAD_GENERIC_CHARACTERISTICS: u32 = 143u32;
pub const NDIS_OBJECT_TYPE_QOS_CAPABILITIES: u32 = 181u32;
pub const NDIS_OBJECT_TYPE_QOS_CLASSIFICATION_ELEMENT: u32 = 183u32;
pub const NDIS_OBJECT_TYPE_QOS_PARAMETERS: u32 = 182u32;
pub const NDIS_OBJECT_TYPE_REQUEST_EX: u32 = 150u32;
pub const NDIS_OBJECT_TYPE_RESTART_GENERAL_ATTRIBUTES: u32 = 162u32;
pub const NDIS_OBJECT_TYPE_RSS_CAPABILITIES: u32 = 136u32;
pub const NDIS_OBJECT_TYPE_RSS_PARAMETERS: u32 = 137u32;
pub const NDIS_OBJECT_TYPE_RSS_PARAMETERS_V2: u32 = 200u32;
pub const NDIS_OBJECT_TYPE_RSS_PROCESSOR_INFO: u32 = 177u32;
pub const NDIS_OBJECT_TYPE_RSS_SET_INDIRECTION_ENTRIES: u32 = 201u32;
pub const NDIS_OBJECT_TYPE_SG_DMA_DESCRIPTION: u32 = 131u32;
pub const NDIS_OBJECT_TYPE_SHARED_MEMORY_PROVIDER_CHARACTERISTICS: u32 = 176u32;
pub const NDIS_OBJECT_TYPE_STATUS_INDICATION: u32 = 152u32;
pub const NDIS_OBJECT_TYPE_SWITCH_OPTIONAL_HANDLERS: u32 = 184u32;
pub const NDIS_OBJECT_TYPE_TIMER_CHARACTERISTICS: u32 = 151u32;
pub const NDIS_OFFLOAD_FLAGS_GROUP_CHECKSUM_CAPABILITIES: u32 = 1u32;
pub const NDIS_OFFLOAD_NOT_SUPPORTED: u32 = 0u32;
pub const NDIS_OFFLOAD_PARAMETERS_CONNECTION_OFFLOAD_DISABLED: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_CONNECTION_OFFLOAD_ENABLED: u32 = 2u32;
pub const NDIS_OFFLOAD_PARAMETERS_IPSECV1_AH_AND_ESP_ENABLED: u32 = 4u32;
pub const NDIS_OFFLOAD_PARAMETERS_IPSECV1_AH_ENABLED: u32 = 2u32;
pub const NDIS_OFFLOAD_PARAMETERS_IPSECV1_DISABLED: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_IPSECV1_ESP_ENABLED: u32 = 3u32;
pub const NDIS_OFFLOAD_PARAMETERS_IPSECV2_AH_AND_ESP_ENABLED: u32 = 4u32;
pub const NDIS_OFFLOAD_PARAMETERS_IPSECV2_AH_ENABLED: u32 = 2u32;
pub const NDIS_OFFLOAD_PARAMETERS_IPSECV2_DISABLED: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_IPSECV2_ESP_ENABLED: u32 = 3u32;
pub const NDIS_OFFLOAD_PARAMETERS_LSOV1_DISABLED: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_LSOV1_ENABLED: u32 = 2u32;
pub const NDIS_OFFLOAD_PARAMETERS_LSOV2_DISABLED: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_LSOV2_ENABLED: u32 = 2u32;
pub const NDIS_OFFLOAD_PARAMETERS_NO_CHANGE: u32 = 0u32;
pub const NDIS_OFFLOAD_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_OFFLOAD_PARAMETERS_REVISION_3: u32 = 3u32;
pub const NDIS_OFFLOAD_PARAMETERS_REVISION_4: u32 = 4u32;
pub const NDIS_OFFLOAD_PARAMETERS_REVISION_5: u32 = 5u32;
pub const NDIS_OFFLOAD_PARAMETERS_RSC_DISABLED: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_RSC_ENABLED: u32 = 2u32;
pub const NDIS_OFFLOAD_PARAMETERS_RX_ENABLED_TX_DISABLED: u32 = 3u32;
pub const NDIS_OFFLOAD_PARAMETERS_SKIP_REGISTRY_UPDATE: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_TX_ENABLED_RX_DISABLED: u32 = 2u32;
pub const NDIS_OFFLOAD_PARAMETERS_TX_RX_DISABLED: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_TX_RX_ENABLED: u32 = 4u32;
pub const NDIS_OFFLOAD_PARAMETERS_USO_DISABLED: u32 = 1u32;
pub const NDIS_OFFLOAD_PARAMETERS_USO_ENABLED: u32 = 2u32;
pub const NDIS_OFFLOAD_REVISION_1: u32 = 1u32;
pub const NDIS_OFFLOAD_REVISION_2: u32 = 2u32;
pub const NDIS_OFFLOAD_REVISION_3: u32 = 3u32;
pub const NDIS_OFFLOAD_REVISION_4: u32 = 4u32;
pub const NDIS_OFFLOAD_REVISION_5: u32 = 5u32;
pub const NDIS_OFFLOAD_REVISION_6: u32 = 6u32;
pub const NDIS_OFFLOAD_REVISION_7: u32 = 7u32;
pub const NDIS_OFFLOAD_SET_NO_CHANGE: u32 = 0u32;
pub const NDIS_OFFLOAD_SET_OFF: u32 = 2u32;
pub const NDIS_OFFLOAD_SET_ON: u32 = 1u32;
pub const NDIS_OFFLOAD_SUPPORTED: u32 = 1u32;
pub const NDIS_OPER_STATE_REVISION_1: u32 = 1u32;
pub const NDIS_PACKET_TYPE_ALL_FUNCTIONAL: u32 = 8192u32;
pub const NDIS_PACKET_TYPE_ALL_LOCAL: u32 = 128u32;
pub const NDIS_PACKET_TYPE_ALL_MULTICAST: u32 = 4u32;
pub const NDIS_PACKET_TYPE_BROADCAST: u32 = 8u32;
pub const NDIS_PACKET_TYPE_DIRECTED: u32 = 1u32;
pub const NDIS_PACKET_TYPE_FUNCTIONAL: u32 = 16384u32;
pub const NDIS_PACKET_TYPE_GROUP: u32 = 4096u32;
pub const NDIS_PACKET_TYPE_MAC_FRAME: u32 = 32768u32;
pub const NDIS_PACKET_TYPE_MULTICAST: u32 = 2u32;
pub const NDIS_PACKET_TYPE_NO_LOCAL: u32 = 65536u32;
pub const NDIS_PACKET_TYPE_PROMISCUOUS: u32 = 32u32;
pub const NDIS_PACKET_TYPE_SMT: u32 = 64u32;
pub const NDIS_PACKET_TYPE_SOURCE_ROUTING: u32 = 16u32;
pub const NDIS_PD_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_PD_CAPS_DRAIN_NOTIFICATIONS_SUPPORTED: u32 = 2u32;
pub const NDIS_PD_CAPS_NOTIFICATION_MODERATION_COUNT_SUPPORTED: u32 = 8u32;
pub const NDIS_PD_CAPS_NOTIFICATION_MODERATION_INTERVAL_SUPPORTED: u32 = 4u32;
pub const NDIS_PD_CAPS_RECEIVE_FILTER_COUNTERS_SUPPORTED: u32 = 1u32;
pub const NDIS_PD_CONFIG_REVISION_1: u32 = 1u32;
pub const NDIS_PM_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_PM_CAPABILITIES_REVISION_2: u32 = 2u32;
pub const NDIS_PM_MAX_PATTERN_ID: u32 = 65535u32;
pub const NDIS_PM_MAX_STRING_SIZE: u32 = 64u32;
pub const NDIS_PM_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_PM_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_PM_PRIVATE_PATTERN_ID: u32 = 1u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_80211_RSN_REKEY_ENABLED: u32 = 128u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_80211_RSN_REKEY_SUPPORTED: u32 = 128u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_ARP_ENABLED: u32 = 1u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_ARP_SUPPORTED: u32 = 1u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_NS_ENABLED: u32 = 2u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_NS_SUPPORTED: u32 = 2u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_PRIORITY_HIGHEST: u32 = 1u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_PRIORITY_LOWEST: u32 = 4294967295u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_PRIORITY_NORMAL: u32 = 268435456u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_REVISION_1: u32 = 1u32;
pub const NDIS_PM_PROTOCOL_OFFLOAD_REVISION_2: u32 = 2u32;
pub const NDIS_PM_SELECTIVE_SUSPEND_ENABLED: u32 = 16u32;
pub const NDIS_PM_SELECTIVE_SUSPEND_SUPPORTED: u32 = 2u32;
pub const NDIS_PM_WAKE_ON_LINK_CHANGE_ENABLED: u32 = 1u32;
pub const NDIS_PM_WAKE_ON_MEDIA_CONNECT_SUPPORTED: u32 = 1u32;
pub const NDIS_PM_WAKE_ON_MEDIA_DISCONNECT_ENABLED: u32 = 2u32;
pub const NDIS_PM_WAKE_ON_MEDIA_DISCONNECT_SUPPORTED: u32 = 2u32;
pub const NDIS_PM_WAKE_PACKET_INDICATION_SUPPORTED: u32 = 1u32;
pub const NDIS_PM_WAKE_PACKET_REVISION_1: u32 = 1u32;
pub const NDIS_PM_WAKE_REASON_REVISION_1: u32 = 1u32;
pub const NDIS_PM_WOL_BITMAP_PATTERN_ENABLED: u32 = 1u32;
pub const NDIS_PM_WOL_BITMAP_PATTERN_SUPPORTED: u32 = 1u32;
pub const NDIS_PM_WOL_EAPOL_REQUEST_ID_MESSAGE_ENABLED: u32 = 65536u32;
pub const NDIS_PM_WOL_EAPOL_REQUEST_ID_MESSAGE_SUPPORTED: u32 = 65536u32;
pub const NDIS_PM_WOL_IPV4_DEST_ADDR_WILDCARD_ENABLED: u32 = 512u32;
pub const NDIS_PM_WOL_IPV4_DEST_ADDR_WILDCARD_SUPPORTED: u32 = 512u32;
pub const NDIS_PM_WOL_IPV4_TCP_SYN_ENABLED: u32 = 4u32;
pub const NDIS_PM_WOL_IPV4_TCP_SYN_SUPPORTED: u32 = 4u32;
pub const NDIS_PM_WOL_IPV6_DEST_ADDR_WILDCARD_ENABLED: u32 = 2048u32;
pub const NDIS_PM_WOL_IPV6_DEST_ADDR_WILDCARD_SUPPORTED: u32 = 2048u32;
pub const NDIS_PM_WOL_IPV6_TCP_SYN_ENABLED: u32 = 8u32;
pub const NDIS_PM_WOL_IPV6_TCP_SYN_SUPPORTED: u32 = 8u32;
pub const NDIS_PM_WOL_MAGIC_PACKET_ENABLED: u32 = 2u32;
pub const NDIS_PM_WOL_MAGIC_PACKET_SUPPORTED: u32 = 2u32;
pub const NDIS_PM_WOL_PATTERN_REVISION_1: u32 = 1u32;
pub const NDIS_PM_WOL_PATTERN_REVISION_2: u32 = 2u32;
pub const NDIS_PM_WOL_PRIORITY_HIGHEST: u32 = 1u32;
pub const NDIS_PM_WOL_PRIORITY_LOWEST: u32 = 4294967295u32;
pub const NDIS_PM_WOL_PRIORITY_NORMAL: u32 = 268435456u32;
pub const NDIS_PNP_WAKE_UP_LINK_CHANGE: u32 = 4u32;
pub const NDIS_PNP_WAKE_UP_MAGIC_PACKET: u32 = 1u32;
pub const NDIS_PNP_WAKE_UP_PATTERN_MATCH: u32 = 2u32;
pub const NDIS_PORT_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_PORT_AUTHENTICATION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_PORT_CHARACTERISTICS_REVISION_1: u32 = 1u32;
pub const NDIS_PORT_CHAR_USE_DEFAULT_AUTH_SETTINGS: u32 = 1u32;
pub const NDIS_PORT_STATE_REVISION_1: u32 = 1u32;
pub const NDIS_PROTOCOL_ID_DEFAULT: u32 = 0u32;
pub const NDIS_PROTOCOL_ID_IP6: u32 = 3u32;
pub const NDIS_PROTOCOL_ID_IPX: u32 = 6u32;
pub const NDIS_PROTOCOL_ID_MASK: u32 = 15u32;
pub const NDIS_PROTOCOL_ID_MAX: u32 = 15u32;
pub const NDIS_PROTOCOL_ID_NBF: u32 = 7u32;
pub const NDIS_PROTOCOL_ID_TCP_IP: u32 = 2u32;
pub const NDIS_PROT_OPTION_ESTIMATED_LENGTH: u32 = 1u32;
pub const NDIS_PROT_OPTION_NO_LOOPBACK: u32 = 2u32;
pub const NDIS_PROT_OPTION_NO_RSVD_ON_RCVPKT: u32 = 4u32;
pub const NDIS_PROT_OPTION_SEND_RESTRICTED: u32 = 8u32;
pub const NDIS_QOS_ACTION_MAXIMUM: u32 = 1u32;
pub const NDIS_QOS_ACTION_PRIORITY: u32 = 0u32;
pub const NDIS_QOS_CAPABILITIES_CEE_DCBX_SUPPORTED: u32 = 4u32;
pub const NDIS_QOS_CAPABILITIES_IEEE_DCBX_SUPPORTED: u32 = 8u32;
pub const NDIS_QOS_CAPABILITIES_MACSEC_BYPASS_SUPPORTED: u32 = 2u32;
pub const NDIS_QOS_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_QOS_CAPABILITIES_STRICT_TSA_SUPPORTED: u32 = 1u32;
pub const NDIS_QOS_CLASSIFICATION_ELEMENT_REVISION_1: u32 = 1u32;
pub const NDIS_QOS_CLASSIFICATION_ENFORCED_BY_MINIPORT: u32 = 16777216u32;
pub const NDIS_QOS_CLASSIFICATION_SET_BY_MINIPORT_MASK: u32 = 4278190080u32;
pub const NDIS_QOS_CONDITION_DEFAULT: u32 = 1u32;
pub const NDIS_QOS_CONDITION_ETHERTYPE: u32 = 5u32;
pub const NDIS_QOS_CONDITION_MAXIMUM: u32 = 7u32;
pub const NDIS_QOS_CONDITION_NETDIRECT_PORT: u32 = 6u32;
pub const NDIS_QOS_CONDITION_RESERVED: u32 = 0u32;
pub const NDIS_QOS_CONDITION_TCP_OR_UDP_PORT: u32 = 4u32;
pub const NDIS_QOS_CONDITION_TCP_PORT: u32 = 2u32;
pub const NDIS_QOS_CONDITION_UDP_PORT: u32 = 3u32;
pub const NDIS_QOS_DEFAULT_SQ_ID: u32 = 0u32;
pub const NDIS_QOS_MAXIMUM_PRIORITIES: u32 = 8u32;
pub const NDIS_QOS_MAXIMUM_TRAFFIC_CLASSES: u32 = 8u32;
pub const NDIS_QOS_OFFLOAD_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_QOS_OFFLOAD_CAPABILITIES_REVISION_2: u32 = 2u32;
pub const NDIS_QOS_OFFLOAD_CAPS_GFT_SQ: u32 = 2u32;
pub const NDIS_QOS_OFFLOAD_CAPS_STANDARD_SQ: u32 = 1u32;
pub const NDIS_QOS_PARAMETERS_CLASSIFICATION_CHANGED: u32 = 65536u32;
pub const NDIS_QOS_PARAMETERS_CLASSIFICATION_CONFIGURED: u32 = 131072u32;
pub const NDIS_QOS_PARAMETERS_ETS_CHANGED: u32 = 1u32;
pub const NDIS_QOS_PARAMETERS_ETS_CONFIGURED: u32 = 2u32;
pub const NDIS_QOS_PARAMETERS_PFC_CHANGED: u32 = 256u32;
pub const NDIS_QOS_PARAMETERS_PFC_CONFIGURED: u32 = 512u32;
pub const NDIS_QOS_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_QOS_PARAMETERS_WILLING: u32 = 2147483648u32;
pub const NDIS_QOS_SQ_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_QOS_SQ_PARAMETERS_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_QOS_SQ_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_QOS_SQ_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_QOS_SQ_RECEIVE_CAP_ENABLED: u32 = 4u32;
pub const NDIS_QOS_SQ_STATS_REVISION_1: u32 = 1u32;
pub const NDIS_QOS_SQ_TRANSMIT_CAP_ENABLED: u32 = 1u32;
pub const NDIS_QOS_SQ_TRANSMIT_RESERVATION_ENABLED: u32 = 2u32;
pub const NDIS_QOS_TSA_CBS: u32 = 1u32;
pub const NDIS_QOS_TSA_ETS: u32 = 2u32;
pub const NDIS_QOS_TSA_MAXIMUM: u32 = 3u32;
pub const NDIS_QOS_TSA_STRICT: u32 = 0u32;
pub const NDIS_RECEIVE_FILTER_ANY_VLAN_SUPPORTED: u32 = 32u32;
pub const NDIS_RECEIVE_FILTER_ARP_HEADER_OPERATION_SUPPORTED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_ARP_HEADER_SPA_SUPPORTED: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_ARP_HEADER_SUPPORTED: u32 = 8u32;
pub const NDIS_RECEIVE_FILTER_ARP_HEADER_TPA_SUPPORTED: u32 = 4u32;
pub const NDIS_RECEIVE_FILTER_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_CAPABILITIES_REVISION_2: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_CLEAR_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_DYNAMIC_PROCESSOR_AFFINITY_CHANGE_FOR_DEFAULT_QUEUE_SUPPORTED: u32 = 64u32;
pub const NDIS_RECEIVE_FILTER_DYNAMIC_PROCESSOR_AFFINITY_CHANGE_SUPPORTED: u32 = 8u32;
pub const NDIS_RECEIVE_FILTER_FIELD_MAC_HEADER_VLAN_UNTAGGED_OR_ZERO: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_FIELD_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_FIELD_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_FLAGS_RESERVED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_GLOBAL_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_IMPLAT_MIN_OF_QUEUES_MODE: u32 = 64u32;
pub const NDIS_RECEIVE_FILTER_IMPLAT_SUM_OF_QUEUES_MODE: u32 = 128u32;
pub const NDIS_RECEIVE_FILTER_INFO_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_INFO_ARRAY_REVISION_2: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_INFO_ARRAY_VPORT_ID_SPECIFIED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_INTERRUPT_VECTOR_COALESCING_SUPPORTED: u32 = 16u32;
pub const NDIS_RECEIVE_FILTER_IPV4_HEADER_PROTOCOL_SUPPORTED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_IPV4_HEADER_SUPPORTED: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_IPV6_HEADER_PROTOCOL_SUPPORTED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_IPV6_HEADER_SUPPORTED: u32 = 4u32;
pub const NDIS_RECEIVE_FILTER_LOOKAHEAD_SPLIT_SUPPORTED: u32 = 4u32;
pub const NDIS_RECEIVE_FILTER_MAC_HEADER_DEST_ADDR_SUPPORTED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_MAC_HEADER_PACKET_TYPE_SUPPORTED: u32 = 32u32;
pub const NDIS_RECEIVE_FILTER_MAC_HEADER_PRIORITY_SUPPORTED: u32 = 16u32;
pub const NDIS_RECEIVE_FILTER_MAC_HEADER_PROTOCOL_SUPPORTED: u32 = 4u32;
pub const NDIS_RECEIVE_FILTER_MAC_HEADER_SOURCE_ADDR_SUPPORTED: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_MAC_HEADER_SUPPORTED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_MAC_HEADER_VLAN_ID_SUPPORTED: u32 = 8u32;
pub const NDIS_RECEIVE_FILTER_MOVE_FILTER_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_MSI_X_SUPPORTED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_PACKET_COALESCING_FILTERS_ENABLED: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_PACKET_COALESCING_SUPPORTED_ON_DEFAULT_QUEUE: u32 = 256u32;
pub const NDIS_RECEIVE_FILTER_PACKET_ENCAPSULATION: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_PACKET_ENCAPSULATION_GRE: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_RESERVED: u32 = 254u32;
pub const NDIS_RECEIVE_FILTER_TEST_HEADER_FIELD_EQUAL_SUPPORTED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_TEST_HEADER_FIELD_MASK_EQUAL_SUPPORTED: u32 = 2u32;
pub const NDIS_RECEIVE_FILTER_TEST_HEADER_FIELD_NOT_EQUAL_SUPPORTED: u32 = 4u32;
pub const NDIS_RECEIVE_FILTER_UDP_HEADER_DEST_PORT_SUPPORTED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_UDP_HEADER_SUPPORTED: u32 = 16u32;
pub const NDIS_RECEIVE_FILTER_VMQ_FILTERS_ENABLED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_VM_QUEUES_ENABLED: u32 = 1u32;
pub const NDIS_RECEIVE_FILTER_VM_QUEUE_SUPPORTED: u32 = 2u32;
pub const NDIS_RECEIVE_HASH_FLAG_ENABLE_HASH: u32 = 1u32;
pub const NDIS_RECEIVE_HASH_FLAG_HASH_INFO_UNCHANGED: u32 = 2u32;
pub const NDIS_RECEIVE_HASH_FLAG_HASH_KEY_UNCHANGED: u32 = 4u32;
pub const NDIS_RECEIVE_HASH_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_QUEUE_ALLOCATION_COMPLETE_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_QUEUE_ALLOCATION_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_QUEUE_FREE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_QUEUE_INFO_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_QUEUE_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_QUEUE_INFO_REVISION_2: u32 = 2u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_CHANGE_MASK: u32 = 4294901760u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_FLAGS_CHANGED: u32 = 65536u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_INTERRUPT_COALESCING_DOMAIN_ID_CHANGED: u32 = 1048576u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_LOOKAHEAD_SPLIT_REQUIRED: u32 = 2u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_NAME_CHANGED: u32 = 524288u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_PER_QUEUE_RECEIVE_INDICATION: u32 = 1u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_PROCESSOR_AFFINITY_CHANGED: u32 = 131072u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_QOS_SQ_ID_CHANGED: u32 = 2097152u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_REVISION_3: u32 = 3u32;
pub const NDIS_RECEIVE_QUEUE_PARAMETERS_SUGGESTED_RECV_BUFFER_NUMBERS_CHANGED: u32 = 262144u32;
pub const NDIS_RECEIVE_SCALE_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_SCALE_CAPABILITIES_REVISION_2: u32 = 2u32;
pub const NDIS_RECEIVE_SCALE_CAPABILITIES_REVISION_3: u32 = 3u32;
pub const NDIS_RECEIVE_SCALE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_SCALE_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_RECEIVE_SCALE_PARAMETERS_REVISION_3: u32 = 3u32;
pub const NDIS_RECEIVE_SCALE_PARAMETERS_V2_REVISION_1: u32 = 1u32;
pub const NDIS_RECEIVE_SCALE_PARAM_ENABLE_RSS: u32 = 1u32;
pub const NDIS_RECEIVE_SCALE_PARAM_HASH_INFO_CHANGED: u32 = 2u32;
pub const NDIS_RECEIVE_SCALE_PARAM_HASH_KEY_CHANGED: u32 = 4u32;
pub const NDIS_RECEIVE_SCALE_PARAM_NUMBER_OF_ENTRIES_CHANGED: u32 = 16u32;
pub const NDIS_RECEIVE_SCALE_PARAM_NUMBER_OF_QUEUES_CHANGED: u32 = 8u32;
pub const NDIS_RING_AUTO_REMOVAL_ERROR: u32 = 1024u32;
pub const NDIS_RING_COUNTER_OVERFLOW: u32 = 256u32;
pub const NDIS_RING_HARD_ERROR: u32 = 16384u32;
pub const NDIS_RING_LOBE_WIRE_FAULT: u32 = 2048u32;
pub const NDIS_RING_REMOVE_RECEIVED: u32 = 512u32;
pub const NDIS_RING_RING_RECOVERY: u32 = 64u32;
pub const NDIS_RING_SIGNAL_LOSS: u32 = 32768u32;
pub const NDIS_RING_SINGLE_STATION: u32 = 128u32;
pub const NDIS_RING_SOFT_ERROR: u32 = 8192u32;
pub const NDIS_RING_TRANSMIT_BEACON: u32 = 4096u32;
pub const NDIS_ROUTING_DOMAIN_ENTRY_REVISION_1: u32 = 1u32;
pub const NDIS_ROUTING_DOMAIN_ISOLATION_ENTRY_REVISION_1: u32 = 1u32;
pub const NDIS_RSC_STATISTICS_REVISION_1: u32 = 1u32;
pub const NDIS_RSS_CAPS_CLASSIFICATION_AT_DPC: u32 = 67108864u32;
pub const NDIS_RSS_CAPS_CLASSIFICATION_AT_ISR: u32 = 33554432u32;
pub const NDIS_RSS_CAPS_HASH_TYPE_TCP_IPV4: u32 = 256u32;
pub const NDIS_RSS_CAPS_HASH_TYPE_TCP_IPV6: u32 = 512u32;
pub const NDIS_RSS_CAPS_HASH_TYPE_TCP_IPV6_EX: u32 = 1024u32;
pub const NDIS_RSS_CAPS_HASH_TYPE_UDP_IPV4: u32 = 2048u32;
pub const NDIS_RSS_CAPS_HASH_TYPE_UDP_IPV6: u32 = 4096u32;
pub const NDIS_RSS_CAPS_HASH_TYPE_UDP_IPV6_EX: u32 = 8192u32;
pub const NDIS_RSS_CAPS_MESSAGE_SIGNALED_INTERRUPTS: u32 = 16777216u32;
pub const NDIS_RSS_CAPS_RSS_AVAILABLE_ON_PORTS: u32 = 268435456u32;
pub const NDIS_RSS_CAPS_SUPPORTS_INDEPENDENT_ENTRY_MOVE: u32 = 1073741824u32;
pub const NDIS_RSS_CAPS_SUPPORTS_MSI_X: u32 = 536870912u32;
pub const NDIS_RSS_CAPS_USING_MSI_X: u32 = 134217728u32;
pub const NDIS_RSS_HASH_SECRET_KEY_MAX_SIZE_REVISION_1: u32 = 40u32;
pub const NDIS_RSS_HASH_SECRET_KEY_MAX_SIZE_REVISION_2: u32 = 40u32;
pub const NDIS_RSS_HASH_SECRET_KEY_MAX_SIZE_REVISION_3: u32 = 40u32;
pub const NDIS_RSS_HASH_SECRET_KEY_SIZE_REVISION_1: u32 = 40u32;
pub const NDIS_RSS_INDIRECTION_TABLE_MAX_SIZE_REVISION_1: u32 = 128u32;
pub const NDIS_RSS_INDIRECTION_TABLE_SIZE_REVISION_1: u32 = 128u32;
pub const NDIS_RSS_PARAM_FLAG_BASE_CPU_UNCHANGED: u32 = 1u32;
pub const NDIS_RSS_PARAM_FLAG_DEFAULT_PROCESSOR_UNCHANGED: u32 = 32u32;
pub const NDIS_RSS_PARAM_FLAG_DISABLE_RSS: u32 = 16u32;
pub const NDIS_RSS_PARAM_FLAG_HASH_INFO_UNCHANGED: u32 = 2u32;
pub const NDIS_RSS_PARAM_FLAG_HASH_KEY_UNCHANGED: u32 = 8u32;
pub const NDIS_RSS_PARAM_FLAG_ITABLE_UNCHANGED: u32 = 4u32;
pub const NDIS_RSS_PROCESSOR_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_RSS_PROCESSOR_INFO_REVISION_2: u32 = 2u32;
pub const NDIS_RSS_SET_INDIRECTION_ENTRIES_REVISION_1: u32 = 1u32;
pub const NDIS_RSS_SET_INDIRECTION_ENTRY_FLAG_DEFAULT_PROCESSOR: u32 = 2u32;
pub const NDIS_RSS_SET_INDIRECTION_ENTRY_FLAG_PRIMARY_PROCESSOR: u32 = 1u32;
pub const NDIS_SIZEOF_NDIS_PM_PROTOCOL_OFFLOAD_REVISION_1: u32 = 240u32;
pub const NDIS_SRIOV_BAR_RESOURCES_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_CAPS_PF_MINIPORT: u32 = 2u32;
pub const NDIS_SRIOV_CAPS_SRIOV_SUPPORTED: u32 = 1u32;
pub const NDIS_SRIOV_CAPS_VF_MINIPORT: u32 = 4u32;
pub const NDIS_SRIOV_CONFIG_STATE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_OVERLYING_ADAPTER_INFO_VERSION_1: u32 = 1u32;
pub const NDIS_SRIOV_PF_LUID_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_PROBED_BARS_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_READ_VF_CONFIG_BLOCK_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_READ_VF_CONFIG_SPACE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_RESET_VF_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_SET_VF_POWER_STATE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_VF_INVALIDATE_CONFIG_BLOCK_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_VF_SERIAL_NUMBER_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_VF_VENDOR_DEVICE_ID_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_WRITE_VF_CONFIG_BLOCK_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SRIOV_WRITE_VF_CONFIG_SPACE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_STATISTICS_FLAGS_VALID_BROADCAST_BYTES_RCV: u32 = 262144u32;
pub const NDIS_STATISTICS_FLAGS_VALID_BROADCAST_BYTES_XMIT: u32 = 2097152u32;
pub const NDIS_STATISTICS_FLAGS_VALID_BROADCAST_FRAMES_RCV: u32 = 4u32;
pub const NDIS_STATISTICS_FLAGS_VALID_BROADCAST_FRAMES_XMIT: u32 = 256u32;
pub const NDIS_STATISTICS_FLAGS_VALID_BYTES_RCV: u32 = 8u32;
pub const NDIS_STATISTICS_FLAGS_VALID_BYTES_XMIT: u32 = 512u32;
pub const NDIS_STATISTICS_FLAGS_VALID_DIRECTED_BYTES_RCV: u32 = 65536u32;
pub const NDIS_STATISTICS_FLAGS_VALID_DIRECTED_BYTES_XMIT: u32 = 524288u32;
pub const NDIS_STATISTICS_FLAGS_VALID_DIRECTED_FRAMES_RCV: u32 = 1u32;
pub const NDIS_STATISTICS_FLAGS_VALID_DIRECTED_FRAMES_XMIT: u32 = 64u32;
pub const NDIS_STATISTICS_FLAGS_VALID_MULTICAST_BYTES_RCV: u32 = 131072u32;
pub const NDIS_STATISTICS_FLAGS_VALID_MULTICAST_BYTES_XMIT: u32 = 1048576u32;
pub const NDIS_STATISTICS_FLAGS_VALID_MULTICAST_FRAMES_RCV: u32 = 2u32;
pub const NDIS_STATISTICS_FLAGS_VALID_MULTICAST_FRAMES_XMIT: u32 = 128u32;
pub const NDIS_STATISTICS_FLAGS_VALID_RCV_DISCARDS: u32 = 16u32;
pub const NDIS_STATISTICS_FLAGS_VALID_RCV_ERROR: u32 = 32u32;
pub const NDIS_STATISTICS_FLAGS_VALID_XMIT_DISCARDS: u32 = 32768u32;
pub const NDIS_STATISTICS_FLAGS_VALID_XMIT_ERROR: u32 = 1024u32;
pub const NDIS_STATISTICS_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS6: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS61: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS620: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS630: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS640: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS650: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS651: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS660: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS670: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS680: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS681: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS682: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS683: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS684: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS685: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS686: u32 = 1u32;
pub const NDIS_SUPPORT_NDIS687: u32 = 1u32;
pub const NDIS_SWITCH_FEATURE_STATUS_CUSTOM_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_FEATURE_STATUS_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_NIC_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_NIC_FLAGS_MAPPED_NIC_UPDATED: u32 = 4u32;
pub const NDIS_SWITCH_NIC_FLAGS_NIC_INITIALIZING: u32 = 1u32;
pub const NDIS_SWITCH_NIC_FLAGS_NIC_SUSPENDED: u32 = 2u32;
pub const NDIS_SWITCH_NIC_FLAGS_NIC_SUSPENDED_LM: u32 = 16u32;
pub const NDIS_SWITCH_NIC_OID_REQUEST_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_NIC_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_NIC_PARAMETERS_REVISION_2: u32 = 2u32;
pub const NDIS_SWITCH_NIC_SAVE_STATE_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_NIC_SAVE_STATE_REVISION_2: u32 = 2u32;
pub const NDIS_SWITCH_OBJECT_SERIALIZATION_VERSION_1: u32 = 1u32;
pub const NDIS_SWITCH_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_ARRAY_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_FEATURE_STATUS_CUSTOM_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_FEATURE_STATUS_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PARAMETERS_FLAG_RESTORING_PORT: u32 = 2u32;
pub const NDIS_SWITCH_PORT_PARAMETERS_FLAG_UNTRUSTED_INTERNAL_PORT: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_CUSTOM_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_DELETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_ENUM_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_ENUM_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_ISOLATION_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_PROFILE_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_ROUTING_DOMAIN_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_SECURITY_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PORT_PROPERTY_SECURITY_REVISION_2: u32 = 2u32;
pub const NDIS_SWITCH_PORT_PROPERTY_VLAN_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PROPERTY_CUSTOM_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PROPERTY_DELETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PROPERTY_ENUM_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PROPERTY_ENUM_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SWITCH_PROPERTY_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_SYSTEM_PROCESSOR_INFO_EX_REVISION_1: u32 = 1u32;
pub const NDIS_TCP_CONNECTION_OFFLOAD_REVISION_1: u32 = 1u32;
pub const NDIS_TCP_CONNECTION_OFFLOAD_REVISION_2: u32 = 2u32;
pub const NDIS_TCP_RECV_SEG_COALESC_OFFLOAD_REVISION_1: u32 = 1u32;
pub const NDIS_TIMEOUT_DPC_REQUEST_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_TIMESTAMP_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_WLAN_WAKE_ON_4WAY_HANDSHAKE_REQUEST_ENABLED: u32 = 8u32;
pub const NDIS_WLAN_WAKE_ON_4WAY_HANDSHAKE_REQUEST_SUPPORTED: u32 = 8u32;
pub const NDIS_WLAN_WAKE_ON_AP_ASSOCIATION_LOST_ENABLED: u32 = 2u32;
pub const NDIS_WLAN_WAKE_ON_AP_ASSOCIATION_LOST_SUPPORTED: u32 = 2u32;
pub const NDIS_WLAN_WAKE_ON_GTK_HANDSHAKE_ERROR_ENABLED: u32 = 4u32;
pub const NDIS_WLAN_WAKE_ON_GTK_HANDSHAKE_ERROR_SUPPORTED: u32 = 4u32;
pub const NDIS_WLAN_WAKE_ON_NLO_DISCOVERY_ENABLED: u32 = 1u32;
pub const NDIS_WLAN_WAKE_ON_NLO_DISCOVERY_SUPPORTED: u32 = 1u32;
pub const NDIS_WMI_DEFAULT_METHOD_ID: u32 = 1u32;
pub const NDIS_WMI_ENUM_ADAPTER_REVISION_1: u32 = 1u32;
pub const NDIS_WMI_EVENT_HEADER_REVISION_1: u32 = 1u32;
pub const NDIS_WMI_METHOD_HEADER_REVISION_1: u32 = 1u32;
pub const NDIS_WMI_OBJECT_TYPE_ENUM_ADAPTER: u32 = 4u32;
pub const NDIS_WMI_OBJECT_TYPE_EVENT: u32 = 3u32;
pub const NDIS_WMI_OBJECT_TYPE_METHOD: u32 = 2u32;
pub const NDIS_WMI_OBJECT_TYPE_OUTPUT_INFO: u32 = 5u32;
pub const NDIS_WMI_OBJECT_TYPE_SET: u32 = 1u32;
pub const NDIS_WMI_PM_ACTIVE_CAPABILITIES_REVISION_1: u32 = 1u32;
pub const NDIS_WMI_PM_ADMIN_CONFIG_REVISION_1: u32 = 1u32;
pub const NDIS_WMI_RECEIVE_QUEUE_INFO_REVISION_1: u32 = 1u32;
pub const NDIS_WMI_RECEIVE_QUEUE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const NDIS_WMI_SET_HEADER_REVISION_1: u32 = 1u32;
pub const NDIS_WWAN_WAKE_ON_PACKET_STATE_ENABLED: u32 = 8u32;
pub const NDIS_WWAN_WAKE_ON_PACKET_STATE_SUPPORTED: u32 = 8u32;
pub const NDIS_WWAN_WAKE_ON_REGISTER_STATE_ENABLED: u32 = 1u32;
pub const NDIS_WWAN_WAKE_ON_REGISTER_STATE_SUPPORTED: u32 = 1u32;
pub const NDIS_WWAN_WAKE_ON_SMS_RECEIVE_ENABLED: u32 = 2u32;
pub const NDIS_WWAN_WAKE_ON_SMS_RECEIVE_SUPPORTED: u32 = 2u32;
pub const NDIS_WWAN_WAKE_ON_UICC_CHANGE_ENABLED: u32 = 16u32;
pub const NDIS_WWAN_WAKE_ON_UICC_CHANGE_SUPPORTED: u32 = 16u32;
pub const NDIS_WWAN_WAKE_ON_USSD_RECEIVE_ENABLED: u32 = 4u32;
pub const NDIS_WWAN_WAKE_ON_USSD_RECEIVE_SUPPORTED: u32 = 4u32;
pub const NDK_ADAPTER_FLAG_CQ_INTERRUPT_MODERATION_SUPPORTED: u32 = 4u32;
pub const NDK_ADAPTER_FLAG_CQ_RESIZE_SUPPORTED: u32 = 256u32;
pub const NDK_ADAPTER_FLAG_IN_ORDER_DMA_SUPPORTED: u32 = 1u32;
pub const NDK_ADAPTER_FLAG_LOOPBACK_CONNECTIONS_SUPPORTED: u32 = 65536u32;
pub const NDK_ADAPTER_FLAG_MULTI_ENGINE_SUPPORTED: u32 = 8u32;
pub const NDK_ADAPTER_FLAG_RDMA_READ_LOCAL_INVALIDATE_SUPPORTED: u32 = 16u32;
pub const NDK_ADAPTER_FLAG_RDMA_READ_SINK_NOT_REQUIRED: u32 = 2u32;
pub const NET_IFLUID_UNSPECIFIED: u32 = 0u32;
pub const NET_IF_ACCESS_BROADCAST: NET_IF_ACCESS_TYPE = 2i32;
pub const NET_IF_ACCESS_LOOPBACK: NET_IF_ACCESS_TYPE = 1i32;
pub const NET_IF_ACCESS_MAXIMUM: NET_IF_ACCESS_TYPE = 5i32;
pub const NET_IF_ACCESS_POINT_TO_MULTI_POINT: NET_IF_ACCESS_TYPE = 4i32;
pub const NET_IF_ACCESS_POINT_TO_POINT: NET_IF_ACCESS_TYPE = 3i32;
pub const NET_IF_ADMIN_STATUS_DOWN: NET_IF_ADMIN_STATUS = 2i32;
pub const NET_IF_ADMIN_STATUS_TESTING: NET_IF_ADMIN_STATUS = 3i32;
pub const NET_IF_ADMIN_STATUS_UP: NET_IF_ADMIN_STATUS = 1i32;
pub const NET_IF_CONNECTION_DEDICATED: NET_IF_CONNECTION_TYPE = 1i32;
pub const NET_IF_CONNECTION_DEMAND: NET_IF_CONNECTION_TYPE = 3i32;
pub const NET_IF_CONNECTION_MAXIMUM: NET_IF_CONNECTION_TYPE = 4i32;
pub const NET_IF_CONNECTION_PASSIVE: NET_IF_CONNECTION_TYPE = 2i32;
pub const NET_IF_DIRECTION_MAXIMUM: NET_IF_DIRECTION_TYPE = 3i32;
pub const NET_IF_DIRECTION_RECEIVEONLY: NET_IF_DIRECTION_TYPE = 2i32;
pub const NET_IF_DIRECTION_SENDONLY: NET_IF_DIRECTION_TYPE = 1i32;
pub const NET_IF_DIRECTION_SENDRECEIVE: NET_IF_DIRECTION_TYPE = 0i32;
pub const NET_IF_OID_COMPARTMENT_ID: u32 = 2u32;
pub const NET_IF_OID_IF_ALIAS: u32 = 1u32;
pub const NET_IF_OID_IF_ENTRY: u32 = 4u32;
pub const NET_IF_OID_NETWORK_GUID: u32 = 3u32;
pub const NET_IF_OPER_STATUS_DORMANT: NET_IF_OPER_STATUS = 5i32;
pub const NET_IF_OPER_STATUS_DORMANT_LOW_POWER: u32 = 8u32;
pub const NET_IF_OPER_STATUS_DORMANT_PAUSED: u32 = 4u32;
pub const NET_IF_OPER_STATUS_DOWN: NET_IF_OPER_STATUS = 2i32;
pub const NET_IF_OPER_STATUS_DOWN_NOT_AUTHENTICATED: u32 = 1u32;
pub const NET_IF_OPER_STATUS_DOWN_NOT_MEDIA_CONNECTED: u32 = 2u32;
pub const NET_IF_OPER_STATUS_LOWER_LAYER_DOWN: NET_IF_OPER_STATUS = 7i32;
pub const NET_IF_OPER_STATUS_NOT_PRESENT: NET_IF_OPER_STATUS = 6i32;
pub const NET_IF_OPER_STATUS_TESTING: NET_IF_OPER_STATUS = 3i32;
pub const NET_IF_OPER_STATUS_UNKNOWN: NET_IF_OPER_STATUS = 4i32;
pub const NET_IF_OPER_STATUS_UP: NET_IF_OPER_STATUS = 1i32;
pub const NET_IF_RCV_ADDRESS_TYPE_NON_VOLATILE: NET_IF_RCV_ADDRESS_TYPE = 3i32;
pub const NET_IF_RCV_ADDRESS_TYPE_OTHER: NET_IF_RCV_ADDRESS_TYPE = 1i32;
pub const NET_IF_RCV_ADDRESS_TYPE_VOLATILE: NET_IF_RCV_ADDRESS_TYPE = 2i32;
pub const NET_SITEID_MAXSYSTEM: u32 = 268435455u32;
pub const NET_SITEID_MAXUSER: u32 = 134217727u32;
pub const NET_SITEID_UNSPECIFIED: u32 = 0u32;
pub const NIIF_FILTER_INTERFACE: u32 = 2u32;
pub const NIIF_HARDWARE_INTERFACE: u32 = 1u32;
pub const NIIF_NDIS_ENDPOINT_INTERFACE: u32 = 64u32;
pub const NIIF_NDIS_ISCSI_INTERFACE: u32 = 128u32;
pub const NIIF_NDIS_RESERVED1: u32 = 4u32;
pub const NIIF_NDIS_RESERVED2: u32 = 8u32;
pub const NIIF_NDIS_RESERVED3: u32 = 16u32;
pub const NIIF_NDIS_RESERVED4: u32 = 256u32;
pub const NIIF_NDIS_WDM_INTERFACE: u32 = 32u32;
pub const Ndis802_11AuthModeAutoSwitch: NDIS_802_11_AUTHENTICATION_MODE = 2i32;
pub const Ndis802_11AuthModeMax: NDIS_802_11_AUTHENTICATION_MODE = 11i32;
pub const Ndis802_11AuthModeOpen: NDIS_802_11_AUTHENTICATION_MODE = 0i32;
pub const Ndis802_11AuthModeShared: NDIS_802_11_AUTHENTICATION_MODE = 1i32;
pub const Ndis802_11AuthModeWPA: NDIS_802_11_AUTHENTICATION_MODE = 3i32;
pub const Ndis802_11AuthModeWPA2: NDIS_802_11_AUTHENTICATION_MODE = 6i32;
pub const Ndis802_11AuthModeWPA2PSK: NDIS_802_11_AUTHENTICATION_MODE = 7i32;
pub const Ndis802_11AuthModeWPA3: NDIS_802_11_AUTHENTICATION_MODE = 8i32;
pub const Ndis802_11AuthModeWPA3Ent: NDIS_802_11_AUTHENTICATION_MODE = 10i32;
pub const Ndis802_11AuthModeWPA3Ent192: NDIS_802_11_AUTHENTICATION_MODE = 8i32;
pub const Ndis802_11AuthModeWPA3SAE: NDIS_802_11_AUTHENTICATION_MODE = 9i32;
pub const Ndis802_11AuthModeWPANone: NDIS_802_11_AUTHENTICATION_MODE = 5i32;
pub const Ndis802_11AuthModeWPAPSK: NDIS_802_11_AUTHENTICATION_MODE = 4i32;
pub const Ndis802_11AutoUnknown: NDIS_802_11_NETWORK_INFRASTRUCTURE = 2i32;
pub const Ndis802_11Automode: NDIS_802_11_NETWORK_TYPE = 4i32;
pub const Ndis802_11DS: NDIS_802_11_NETWORK_TYPE = 1i32;
pub const Ndis802_11Encryption1Enabled: NDIS_802_11_WEP_STATUS = 0i32;
pub const Ndis802_11Encryption1KeyAbsent: NDIS_802_11_WEP_STATUS = 2i32;
pub const Ndis802_11Encryption2Enabled: NDIS_802_11_WEP_STATUS = 4i32;
pub const Ndis802_11Encryption2KeyAbsent: NDIS_802_11_WEP_STATUS = 5i32;
pub const Ndis802_11Encryption3Enabled: NDIS_802_11_WEP_STATUS = 6i32;
pub const Ndis802_11Encryption3KeyAbsent: NDIS_802_11_WEP_STATUS = 7i32;
pub const Ndis802_11EncryptionDisabled: NDIS_802_11_WEP_STATUS = 1i32;
pub const Ndis802_11EncryptionNotSupported: NDIS_802_11_WEP_STATUS = 3i32;
pub const Ndis802_11FH: NDIS_802_11_NETWORK_TYPE = 0i32;
pub const Ndis802_11IBSS: NDIS_802_11_NETWORK_INFRASTRUCTURE = 0i32;
pub const Ndis802_11Infrastructure: NDIS_802_11_NETWORK_INFRASTRUCTURE = 1i32;
pub const Ndis802_11InfrastructureMax: NDIS_802_11_NETWORK_INFRASTRUCTURE = 3i32;
pub const Ndis802_11MediaStreamOff: NDIS_802_11_MEDIA_STREAM_MODE = 0i32;
pub const Ndis802_11MediaStreamOn: NDIS_802_11_MEDIA_STREAM_MODE = 1i32;
pub const Ndis802_11NetworkTypeMax: NDIS_802_11_NETWORK_TYPE = 5i32;
pub const Ndis802_11OFDM24: NDIS_802_11_NETWORK_TYPE = 3i32;
pub const Ndis802_11OFDM5: NDIS_802_11_NETWORK_TYPE = 2i32;
pub const Ndis802_11PowerModeCAM: NDIS_802_11_POWER_MODE = 0i32;
pub const Ndis802_11PowerModeFast_PSP: NDIS_802_11_POWER_MODE = 2i32;
pub const Ndis802_11PowerModeMAX_PSP: NDIS_802_11_POWER_MODE = 1i32;
pub const Ndis802_11PowerModeMax: NDIS_802_11_POWER_MODE = 3i32;
pub const Ndis802_11PrivFilter8021xWEP: NDIS_802_11_PRIVACY_FILTER = 1i32;
pub const Ndis802_11PrivFilterAcceptAll: NDIS_802_11_PRIVACY_FILTER = 0i32;
pub const Ndis802_11RadioStatusHardwareOff: NDIS_802_11_RADIO_STATUS = 1i32;
pub const Ndis802_11RadioStatusHardwareSoftwareOff: NDIS_802_11_RADIO_STATUS = 3i32;
pub const Ndis802_11RadioStatusMax: NDIS_802_11_RADIO_STATUS = 4i32;
pub const Ndis802_11RadioStatusOn: NDIS_802_11_RADIO_STATUS = 0i32;
pub const Ndis802_11RadioStatusSoftwareOff: NDIS_802_11_RADIO_STATUS = 2i32;
pub const Ndis802_11ReloadWEPKeys: NDIS_802_11_RELOAD_DEFAULTS = 0i32;
pub const Ndis802_11StatusTypeMax: NDIS_802_11_STATUS_TYPE = 3i32;
pub const Ndis802_11StatusType_Authentication: NDIS_802_11_STATUS_TYPE = 0i32;
pub const Ndis802_11StatusType_MediaStreamMode: NDIS_802_11_STATUS_TYPE = 1i32;
pub const Ndis802_11StatusType_PMKID_CandidateList: NDIS_802_11_STATUS_TYPE = 2i32;
pub const Ndis802_11WEPDisabled: NDIS_802_11_WEP_STATUS = 1i32;
pub const Ndis802_11WEPEnabled: NDIS_802_11_WEP_STATUS = 0i32;
pub const Ndis802_11WEPKeyAbsent: NDIS_802_11_WEP_STATUS = 2i32;
pub const Ndis802_11WEPNotSupported: NDIS_802_11_WEP_STATUS = 3i32;
pub const NdisDefinitelyNetworkChange: NDIS_NETWORK_CHANGE_TYPE = 2i32;
pub const NdisDeviceStateD0: NDIS_DEVICE_POWER_STATE = 1i32;
pub const NdisDeviceStateD1: NDIS_DEVICE_POWER_STATE = 2i32;
pub const NdisDeviceStateD2: NDIS_DEVICE_POWER_STATE = 3i32;
pub const NdisDeviceStateD3: NDIS_DEVICE_POWER_STATE = 4i32;
pub const NdisDeviceStateMaximum: NDIS_DEVICE_POWER_STATE = 5i32;
pub const NdisDeviceStateUnspecified: NDIS_DEVICE_POWER_STATE = 0i32;
pub const NdisFddiRingDetect: NDIS_FDDI_RING_MGT_STATE = 4i32;
pub const NdisFddiRingDirected: NDIS_FDDI_RING_MGT_STATE = 7i32;
pub const NdisFddiRingIsolated: NDIS_FDDI_RING_MGT_STATE = 1i32;
pub const NdisFddiRingNonOperational: NDIS_FDDI_RING_MGT_STATE = 2i32;
pub const NdisFddiRingNonOperationalDup: NDIS_FDDI_RING_MGT_STATE = 5i32;
pub const NdisFddiRingOperational: NDIS_FDDI_RING_MGT_STATE = 3i32;
pub const NdisFddiRingOperationalDup: NDIS_FDDI_RING_MGT_STATE = 6i32;
pub const NdisFddiRingTrace: NDIS_FDDI_RING_MGT_STATE = 8i32;
pub const NdisFddiStateActive: NDIS_FDDI_LCONNECTION_STATE = 9i32;
pub const NdisFddiStateBreak: NDIS_FDDI_LCONNECTION_STATE = 2i32;
pub const NdisFddiStateConnect: NDIS_FDDI_LCONNECTION_STATE = 4i32;
pub const NdisFddiStateJoin: NDIS_FDDI_LCONNECTION_STATE = 7i32;
pub const NdisFddiStateMaintenance: NDIS_FDDI_LCONNECTION_STATE = 10i32;
pub const NdisFddiStateNext: NDIS_FDDI_LCONNECTION_STATE = 5i32;
pub const NdisFddiStateOff: NDIS_FDDI_LCONNECTION_STATE = 1i32;
pub const NdisFddiStateSignal: NDIS_FDDI_LCONNECTION_STATE = 6i32;
pub const NdisFddiStateTrace: NDIS_FDDI_LCONNECTION_STATE = 3i32;
pub const NdisFddiStateVerify: NDIS_FDDI_LCONNECTION_STATE = 8i32;
pub const NdisFddiTypeCWrapA: NDIS_FDDI_ATTACHMENT_TYPE = 10i32;
pub const NdisFddiTypeCWrapB: NDIS_FDDI_ATTACHMENT_TYPE = 11i32;
pub const NdisFddiTypeCWrapS: NDIS_FDDI_ATTACHMENT_TYPE = 12i32;
pub const NdisFddiTypeIsolated: NDIS_FDDI_ATTACHMENT_TYPE = 1i32;
pub const NdisFddiTypeLocalA: NDIS_FDDI_ATTACHMENT_TYPE = 2i32;
pub const NdisFddiTypeLocalAB: NDIS_FDDI_ATTACHMENT_TYPE = 4i32;
pub const NdisFddiTypeLocalB: NDIS_FDDI_ATTACHMENT_TYPE = 3i32;
pub const NdisFddiTypeLocalS: NDIS_FDDI_ATTACHMENT_TYPE = 5i32;
pub const NdisFddiTypeThrough: NDIS_FDDI_ATTACHMENT_TYPE = 13i32;
pub const NdisFddiTypeWrapA: NDIS_FDDI_ATTACHMENT_TYPE = 6i32;
pub const NdisFddiTypeWrapAB: NDIS_FDDI_ATTACHMENT_TYPE = 8i32;
pub const NdisFddiTypeWrapB: NDIS_FDDI_ATTACHMENT_TYPE = 7i32;
pub const NdisFddiTypeWrapS: NDIS_FDDI_ATTACHMENT_TYPE = 9i32;
pub const NdisHardwareStatusClosing: NDIS_HARDWARE_STATUS = 3i32;
pub const NdisHardwareStatusInitializing: NDIS_HARDWARE_STATUS = 1i32;
pub const NdisHardwareStatusNotReady: NDIS_HARDWARE_STATUS = 4i32;
pub const NdisHardwareStatusReady: NDIS_HARDWARE_STATUS = 0i32;
pub const NdisHardwareStatusReset: NDIS_HARDWARE_STATUS = 2i32;
pub const NdisHashFunctionReserved1: u32 = 2u32;
pub const NdisHashFunctionReserved2: u32 = 4u32;
pub const NdisHashFunctionReserved3: u32 = 8u32;
pub const NdisHashFunctionToeplitz: u32 = 1u32;
pub const NdisInterruptModerationDisabled: NDIS_INTERRUPT_MODERATION = 3i32;
pub const NdisInterruptModerationEnabled: NDIS_INTERRUPT_MODERATION = 2i32;
pub const NdisInterruptModerationNotSupported: NDIS_INTERRUPT_MODERATION = 1i32;
pub const NdisInterruptModerationUnknown: NDIS_INTERRUPT_MODERATION = 0i32;
pub const NdisMediaStateConnected: NDIS_MEDIA_STATE = 0i32;
pub const NdisMediaStateDisconnected: NDIS_MEDIA_STATE = 1i32;
pub const NdisMedium1394: NDIS_MEDIUM = 13i32;
pub const NdisMedium802_3: NDIS_MEDIUM = 0i32;
pub const NdisMedium802_5: NDIS_MEDIUM = 1i32;
pub const NdisMediumArcnet878_2: NDIS_MEDIUM = 7i32;
pub const NdisMediumArcnetRaw: NDIS_MEDIUM = 6i32;
pub const NdisMediumAtm: NDIS_MEDIUM = 8i32;
pub const NdisMediumBpc: NDIS_MEDIUM = 11i32;
pub const NdisMediumCoWan: NDIS_MEDIUM = 12i32;
pub const NdisMediumDix: NDIS_MEDIUM = 5i32;
pub const NdisMediumFddi: NDIS_MEDIUM = 2i32;
pub const NdisMediumIP: NDIS_MEDIUM = 19i32;
pub const NdisMediumInfiniBand: NDIS_MEDIUM = 14i32;
pub const NdisMediumIrda: NDIS_MEDIUM = 10i32;
pub const NdisMediumLocalTalk: NDIS_MEDIUM = 4i32;
pub const NdisMediumLoopback: NDIS_MEDIUM = 17i32;
pub const NdisMediumMax: NDIS_MEDIUM = 20i32;
pub const NdisMediumNative802_11: NDIS_MEDIUM = 16i32;
pub const NdisMediumTunnel: NDIS_MEDIUM = 15i32;
pub const NdisMediumWan: NDIS_MEDIUM = 3i32;
pub const NdisMediumWiMAX: NDIS_MEDIUM = 18i32;
pub const NdisMediumWirelessWan: NDIS_MEDIUM = 9i32;
pub const NdisNetworkChangeFromMediaConnect: NDIS_NETWORK_CHANGE_TYPE = 3i32;
pub const NdisNetworkChangeMax: NDIS_NETWORK_CHANGE_TYPE = 4i32;
pub const NdisPauseFunctionsReceiveOnly: NDIS_SUPPORTED_PAUSE_FUNCTIONS = 2i32;
pub const NdisPauseFunctionsSendAndReceive: NDIS_SUPPORTED_PAUSE_FUNCTIONS = 3i32;
pub const NdisPauseFunctionsSendOnly: NDIS_SUPPORTED_PAUSE_FUNCTIONS = 1i32;
pub const NdisPauseFunctionsUnknown: NDIS_SUPPORTED_PAUSE_FUNCTIONS = 4i32;
pub const NdisPauseFunctionsUnsupported: NDIS_SUPPORTED_PAUSE_FUNCTIONS = 0i32;
pub const NdisPhysicalMedium1394: NDIS_PHYSICAL_MEDIUM = 7i32;
pub const NdisPhysicalMedium802_3: NDIS_PHYSICAL_MEDIUM = 14i32;
pub const NdisPhysicalMedium802_5: NDIS_PHYSICAL_MEDIUM = 15i32;
pub const NdisPhysicalMediumBluetooth: NDIS_PHYSICAL_MEDIUM = 10i32;
pub const NdisPhysicalMediumCableModem: NDIS_PHYSICAL_MEDIUM = 2i32;
pub const NdisPhysicalMediumDSL: NDIS_PHYSICAL_MEDIUM = 5i32;
pub const NdisPhysicalMediumFibreChannel: NDIS_PHYSICAL_MEDIUM = 6i32;
pub const NdisPhysicalMediumInfiniband: NDIS_PHYSICAL_MEDIUM = 11i32;
pub const NdisPhysicalMediumIrda: NDIS_PHYSICAL_MEDIUM = 16i32;
pub const NdisPhysicalMediumMax: NDIS_PHYSICAL_MEDIUM = 21i32;
pub const NdisPhysicalMediumNative802_11: NDIS_PHYSICAL_MEDIUM = 9i32;
pub const NdisPhysicalMediumNative802_15_4: NDIS_PHYSICAL_MEDIUM = 20i32;
pub const NdisPhysicalMediumOther: NDIS_PHYSICAL_MEDIUM = 19i32;
pub const NdisPhysicalMediumPhoneLine: NDIS_PHYSICAL_MEDIUM = 3i32;
pub const NdisPhysicalMediumPowerLine: NDIS_PHYSICAL_MEDIUM = 4i32;
pub const NdisPhysicalMediumUWB: NDIS_PHYSICAL_MEDIUM = 13i32;
pub const NdisPhysicalMediumUnspecified: NDIS_PHYSICAL_MEDIUM = 0i32;
pub const NdisPhysicalMediumWiMax: NDIS_PHYSICAL_MEDIUM = 12i32;
pub const NdisPhysicalMediumWiredCoWan: NDIS_PHYSICAL_MEDIUM = 18i32;
pub const NdisPhysicalMediumWiredWAN: NDIS_PHYSICAL_MEDIUM = 17i32;
pub const NdisPhysicalMediumWirelessLan: NDIS_PHYSICAL_MEDIUM = 1i32;
pub const NdisPhysicalMediumWirelessWan: NDIS_PHYSICAL_MEDIUM = 8i32;
pub const NdisPortAuthorizationUnknown: NDIS_PORT_AUTHORIZATION_STATE = 0i32;
pub const NdisPortAuthorized: NDIS_PORT_AUTHORIZATION_STATE = 1i32;
pub const NdisPortControlStateControlled: NDIS_PORT_CONTROL_STATE = 1i32;
pub const NdisPortControlStateUncontrolled: NDIS_PORT_CONTROL_STATE = 2i32;
pub const NdisPortControlStateUnknown: NDIS_PORT_CONTROL_STATE = 0i32;
pub const NdisPortReauthorizing: NDIS_PORT_AUTHORIZATION_STATE = 3i32;
pub const NdisPortType8021xSupplicant: NDIS_PORT_TYPE = 3i32;
pub const NdisPortTypeBridge: NDIS_PORT_TYPE = 1i32;
pub const NdisPortTypeMax: NDIS_PORT_TYPE = 4i32;
pub const NdisPortTypeRasConnection: NDIS_PORT_TYPE = 2i32;
pub const NdisPortTypeUndefined: NDIS_PORT_TYPE = 0i32;
pub const NdisPortUnauthorized: NDIS_PORT_AUTHORIZATION_STATE = 2i32;
pub const NdisPossibleNetworkChange: NDIS_NETWORK_CHANGE_TYPE = 1i32;
pub const NdisProcessorVendorAuthenticAMD: NDIS_PROCESSOR_VENDOR = 2i32;
pub const NdisProcessorVendorGenuinIntel: NDIS_PROCESSOR_VENDOR = 1i32;
pub const NdisProcessorVendorGenuineIntel: NDIS_PROCESSOR_VENDOR = 1i32;
pub const NdisProcessorVendorUnknown: NDIS_PROCESSOR_VENDOR = 0i32;
pub const NdisRequestClose: NDIS_REQUEST_TYPE = 4i32;
pub const NdisRequestGeneric1: NDIS_REQUEST_TYPE = 8i32;
pub const NdisRequestGeneric2: NDIS_REQUEST_TYPE = 9i32;
pub const NdisRequestGeneric3: NDIS_REQUEST_TYPE = 10i32;
pub const NdisRequestGeneric4: NDIS_REQUEST_TYPE = 11i32;
pub const NdisRequestOpen: NDIS_REQUEST_TYPE = 3i32;
pub const NdisRequestQueryInformation: NDIS_REQUEST_TYPE = 0i32;
pub const NdisRequestQueryStatistics: NDIS_REQUEST_TYPE = 2i32;
pub const NdisRequestReset: NDIS_REQUEST_TYPE = 7i32;
pub const NdisRequestSend: NDIS_REQUEST_TYPE = 5i32;
pub const NdisRequestSetInformation: NDIS_REQUEST_TYPE = 1i32;
pub const NdisRequestTransferData: NDIS_REQUEST_TYPE = 6i32;
pub const NdisRingStateClosed: NDIS_802_5_RING_STATE = 2i32;
pub const NdisRingStateClosing: NDIS_802_5_RING_STATE = 4i32;
pub const NdisRingStateOpenFailure: NDIS_802_5_RING_STATE = 5i32;
pub const NdisRingStateOpened: NDIS_802_5_RING_STATE = 1i32;
pub const NdisRingStateOpening: NDIS_802_5_RING_STATE = 3i32;
pub const NdisRingStateRingFailure: NDIS_802_5_RING_STATE = 6i32;
pub const NdisWanErrorControl: NDIS_WAN_QUALITY = 1i32;
pub const NdisWanHeaderEthernet: NDIS_WAN_HEADER_FORMAT = 1i32;
pub const NdisWanHeaderNative: NDIS_WAN_HEADER_FORMAT = 0i32;
pub const NdisWanMediumAgileVPN: NDIS_WAN_MEDIUM_SUBTYPE = 14i32;
pub const NdisWanMediumAtm: NDIS_WAN_MEDIUM_SUBTYPE = 5i32;
pub const NdisWanMediumFrameRelay: NDIS_WAN_MEDIUM_SUBTYPE = 4i32;
pub const NdisWanMediumGre: NDIS_WAN_MEDIUM_SUBTYPE = 15i32;
pub const NdisWanMediumHub: NDIS_WAN_MEDIUM_SUBTYPE = 0i32;
pub const NdisWanMediumIrda: NDIS_WAN_MEDIUM_SUBTYPE = 10i32;
pub const NdisWanMediumIsdn: NDIS_WAN_MEDIUM_SUBTYPE = 2i32;
pub const NdisWanMediumL2TP: NDIS_WAN_MEDIUM_SUBTYPE = 9i32;
pub const NdisWanMediumPPTP: NDIS_WAN_MEDIUM_SUBTYPE = 8i32;
pub const NdisWanMediumParallel: NDIS_WAN_MEDIUM_SUBTYPE = 11i32;
pub const NdisWanMediumPppoe: NDIS_WAN_MEDIUM_SUBTYPE = 12i32;
pub const NdisWanMediumSSTP: NDIS_WAN_MEDIUM_SUBTYPE = 13i32;
pub const NdisWanMediumSW56K: NDIS_WAN_MEDIUM_SUBTYPE = 7i32;
pub const NdisWanMediumSerial: NDIS_WAN_MEDIUM_SUBTYPE = 3i32;
pub const NdisWanMediumSonet: NDIS_WAN_MEDIUM_SUBTYPE = 6i32;
pub const NdisWanMediumSubTypeMax: NDIS_WAN_MEDIUM_SUBTYPE = 16i32;
pub const NdisWanMediumX_25: NDIS_WAN_MEDIUM_SUBTYPE = 1i32;
pub const NdisWanRaw: NDIS_WAN_QUALITY = 0i32;
pub const NdisWanReliable: NDIS_WAN_QUALITY = 2i32;
pub const NdkInfiniBand: NDK_RDMA_TECHNOLOGY = 2i32;
pub const NdkMaxTechnology: NDK_RDMA_TECHNOLOGY = 5i32;
pub const NdkRoCE: NDK_RDMA_TECHNOLOGY = 3i32;
pub const NdkRoCEv2: NDK_RDMA_TECHNOLOGY = 4i32;
pub const NdkUndefined: NDK_RDMA_TECHNOLOGY = 0i32;
pub const NdkiWarp: NDK_RDMA_TECHNOLOGY = 1i32;
pub const OFFLOAD_INBOUND_SA: u32 = 1u32;
pub const OFFLOAD_IPSEC_CONF_3_DES: OFFLOAD_CONF_ALGO = 3i32;
pub const OFFLOAD_IPSEC_CONF_DES: OFFLOAD_CONF_ALGO = 1i32;
pub const OFFLOAD_IPSEC_CONF_MAX: OFFLOAD_CONF_ALGO = 4i32;
pub const OFFLOAD_IPSEC_CONF_NONE: OFFLOAD_CONF_ALGO = 0i32;
pub const OFFLOAD_IPSEC_CONF_RESERVED: OFFLOAD_CONF_ALGO = 2i32;
pub const OFFLOAD_IPSEC_INTEGRITY_MAX: OFFLOAD_INTEGRITY_ALGO = 3i32;
pub const OFFLOAD_IPSEC_INTEGRITY_MD5: OFFLOAD_INTEGRITY_ALGO = 1i32;
pub const OFFLOAD_IPSEC_INTEGRITY_NONE: OFFLOAD_INTEGRITY_ALGO = 0i32;
pub const OFFLOAD_IPSEC_INTEGRITY_SHA: OFFLOAD_INTEGRITY_ALGO = 2i32;
pub const OFFLOAD_IPSEC_UDPESP_ENCAPTYPE_IKE: UDP_ENCAP_TYPE = 0i32;
pub const OFFLOAD_IPSEC_UDPESP_ENCAPTYPE_OTHER: UDP_ENCAP_TYPE = 1i32;
pub const OFFLOAD_MAX_SAS: u32 = 3u32;
pub const OFFLOAD_OUTBOUND_SA: u32 = 2u32;
pub const OID_1394_LOCAL_NODE_INFO: u32 = 201392385u32;
pub const OID_1394_VC_INFO: u32 = 201392386u32;
pub const OID_802_11_ADD_KEY: u32 = 218169629u32;
pub const OID_802_11_ADD_WEP: u32 = 218169619u32;
pub const OID_802_11_ASSOCIATION_INFORMATION: u32 = 218169631u32;
pub const OID_802_11_AUTHENTICATION_MODE: u32 = 218169624u32;
pub const OID_802_11_BSSID: u32 = 218169601u32;
pub const OID_802_11_BSSID_LIST: u32 = 218169879u32;
pub const OID_802_11_BSSID_LIST_SCAN: u32 = 218169626u32;
pub const OID_802_11_CAPABILITY: u32 = 218169634u32;
pub const OID_802_11_CONFIGURATION: u32 = 218169873u32;
pub const OID_802_11_DESIRED_RATES: u32 = 218169872u32;
pub const OID_802_11_DISASSOCIATE: u32 = 218169621u32;
pub const OID_802_11_ENCRYPTION_STATUS: u32 = 218169627u32;
pub const OID_802_11_FRAGMENTATION_THRESHOLD: u32 = 218169865u32;
pub const OID_802_11_INFRASTRUCTURE_MODE: u32 = 218169608u32;
pub const OID_802_11_MEDIA_STREAM_MODE: u32 = 218169633u32;
pub const OID_802_11_NETWORK_TYPES_SUPPORTED: u32 = 218169859u32;
pub const OID_802_11_NETWORK_TYPE_IN_USE: u32 = 218169860u32;
pub const OID_802_11_NON_BCAST_SSID_LIST: u32 = 218169636u32;
pub const OID_802_11_NUMBER_OF_ANTENNAS: u32 = 218169867u32;
pub const OID_802_11_PMKID: u32 = 218169635u32;
pub const OID_802_11_POWER_MODE: u32 = 218169878u32;
pub const OID_802_11_PRIVACY_FILTER: u32 = 218169625u32;
pub const OID_802_11_RADIO_STATUS: u32 = 218169637u32;
pub const OID_802_11_RELOAD_DEFAULTS: u32 = 218169628u32;
pub const OID_802_11_REMOVE_KEY: u32 = 218169630u32;
pub const OID_802_11_REMOVE_WEP: u32 = 218169620u32;
pub const OID_802_11_RSSI: u32 = 218169862u32;
pub const OID_802_11_RSSI_TRIGGER: u32 = 218169863u32;
pub const OID_802_11_RTS_THRESHOLD: u32 = 218169866u32;
pub const OID_802_11_RX_ANTENNA_SELECTED: u32 = 218169868u32;
pub const OID_802_11_SSID: u32 = 218169602u32;
pub const OID_802_11_STATISTICS: u32 = 218235410u32;
pub const OID_802_11_SUPPORTED_RATES: u32 = 218169870u32;
pub const OID_802_11_TEST: u32 = 218169632u32;
pub const OID_802_11_TX_ANTENNA_SELECTED: u32 = 218169869u32;
pub const OID_802_11_TX_POWER_LEVEL: u32 = 218169861u32;
pub const OID_802_11_WEP_STATUS: u32 = 218169627u32;
pub const OID_802_3_ADD_MULTICAST_ADDRESS: u32 = 16843272u32;
pub const OID_802_3_CURRENT_ADDRESS: u32 = 16843010u32;
pub const OID_802_3_DELETE_MULTICAST_ADDRESS: u32 = 16843273u32;
pub const OID_802_3_MAC_OPTIONS: u32 = 16843013u32;
pub const OID_802_3_MAXIMUM_LIST_SIZE: u32 = 16843012u32;
pub const OID_802_3_MULTICAST_LIST: u32 = 16843011u32;
pub const OID_802_3_PERMANENT_ADDRESS: u32 = 16843009u32;
pub const OID_802_3_RCV_ERROR_ALIGNMENT: u32 = 16908545u32;
pub const OID_802_3_RCV_OVERRUN: u32 = 16908803u32;
pub const OID_802_3_XMIT_DEFERRED: u32 = 16908801u32;
pub const OID_802_3_XMIT_HEARTBEAT_FAILURE: u32 = 16908805u32;
pub const OID_802_3_XMIT_LATE_COLLISIONS: u32 = 16908807u32;
pub const OID_802_3_XMIT_MAX_COLLISIONS: u32 = 16908802u32;
pub const OID_802_3_XMIT_MORE_COLLISIONS: u32 = 16908547u32;
pub const OID_802_3_XMIT_ONE_COLLISION: u32 = 16908546u32;
pub const OID_802_3_XMIT_TIMES_CRS_LOST: u32 = 16908806u32;
pub const OID_802_3_XMIT_UNDERRUN: u32 = 16908804u32;
pub const OID_802_5_ABORT_DELIMETERS: u32 = 33686019u32;
pub const OID_802_5_AC_ERRORS: u32 = 33686018u32;
pub const OID_802_5_BURST_ERRORS: u32 = 33686017u32;
pub const OID_802_5_CURRENT_ADDRESS: u32 = 33620226u32;
pub const OID_802_5_CURRENT_FUNCTIONAL: u32 = 33620227u32;
pub const OID_802_5_CURRENT_GROUP: u32 = 33620228u32;
pub const OID_802_5_CURRENT_RING_STATE: u32 = 33620231u32;
pub const OID_802_5_CURRENT_RING_STATUS: u32 = 33620230u32;
pub const OID_802_5_FRAME_COPIED_ERRORS: u32 = 33686020u32;
pub const OID_802_5_FREQUENCY_ERRORS: u32 = 33686021u32;
pub const OID_802_5_INTERNAL_ERRORS: u32 = 33686023u32;
pub const OID_802_5_LAST_OPEN_STATUS: u32 = 33620229u32;
pub const OID_802_5_LINE_ERRORS: u32 = 33685761u32;
pub const OID_802_5_LOST_FRAMES: u32 = 33685762u32;
pub const OID_802_5_PERMANENT_ADDRESS: u32 = 33620225u32;
pub const OID_802_5_TOKEN_ERRORS: u32 = 33686022u32;
pub const OID_ARCNET_CURRENT_ADDRESS: u32 = 100729090u32;
pub const OID_ARCNET_PERMANENT_ADDRESS: u32 = 100729089u32;
pub const OID_ARCNET_RECONFIGURATIONS: u32 = 100794881u32;
pub const OID_ATM_ACQUIRE_ACCESS_NET_RESOURCES: u32 = 134283779u32;
pub const OID_ATM_ALIGNMENT_REQUIRED: u32 = 134283784u32;
pub const OID_ATM_ASSIGNED_VPI: u32 = 134283778u32;
pub const OID_ATM_CALL_ALERTING: u32 = 134283788u32;
pub const OID_ATM_CALL_NOTIFY: u32 = 134283790u32;
pub const OID_ATM_CALL_PROCEEDING: u32 = 134283787u32;
pub const OID_ATM_CELLS_HEC_ERROR: u32 = 134349314u32;
pub const OID_ATM_DIGITAL_BROADCAST_VPIVCI: u32 = 134283782u32;
pub const OID_ATM_GET_NEAREST_FLOW: u32 = 134283783u32;
pub const OID_ATM_HW_CURRENT_ADDRESS: u32 = 134283524u32;
pub const OID_ATM_ILMI_VPIVCI: u32 = 134283781u32;
pub const OID_ATM_LECS_ADDRESS: u32 = 134283785u32;
pub const OID_ATM_MAX_AAL0_PACKET_SIZE: u32 = 134283528u32;
pub const OID_ATM_MAX_AAL1_PACKET_SIZE: u32 = 134283529u32;
pub const OID_ATM_MAX_AAL34_PACKET_SIZE: u32 = 134283530u32;
pub const OID_ATM_MAX_AAL5_PACKET_SIZE: u32 = 134283531u32;
pub const OID_ATM_MAX_ACTIVE_VCI_BITS: u32 = 134283526u32;
pub const OID_ATM_MAX_ACTIVE_VCS: u32 = 134283525u32;
pub const OID_ATM_MAX_ACTIVE_VPI_BITS: u32 = 134283527u32;
pub const OID_ATM_MY_IP_NM_ADDRESS: u32 = 134283791u32;
pub const OID_ATM_PARTY_ALERTING: u32 = 134283789u32;
pub const OID_ATM_RCV_CELLS_DROPPED: u32 = 134349059u32;
pub const OID_ATM_RCV_CELLS_OK: u32 = 134349057u32;
pub const OID_ATM_RCV_INVALID_VPI_VCI: u32 = 134349313u32;
pub const OID_ATM_RCV_REASSEMBLY_ERROR: u32 = 134349315u32;
pub const OID_ATM_RELEASE_ACCESS_NET_RESOURCES: u32 = 134283780u32;
pub const OID_ATM_SERVICE_ADDRESS: u32 = 134283786u32;
pub const OID_ATM_SIGNALING_VPIVCI: u32 = 134283777u32;
pub const OID_ATM_SUPPORTED_AAL_TYPES: u32 = 134283523u32;
pub const OID_ATM_SUPPORTED_SERVICE_CATEGORY: u32 = 134283522u32;
pub const OID_ATM_SUPPORTED_VC_RATES: u32 = 134283521u32;
pub const OID_ATM_XMIT_CELLS_OK: u32 = 134349058u32;
pub const OID_CO_ADDRESS_CHANGE: u32 = 4261412871u32;
pub const OID_CO_ADD_ADDRESS: u32 = 4261412868u32;
pub const OID_CO_ADD_PVC: u32 = 4261412865u32;
pub const OID_CO_AF_CLOSE: u32 = 4261412874u32;
pub const OID_CO_DELETE_ADDRESS: u32 = 4261412869u32;
pub const OID_CO_DELETE_PVC: u32 = 4261412866u32;
pub const OID_CO_GET_ADDRESSES: u32 = 4261412870u32;
pub const OID_CO_GET_CALL_INFORMATION: u32 = 4261412867u32;
pub const OID_CO_SIGNALING_DISABLED: u32 = 4261412873u32;
pub const OID_CO_SIGNALING_ENABLED: u32 = 4261412872u32;
pub const OID_CO_TAPI_ADDRESS_CAPS: u32 = 4261416963u32;
pub const OID_CO_TAPI_CM_CAPS: u32 = 4261416961u32;
pub const OID_CO_TAPI_DONT_REPORT_DIGITS: u32 = 4261416969u32;
pub const OID_CO_TAPI_GET_CALL_DIAGNOSTICS: u32 = 4261416967u32;
pub const OID_CO_TAPI_LINE_CAPS: u32 = 4261416962u32;
pub const OID_CO_TAPI_REPORT_DIGITS: u32 = 4261416968u32;
pub const OID_CO_TAPI_TRANSLATE_NDIS_CALLPARAMS: u32 = 4261416965u32;
pub const OID_CO_TAPI_TRANSLATE_TAPI_CALLPARAMS: u32 = 4261416964u32;
pub const OID_CO_TAPI_TRANSLATE_TAPI_SAP: u32 = 4261416966u32;
pub const OID_FDDI_ATTACHMENT_TYPE: u32 = 50462977u32;
pub const OID_FDDI_DOWNSTREAM_NODE_LONG: u32 = 50462979u32;
pub const OID_FDDI_FRAMES_LOST: u32 = 50462981u32;
pub const OID_FDDI_FRAME_ERRORS: u32 = 50462980u32;
pub const OID_FDDI_IF_ADMIN_STATUS: u32 = 50528894u32;
pub const OID_FDDI_IF_DESCR: u32 = 50528889u32;
pub const OID_FDDI_IF_IN_DISCARDS: u32 = 50528900u32;
pub const OID_FDDI_IF_IN_ERRORS: u32 = 50528901u32;
pub const OID_FDDI_IF_IN_NUCAST_PKTS: u32 = 50528899u32;
pub const OID_FDDI_IF_IN_OCTETS: u32 = 50528897u32;
pub const OID_FDDI_IF_IN_UCAST_PKTS: u32 = 50528898u32;
pub const OID_FDDI_IF_IN_UNKNOWN_PROTOS: u32 = 50528902u32;
pub const OID_FDDI_IF_LAST_CHANGE: u32 = 50528896u32;
pub const OID_FDDI_IF_MTU: u32 = 50528891u32;
pub const OID_FDDI_IF_OPER_STATUS: u32 = 50528895u32;
pub const OID_FDDI_IF_OUT_DISCARDS: u32 = 50528906u32;
pub const OID_FDDI_IF_OUT_ERRORS: u32 = 50528907u32;
pub const OID_FDDI_IF_OUT_NUCAST_PKTS: u32 = 50528905u32;
pub const OID_FDDI_IF_OUT_OCTETS: u32 = 50528903u32;
pub const OID_FDDI_IF_OUT_QLEN: u32 = 50528908u32;
pub const OID_FDDI_IF_OUT_UCAST_PKTS: u32 = 50528904u32;
pub const OID_FDDI_IF_PHYS_ADDRESS: u32 = 50528893u32;
pub const OID_FDDI_IF_SPECIFIC: u32 = 50528909u32;
pub const OID_FDDI_IF_SPEED: u32 = 50528892u32;
pub const OID_FDDI_IF_TYPE: u32 = 50528890u32;
pub const OID_FDDI_LCONNECTION_STATE: u32 = 50462985u32;
pub const OID_FDDI_LCT_FAILURES: u32 = 50462983u32;
pub const OID_FDDI_LEM_REJECTS: u32 = 50462984u32;
pub const OID_FDDI_LONG_CURRENT_ADDR: u32 = 50397442u32;
pub const OID_FDDI_LONG_MAX_LIST_SIZE: u32 = 50397444u32;
pub const OID_FDDI_LONG_MULTICAST_LIST: u32 = 50397443u32;
pub const OID_FDDI_LONG_PERMANENT_ADDR: u32 = 50397441u32;
pub const OID_FDDI_MAC_AVAILABLE_PATHS: u32 = 50528803u32;
pub const OID_FDDI_MAC_BRIDGE_FUNCTIONS: u32 = 50528800u32;
pub const OID_FDDI_MAC_COPIED_CT: u32 = 50528828u32;
pub const OID_FDDI_MAC_CURRENT_PATH: u32 = 50528804u32;
pub const OID_FDDI_MAC_DA_FLAG: u32 = 50528842u32;
pub const OID_FDDI_MAC_DOWNSTREAM_NBR: u32 = 50528806u32;
pub const OID_FDDI_MAC_DOWNSTREAM_PORT_TYPE: u32 = 50528811u32;
pub const OID_FDDI_MAC_DUP_ADDRESS_TEST: u32 = 50528809u32;
pub const OID_FDDI_MAC_ERROR_CT: u32 = 50528831u32;
pub const OID_FDDI_MAC_FRAME_CT: u32 = 50528827u32;
pub const OID_FDDI_MAC_FRAME_ERROR_FLAG: u32 = 50528844u32;
pub const OID_FDDI_MAC_FRAME_ERROR_RATIO: u32 = 50528838u32;
pub const OID_FDDI_MAC_FRAME_ERROR_THRESHOLD: u32 = 50528837u32;
pub const OID_FDDI_MAC_FRAME_STATUS_FUNCTIONS: u32 = 50528799u32;
pub const OID_FDDI_MAC_HARDWARE_PRESENT: u32 = 50528847u32;
pub const OID_FDDI_MAC_INDEX: u32 = 50528812u32;
pub const OID_FDDI_MAC_LATE_CT: u32 = 50528835u32;
pub const OID_FDDI_MAC_LONG_GRP_ADDRESS: u32 = 50528814u32;
pub const OID_FDDI_MAC_LOST_CT: u32 = 50528832u32;
pub const OID_FDDI_MAC_MA_UNITDATA_AVAILABLE: u32 = 50528846u32;
pub const OID_FDDI_MAC_MA_UNITDATA_ENABLE: u32 = 50528848u32;
pub const OID_FDDI_MAC_NOT_COPIED_CT: u32 = 50528834u32;
pub const OID_FDDI_MAC_NOT_COPIED_FLAG: u32 = 50528845u32;
pub const OID_FDDI_MAC_NOT_COPIED_RATIO: u32 = 50528840u32;
pub const OID_FDDI_MAC_NOT_COPIED_THRESHOLD: u32 = 50528839u32;
pub const OID_FDDI_MAC_OLD_DOWNSTREAM_NBR: u32 = 50528808u32;
pub const OID_FDDI_MAC_OLD_UPSTREAM_NBR: u32 = 50528807u32;
pub const OID_FDDI_MAC_REQUESTED_PATHS: u32 = 50528810u32;
pub const OID_FDDI_MAC_RING_OP_CT: u32 = 50528836u32;
pub const OID_FDDI_MAC_RMT_STATE: u32 = 50528841u32;
pub const OID_FDDI_MAC_SHORT_GRP_ADDRESS: u32 = 50528815u32;
pub const OID_FDDI_MAC_SMT_ADDRESS: u32 = 50528813u32;
pub const OID_FDDI_MAC_TOKEN_CT: u32 = 50528830u32;
pub const OID_FDDI_MAC_TRANSMIT_CT: u32 = 50528829u32;
pub const OID_FDDI_MAC_TVX_CAPABILITY: u32 = 50528802u32;
pub const OID_FDDI_MAC_TVX_EXPIRED_CT: u32 = 50528833u32;
pub const OID_FDDI_MAC_TVX_VALUE: u32 = 50528819u32;
pub const OID_FDDI_MAC_T_MAX: u32 = 50528818u32;
pub const OID_FDDI_MAC_T_MAX_CAPABILITY: u32 = 50528801u32;
pub const OID_FDDI_MAC_T_NEG: u32 = 50528817u32;
pub const OID_FDDI_MAC_T_PRI0: u32 = 50528820u32;
pub const OID_FDDI_MAC_T_PRI1: u32 = 50528821u32;
pub const OID_FDDI_MAC_T_PRI2: u32 = 50528822u32;
pub const OID_FDDI_MAC_T_PRI3: u32 = 50528823u32;
pub const OID_FDDI_MAC_T_PRI4: u32 = 50528824u32;
pub const OID_FDDI_MAC_T_PRI5: u32 = 50528825u32;
pub const OID_FDDI_MAC_T_PRI6: u32 = 50528826u32;
pub const OID_FDDI_MAC_T_REQ: u32 = 50528816u32;
pub const OID_FDDI_MAC_UNDA_FLAG: u32 = 50528843u32;
pub const OID_FDDI_MAC_UPSTREAM_NBR: u32 = 50528805u32;
pub const OID_FDDI_PATH_CONFIGURATION: u32 = 50528854u32;
pub const OID_FDDI_PATH_INDEX: u32 = 50528849u32;
pub const OID_FDDI_PATH_MAX_T_REQ: u32 = 50528859u32;
pub const OID_FDDI_PATH_RING_LATENCY: u32 = 50528850u32;
pub const OID_FDDI_PATH_SBA_AVAILABLE: u32 = 50528856u32;
pub const OID_FDDI_PATH_SBA_OVERHEAD: u32 = 50528853u32;
pub const OID_FDDI_PATH_SBA_PAYLOAD: u32 = 50528852u32;
pub const OID_FDDI_PATH_TRACE_STATUS: u32 = 50528851u32;
pub const OID_FDDI_PATH_TVX_LOWER_BOUND: u32 = 50528857u32;
pub const OID_FDDI_PATH_T_MAX_LOWER_BOUND: u32 = 50528858u32;
pub const OID_FDDI_PATH_T_R_MODE: u32 = 50528855u32;
pub const OID_FDDI_PORT_ACTION: u32 = 50528888u32;
pub const OID_FDDI_PORT_AVAILABLE_PATHS: u32 = 50528867u32;
pub const OID_FDDI_PORT_BS_FLAG: u32 = 50528873u32;
pub const OID_FDDI_PORT_CONNECTION_CAPABILITIES: u32 = 50528870u32;
pub const OID_FDDI_PORT_CONNECTION_POLICIES: u32 = 50528862u32;
pub const OID_FDDI_PORT_CONNNECT_STATE: u32 = 50528882u32;
pub const OID_FDDI_PORT_CURRENT_PATH: u32 = 50528864u32;
pub const OID_FDDI_PORT_EB_ERROR_CT: u32 = 50528875u32;
pub const OID_FDDI_PORT_HARDWARE_PRESENT: u32 = 50528886u32;
pub const OID_FDDI_PORT_INDEX: u32 = 50528871u32;
pub const OID_FDDI_PORT_LCT_FAIL_CT: u32 = 50528876u32;
pub const OID_FDDI_PORT_LEM_CT: u32 = 50528879u32;
pub const OID_FDDI_PORT_LEM_REJECT_CT: u32 = 50528878u32;
pub const OID_FDDI_PORT_LER_ALARM: u32 = 50528881u32;
pub const OID_FDDI_PORT_LER_CUTOFF: u32 = 50528880u32;
pub const OID_FDDI_PORT_LER_ESTIMATE: u32 = 50528877u32;
pub const OID_FDDI_PORT_LER_FLAG: u32 = 50528885u32;
pub const OID_FDDI_PORT_MAC_INDICATED: u32 = 50528863u32;
pub const OID_FDDI_PORT_MAC_LOOP_TIME: u32 = 50528868u32;
pub const OID_FDDI_PORT_MAC_PLACEMENT: u32 = 50528866u32;
pub const OID_FDDI_PORT_MAINT_LS: u32 = 50528872u32;
pub const OID_FDDI_PORT_MY_TYPE: u32 = 50528860u32;
pub const OID_FDDI_PORT_NEIGHBOR_TYPE: u32 = 50528861u32;
pub const OID_FDDI_PORT_PCM_STATE: u32 = 50528883u32;
pub const OID_FDDI_PORT_PC_LS: u32 = 50528874u32;
pub const OID_FDDI_PORT_PC_WITHHOLD: u32 = 50528884u32;
pub const OID_FDDI_PORT_PMD_CLASS: u32 = 50528869u32;
pub const OID_FDDI_PORT_REQUESTED_PATHS: u32 = 50528865u32;
pub const OID_FDDI_RING_MGT_STATE: u32 = 50462982u32;
pub const OID_FDDI_SHORT_CURRENT_ADDR: u32 = 50397446u32;
pub const OID_FDDI_SHORT_MAX_LIST_SIZE: u32 = 50397448u32;
pub const OID_FDDI_SHORT_MULTICAST_LIST: u32 = 50397447u32;
pub const OID_FDDI_SHORT_PERMANENT_ADDR: u32 = 50397445u32;
pub const OID_FDDI_SMT_AVAILABLE_PATHS: u32 = 50528779u32;
pub const OID_FDDI_SMT_BYPASS_PRESENT: u32 = 50528788u32;
pub const OID_FDDI_SMT_CF_STATE: u32 = 50528790u32;
pub const OID_FDDI_SMT_CONFIG_CAPABILITIES: u32 = 50528780u32;
pub const OID_FDDI_SMT_CONFIG_POLICY: u32 = 50528781u32;
pub const OID_FDDI_SMT_CONNECTION_POLICY: u32 = 50528782u32;
pub const OID_FDDI_SMT_ECM_STATE: u32 = 50528789u32;
pub const OID_FDDI_SMT_HI_VERSION_ID: u32 = 50528771u32;
pub const OID_FDDI_SMT_HOLD_STATE: u32 = 50528791u32;
pub const OID_FDDI_SMT_LAST_SET_STATION_ID: u32 = 50528798u32;
pub const OID_FDDI_SMT_LO_VERSION_ID: u32 = 50528772u32;
pub const OID_FDDI_SMT_MAC_CT: u32 = 50528776u32;
pub const OID_FDDI_SMT_MAC_INDEXES: u32 = 50528787u32;
pub const OID_FDDI_SMT_MANUFACTURER_DATA: u32 = 50528773u32;
pub const OID_FDDI_SMT_MASTER_CT: u32 = 50528778u32;
pub const OID_FDDI_SMT_MIB_VERSION_ID: u32 = 50528775u32;
pub const OID_FDDI_SMT_MSG_TIME_STAMP: u32 = 50528795u32;
pub const OID_FDDI_SMT_NON_MASTER_CT: u32 = 50528777u32;
pub const OID_FDDI_SMT_OP_VERSION_ID: u32 = 50528770u32;
pub const OID_FDDI_SMT_PEER_WRAP_FLAG: u32 = 50528794u32;
pub const OID_FDDI_SMT_PORT_INDEXES: u32 = 50528786u32;
pub const OID_FDDI_SMT_REMOTE_DISCONNECT_FLAG: u32 = 50528792u32;
pub const OID_FDDI_SMT_SET_COUNT: u32 = 50528797u32;
pub const OID_FDDI_SMT_STATION_ACTION: u32 = 50528887u32;
pub const OID_FDDI_SMT_STATION_ID: u32 = 50528769u32;
pub const OID_FDDI_SMT_STATION_STATUS: u32 = 50528793u32;
pub const OID_FDDI_SMT_STAT_RPT_POLICY: u32 = 50528784u32;
pub const OID_FDDI_SMT_TRACE_MAX_EXPIRATION: u32 = 50528785u32;
pub const OID_FDDI_SMT_TRANSITION_TIME_STAMP: u32 = 50528796u32;
pub const OID_FDDI_SMT_T_NOTIFY: u32 = 50528783u32;
pub const OID_FDDI_SMT_USER_DATA: u32 = 50528774u32;
pub const OID_FDDI_UPSTREAM_NODE_LONG: u32 = 50462978u32;
pub const OID_FFP_ADAPTER_STATS: u32 = 4227990033u32;
pub const OID_FFP_CONTROL: u32 = 4227924498u32;
pub const OID_FFP_DATA: u32 = 4227924500u32;
pub const OID_FFP_DRIVER_STATS: u32 = 4227990032u32;
pub const OID_FFP_FLUSH: u32 = 4227924497u32;
pub const OID_FFP_PARAMS: u32 = 4227924499u32;
pub const OID_FFP_SUPPORT: u32 = 4227924496u32;
pub const OID_GEN_ADMIN_STATUS: u32 = 66184u32;
pub const OID_GEN_ALIAS: u32 = 66185u32;
pub const OID_GEN_BROADCAST_BYTES_RCV: u32 = 131595u32;
pub const OID_GEN_BROADCAST_BYTES_XMIT: u32 = 131589u32;
pub const OID_GEN_BROADCAST_FRAMES_RCV: u32 = 131596u32;
pub const OID_GEN_BROADCAST_FRAMES_XMIT: u32 = 131590u32;
pub const OID_GEN_BYTES_RCV: u32 = 131609u32;
pub const OID_GEN_BYTES_XMIT: u32 = 131610u32;
pub const OID_GEN_CO_BYTES_RCV: u32 = 131591u32;
pub const OID_GEN_CO_BYTES_XMIT: u32 = 131585u32;
pub const OID_GEN_CO_BYTES_XMIT_OUTSTANDING: u32 = 131617u32;
pub const OID_GEN_CO_DEVICE_PROFILE: u32 = 131602u32;
pub const OID_GEN_CO_DRIVER_VERSION: u32 = 65808u32;
pub const OID_GEN_CO_GET_NETCARD_TIME: u32 = 131600u32;
pub const OID_GEN_CO_GET_TIME_CAPS: u32 = 131599u32;
pub const OID_GEN_CO_HARDWARE_STATUS: u32 = 65794u32;
pub const OID_GEN_CO_LINK_SPEED: u32 = 65799u32;
pub const OID_GEN_CO_MAC_OPTIONS: u32 = 65811u32;
pub const OID_GEN_CO_MEDIA_CONNECT_STATUS: u32 = 65812u32;
pub const OID_GEN_CO_MEDIA_IN_USE: u32 = 65796u32;
pub const OID_GEN_CO_MEDIA_SUPPORTED: u32 = 65795u32;
pub const OID_GEN_CO_MINIMUM_LINK_SPEED: u32 = 131360u32;
pub const OID_GEN_CO_NETCARD_LOAD: u32 = 131601u32;
pub const OID_GEN_CO_PROTOCOL_OPTIONS: u32 = 65810u32;
pub const OID_GEN_CO_RCV_CRC_ERROR: u32 = 131597u32;
pub const OID_GEN_CO_RCV_PDUS_ERROR: u32 = 131332u32;
pub const OID_GEN_CO_RCV_PDUS_NO_BUFFER: u32 = 131333u32;
pub const OID_GEN_CO_RCV_PDUS_OK: u32 = 131330u32;
pub const OID_GEN_CO_SUPPORTED_GUIDS: u32 = 65815u32;
pub const OID_GEN_CO_SUPPORTED_LIST: u32 = 65793u32;
pub const OID_GEN_CO_TRANSMIT_QUEUE_LENGTH: u32 = 131598u32;
pub const OID_GEN_CO_VENDOR_DESCRIPTION: u32 = 65805u32;
pub const OID_GEN_CO_VENDOR_DRIVER_VERSION: u32 = 65814u32;
pub const OID_GEN_CO_VENDOR_ID: u32 = 65804u32;
pub const OID_GEN_CO_XMIT_PDUS_ERROR: u32 = 131331u32;
pub const OID_GEN_CO_XMIT_PDUS_OK: u32 = 131329u32;
pub const OID_GEN_CURRENT_LOOKAHEAD: u32 = 65807u32;
pub const OID_GEN_CURRENT_PACKET_FILTER: u32 = 65806u32;
pub const OID_GEN_DEVICE_PROFILE: u32 = 131602u32;
pub const OID_GEN_DIRECTED_BYTES_RCV: u32 = 131591u32;
pub const OID_GEN_DIRECTED_BYTES_XMIT: u32 = 131585u32;
pub const OID_GEN_DIRECTED_FRAMES_RCV: u32 = 131592u32;
pub const OID_GEN_DIRECTED_FRAMES_XMIT: u32 = 131586u32;
pub const OID_GEN_DISCONTINUITY_TIME: u32 = 66178u32;
pub const OID_GEN_DRIVER_VERSION: u32 = 65808u32;
pub const OID_GEN_ENUMERATE_PORTS: u32 = 66061u32;
pub const OID_GEN_FRIENDLY_NAME: u32 = 131606u32;
pub const OID_GEN_GET_NETCARD_TIME: u32 = 131600u32;
pub const OID_GEN_GET_TIME_CAPS: u32 = 131599u32;
pub const OID_GEN_HARDWARE_STATUS: u32 = 65794u32;
pub const OID_GEN_HD_SPLIT_CURRENT_CONFIG: u32 = 66080u32;
pub const OID_GEN_HD_SPLIT_PARAMETERS: u32 = 66078u32;
pub const OID_GEN_INIT_TIME_MS: u32 = 131603u32;
pub const OID_GEN_INTERFACE_INFO: u32 = 66183u32;
pub const OID_GEN_INTERRUPT_MODERATION: u32 = 66057u32;
pub const OID_GEN_IP_OPER_STATUS: u32 = 66189u32;
pub const OID_GEN_ISOLATION_PARAMETERS: u32 = 66304u32;
pub const OID_GEN_LAST_CHANGE: u32 = 66177u32;
pub const OID_GEN_LINK_PARAMETERS: u32 = 66056u32;
pub const OID_GEN_LINK_SPEED: u32 = 65799u32;
pub const OID_GEN_LINK_SPEED_EX: u32 = 66187u32;
pub const OID_GEN_LINK_STATE: u32 = 66055u32;
pub const OID_GEN_MACHINE_NAME: u32 = 66074u32;
pub const OID_GEN_MAC_ADDRESS: u32 = 66053u32;
pub const OID_GEN_MAC_OPTIONS: u32 = 65811u32;
pub const OID_GEN_MAXIMUM_FRAME_SIZE: u32 = 65798u32;
pub const OID_GEN_MAXIMUM_LOOKAHEAD: u32 = 65797u32;
pub const OID_GEN_MAXIMUM_SEND_PACKETS: u32 = 65813u32;
pub const OID_GEN_MAXIMUM_TOTAL_SIZE: u32 = 65809u32;
pub const OID_GEN_MAX_LINK_SPEED: u32 = 66054u32;
pub const OID_GEN_MEDIA_CAPABILITIES: u32 = 66049u32;
pub const OID_GEN_MEDIA_CONNECT_STATUS: u32 = 65812u32;
pub const OID_GEN_MEDIA_CONNECT_STATUS_EX: u32 = 66186u32;
pub const OID_GEN_MEDIA_DUPLEX_STATE: u32 = 66188u32;
pub const OID_GEN_MEDIA_IN_USE: u32 = 65796u32;
pub const OID_GEN_MEDIA_SENSE_COUNTS: u32 = 131605u32;
pub const OID_GEN_MEDIA_SUPPORTED: u32 = 65795u32;
pub const OID_GEN_MINIPORT_RESTART_ATTRIBUTES: u32 = 66077u32;
pub const OID_GEN_MULTICAST_BYTES_RCV: u32 = 131593u32;
pub const OID_GEN_MULTICAST_BYTES_XMIT: u32 = 131587u32;
pub const OID_GEN_MULTICAST_FRAMES_RCV: u32 = 131594u32;
pub const OID_GEN_MULTICAST_FRAMES_XMIT: u32 = 131588u32;
pub const OID_GEN_NDIS_RESERVED_1: u32 = 131607u32;
pub const OID_GEN_NDIS_RESERVED_2: u32 = 131608u32;
pub const OID_GEN_NDIS_RESERVED_3: u32 = 66058u32;
pub const OID_GEN_NDIS_RESERVED_4: u32 = 66059u32;
pub const OID_GEN_NDIS_RESERVED_5: u32 = 66060u32;
pub const OID_GEN_NDIS_RESERVED_6: u32 = 66066u32;
pub const OID_GEN_NDIS_RESERVED_7: u32 = 131614u32;
pub const OID_GEN_NETCARD_LOAD: u32 = 131601u32;
pub const OID_GEN_NETWORK_LAYER_ADDRESSES: u32 = 65816u32;
pub const OID_GEN_OPERATIONAL_STATUS: u32 = 66179u32;
pub const OID_GEN_PCI_DEVICE_CUSTOM_PROPERTIES: u32 = 66065u32;
pub const OID_GEN_PHYSICAL_MEDIUM: u32 = 66050u32;
pub const OID_GEN_PHYSICAL_MEDIUM_EX: u32 = 66067u32;
pub const OID_GEN_PORT_AUTHENTICATION_PARAMETERS: u32 = 66063u32;
pub const OID_GEN_PORT_STATE: u32 = 66062u32;
pub const OID_GEN_PROMISCUOUS_MODE: u32 = 66176u32;
pub const OID_GEN_PROTOCOL_OPTIONS: u32 = 65810u32;
pub const OID_GEN_RCV_CRC_ERROR: u32 = 131597u32;
pub const OID_GEN_RCV_DISCARDS: u32 = 131611u32;
pub const OID_GEN_RCV_ERROR: u32 = 131332u32;
pub const OID_GEN_RCV_LINK_SPEED: u32 = 66181u32;
pub const OID_GEN_RCV_NO_BUFFER: u32 = 131333u32;
pub const OID_GEN_RCV_OK: u32 = 131330u32;
pub const OID_GEN_RECEIVE_BLOCK_SIZE: u32 = 65803u32;
pub const OID_GEN_RECEIVE_BUFFER_SPACE: u32 = 65801u32;
pub const OID_GEN_RECEIVE_HASH: u32 = 66079u32;
pub const OID_GEN_RECEIVE_SCALE_CAPABILITIES: u32 = 66051u32;
pub const OID_GEN_RECEIVE_SCALE_PARAMETERS: u32 = 66052u32;
pub const OID_GEN_RECEIVE_SCALE_PARAMETERS_V2: u32 = 66068u32;
pub const OID_GEN_RESET_COUNTS: u32 = 131604u32;
pub const OID_GEN_RNDIS_CONFIG_PARAMETER: u32 = 66075u32;
pub const OID_GEN_RSS_SET_INDIRECTION_TABLE_ENTRIES: u32 = 66240u32;
pub const OID_GEN_STATISTICS: u32 = 131334u32;
pub const OID_GEN_SUPPORTED_GUIDS: u32 = 65815u32;
pub const OID_GEN_SUPPORTED_LIST: u32 = 65793u32;
pub const OID_GEN_TIMEOUT_DPC_REQUEST_CAPABILITIES: u32 = 66064u32;
pub const OID_GEN_TRANSMIT_BLOCK_SIZE: u32 = 65802u32;
pub const OID_GEN_TRANSMIT_BUFFER_SPACE: u32 = 65800u32;
pub const OID_GEN_TRANSMIT_QUEUE_LENGTH: u32 = 131598u32;
pub const OID_GEN_TRANSPORT_HEADER_OFFSET: u32 = 65817u32;
pub const OID_GEN_UNKNOWN_PROTOS: u32 = 66182u32;
pub const OID_GEN_VENDOR_DESCRIPTION: u32 = 65805u32;
pub const OID_GEN_VENDOR_DRIVER_VERSION: u32 = 65814u32;
pub const OID_GEN_VENDOR_ID: u32 = 65804u32;
pub const OID_GEN_VLAN_ID: u32 = 66076u32;
pub const OID_GEN_XMIT_DISCARDS: u32 = 131612u32;
pub const OID_GEN_XMIT_ERROR: u32 = 131331u32;
pub const OID_GEN_XMIT_LINK_SPEED: u32 = 66180u32;
pub const OID_GEN_XMIT_OK: u32 = 131329u32;
pub const OID_GFT_ACTIVATE_FLOW_ENTRIES: u32 = 66575u32;
pub const OID_GFT_ADD_FLOW_ENTRIES: u32 = 66572u32;
pub const OID_GFT_ALLOCATE_COUNTERS: u32 = 66567u32;
pub const OID_GFT_COUNTER_VALUES: u32 = 66570u32;
pub const OID_GFT_CREATE_LOGICAL_VPORT: u32 = 66584u32;
pub const OID_GFT_CREATE_TABLE: u32 = 66564u32;
pub const OID_GFT_CURRENT_CAPABILITIES: u32 = 66562u32;
pub const OID_GFT_DEACTIVATE_FLOW_ENTRIES: u32 = 66576u32;
pub const OID_GFT_DELETE_FLOW_ENTRIES: u32 = 66573u32;
pub const OID_GFT_DELETE_LOGICAL_VPORT: u32 = 66585u32;
pub const OID_GFT_DELETE_PROFILE: u32 = 66582u32;
pub const OID_GFT_DELETE_TABLE: u32 = 66565u32;
pub const OID_GFT_ENUM_COUNTERS: u32 = 66569u32;
pub const OID_GFT_ENUM_FLOW_ENTRIES: u32 = 66574u32;
pub const OID_GFT_ENUM_LOGICAL_VPORTS: u32 = 66586u32;
pub const OID_GFT_ENUM_PROFILES: u32 = 66581u32;
pub const OID_GFT_ENUM_TABLES: u32 = 66566u32;
pub const OID_GFT_EXACT_MATCH_PROFILE: u32 = 66578u32;
pub const OID_GFT_FLOW_ENTRY_PARAMETERS: u32 = 66577u32;
pub const OID_GFT_FREE_COUNTERS: u32 = 66568u32;
pub const OID_GFT_GLOBAL_PARAMETERS: u32 = 66563u32;
pub const OID_GFT_HARDWARE_CAPABILITIES: u32 = 66561u32;
pub const OID_GFT_HEADER_TRANSPOSITION_PROFILE: u32 = 66579u32;
pub const OID_GFT_STATISTICS: u32 = 66571u32;
pub const OID_GFT_VPORT_PARAMETERS: u32 = 66583u32;
pub const OID_GFT_WILDCARD_MATCH_PROFILE: u32 = 66580u32;
pub const OID_IP4_OFFLOAD_STATS: u32 = 4227924489u32;
pub const OID_IP6_OFFLOAD_STATS: u32 = 4227924490u32;
pub const OID_IRDA_EXTRA_RCV_BOFS: u32 = 167838208u32;
pub const OID_IRDA_LINK_SPEED: u32 = 167837955u32;
pub const OID_IRDA_MAX_RECEIVE_WINDOW_SIZE: u32 = 167838212u32;
pub const OID_IRDA_MAX_SEND_WINDOW_SIZE: u32 = 167838213u32;
pub const OID_IRDA_MAX_UNICAST_LIST_SIZE: u32 = 167838211u32;
pub const OID_IRDA_MEDIA_BUSY: u32 = 167837956u32;
pub const OID_IRDA_RATE_SNIFF: u32 = 167838209u32;
pub const OID_IRDA_RECEIVING: u32 = 167837952u32;
pub const OID_IRDA_RESERVED1: u32 = 167838218u32;
pub const OID_IRDA_RESERVED2: u32 = 167838223u32;
pub const OID_IRDA_SUPPORTED_SPEEDS: u32 = 167837954u32;
pub const OID_IRDA_TURNAROUND_TIME: u32 = 167837953u32;
pub const OID_IRDA_UNICAST_LIST: u32 = 167838210u32;
pub const OID_KDNET_ADD_PF: u32 = 131619u32;
pub const OID_KDNET_ENUMERATE_PFS: u32 = 131618u32;
pub const OID_KDNET_QUERY_PF_INFORMATION: u32 = 131621u32;
pub const OID_KDNET_REMOVE_PF: u32 = 131620u32;
pub const OID_LTALK_COLLISIONS: u32 = 84017666u32;
pub const OID_LTALK_CURRENT_NODE_ID: u32 = 83951874u32;
pub const OID_LTALK_DEFERS: u32 = 84017667u32;
pub const OID_LTALK_FCS_ERRORS: u32 = 84017670u32;
pub const OID_LTALK_IN_BROADCASTS: u32 = 84017409u32;
pub const OID_LTALK_IN_LENGTH_ERRORS: u32 = 84017410u32;
pub const OID_LTALK_NO_DATA_ERRORS: u32 = 84017668u32;
pub const OID_LTALK_OUT_NO_HANDLERS: u32 = 84017665u32;
pub const OID_LTALK_RANDOM_CTS_ERRORS: u32 = 84017669u32;
pub const OID_NDK_CONNECTIONS: u32 = 4228121091u32;
pub const OID_NDK_LOCAL_ENDPOINTS: u32 = 4228121092u32;
pub const OID_NDK_SET_STATE: u32 = 4228121089u32;
pub const OID_NDK_STATISTICS: u32 = 4228121090u32;
pub const OID_NIC_SWITCH_ALLOCATE_VF: u32 = 66117u32;
pub const OID_NIC_SWITCH_CREATE_SWITCH: u32 = 66103u32;
pub const OID_NIC_SWITCH_CREATE_VPORT: u32 = 66113u32;
pub const OID_NIC_SWITCH_CURRENT_CAPABILITIES: u32 = 66095u32;
pub const OID_NIC_SWITCH_DELETE_SWITCH: u32 = 66105u32;
pub const OID_NIC_SWITCH_DELETE_VPORT: u32 = 66116u32;
pub const OID_NIC_SWITCH_ENUM_SWITCHES: u32 = 66112u32;
pub const OID_NIC_SWITCH_ENUM_VFS: u32 = 66120u32;
pub const OID_NIC_SWITCH_ENUM_VPORTS: u32 = 66115u32;
pub const OID_NIC_SWITCH_FREE_VF: u32 = 66118u32;
pub const OID_NIC_SWITCH_HARDWARE_CAPABILITIES: u32 = 66094u32;
pub const OID_NIC_SWITCH_PARAMETERS: u32 = 66104u32;
pub const OID_NIC_SWITCH_VF_PARAMETERS: u32 = 66119u32;
pub const OID_NIC_SWITCH_VPORT_PARAMETERS: u32 = 66114u32;
pub const OID_OFFLOAD_ENCAPSULATION: u32 = 16843018u32;
pub const OID_PACKET_COALESCING_FILTER_MATCH_COUNT: u32 = 66101u32;
pub const OID_PD_CLOSE_PROVIDER: u32 = 66818u32;
pub const OID_PD_OPEN_PROVIDER: u32 = 66817u32;
pub const OID_PD_QUERY_CURRENT_CONFIG: u32 = 66819u32;
pub const OID_PM_ADD_PROTOCOL_OFFLOAD: u32 = 4244701453u32;
pub const OID_PM_ADD_WOL_PATTERN: u32 = 4244701450u32;
pub const OID_PM_CURRENT_CAPABILITIES: u32 = 4244701447u32;
pub const OID_PM_GET_PROTOCOL_OFFLOAD: u32 = 4244701454u32;
pub const OID_PM_HARDWARE_CAPABILITIES: u32 = 4244701448u32;
pub const OID_PM_PARAMETERS: u32 = 4244701449u32;
pub const OID_PM_PROTOCOL_OFFLOAD_LIST: u32 = 4244701456u32;
pub const OID_PM_REMOVE_PROTOCOL_OFFLOAD: u32 = 4244701455u32;
pub const OID_PM_REMOVE_WOL_PATTERN: u32 = 4244701451u32;
pub const OID_PM_RESERVED_1: u32 = 4244701457u32;
pub const OID_PM_WOL_PATTERN_LIST: u32 = 4244701452u32;
pub const OID_PNP_ADD_WAKE_UP_PATTERN: u32 = 4244701443u32;
pub const OID_PNP_CAPABILITIES: u32 = 4244701440u32;
pub const OID_PNP_ENABLE_WAKE_UP: u32 = 4244701446u32;
pub const OID_PNP_QUERY_POWER: u32 = 4244701442u32;
pub const OID_PNP_REMOVE_WAKE_UP_PATTERN: u32 = 4244701444u32;
pub const OID_PNP_SET_POWER: u32 = 4244701441u32;
pub const OID_PNP_WAKE_UP_ERROR: u32 = 4244767233u32;
pub const OID_PNP_WAKE_UP_OK: u32 = 4244767232u32;
pub const OID_PNP_WAKE_UP_PATTERN_LIST: u32 = 4244701445u32;
pub const OID_QOS_CURRENT_CAPABILITIES: u32 = 4228186114u32;
pub const OID_QOS_HARDWARE_CAPABILITIES: u32 = 4228186113u32;
pub const OID_QOS_OFFLOAD_CREATE_SQ: u32 = 67075u32;
pub const OID_QOS_OFFLOAD_CURRENT_CAPABILITIES: u32 = 67074u32;
pub const OID_QOS_OFFLOAD_DELETE_SQ: u32 = 67076u32;
pub const OID_QOS_OFFLOAD_ENUM_SQS: u32 = 67078u32;
pub const OID_QOS_OFFLOAD_HARDWARE_CAPABILITIES: u32 = 67073u32;
pub const OID_QOS_OFFLOAD_SQ_STATS: u32 = 67079u32;
pub const OID_QOS_OFFLOAD_UPDATE_SQ: u32 = 67077u32;
pub const OID_QOS_OPERATIONAL_PARAMETERS: u32 = 4228186116u32;
pub const OID_QOS_PARAMETERS: u32 = 4228186115u32;
pub const OID_QOS_REMOTE_PARAMETERS: u32 = 4228186117u32;
pub const OID_QOS_RESERVED1: u32 = 4211147008u32;
pub const OID_QOS_RESERVED10: u32 = 4211147017u32;
pub const OID_QOS_RESERVED11: u32 = 4211147018u32;
pub const OID_QOS_RESERVED12: u32 = 4211147019u32;
pub const OID_QOS_RESERVED13: u32 = 4211147020u32;
pub const OID_QOS_RESERVED14: u32 = 4211147021u32;
pub const OID_QOS_RESERVED15: u32 = 4211147022u32;
pub const OID_QOS_RESERVED16: u32 = 4211147023u32;
pub const OID_QOS_RESERVED17: u32 = 4211147024u32;
pub const OID_QOS_RESERVED18: u32 = 4211147025u32;
pub const OID_QOS_RESERVED19: u32 = 4211147026u32;
pub const OID_QOS_RESERVED2: u32 = 4211147009u32;
pub const OID_QOS_RESERVED20: u32 = 4211147027u32;
pub const OID_QOS_RESERVED3: u32 = 4211147010u32;
pub const OID_QOS_RESERVED4: u32 = 4211147011u32;
pub const OID_QOS_RESERVED5: u32 = 4211147012u32;
pub const OID_QOS_RESERVED6: u32 = 4211147013u32;
pub const OID_QOS_RESERVED7: u32 = 4211147014u32;
pub const OID_QOS_RESERVED8: u32 = 4211147015u32;
pub const OID_QOS_RESERVED9: u32 = 4211147016u32;
pub const OID_RECEIVE_FILTER_ALLOCATE_QUEUE: u32 = 66083u32;
pub const OID_RECEIVE_FILTER_CLEAR_FILTER: u32 = 66088u32;
pub const OID_RECEIVE_FILTER_CURRENT_CAPABILITIES: u32 = 66093u32;
pub const OID_RECEIVE_FILTER_ENUM_FILTERS: u32 = 66089u32;
pub const OID_RECEIVE_FILTER_ENUM_QUEUES: u32 = 66085u32;
pub const OID_RECEIVE_FILTER_FREE_QUEUE: u32 = 66084u32;
pub const OID_RECEIVE_FILTER_GLOBAL_PARAMETERS: u32 = 66082u32;
pub const OID_RECEIVE_FILTER_HARDWARE_CAPABILITIES: u32 = 66081u32;
pub const OID_RECEIVE_FILTER_MOVE_FILTER: u32 = 66096u32;
pub const OID_RECEIVE_FILTER_PARAMETERS: u32 = 66090u32;
pub const OID_RECEIVE_FILTER_QUEUE_ALLOCATION_COMPLETE: u32 = 66091u32;
pub const OID_RECEIVE_FILTER_QUEUE_PARAMETERS: u32 = 66086u32;
pub const OID_RECEIVE_FILTER_SET_FILTER: u32 = 66087u32;
pub const OID_SRIOV_BAR_RESOURCES: u32 = 66137u32;
pub const OID_SRIOV_CONFIG_STATE: u32 = 66145u32;
pub const OID_SRIOV_CURRENT_CAPABILITIES: u32 = 66128u32;
pub const OID_SRIOV_HARDWARE_CAPABILITIES: u32 = 66121u32;
pub const OID_SRIOV_OVERLYING_ADAPTER_INFO: u32 = 66152u32;
pub const OID_SRIOV_PF_LUID: u32 = 66144u32;
pub const OID_SRIOV_PROBED_BARS: u32 = 66136u32;
pub const OID_SRIOV_READ_VF_CONFIG_BLOCK: u32 = 66131u32;
pub const OID_SRIOV_READ_VF_CONFIG_SPACE: u32 = 66129u32;
pub const OID_SRIOV_RESET_VF: u32 = 66133u32;
pub const OID_SRIOV_SET_VF_POWER_STATE: u32 = 66134u32;
pub const OID_SRIOV_VF_INVALIDATE_CONFIG_BLOCK: u32 = 66153u32;
pub const OID_SRIOV_VF_SERIAL_NUMBER: u32 = 66146u32;
pub const OID_SRIOV_VF_VENDOR_DEVICE_ID: u32 = 66135u32;
pub const OID_SRIOV_WRITE_VF_CONFIG_BLOCK: u32 = 66132u32;
pub const OID_SRIOV_WRITE_VF_CONFIG_SPACE: u32 = 66130u32;
pub const OID_SWITCH_FEATURE_STATUS_QUERY: u32 = 66151u32;
pub const OID_SWITCH_NIC_ARRAY: u32 = 66167u32;
pub const OID_SWITCH_NIC_CONNECT: u32 = 66171u32;
pub const OID_SWITCH_NIC_CREATE: u32 = 66170u32;
pub const OID_SWITCH_NIC_DELETE: u32 = 66173u32;
pub const OID_SWITCH_NIC_DIRECT_REQUEST: u32 = 66198u32;
pub const OID_SWITCH_NIC_DISCONNECT: u32 = 66172u32;
pub const OID_SWITCH_NIC_REQUEST: u32 = 66160u32;
pub const OID_SWITCH_NIC_RESTORE: u32 = 66194u32;
pub const OID_SWITCH_NIC_RESTORE_COMPLETE: u32 = 66195u32;
pub const OID_SWITCH_NIC_RESUME: u32 = 66200u32;
pub const OID_SWITCH_NIC_SAVE: u32 = 66192u32;
pub const OID_SWITCH_NIC_SAVE_COMPLETE: u32 = 66193u32;
pub const OID_SWITCH_NIC_SUSPEND: u32 = 66199u32;
pub const OID_SWITCH_NIC_SUSPENDED_LM_SOURCE_FINISHED: u32 = 66202u32;
pub const OID_SWITCH_NIC_SUSPENDED_LM_SOURCE_STARTED: u32 = 66201u32;
pub const OID_SWITCH_NIC_UPDATED: u32 = 66196u32;
pub const OID_SWITCH_PARAMETERS: u32 = 66165u32;
pub const OID_SWITCH_PORT_ARRAY: u32 = 66166u32;
pub const OID_SWITCH_PORT_CREATE: u32 = 66168u32;
pub const OID_SWITCH_PORT_DELETE: u32 = 66169u32;
pub const OID_SWITCH_PORT_FEATURE_STATUS_QUERY: u32 = 66174u32;
pub const OID_SWITCH_PORT_PROPERTY_ADD: u32 = 66161u32;
pub const OID_SWITCH_PORT_PROPERTY_DELETE: u32 = 66163u32;
pub const OID_SWITCH_PORT_PROPERTY_ENUM: u32 = 66164u32;
pub const OID_SWITCH_PORT_PROPERTY_UPDATE: u32 = 66162u32;
pub const OID_SWITCH_PORT_TEARDOWN: u32 = 66175u32;
pub const OID_SWITCH_PORT_UPDATED: u32 = 66197u32;
pub const OID_SWITCH_PROPERTY_ADD: u32 = 66147u32;
pub const OID_SWITCH_PROPERTY_DELETE: u32 = 66149u32;
pub const OID_SWITCH_PROPERTY_ENUM: u32 = 66150u32;
pub const OID_SWITCH_PROPERTY_UPDATE: u32 = 66148u32;
pub const OID_TAPI_ACCEPT: u32 = 117637377u32;
pub const OID_TAPI_ANSWER: u32 = 117637378u32;
pub const OID_TAPI_CLOSE: u32 = 117637379u32;
pub const OID_TAPI_CLOSE_CALL: u32 = 117637380u32;
pub const OID_TAPI_CONDITIONAL_MEDIA_DETECTION: u32 = 117637381u32;
pub const OID_TAPI_CONFIG_DIALOG: u32 = 117637382u32;
pub const OID_TAPI_DEV_SPECIFIC: u32 = 117637383u32;
pub const OID_TAPI_DIAL: u32 = 117637384u32;
pub const OID_TAPI_DROP: u32 = 117637385u32;
pub const OID_TAPI_GATHER_DIGITS: u32 = 117637411u32;
pub const OID_TAPI_GET_ADDRESS_CAPS: u32 = 117637386u32;
pub const OID_TAPI_GET_ADDRESS_ID: u32 = 117637387u32;
pub const OID_TAPI_GET_ADDRESS_STATUS: u32 = 117637388u32;
pub const OID_TAPI_GET_CALL_ADDRESS_ID: u32 = 117637389u32;
pub const OID_TAPI_GET_CALL_INFO: u32 = 117637390u32;
pub const OID_TAPI_GET_CALL_STATUS: u32 = 117637391u32;
pub const OID_TAPI_GET_DEV_CAPS: u32 = 117637392u32;
pub const OID_TAPI_GET_DEV_CONFIG: u32 = 117637393u32;
pub const OID_TAPI_GET_EXTENSION_ID: u32 = 117637394u32;
pub const OID_TAPI_GET_ID: u32 = 117637395u32;
pub const OID_TAPI_GET_LINE_DEV_STATUS: u32 = 117637396u32;
pub const OID_TAPI_MAKE_CALL: u32 = 117637397u32;
pub const OID_TAPI_MONITOR_DIGITS: u32 = 117637412u32;
pub const OID_TAPI_NEGOTIATE_EXT_VERSION: u32 = 117637398u32;
pub const OID_TAPI_OPEN: u32 = 117637399u32;
pub const OID_TAPI_PROVIDER_INITIALIZE: u32 = 117637400u32;
pub const OID_TAPI_PROVIDER_SHUTDOWN: u32 = 117637401u32;
pub const OID_TAPI_SECURE_CALL: u32 = 117637402u32;
pub const OID_TAPI_SELECT_EXT_VERSION: u32 = 117637403u32;
pub const OID_TAPI_SEND_USER_USER_INFO: u32 = 117637404u32;
pub const OID_TAPI_SET_APP_SPECIFIC: u32 = 117637405u32;
pub const OID_TAPI_SET_CALL_PARAMS: u32 = 117637406u32;
pub const OID_TAPI_SET_DEFAULT_MEDIA_DETECTION: u32 = 117637407u32;
pub const OID_TAPI_SET_DEV_CONFIG: u32 = 117637408u32;
pub const OID_TAPI_SET_MEDIA_MODE: u32 = 117637409u32;
pub const OID_TAPI_SET_STATUS_MESSAGES: u32 = 117637410u32;
pub const OID_TCP4_OFFLOAD_STATS: u32 = 4227924487u32;
pub const OID_TCP6_OFFLOAD_STATS: u32 = 4227924488u32;
pub const OID_TCP_CONNECTION_OFFLOAD_CURRENT_CONFIG: u32 = 4227924494u32;
pub const OID_TCP_CONNECTION_OFFLOAD_HARDWARE_CAPABILITIES: u32 = 4227924495u32;
pub const OID_TCP_CONNECTION_OFFLOAD_PARAMETERS: u32 = 4228055553u32;
pub const OID_TCP_OFFLOAD_CURRENT_CONFIG: u32 = 4227924491u32;
pub const OID_TCP_OFFLOAD_HARDWARE_CAPABILITIES: u32 = 4227924493u32;
pub const OID_TCP_OFFLOAD_PARAMETERS: u32 = 4227924492u32;
pub const OID_TCP_RSC_STATISTICS: u32 = 131613u32;
pub const OID_TCP_SAN_SUPPORT: u32 = 4227924484u32;
pub const OID_TCP_TASK_IPSEC_ADD_SA: u32 = 4227924482u32;
pub const OID_TCP_TASK_IPSEC_ADD_UDPESP_SA: u32 = 4227924485u32;
pub const OID_TCP_TASK_IPSEC_DELETE_SA: u32 = 4227924483u32;
pub const OID_TCP_TASK_IPSEC_DELETE_UDPESP_SA: u32 = 4227924486u32;
pub const OID_TCP_TASK_IPSEC_OFFLOAD_V2_ADD_SA: u32 = 4228055554u32;
pub const OID_TCP_TASK_IPSEC_OFFLOAD_V2_ADD_SA_EX: u32 = 4228055557u32;
pub const OID_TCP_TASK_IPSEC_OFFLOAD_V2_DELETE_SA: u32 = 4228055555u32;
pub const OID_TCP_TASK_IPSEC_OFFLOAD_V2_UPDATE_SA: u32 = 4228055556u32;
pub const OID_TCP_TASK_OFFLOAD: u32 = 4227924481u32;
pub const OID_TIMESTAMP_CAPABILITY: u32 = 10485761u32;
pub const OID_TIMESTAMP_CURRENT_CONFIG: u32 = 10485762u32;
pub const OID_TIMESTAMP_GET_CROSSTIMESTAMP: u32 = 10485763u32;
pub const OID_TUNNEL_INTERFACE_RELEASE_OID: u32 = 251724039u32;
pub const OID_TUNNEL_INTERFACE_SET_OID: u32 = 251724038u32;
pub const OID_VLAN_RESERVED1: u32 = 66097u32;
pub const OID_VLAN_RESERVED2: u32 = 66098u32;
pub const OID_VLAN_RESERVED3: u32 = 66099u32;
pub const OID_VLAN_RESERVED4: u32 = 66100u32;
pub const OID_WAN_CO_GET_COMP_INFO: u32 = 67175040u32;
pub const OID_WAN_CO_GET_INFO: u32 = 67174784u32;
pub const OID_WAN_CO_GET_LINK_INFO: u32 = 67174786u32;
pub const OID_WAN_CO_GET_STATS_INFO: u32 = 67175042u32;
pub const OID_WAN_CO_SET_COMP_INFO: u32 = 67175041u32;
pub const OID_WAN_CO_SET_LINK_INFO: u32 = 67174785u32;
pub const OID_WAN_CURRENT_ADDRESS: u32 = 67174658u32;
pub const OID_WAN_GET_BRIDGE_INFO: u32 = 67174922u32;
pub const OID_WAN_GET_COMP_INFO: u32 = 67174924u32;
pub const OID_WAN_GET_INFO: u32 = 67174663u32;
pub const OID_WAN_GET_LINK_INFO: u32 = 67174665u32;
pub const OID_WAN_GET_STATS_INFO: u32 = 67174926u32;
pub const OID_WAN_HEADER_FORMAT: u32 = 67174662u32;
pub const OID_WAN_LINE_COUNT: u32 = 67174666u32;
pub const OID_WAN_MEDIUM_SUBTYPE: u32 = 67174661u32;
pub const OID_WAN_PERMANENT_ADDRESS: u32 = 67174657u32;
pub const OID_WAN_PROTOCOL_CAPS: u32 = 67174667u32;
pub const OID_WAN_PROTOCOL_TYPE: u32 = 67174660u32;
pub const OID_WAN_QUALITY_OF_SERVICE: u32 = 67174659u32;
pub const OID_WAN_SET_BRIDGE_INFO: u32 = 67174923u32;
pub const OID_WAN_SET_COMP_INFO: u32 = 67174925u32;
pub const OID_WAN_SET_LINK_INFO: u32 = 67174664u32;
pub const OID_WWAN_AUTH_CHALLENGE: u32 = 234946837u32;
pub const OID_WWAN_BASE_STATIONS_INFO: u32 = 234946888u32;
pub const OID_WWAN_CONNECT: u32 = 234946828u32;
pub const OID_WWAN_CREATE_MAC: u32 = 234946854u32;
pub const OID_WWAN_DELETE_MAC: u32 = 234946855u32;
pub const OID_WWAN_DEVICE_BINDINGS: u32 = 234946865u32;
pub const OID_WWAN_DEVICE_CAPS: u32 = 234946817u32;
pub const OID_WWAN_DEVICE_CAPS_EX: u32 = 234946862u32;
pub const OID_WWAN_DEVICE_RESET: u32 = 234946887u32;
pub const OID_WWAN_DEVICE_SERVICE_COMMAND: u32 = 234946840u32;
pub const OID_WWAN_DEVICE_SERVICE_SESSION: u32 = 234946851u32;
pub const OID_WWAN_DEVICE_SERVICE_SESSION_WRITE: u32 = 234946852u32;
pub const OID_WWAN_DRIVER_CAPS: u32 = 234946816u32;
pub const OID_WWAN_ENUMERATE_DEVICE_SERVICES: u32 = 234946838u32;
pub const OID_WWAN_ENUMERATE_DEVICE_SERVICE_COMMANDS: u32 = 234946850u32;
pub const OID_WWAN_HOME_PROVIDER: u32 = 234946822u32;
pub const OID_WWAN_IMS_VOICE_STATE: u32 = 234946867u32;
pub const OID_WWAN_LOCATION_STATE: u32 = 234946869u32;
pub const OID_WWAN_LTE_ATTACH_CONFIG: u32 = 234946882u32;
pub const OID_WWAN_LTE_ATTACH_STATUS: u32 = 234946883u32;
pub const OID_WWAN_MBIM_VERSION: u32 = 234946860u32;
pub const OID_WWAN_MODEM_CONFIG_INFO: u32 = 234946884u32;
pub const OID_WWAN_MODEM_LOGGING_CONFIG: u32 = 234946891u32;
pub const OID_WWAN_MPDP: u32 = 234946889u32;
pub const OID_WWAN_NETWORK_BLACKLIST: u32 = 234946881u32;
pub const OID_WWAN_NETWORK_IDLE_HINT: u32 = 234946871u32;
pub const OID_WWAN_NETWORK_PARAMS: u32 = 234946893u32;
pub const OID_WWAN_NITZ: u32 = 234946870u32;
pub const OID_WWAN_PACKET_SERVICE: u32 = 234946826u32;
pub const OID_WWAN_PCO: u32 = 234946885u32;
pub const OID_WWAN_PIN: u32 = 234946820u32;
pub const OID_WWAN_PIN_EX: u32 = 234946849u32;
pub const OID_WWAN_PIN_EX2: u32 = 234946859u32;
pub const OID_WWAN_PIN_LIST: u32 = 234946821u32;
pub const OID_WWAN_PREFERRED_MULTICARRIER_PROVIDERS: u32 = 234946853u32;
pub const OID_WWAN_PREFERRED_PROVIDERS: u32 = 234946823u32;
pub const OID_WWAN_PRESHUTDOWN: u32 = 234946872u32;
pub const OID_WWAN_PROVISIONED_CONTEXTS: u32 = 234946829u32;
pub const OID_WWAN_PS_MEDIA_CONFIG: u32 = 234946878u32;
pub const OID_WWAN_RADIO_STATE: u32 = 234946819u32;
pub const OID_WWAN_READY_INFO: u32 = 234946818u32;
pub const OID_WWAN_REGISTER_PARAMS: u32 = 234946892u32;
pub const OID_WWAN_REGISTER_STATE: u32 = 234946825u32;
pub const OID_WWAN_REGISTER_STATE_EX: u32 = 234946866u32;
pub const OID_WWAN_SAR_CONFIG: u32 = 234946879u32;
pub const OID_WWAN_SAR_TRANSMISSION_STATUS: u32 = 234946880u32;
pub const OID_WWAN_SERVICE_ACTIVATION: u32 = 234946830u32;
pub const OID_WWAN_SIGNAL_STATE: u32 = 234946827u32;
pub const OID_WWAN_SIGNAL_STATE_EX: u32 = 234946868u32;
pub const OID_WWAN_SLOT_INFO_STATUS: u32 = 234946864u32;
pub const OID_WWAN_SMS_CONFIGURATION: u32 = 234946831u32;
pub const OID_WWAN_SMS_DELETE: u32 = 234946834u32;
pub const OID_WWAN_SMS_READ: u32 = 234946832u32;
pub const OID_WWAN_SMS_SEND: u32 = 234946833u32;
pub const OID_WWAN_SMS_STATUS: u32 = 234946835u32;
pub const OID_WWAN_SUBSCRIBE_DEVICE_SERVICE_EVENTS: u32 = 234946839u32;
pub const OID_WWAN_SYS_CAPS: u32 = 234946861u32;
pub const OID_WWAN_SYS_SLOTMAPPINGS: u32 = 234946863u32;
pub const OID_WWAN_UE_POLICY: u32 = 234946894u32;
pub const OID_WWAN_UICC_ACCESS_BINARY: u32 = 234946857u32;
pub const OID_WWAN_UICC_ACCESS_RECORD: u32 = 234946858u32;
pub const OID_WWAN_UICC_APDU: u32 = 234946876u32;
pub const OID_WWAN_UICC_APP_LIST: u32 = 234946890u32;
pub const OID_WWAN_UICC_ATR: u32 = 234946873u32;
pub const OID_WWAN_UICC_CLOSE_CHANNEL: u32 = 234946875u32;
pub const OID_WWAN_UICC_FILE_STATUS: u32 = 234946856u32;
pub const OID_WWAN_UICC_OPEN_CHANNEL: u32 = 234946874u32;
pub const OID_WWAN_UICC_RESET: u32 = 234946886u32;
pub const OID_WWAN_UICC_TERMINAL_CAPABILITY: u32 = 234946877u32;
pub const OID_WWAN_USSD: u32 = 234946841u32;
pub const OID_WWAN_VENDOR_SPECIFIC: u32 = 234946836u32;
pub const OID_WWAN_VISIBLE_PROVIDERS: u32 = 234946824u32;
pub const OID_XBOX_ACC_RESERVED0: u32 = 4194304000u32;
pub const READABLE_LOCAL_CLOCK: u32 = 1u32;
pub const RECEIVE_TIME_INDICATION_CAPABLE: u32 = 8u32;
pub const TIMED_SEND_CAPABLE: u32 = 16u32;
pub const TIME_STAMP_CAPABLE: u32 = 32u32;
pub const TUNNEL_TYPE_6TO4: TUNNEL_TYPE = 11i32;
pub const TUNNEL_TYPE_DIRECT: TUNNEL_TYPE = 2i32;
pub const TUNNEL_TYPE_IPHTTPS: TUNNEL_TYPE = 15i32;
pub const TUNNEL_TYPE_ISATAP: TUNNEL_TYPE = 13i32;
pub const TUNNEL_TYPE_NONE: TUNNEL_TYPE = 0i32;
pub const TUNNEL_TYPE_OTHER: TUNNEL_TYPE = 1i32;
pub const TUNNEL_TYPE_TEREDO: TUNNEL_TYPE = 14i32;
pub const UNSPECIFIED_NETWORK_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x12ba5bde_143e_4c0d_b66d_2379bb141913);
pub const WAN_PROTOCOL_KEEPS_STATS: u32 = 1u32;
pub const fNDIS_GUID_ALLOW_READ: u32 = 32u32;
pub const fNDIS_GUID_ALLOW_WRITE: u32 = 64u32;
pub const fNDIS_GUID_ANSI_STRING: u32 = 4u32;
pub const fNDIS_GUID_ARRAY: u32 = 16u32;
pub const fNDIS_GUID_METHOD: u32 = 128u32;
pub const fNDIS_GUID_NDIS_RESERVED: u32 = 256u32;
pub const fNDIS_GUID_SUPPORT_COMMON_HEADER: u32 = 512u32;
pub const fNDIS_GUID_TO_OID: u32 = 1u32;
pub const fNDIS_GUID_TO_STATUS: u32 = 2u32;
pub const fNDIS_GUID_UNICODE_STRING: u32 = 8u32;
pub type IF_ADMINISTRATIVE_STATE = i32;
pub type IF_OPER_STATUS = i32;
pub type NDIS_802_11_AUTHENTICATION_MODE = i32;
pub type NDIS_802_11_MEDIA_STREAM_MODE = i32;
pub type NDIS_802_11_NETWORK_INFRASTRUCTURE = i32;
pub type NDIS_802_11_NETWORK_TYPE = i32;
pub type NDIS_802_11_POWER_MODE = i32;
pub type NDIS_802_11_PRIVACY_FILTER = i32;
pub type NDIS_802_11_RADIO_STATUS = i32;
pub type NDIS_802_11_RELOAD_DEFAULTS = i32;
pub type NDIS_802_11_STATUS_TYPE = i32;
pub type NDIS_802_11_WEP_STATUS = i32;
pub type NDIS_802_5_RING_STATE = i32;
pub type NDIS_DEVICE_POWER_STATE = i32;
pub type NDIS_FDDI_ATTACHMENT_TYPE = i32;
pub type NDIS_FDDI_LCONNECTION_STATE = i32;
pub type NDIS_FDDI_RING_MGT_STATE = i32;
pub type NDIS_HARDWARE_STATUS = i32;
pub type NDIS_INTERRUPT_MODERATION = i32;
pub type NDIS_MEDIA_STATE = i32;
pub type NDIS_MEDIUM = i32;
pub type NDIS_NETWORK_CHANGE_TYPE = i32;
pub type NDIS_PHYSICAL_MEDIUM = i32;
pub type NDIS_PORT_AUTHORIZATION_STATE = i32;
pub type NDIS_PORT_CONTROL_STATE = i32;
pub type NDIS_PORT_TYPE = i32;
pub type NDIS_PROCESSOR_VENDOR = i32;
pub type NDIS_REQUEST_TYPE = i32;
pub type NDIS_SUPPORTED_PAUSE_FUNCTIONS = i32;
pub type NDIS_WAN_HEADER_FORMAT = i32;
pub type NDIS_WAN_MEDIUM_SUBTYPE = i32;
pub type NDIS_WAN_QUALITY = i32;
pub type NDK_RDMA_TECHNOLOGY = i32;
pub type NET_IF_ACCESS_TYPE = i32;
pub type NET_IF_ADMIN_STATUS = i32;
pub type NET_IF_CONNECTION_TYPE = i32;
pub type NET_IF_DIRECTION_TYPE = i32;
pub type NET_IF_MEDIA_CONNECT_STATE = i32;
pub type NET_IF_MEDIA_DUPLEX_STATE = i32;
pub type NET_IF_OPER_STATUS = i32;
pub type NET_IF_RCV_ADDRESS_TYPE = i32;
pub type OFFLOAD_CONF_ALGO = i32;
pub type OFFLOAD_INTEGRITY_ALGO = i32;
pub type OFFLOAD_OPERATION_E = i32;
pub type TUNNEL_TYPE = i32;
pub type UDP_ENCAP_TYPE = i32;
#[repr(C)]
pub struct BSSID_INFO {
    pub BSSID: [u8; 6],
    pub PMKID: [u8; 16],
}
impl ::core::marker::Copy for BSSID_INFO {}
impl ::core::clone::Clone for BSSID_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GEN_GET_NETCARD_TIME {
    pub ReadTime: u64,
}
impl ::core::marker::Copy for GEN_GET_NETCARD_TIME {}
impl ::core::clone::Clone for GEN_GET_NETCARD_TIME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GEN_GET_TIME_CAPS {
    pub Flags: u32,
    pub ClockPrecision: u32,
}
impl ::core::marker::Copy for GEN_GET_TIME_CAPS {}
impl ::core::clone::Clone for GEN_GET_TIME_CAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IF_COUNTED_STRING_LH {
    pub Length: u16,
    pub String: [u16; 257],
}
impl ::core::marker::Copy for IF_COUNTED_STRING_LH {}
impl ::core::clone::Clone for IF_COUNTED_STRING_LH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IF_PHYSICAL_ADDRESS_LH {
    pub Length: u16,
    pub Address: [u8; 32],
}
impl ::core::marker::Copy for IF_PHYSICAL_ADDRESS_LH {}
impl ::core::clone::Clone for IF_PHYSICAL_ADDRESS_LH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_AI_REQFI {
    pub Capabilities: u16,
    pub ListenInterval: u16,
    pub CurrentAPAddress: [u8; 6],
}
impl ::core::marker::Copy for NDIS_802_11_AI_REQFI {}
impl ::core::clone::Clone for NDIS_802_11_AI_REQFI {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_AI_RESFI {
    pub Capabilities: u16,
    pub StatusCode: u16,
    pub AssociationId: u16,
}
impl ::core::marker::Copy for NDIS_802_11_AI_RESFI {}
impl ::core::clone::Clone for NDIS_802_11_AI_RESFI {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_ASSOCIATION_INFORMATION {
    pub Length: u32,
    pub AvailableRequestFixedIEs: u16,
    pub RequestFixedIEs: NDIS_802_11_AI_REQFI,
    pub RequestIELength: u32,
    pub OffsetRequestIEs: u32,
    pub AvailableResponseFixedIEs: u16,
    pub ResponseFixedIEs: NDIS_802_11_AI_RESFI,
    pub ResponseIELength: u32,
    pub OffsetResponseIEs: u32,
}
impl ::core::marker::Copy for NDIS_802_11_ASSOCIATION_INFORMATION {}
impl ::core::clone::Clone for NDIS_802_11_ASSOCIATION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_AUTHENTICATION_ENCRYPTION {
    pub AuthModeSupported: NDIS_802_11_AUTHENTICATION_MODE,
    pub EncryptStatusSupported: NDIS_802_11_WEP_STATUS,
}
impl ::core::marker::Copy for NDIS_802_11_AUTHENTICATION_ENCRYPTION {}
impl ::core::clone::Clone for NDIS_802_11_AUTHENTICATION_ENCRYPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_AUTHENTICATION_EVENT {
    pub Status: NDIS_802_11_STATUS_INDICATION,
    pub Request: [NDIS_802_11_AUTHENTICATION_REQUEST; 1],
}
impl ::core::marker::Copy for NDIS_802_11_AUTHENTICATION_EVENT {}
impl ::core::clone::Clone for NDIS_802_11_AUTHENTICATION_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_AUTHENTICATION_REQUEST {
    pub Length: u32,
    pub Bssid: [u8; 6],
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_802_11_AUTHENTICATION_REQUEST {}
impl ::core::clone::Clone for NDIS_802_11_AUTHENTICATION_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_BSSID_LIST {
    pub NumberOfItems: u32,
    pub Bssid: [NDIS_WLAN_BSSID; 1],
}
impl ::core::marker::Copy for NDIS_802_11_BSSID_LIST {}
impl ::core::clone::Clone for NDIS_802_11_BSSID_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_BSSID_LIST_EX {
    pub NumberOfItems: u32,
    pub Bssid: [NDIS_WLAN_BSSID_EX; 1],
}
impl ::core::marker::Copy for NDIS_802_11_BSSID_LIST_EX {}
impl ::core::clone::Clone for NDIS_802_11_BSSID_LIST_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_CAPABILITY {
    pub Length: u32,
    pub Version: u32,
    pub NoOfPMKIDs: u32,
    pub NoOfAuthEncryptPairsSupported: u32,
    pub AuthenticationEncryptionSupported: [NDIS_802_11_AUTHENTICATION_ENCRYPTION; 1],
}
impl ::core::marker::Copy for NDIS_802_11_CAPABILITY {}
impl ::core::clone::Clone for NDIS_802_11_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_CONFIGURATION {
    pub Length: u32,
    pub BeaconPeriod: u32,
    pub ATIMWindow: u32,
    pub DSConfig: u32,
    pub FHConfig: NDIS_802_11_CONFIGURATION_FH,
}
impl ::core::marker::Copy for NDIS_802_11_CONFIGURATION {}
impl ::core::clone::Clone for NDIS_802_11_CONFIGURATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_CONFIGURATION_FH {
    pub Length: u32,
    pub HopPattern: u32,
    pub HopSet: u32,
    pub DwellTime: u32,
}
impl ::core::marker::Copy for NDIS_802_11_CONFIGURATION_FH {}
impl ::core::clone::Clone for NDIS_802_11_CONFIGURATION_FH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_FIXED_IEs {
    pub Timestamp: [u8; 8],
    pub BeaconInterval: u16,
    pub Capabilities: u16,
}
impl ::core::marker::Copy for NDIS_802_11_FIXED_IEs {}
impl ::core::clone::Clone for NDIS_802_11_FIXED_IEs {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_KEY {
    pub Length: u32,
    pub KeyIndex: u32,
    pub KeyLength: u32,
    pub BSSID: [u8; 6],
    pub KeyRSC: u64,
    pub KeyMaterial: [u8; 1],
}
impl ::core::marker::Copy for NDIS_802_11_KEY {}
impl ::core::clone::Clone for NDIS_802_11_KEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_NETWORK_TYPE_LIST {
    pub NumberOfItems: u32,
    pub NetworkType: [NDIS_802_11_NETWORK_TYPE; 1],
}
impl ::core::marker::Copy for NDIS_802_11_NETWORK_TYPE_LIST {}
impl ::core::clone::Clone for NDIS_802_11_NETWORK_TYPE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_NON_BCAST_SSID_LIST {
    pub NumberOfItems: u32,
    pub Non_Bcast_Ssid: [NDIS_802_11_SSID; 1],
}
impl ::core::marker::Copy for NDIS_802_11_NON_BCAST_SSID_LIST {}
impl ::core::clone::Clone for NDIS_802_11_NON_BCAST_SSID_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_PMKID {
    pub Length: u32,
    pub BSSIDInfoCount: u32,
    pub BSSIDInfo: [BSSID_INFO; 1],
}
impl ::core::marker::Copy for NDIS_802_11_PMKID {}
impl ::core::clone::Clone for NDIS_802_11_PMKID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_PMKID_CANDIDATE_LIST {
    pub Version: u32,
    pub NumCandidates: u32,
    pub CandidateList: [PMKID_CANDIDATE; 1],
}
impl ::core::marker::Copy for NDIS_802_11_PMKID_CANDIDATE_LIST {}
impl ::core::clone::Clone for NDIS_802_11_PMKID_CANDIDATE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_REMOVE_KEY {
    pub Length: u32,
    pub KeyIndex: u32,
    pub BSSID: [u8; 6],
}
impl ::core::marker::Copy for NDIS_802_11_REMOVE_KEY {}
impl ::core::clone::Clone for NDIS_802_11_REMOVE_KEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_SSID {
    pub SsidLength: u32,
    pub Ssid: [u8; 32],
}
impl ::core::marker::Copy for NDIS_802_11_SSID {}
impl ::core::clone::Clone for NDIS_802_11_SSID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_STATISTICS {
    pub Length: u32,
    pub TransmittedFragmentCount: i64,
    pub MulticastTransmittedFrameCount: i64,
    pub FailedCount: i64,
    pub RetryCount: i64,
    pub MultipleRetryCount: i64,
    pub RTSSuccessCount: i64,
    pub RTSFailureCount: i64,
    pub ACKFailureCount: i64,
    pub FrameDuplicateCount: i64,
    pub ReceivedFragmentCount: i64,
    pub MulticastReceivedFrameCount: i64,
    pub FCSErrorCount: i64,
    pub TKIPLocalMICFailures: i64,
    pub TKIPICVErrorCount: i64,
    pub TKIPCounterMeasuresInvoked: i64,
    pub TKIPReplays: i64,
    pub CCMPFormatErrors: i64,
    pub CCMPReplays: i64,
    pub CCMPDecryptErrors: i64,
    pub FourWayHandshakeFailures: i64,
    pub WEPUndecryptableCount: i64,
    pub WEPICVErrorCount: i64,
    pub DecryptSuccessCount: i64,
    pub DecryptFailureCount: i64,
}
impl ::core::marker::Copy for NDIS_802_11_STATISTICS {}
impl ::core::clone::Clone for NDIS_802_11_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_STATUS_INDICATION {
    pub StatusType: NDIS_802_11_STATUS_TYPE,
}
impl ::core::marker::Copy for NDIS_802_11_STATUS_INDICATION {}
impl ::core::clone::Clone for NDIS_802_11_STATUS_INDICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_TEST {
    pub Length: u32,
    pub Type: u32,
    pub Anonymous: NDIS_802_11_TEST_0,
}
impl ::core::marker::Copy for NDIS_802_11_TEST {}
impl ::core::clone::Clone for NDIS_802_11_TEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union NDIS_802_11_TEST_0 {
    pub AuthenticationEvent: NDIS_802_11_AUTHENTICATION_EVENT,
    pub RssiTrigger: i32,
}
impl ::core::marker::Copy for NDIS_802_11_TEST_0 {}
impl ::core::clone::Clone for NDIS_802_11_TEST_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_VARIABLE_IEs {
    pub ElementID: u8,
    pub Length: u8,
    pub data: [u8; 1],
}
impl ::core::marker::Copy for NDIS_802_11_VARIABLE_IEs {}
impl ::core::clone::Clone for NDIS_802_11_VARIABLE_IEs {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_802_11_WEP {
    pub Length: u32,
    pub KeyIndex: u32,
    pub KeyLength: u32,
    pub KeyMaterial: [u8; 1],
}
impl ::core::marker::Copy for NDIS_802_11_WEP {}
impl ::core::clone::Clone for NDIS_802_11_WEP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_CO_DEVICE_PROFILE {
    pub DeviceDescription: NDIS_VAR_DATA_DESC,
    pub DevSpecificInfo: NDIS_VAR_DATA_DESC,
    pub ulTAPISupplementaryPassThru: u32,
    pub ulAddressModes: u32,
    pub ulNumAddresses: u32,
    pub ulBearerModes: u32,
    pub ulMaxTxRate: u32,
    pub ulMinTxRate: u32,
    pub ulMaxRxRate: u32,
    pub ulMinRxRate: u32,
    pub ulMediaModes: u32,
    pub ulGenerateToneModes: u32,
    pub ulGenerateToneMaxNumFreq: u32,
    pub ulGenerateDigitModes: u32,
    pub ulMonitorToneMaxNumFreq: u32,
    pub ulMonitorToneMaxNumEntries: u32,
    pub ulMonitorDigitModes: u32,
    pub ulGatherDigitsMinTimeout: u32,
    pub ulGatherDigitsMaxTimeout: u32,
    pub ulDevCapFlags: u32,
    pub ulMaxNumActiveCalls: u32,
    pub ulAnswerMode: u32,
    pub ulUUIAcceptSize: u32,
    pub ulUUIAnswerSize: u32,
    pub ulUUIMakeCallSize: u32,
    pub ulUUIDropSize: u32,
    pub ulUUISendUserUserInfoSize: u32,
    pub ulUUICallInfoSize: u32,
}
impl ::core::marker::Copy for NDIS_CO_DEVICE_PROFILE {}
impl ::core::clone::Clone for NDIS_CO_DEVICE_PROFILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_CO_LINK_SPEED {
    pub Outbound: u32,
    pub Inbound: u32,
}
impl ::core::marker::Copy for NDIS_CO_LINK_SPEED {}
impl ::core::clone::Clone for NDIS_CO_LINK_SPEED {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_GUID {
    pub Guid: ::windows_sys::core::GUID,
    pub Anonymous: NDIS_GUID_0,
    pub Size: u32,
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_GUID {}
impl ::core::clone::Clone for NDIS_GUID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union NDIS_GUID_0 {
    pub Oid: u32,
    pub Status: i32,
}
impl ::core::marker::Copy for NDIS_GUID_0 {}
impl ::core::clone::Clone for NDIS_GUID_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_HARDWARE_CROSSTIMESTAMP {
    pub Header: NDIS_OBJECT_HEADER,
    pub Flags: u32,
    pub SystemTimestamp1: u64,
    pub HardwareClockTimestamp: u64,
    pub SystemTimestamp2: u64,
}
impl ::core::marker::Copy for NDIS_HARDWARE_CROSSTIMESTAMP {}
impl ::core::clone::Clone for NDIS_HARDWARE_CROSSTIMESTAMP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct NDIS_INTERFACE_INFORMATION {
    pub ifOperStatus: NET_IF_OPER_STATUS,
    pub ifOperStatusFlags: u32,
    pub MediaConnectState: NET_IF_MEDIA_CONNECT_STATE,
    pub MediaDuplexState: NET_IF_MEDIA_DUPLEX_STATE,
    pub ifMtu: u32,
    pub ifPromiscuousMode: super::super::Foundation::BOOLEAN,
    pub ifDeviceWakeUpEnable: super::super::Foundation::BOOLEAN,
    pub XmitLinkSpeed: u64,
    pub RcvLinkSpeed: u64,
    pub ifLastChange: u64,
    pub ifCounterDiscontinuityTime: u64,
    pub ifInUnknownProtos: u64,
    pub ifInDiscards: u64,
    pub ifInErrors: u64,
    pub ifHCInOctets: u64,
    pub ifHCInUcastPkts: u64,
    pub ifHCInMulticastPkts: u64,
    pub ifHCInBroadcastPkts: u64,
    pub ifHCOutOctets: u64,
    pub ifHCOutUcastPkts: u64,
    pub ifHCOutMulticastPkts: u64,
    pub ifHCOutBroadcastPkts: u64,
    pub ifOutErrors: u64,
    pub ifOutDiscards: u64,
    pub ifHCInUcastOctets: u64,
    pub ifHCInMulticastOctets: u64,
    pub ifHCInBroadcastOctets: u64,
    pub ifHCOutUcastOctets: u64,
    pub ifHCOutMulticastOctets: u64,
    pub ifHCOutBroadcastOctets: u64,
    pub CompartmentId: u32,
    pub SupportedStatistics: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for NDIS_INTERFACE_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for NDIS_INTERFACE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_INTERRUPT_MODERATION_PARAMETERS {
    pub Header: NDIS_OBJECT_HEADER,
    pub Flags: u32,
    pub InterruptModeration: NDIS_INTERRUPT_MODERATION,
}
impl ::core::marker::Copy for NDIS_INTERRUPT_MODERATION_PARAMETERS {}
impl ::core::clone::Clone for NDIS_INTERRUPT_MODERATION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_IPSEC_OFFLOAD_V1 {
    pub Supported: NDIS_IPSEC_OFFLOAD_V1_2,
    pub IPv4AH: NDIS_IPSEC_OFFLOAD_V1_0,
    pub IPv4ESP: NDIS_IPSEC_OFFLOAD_V1_1,
}
impl ::core::marker::Copy for NDIS_IPSEC_OFFLOAD_V1 {}
impl ::core::clone::Clone for NDIS_IPSEC_OFFLOAD_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_IPSEC_OFFLOAD_V1_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for NDIS_IPSEC_OFFLOAD_V1_0 {}
impl ::core::clone::Clone for NDIS_IPSEC_OFFLOAD_V1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_IPSEC_OFFLOAD_V1_1 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for NDIS_IPSEC_OFFLOAD_V1_1 {}
impl ::core::clone::Clone for NDIS_IPSEC_OFFLOAD_V1_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_IPSEC_OFFLOAD_V1_2 {
    pub Encapsulation: u32,
    pub AhEspCombined: u32,
    pub TransportTunnelCombined: u32,
    pub IPv4Options: u32,
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_IPSEC_OFFLOAD_V1_2 {}
impl ::core::clone::Clone for NDIS_IPSEC_OFFLOAD_V1_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_IP_OPER_STATE {
    pub Header: NDIS_OBJECT_HEADER,
    pub Flags: u32,
    pub IpOperationalStatus: NDIS_IP_OPER_STATUS,
}
impl ::core::marker::Copy for NDIS_IP_OPER_STATE {}
impl ::core::clone::Clone for NDIS_IP_OPER_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_IP_OPER_STATUS {
    pub AddressFamily: u32,
    pub OperationalStatus: NET_IF_OPER_STATUS,
    pub OperationalStatusFlags: u32,
}
impl ::core::marker::Copy for NDIS_IP_OPER_STATUS {}
impl ::core::clone::Clone for NDIS_IP_OPER_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_IP_OPER_STATUS_INFO {
    pub Header: NDIS_OBJECT_HEADER,
    pub Flags: u32,
    pub NumberofAddressFamiliesReturned: u32,
    pub IpOperationalStatus: [NDIS_IP_OPER_STATUS; 32],
}
impl ::core::marker::Copy for NDIS_IP_OPER_STATUS_INFO {}
impl ::core::clone::Clone for NDIS_IP_OPER_STATUS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_IRDA_PACKET_INFO {
    pub ExtraBOFs: u32,
    pub MinTurnAroundTime: u32,
}
impl ::core::marker::Copy for NDIS_IRDA_PACKET_INFO {}
impl ::core::clone::Clone for NDIS_IRDA_PACKET_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_LINK_PARAMETERS {
    pub Header: NDIS_OBJECT_HEADER,
    pub MediaDuplexState: NET_IF_MEDIA_DUPLEX_STATE,
    pub XmitLinkSpeed: u64,
    pub RcvLinkSpeed: u64,
    pub PauseFunctions: NDIS_SUPPORTED_PAUSE_FUNCTIONS,
    pub AutoNegotiationFlags: u32,
}
impl ::core::marker::Copy for NDIS_LINK_PARAMETERS {}
impl ::core::clone::Clone for NDIS_LINK_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_LINK_SPEED {
    pub XmitLinkSpeed: u64,
    pub RcvLinkSpeed: u64,
}
impl ::core::marker::Copy for NDIS_LINK_SPEED {}
impl ::core::clone::Clone for NDIS_LINK_SPEED {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_LINK_STATE {
    pub Header: NDIS_OBJECT_HEADER,
    pub MediaConnectState: NET_IF_MEDIA_CONNECT_STATE,
    pub MediaDuplexState: NET_IF_MEDIA_DUPLEX_STATE,
    pub XmitLinkSpeed: u64,
    pub RcvLinkSpeed: u64,
    pub PauseFunctions: NDIS_SUPPORTED_PAUSE_FUNCTIONS,
    pub AutoNegotiationFlags: u32,
}
impl ::core::marker::Copy for NDIS_LINK_STATE {}
impl ::core::clone::Clone for NDIS_LINK_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_OBJECT_HEADER {
    pub Type: u8,
    pub Revision: u8,
    pub Size: u16,
}
impl ::core::marker::Copy for NDIS_OBJECT_HEADER {}
impl ::core::clone::Clone for NDIS_OBJECT_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_OFFLOAD {
    pub Header: NDIS_OBJECT_HEADER,
    pub Checksum: NDIS_TCP_IP_CHECKSUM_OFFLOAD,
    pub LsoV1: NDIS_TCP_LARGE_SEND_OFFLOAD_V1,
    pub IPsecV1: NDIS_IPSEC_OFFLOAD_V1,
    pub LsoV2: NDIS_TCP_LARGE_SEND_OFFLOAD_V2,
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_OFFLOAD {}
impl ::core::clone::Clone for NDIS_OFFLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_OFFLOAD_PARAMETERS {
    pub Header: NDIS_OBJECT_HEADER,
    pub IPv4Checksum: u8,
    pub TCPIPv4Checksum: u8,
    pub UDPIPv4Checksum: u8,
    pub TCPIPv6Checksum: u8,
    pub UDPIPv6Checksum: u8,
    pub LsoV1: u8,
    pub IPsecV1: u8,
    pub LsoV2IPv4: u8,
    pub LsoV2IPv6: u8,
    pub TcpConnectionIPv4: u8,
    pub TcpConnectionIPv6: u8,
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_OFFLOAD_PARAMETERS {}
impl ::core::clone::Clone for NDIS_OFFLOAD_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_OPER_STATE {
    pub Header: NDIS_OBJECT_HEADER,
    pub OperationalStatus: NET_IF_OPER_STATUS,
    pub OperationalStatusFlags: u32,
}
impl ::core::marker::Copy for NDIS_OPER_STATE {}
impl ::core::clone::Clone for NDIS_OPER_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_PCI_DEVICE_CUSTOM_PROPERTIES {
    pub Header: NDIS_OBJECT_HEADER,
    pub DeviceType: u32,
    pub CurrentSpeedAndMode: u32,
    pub CurrentPayloadSize: u32,
    pub MaxPayloadSize: u32,
    pub MaxReadRequestSize: u32,
    pub CurrentLinkSpeed: u32,
    pub CurrentLinkWidth: u32,
    pub MaxLinkSpeed: u32,
    pub MaxLinkWidth: u32,
    pub PciExpressVersion: u32,
    pub InterruptType: u32,
    pub MaxInterruptMessages: u32,
}
impl ::core::marker::Copy for NDIS_PCI_DEVICE_CUSTOM_PROPERTIES {}
impl ::core::clone::Clone for NDIS_PCI_DEVICE_CUSTOM_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_PM_PACKET_PATTERN {
    pub Priority: u32,
    pub Reserved: u32,
    pub MaskSize: u32,
    pub PatternOffset: u32,
    pub PatternSize: u32,
    pub PatternFlags: u32,
}
impl ::core::marker::Copy for NDIS_PM_PACKET_PATTERN {}
impl ::core::clone::Clone for NDIS_PM_PACKET_PATTERN {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_PM_WAKE_UP_CAPABILITIES {
    pub MinMagicPacketWakeUp: NDIS_DEVICE_POWER_STATE,
    pub MinPatternWakeUp: NDIS_DEVICE_POWER_STATE,
    pub MinLinkChangeWakeUp: NDIS_DEVICE_POWER_STATE,
}
impl ::core::marker::Copy for NDIS_PM_WAKE_UP_CAPABILITIES {}
impl ::core::clone::Clone for NDIS_PM_WAKE_UP_CAPABILITIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_PNP_CAPABILITIES {
    pub Flags: u32,
    pub WakeUpCapabilities: NDIS_PM_WAKE_UP_CAPABILITIES,
}
impl ::core::marker::Copy for NDIS_PNP_CAPABILITIES {}
impl ::core::clone::Clone for NDIS_PNP_CAPABILITIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_PORT {
    pub Next: *mut NDIS_PORT,
    pub NdisReserved: *mut ::core::ffi::c_void,
    pub MiniportReserved: *mut ::core::ffi::c_void,
    pub ProtocolReserved: *mut ::core::ffi::c_void,
    pub PortCharacteristics: NDIS_PORT_CHARACTERISTICS,
}
impl ::core::marker::Copy for NDIS_PORT {}
impl ::core::clone::Clone for NDIS_PORT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_PORT_ARRAY {
    pub Header: NDIS_OBJECT_HEADER,
    pub NumberOfPorts: u32,
    pub OffsetFirstPort: u32,
    pub ElementSize: u32,
    pub Ports: [NDIS_PORT_CHARACTERISTICS; 1],
}
impl ::core::marker::Copy for NDIS_PORT_ARRAY {}
impl ::core::clone::Clone for NDIS_PORT_ARRAY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_PORT_AUTHENTICATION_PARAMETERS {
    pub Header: NDIS_OBJECT_HEADER,
    pub SendControlState: NDIS_PORT_CONTROL_STATE,
    pub RcvControlState: NDIS_PORT_CONTROL_STATE,
    pub SendAuthorizationState: NDIS_PORT_AUTHORIZATION_STATE,
    pub RcvAuthorizationState: NDIS_PORT_AUTHORIZATION_STATE,
}
impl ::core::marker::Copy for NDIS_PORT_AUTHENTICATION_PARAMETERS {}
impl ::core::clone::Clone for NDIS_PORT_AUTHENTICATION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_PORT_CHARACTERISTICS {
    pub Header: NDIS_OBJECT_HEADER,
    pub PortNumber: u32,
    pub Flags: u32,
    pub Type: NDIS_PORT_TYPE,
    pub MediaConnectState: NET_IF_MEDIA_CONNECT_STATE,
    pub XmitLinkSpeed: u64,
    pub RcvLinkSpeed: u64,
    pub Direction: NET_IF_DIRECTION_TYPE,
    pub SendControlState: NDIS_PORT_CONTROL_STATE,
    pub RcvControlState: NDIS_PORT_CONTROL_STATE,
    pub SendAuthorizationState: NDIS_PORT_AUTHORIZATION_STATE,
    pub RcvAuthorizationState: NDIS_PORT_AUTHORIZATION_STATE,
}
impl ::core::marker::Copy for NDIS_PORT_CHARACTERISTICS {}
impl ::core::clone::Clone for NDIS_PORT_CHARACTERISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_PORT_STATE {
    pub Header: NDIS_OBJECT_HEADER,
    pub MediaConnectState: NET_IF_MEDIA_CONNECT_STATE,
    pub XmitLinkSpeed: u64,
    pub RcvLinkSpeed: u64,
    pub Direction: NET_IF_DIRECTION_TYPE,
    pub SendControlState: NDIS_PORT_CONTROL_STATE,
    pub RcvControlState: NDIS_PORT_CONTROL_STATE,
    pub SendAuthorizationState: NDIS_PORT_AUTHORIZATION_STATE,
    pub RcvAuthorizationState: NDIS_PORT_AUTHORIZATION_STATE,
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_PORT_STATE {}
impl ::core::clone::Clone for NDIS_PORT_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_RECEIVE_HASH_PARAMETERS {
    pub Header: NDIS_OBJECT_HEADER,
    pub Flags: u32,
    pub HashInformation: u32,
    pub HashSecretKeySize: u16,
    pub HashSecretKeyOffset: u32,
}
impl ::core::marker::Copy for NDIS_RECEIVE_HASH_PARAMETERS {}
impl ::core::clone::Clone for NDIS_RECEIVE_HASH_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_RECEIVE_SCALE_CAPABILITIES {
    pub Header: NDIS_OBJECT_HEADER,
    pub CapabilitiesFlags: u32,
    pub NumberOfInterruptMessages: u32,
    pub NumberOfReceiveQueues: u32,
}
impl ::core::marker::Copy for NDIS_RECEIVE_SCALE_CAPABILITIES {}
impl ::core::clone::Clone for NDIS_RECEIVE_SCALE_CAPABILITIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_RECEIVE_SCALE_PARAMETERS {
    pub Header: NDIS_OBJECT_HEADER,
    pub Flags: u16,
    pub BaseCpuNumber: u16,
    pub HashInformation: u32,
    pub IndirectionTableSize: u16,
    pub IndirectionTableOffset: u32,
    pub HashSecretKeySize: u16,
    pub HashSecretKeyOffset: u32,
}
impl ::core::marker::Copy for NDIS_RECEIVE_SCALE_PARAMETERS {}
impl ::core::clone::Clone for NDIS_RECEIVE_SCALE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_STATISTICS_INFO {
    pub Header: NDIS_OBJECT_HEADER,
    pub SupportedStatistics: u32,
    pub ifInDiscards: u64,
    pub ifInErrors: u64,
    pub ifHCInOctets: u64,
    pub ifHCInUcastPkts: u64,
    pub ifHCInMulticastPkts: u64,
    pub ifHCInBroadcastPkts: u64,
    pub ifHCOutOctets: u64,
    pub ifHCOutUcastPkts: u64,
    pub ifHCOutMulticastPkts: u64,
    pub ifHCOutBroadcastPkts: u64,
    pub ifOutErrors: u64,
    pub ifOutDiscards: u64,
    pub ifHCInUcastOctets: u64,
    pub ifHCInMulticastOctets: u64,
    pub ifHCInBroadcastOctets: u64,
    pub ifHCOutUcastOctets: u64,
    pub ifHCOutMulticastOctets: u64,
    pub ifHCOutBroadcastOctets: u64,
}
impl ::core::marker::Copy for NDIS_STATISTICS_INFO {}
impl ::core::clone::Clone for NDIS_STATISTICS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_STATISTICS_VALUE {
    pub Oid: u32,
    pub DataLength: u32,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for NDIS_STATISTICS_VALUE {}
impl ::core::clone::Clone for NDIS_STATISTICS_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_STATISTICS_VALUE_EX {
    pub Oid: u32,
    pub DataLength: u32,
    pub Length: u32,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for NDIS_STATISTICS_VALUE_EX {}
impl ::core::clone::Clone for NDIS_STATISTICS_VALUE_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_CONNECTION_OFFLOAD {
    pub Header: NDIS_OBJECT_HEADER,
    pub Encapsulation: u32,
    pub _bitfield: u32,
    pub TcpConnectionOffloadCapacity: u32,
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_TCP_CONNECTION_OFFLOAD {}
impl ::core::clone::Clone for NDIS_TCP_CONNECTION_OFFLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_IP_CHECKSUM_OFFLOAD {
    pub IPv4Transmit: NDIS_TCP_IP_CHECKSUM_OFFLOAD_1,
    pub IPv4Receive: NDIS_TCP_IP_CHECKSUM_OFFLOAD_0,
    pub IPv6Transmit: NDIS_TCP_IP_CHECKSUM_OFFLOAD_3,
    pub IPv6Receive: NDIS_TCP_IP_CHECKSUM_OFFLOAD_2,
}
impl ::core::marker::Copy for NDIS_TCP_IP_CHECKSUM_OFFLOAD {}
impl ::core::clone::Clone for NDIS_TCP_IP_CHECKSUM_OFFLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_IP_CHECKSUM_OFFLOAD_0 {
    pub Encapsulation: u32,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for NDIS_TCP_IP_CHECKSUM_OFFLOAD_0 {}
impl ::core::clone::Clone for NDIS_TCP_IP_CHECKSUM_OFFLOAD_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_IP_CHECKSUM_OFFLOAD_1 {
    pub Encapsulation: u32,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for NDIS_TCP_IP_CHECKSUM_OFFLOAD_1 {}
impl ::core::clone::Clone for NDIS_TCP_IP_CHECKSUM_OFFLOAD_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_IP_CHECKSUM_OFFLOAD_2 {
    pub Encapsulation: u32,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for NDIS_TCP_IP_CHECKSUM_OFFLOAD_2 {}
impl ::core::clone::Clone for NDIS_TCP_IP_CHECKSUM_OFFLOAD_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_IP_CHECKSUM_OFFLOAD_3 {
    pub Encapsulation: u32,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for NDIS_TCP_IP_CHECKSUM_OFFLOAD_3 {}
impl ::core::clone::Clone for NDIS_TCP_IP_CHECKSUM_OFFLOAD_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_LARGE_SEND_OFFLOAD_V1 {
    pub IPv4: NDIS_TCP_LARGE_SEND_OFFLOAD_V1_0,
}
impl ::core::marker::Copy for NDIS_TCP_LARGE_SEND_OFFLOAD_V1 {}
impl ::core::clone::Clone for NDIS_TCP_LARGE_SEND_OFFLOAD_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_LARGE_SEND_OFFLOAD_V1_0 {
    pub Encapsulation: u32,
    pub MaxOffLoadSize: u32,
    pub MinSegmentCount: u32,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for NDIS_TCP_LARGE_SEND_OFFLOAD_V1_0 {}
impl ::core::clone::Clone for NDIS_TCP_LARGE_SEND_OFFLOAD_V1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_LARGE_SEND_OFFLOAD_V2 {
    pub IPv4: NDIS_TCP_LARGE_SEND_OFFLOAD_V2_0,
    pub IPv6: NDIS_TCP_LARGE_SEND_OFFLOAD_V2_1,
}
impl ::core::marker::Copy for NDIS_TCP_LARGE_SEND_OFFLOAD_V2 {}
impl ::core::clone::Clone for NDIS_TCP_LARGE_SEND_OFFLOAD_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_LARGE_SEND_OFFLOAD_V2_0 {
    pub Encapsulation: u32,
    pub MaxOffLoadSize: u32,
    pub MinSegmentCount: u32,
}
impl ::core::marker::Copy for NDIS_TCP_LARGE_SEND_OFFLOAD_V2_0 {}
impl ::core::clone::Clone for NDIS_TCP_LARGE_SEND_OFFLOAD_V2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TCP_LARGE_SEND_OFFLOAD_V2_1 {
    pub Encapsulation: u32,
    pub MaxOffLoadSize: u32,
    pub MinSegmentCount: u32,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for NDIS_TCP_LARGE_SEND_OFFLOAD_V2_1 {}
impl ::core::clone::Clone for NDIS_TCP_LARGE_SEND_OFFLOAD_V2_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_TIMEOUT_DPC_REQUEST_CAPABILITIES {
    pub Header: NDIS_OBJECT_HEADER,
    pub Flags: u32,
    pub TimeoutArrayLength: u32,
    pub TimeoutArray: [u32; 1],
}
impl ::core::marker::Copy for NDIS_TIMEOUT_DPC_REQUEST_CAPABILITIES {}
impl ::core::clone::Clone for NDIS_TIMEOUT_DPC_REQUEST_CAPABILITIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct NDIS_TIMESTAMP_CAPABILITIES {
    pub Header: NDIS_OBJECT_HEADER,
    pub HardwareClockFrequencyHz: u64,
    pub CrossTimestamp: super::super::Foundation::BOOLEAN,
    pub Reserved1: u64,
    pub Reserved2: u64,
    pub TimestampFlags: NDIS_TIMESTAMP_CAPABILITY_FLAGS,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for NDIS_TIMESTAMP_CAPABILITIES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for NDIS_TIMESTAMP_CAPABILITIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct NDIS_TIMESTAMP_CAPABILITY_FLAGS {
    pub PtpV2OverUdpIPv4EventMsgReceiveHw: super::super::Foundation::BOOLEAN,
    pub PtpV2OverUdpIPv4AllMsgReceiveHw: super::super::Foundation::BOOLEAN,
    pub PtpV2OverUdpIPv4EventMsgTransmitHw: super::super::Foundation::BOOLEAN,
    pub PtpV2OverUdpIPv4AllMsgTransmitHw: super::super::Foundation::BOOLEAN,
    pub PtpV2OverUdpIPv6EventMsgReceiveHw: super::super::Foundation::BOOLEAN,
    pub PtpV2OverUdpIPv6AllMsgReceiveHw: super::super::Foundation::BOOLEAN,
    pub PtpV2OverUdpIPv6EventMsgTransmitHw: super::super::Foundation::BOOLEAN,
    pub PtpV2OverUdpIPv6AllMsgTransmitHw: super::super::Foundation::BOOLEAN,
    pub AllReceiveHw: super::super::Foundation::BOOLEAN,
    pub AllTransmitHw: super::super::Foundation::BOOLEAN,
    pub TaggedTransmitHw: super::super::Foundation::BOOLEAN,
    pub AllReceiveSw: super::super::Foundation::BOOLEAN,
    pub AllTransmitSw: super::super::Foundation::BOOLEAN,
    pub TaggedTransmitSw: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for NDIS_TIMESTAMP_CAPABILITY_FLAGS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for NDIS_TIMESTAMP_CAPABILITY_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_VAR_DATA_DESC {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Offset: usize,
}
impl ::core::marker::Copy for NDIS_VAR_DATA_DESC {}
impl ::core::clone::Clone for NDIS_VAR_DATA_DESC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WAN_PROTOCOL_CAPS {
    pub Flags: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for NDIS_WAN_PROTOCOL_CAPS {}
impl ::core::clone::Clone for NDIS_WAN_PROTOCOL_CAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WLAN_BSSID {
    pub Length: u32,
    pub MacAddress: [u8; 6],
    pub Reserved: [u8; 2],
    pub Ssid: NDIS_802_11_SSID,
    pub Privacy: u32,
    pub Rssi: i32,
    pub NetworkTypeInUse: NDIS_802_11_NETWORK_TYPE,
    pub Configuration: NDIS_802_11_CONFIGURATION,
    pub InfrastructureMode: NDIS_802_11_NETWORK_INFRASTRUCTURE,
    pub SupportedRates: [u8; 8],
}
impl ::core::marker::Copy for NDIS_WLAN_BSSID {}
impl ::core::clone::Clone for NDIS_WLAN_BSSID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WLAN_BSSID_EX {
    pub Length: u32,
    pub MacAddress: [u8; 6],
    pub Reserved: [u8; 2],
    pub Ssid: NDIS_802_11_SSID,
    pub Privacy: u32,
    pub Rssi: i32,
    pub NetworkTypeInUse: NDIS_802_11_NETWORK_TYPE,
    pub Configuration: NDIS_802_11_CONFIGURATION,
    pub InfrastructureMode: NDIS_802_11_NETWORK_INFRASTRUCTURE,
    pub SupportedRates: [u8; 16],
    pub IELength: u32,
    pub IEs: [u8; 1],
}
impl ::core::marker::Copy for NDIS_WLAN_BSSID_EX {}
impl ::core::clone::Clone for NDIS_WLAN_BSSID_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_ENUM_ADAPTER {
    pub Header: NDIS_OBJECT_HEADER,
    pub IfIndex: u32,
    pub NetLuid: NET_LUID_LH,
    pub DeviceNameLength: u16,
    pub DeviceName: [u8; 1],
}
impl ::core::marker::Copy for NDIS_WMI_ENUM_ADAPTER {}
impl ::core::clone::Clone for NDIS_WMI_ENUM_ADAPTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_EVENT_HEADER {
    pub Header: NDIS_OBJECT_HEADER,
    pub IfIndex: u32,
    pub NetLuid: NET_LUID_LH,
    pub RequestId: u64,
    pub PortNumber: u32,
    pub DeviceNameLength: u32,
    pub DeviceNameOffset: u32,
    pub Padding: [u8; 4],
}
impl ::core::marker::Copy for NDIS_WMI_EVENT_HEADER {}
impl ::core::clone::Clone for NDIS_WMI_EVENT_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_IPSEC_OFFLOAD_V1 {
    pub Supported: NDIS_WMI_IPSEC_OFFLOAD_V1_2,
    pub IPv4AH: NDIS_WMI_IPSEC_OFFLOAD_V1_0,
    pub IPv4ESP: NDIS_WMI_IPSEC_OFFLOAD_V1_1,
}
impl ::core::marker::Copy for NDIS_WMI_IPSEC_OFFLOAD_V1 {}
impl ::core::clone::Clone for NDIS_WMI_IPSEC_OFFLOAD_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_IPSEC_OFFLOAD_V1_0 {
    pub Md5: u32,
    pub Sha_1: u32,
    pub Transport: u32,
    pub Tunnel: u32,
    pub Send: u32,
    pub Receive: u32,
}
impl ::core::marker::Copy for NDIS_WMI_IPSEC_OFFLOAD_V1_0 {}
impl ::core::clone::Clone for NDIS_WMI_IPSEC_OFFLOAD_V1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_IPSEC_OFFLOAD_V1_1 {
    pub Des: u32,
    pub Reserved: u32,
    pub TripleDes: u32,
    pub NullEsp: u32,
    pub Transport: u32,
    pub Tunnel: u32,
    pub Send: u32,
    pub Receive: u32,
}
impl ::core::marker::Copy for NDIS_WMI_IPSEC_OFFLOAD_V1_1 {}
impl ::core::clone::Clone for NDIS_WMI_IPSEC_OFFLOAD_V1_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_IPSEC_OFFLOAD_V1_2 {
    pub Encapsulation: u32,
    pub AhEspCombined: u32,
    pub TransportTunnelCombined: u32,
    pub IPv4Options: u32,
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_WMI_IPSEC_OFFLOAD_V1_2 {}
impl ::core::clone::Clone for NDIS_WMI_IPSEC_OFFLOAD_V1_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_METHOD_HEADER {
    pub Header: NDIS_OBJECT_HEADER,
    pub PortNumber: u32,
    pub NetLuid: NET_LUID_LH,
    pub RequestId: u64,
    pub Timeout: u32,
    pub Padding: [u8; 4],
}
impl ::core::marker::Copy for NDIS_WMI_METHOD_HEADER {}
impl ::core::clone::Clone for NDIS_WMI_METHOD_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_OFFLOAD {
    pub Header: NDIS_OBJECT_HEADER,
    pub Checksum: NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD,
    pub LsoV1: NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V1,
    pub IPsecV1: NDIS_WMI_IPSEC_OFFLOAD_V1,
    pub LsoV2: NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2,
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_WMI_OFFLOAD {}
impl ::core::clone::Clone for NDIS_WMI_OFFLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_OUTPUT_INFO {
    pub Header: NDIS_OBJECT_HEADER,
    pub Flags: u32,
    pub SupportedRevision: u8,
    pub DataOffset: u32,
}
impl ::core::marker::Copy for NDIS_WMI_OUTPUT_INFO {}
impl ::core::clone::Clone for NDIS_WMI_OUTPUT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_SET_HEADER {
    pub Header: NDIS_OBJECT_HEADER,
    pub PortNumber: u32,
    pub NetLuid: NET_LUID_LH,
    pub RequestId: u64,
    pub Timeout: u32,
    pub Padding: [u8; 4],
}
impl ::core::marker::Copy for NDIS_WMI_SET_HEADER {}
impl ::core::clone::Clone for NDIS_WMI_SET_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_CONNECTION_OFFLOAD {
    pub Header: NDIS_OBJECT_HEADER,
    pub Encapsulation: u32,
    pub SupportIPv4: u32,
    pub SupportIPv6: u32,
    pub SupportIPv6ExtensionHeaders: u32,
    pub SupportSack: u32,
    pub TcpConnectionOffloadCapacity: u32,
    pub Flags: u32,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_CONNECTION_OFFLOAD {}
impl ::core::clone::Clone for NDIS_WMI_TCP_CONNECTION_OFFLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD {
    pub IPv4Transmit: NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_1,
    pub IPv4Receive: NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_0,
    pub IPv6Transmit: NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_3,
    pub IPv6Receive: NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_2,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD {}
impl ::core::clone::Clone for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_0 {
    pub Encapsulation: u32,
    pub IpOptionsSupported: u32,
    pub TcpOptionsSupported: u32,
    pub TcpChecksum: u32,
    pub UdpChecksum: u32,
    pub IpChecksum: u32,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_0 {}
impl ::core::clone::Clone for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_1 {
    pub Encapsulation: u32,
    pub IpOptionsSupported: u32,
    pub TcpOptionsSupported: u32,
    pub TcpChecksum: u32,
    pub UdpChecksum: u32,
    pub IpChecksum: u32,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_1 {}
impl ::core::clone::Clone for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_2 {
    pub Encapsulation: u32,
    pub IpExtensionHeadersSupported: u32,
    pub TcpOptionsSupported: u32,
    pub TcpChecksum: u32,
    pub UdpChecksum: u32,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_2 {}
impl ::core::clone::Clone for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_3 {
    pub Encapsulation: u32,
    pub IpExtensionHeadersSupported: u32,
    pub TcpOptionsSupported: u32,
    pub TcpChecksum: u32,
    pub UdpChecksum: u32,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_3 {}
impl ::core::clone::Clone for NDIS_WMI_TCP_IP_CHECKSUM_OFFLOAD_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V1 {
    pub IPv4: NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V1_0,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V1 {}
impl ::core::clone::Clone for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V1_0 {
    pub Encapsulation: u32,
    pub MaxOffLoadSize: u32,
    pub MinSegmentCount: u32,
    pub TcpOptions: u32,
    pub IpOptions: u32,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V1_0 {}
impl ::core::clone::Clone for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2 {
    pub IPv4: NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2_0,
    pub IPv6: NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2_1,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2 {}
impl ::core::clone::Clone for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2_0 {
    pub Encapsulation: u32,
    pub MaxOffLoadSize: u32,
    pub MinSegmentCount: u32,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2_0 {}
impl ::core::clone::Clone for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2_1 {
    pub Encapsulation: u32,
    pub MaxOffLoadSize: u32,
    pub MinSegmentCount: u32,
    pub IpExtensionHeadersSupported: u32,
    pub TcpOptionsSupported: u32,
}
impl ::core::marker::Copy for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2_1 {}
impl ::core::clone::Clone for NDIS_WMI_TCP_LARGE_SEND_OFFLOAD_V2_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDK_ADAPTER_INFO {
    pub Version: NDK_VERSION,
    pub VendorId: u32,
    pub DeviceId: u32,
    pub MaxRegistrationSize: usize,
    pub MaxWindowSize: usize,
    pub FRMRPageCount: u32,
    pub MaxInitiatorRequestSge: u32,
    pub MaxReceiveRequestSge: u32,
    pub MaxReadRequestSge: u32,
    pub MaxTransferLength: u32,
    pub MaxInlineDataSize: u32,
    pub MaxInboundReadLimit: u32,
    pub MaxOutboundReadLimit: u32,
    pub MaxReceiveQueueDepth: u32,
    pub MaxInitiatorQueueDepth: u32,
    pub MaxSrqDepth: u32,
    pub MaxCqDepth: u32,
    pub LargeRequestThreshold: u32,
    pub MaxCallerData: u32,
    pub MaxCalleeData: u32,
    pub AdapterFlags: u32,
    pub RdmaTechnology: NDK_RDMA_TECHNOLOGY,
}
impl ::core::marker::Copy for NDK_ADAPTER_INFO {}
impl ::core::clone::Clone for NDK_ADAPTER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDK_VERSION {
    pub Major: u16,
    pub Minor: u16,
}
impl ::core::marker::Copy for NDK_VERSION {}
impl ::core::clone::Clone for NDK_VERSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_ADDRESS {
    pub AddressLength: u16,
    pub AddressType: u16,
    pub Address: [u8; 1],
}
impl ::core::marker::Copy for NETWORK_ADDRESS {}
impl ::core::clone::Clone for NETWORK_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_ADDRESS_IP {
    pub sin_port: u16,
    pub IN_ADDR: u32,
    pub sin_zero: [u8; 8],
}
impl ::core::marker::Copy for NETWORK_ADDRESS_IP {}
impl ::core::clone::Clone for NETWORK_ADDRESS_IP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_ADDRESS_IP6 {
    pub sin6_port: u16,
    pub sin6_flowinfo: u32,
    pub sin6_addr: [u16; 8],
    pub sin6_scope_id: u32,
}
impl ::core::marker::Copy for NETWORK_ADDRESS_IP6 {}
impl ::core::clone::Clone for NETWORK_ADDRESS_IP6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_ADDRESS_IPX {
    pub NetworkAddress: u32,
    pub NodeAddress: [u8; 6],
    pub Socket: u16,
}
impl ::core::marker::Copy for NETWORK_ADDRESS_IPX {}
impl ::core::clone::Clone for NETWORK_ADDRESS_IPX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_ADDRESS_LIST {
    pub AddressCount: i32,
    pub AddressType: u16,
    pub Address: [NETWORK_ADDRESS; 1],
}
impl ::core::marker::Copy for NETWORK_ADDRESS_LIST {}
impl ::core::clone::Clone for NETWORK_ADDRESS_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NET_IF_ALIAS_LH {
    pub ifAliasLength: u16,
    pub ifAliasOffset: u16,
}
impl ::core::marker::Copy for NET_IF_ALIAS_LH {}
impl ::core::clone::Clone for NET_IF_ALIAS_LH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NET_IF_RCV_ADDRESS_LH {
    pub ifRcvAddressType: NET_IF_RCV_ADDRESS_TYPE,
    pub ifRcvAddressLength: u16,
    pub ifRcvAddressOffset: u16,
}
impl ::core::marker::Copy for NET_IF_RCV_ADDRESS_LH {}
impl ::core::clone::Clone for NET_IF_RCV_ADDRESS_LH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union NET_LUID_LH {
    pub Value: u64,
    pub Info: NET_LUID_LH_0,
}
impl ::core::marker::Copy for NET_LUID_LH {}
impl ::core::clone::Clone for NET_LUID_LH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NET_LUID_LH_0 {
    pub _bitfield: u64,
}
impl ::core::marker::Copy for NET_LUID_LH_0 {}
impl ::core::clone::Clone for NET_LUID_LH_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NET_PHYSICAL_LOCATION_LH {
    pub BusNumber: u32,
    pub SlotNumber: u32,
    pub FunctionNumber: u32,
}
impl ::core::marker::Copy for NET_PHYSICAL_LOCATION_LH {}
impl ::core::clone::Clone for NET_PHYSICAL_LOCATION_LH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OFFLOAD_ALGO_INFO {
    pub algoIdentifier: u32,
    pub algoKeylen: u32,
    pub algoRounds: u32,
}
impl ::core::marker::Copy for OFFLOAD_ALGO_INFO {}
impl ::core::clone::Clone for OFFLOAD_ALGO_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct OFFLOAD_IPSEC_ADD_SA {
    pub SrcAddr: u32,
    pub SrcMask: u32,
    pub DestAddr: u32,
    pub DestMask: u32,
    pub Protocol: u32,
    pub SrcPort: u16,
    pub DestPort: u16,
    pub SrcTunnelAddr: u32,
    pub DestTunnelAddr: u32,
    pub Flags: u16,
    pub NumSAs: i16,
    pub SecAssoc: [OFFLOAD_SECURITY_ASSOCIATION; 3],
    pub OffloadHandle: super::super::Foundation::HANDLE,
    pub KeyLen: u32,
    pub KeyMat: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for OFFLOAD_IPSEC_ADD_SA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for OFFLOAD_IPSEC_ADD_SA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct OFFLOAD_IPSEC_ADD_UDPESP_SA {
    pub SrcAddr: u32,
    pub SrcMask: u32,
    pub DstAddr: u32,
    pub DstMask: u32,
    pub Protocol: u32,
    pub SrcPort: u16,
    pub DstPort: u16,
    pub SrcTunnelAddr: u32,
    pub DstTunnelAddr: u32,
    pub Flags: u16,
    pub NumSAs: i16,
    pub SecAssoc: [OFFLOAD_SECURITY_ASSOCIATION; 3],
    pub OffloadHandle: super::super::Foundation::HANDLE,
    pub EncapTypeEntry: OFFLOAD_IPSEC_UDPESP_ENCAPTYPE_ENTRY,
    pub EncapTypeEntryOffldHandle: super::super::Foundation::HANDLE,
    pub KeyLen: u32,
    pub KeyMat: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for OFFLOAD_IPSEC_ADD_UDPESP_SA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for OFFLOAD_IPSEC_ADD_UDPESP_SA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct OFFLOAD_IPSEC_DELETE_SA {
    pub OffloadHandle: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for OFFLOAD_IPSEC_DELETE_SA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for OFFLOAD_IPSEC_DELETE_SA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct OFFLOAD_IPSEC_DELETE_UDPESP_SA {
    pub OffloadHandle: super::super::Foundation::HANDLE,
    pub EncapTypeEntryOffldHandle: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for OFFLOAD_IPSEC_DELETE_UDPESP_SA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for OFFLOAD_IPSEC_DELETE_UDPESP_SA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OFFLOAD_IPSEC_UDPESP_ENCAPTYPE_ENTRY {
    pub UdpEncapType: UDP_ENCAP_TYPE,
    pub DstEncapPort: u16,
}
impl ::core::marker::Copy for OFFLOAD_IPSEC_UDPESP_ENCAPTYPE_ENTRY {}
impl ::core::clone::Clone for OFFLOAD_IPSEC_UDPESP_ENCAPTYPE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OFFLOAD_SECURITY_ASSOCIATION {
    pub Operation: OFFLOAD_OPERATION_E,
    pub SPI: u32,
    pub IntegrityAlgo: OFFLOAD_ALGO_INFO,
    pub ConfAlgo: OFFLOAD_ALGO_INFO,
    pub Reserved: OFFLOAD_ALGO_INFO,
}
impl ::core::marker::Copy for OFFLOAD_SECURITY_ASSOCIATION {}
impl ::core::clone::Clone for OFFLOAD_SECURITY_ASSOCIATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PMKID_CANDIDATE {
    pub BSSID: [u8; 6],
    pub Flags: u32,
}
impl ::core::marker::Copy for PMKID_CANDIDATE {}
impl ::core::clone::Clone for PMKID_CANDIDATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TRANSPORT_HEADER_OFFSET {
    pub ProtocolType: u16,
    pub HeaderOffset: u16,
}
impl ::core::marker::Copy for TRANSPORT_HEADER_OFFSET {}
impl ::core::clone::Clone for TRANSPORT_HEADER_OFFSET {
    fn clone(&self) -> Self {
        *self
    }
}
