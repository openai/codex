#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildCommDCBA(lpdef : ::windows_sys::core::PCSTR, lpdcb : *mut DCB) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildCommDCBAndTimeoutsA(lpdef : ::windows_sys::core::PCSTR, lpdcb : *mut DCB, lpcommtimeouts : *mut COMMTIMEOUTS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildCommDCBAndTimeoutsW(lpdef : ::windows_sys::core::PCWSTR, lpdcb : *mut DCB, lpcommtimeouts : *mut COMMTIMEOUTS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildCommDCBW(lpdef : ::windows_sys::core::PCWSTR, lpdcb : *mut DCB) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClearCommBreak(hfile : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClearCommError(hfile : super::super::Foundation:: HANDLE, lperrors : *mut CLEAR_COMM_ERROR_FLAGS, lpstat : *mut COMSTAT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CommConfigDialogA(lpszname : ::windows_sys::core::PCSTR, hwnd : super::super::Foundation:: HWND, lpcc : *mut COMMCONFIG) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CommConfigDialogW(lpszname : ::windows_sys::core::PCWSTR, hwnd : super::super::Foundation:: HWND, lpcc : *mut COMMCONFIG) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EscapeCommFunction(hfile : super::super::Foundation:: HANDLE, dwfunc : ESCAPE_COMM_FUNCTION) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCommConfig(hcommdev : super::super::Foundation:: HANDLE, lpcc : *mut COMMCONFIG, lpdwsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCommMask(hfile : super::super::Foundation:: HANDLE, lpevtmask : *mut COMM_EVENT_MASK) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCommModemStatus(hfile : super::super::Foundation:: HANDLE, lpmodemstat : *mut MODEM_STATUS_FLAGS) -> super::super::Foundation:: BOOL);
::windows_targets::link!("api-ms-win-core-comm-l1-1-2.dll" "system" fn GetCommPorts(lpportnumbers : *mut u32, uportnumberscount : u32, puportnumbersfound : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCommProperties(hfile : super::super::Foundation:: HANDLE, lpcommprop : *mut COMMPROP) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCommState(hfile : super::super::Foundation:: HANDLE, lpdcb : *mut DCB) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCommTimeouts(hfile : super::super::Foundation:: HANDLE, lpcommtimeouts : *mut COMMTIMEOUTS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDefaultCommConfigA(lpszname : ::windows_sys::core::PCSTR, lpcc : *mut COMMCONFIG, lpdwsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDefaultCommConfigW(lpszname : ::windows_sys::core::PCWSTR, lpcc : *mut COMMCONFIG, lpdwsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("api-ms-win-core-comm-l1-1-1.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenCommPort(uportnumber : u32, dwdesiredaccess : u32, dwflagsandattributes : u32) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PurgeComm(hfile : super::super::Foundation:: HANDLE, dwflags : PURGE_COMM_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCommBreak(hfile : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCommConfig(hcommdev : super::super::Foundation:: HANDLE, lpcc : *const COMMCONFIG, dwsize : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCommMask(hfile : super::super::Foundation:: HANDLE, dwevtmask : COMM_EVENT_MASK) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCommState(hfile : super::super::Foundation:: HANDLE, lpdcb : *const DCB) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCommTimeouts(hfile : super::super::Foundation:: HANDLE, lpcommtimeouts : *const COMMTIMEOUTS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetDefaultCommConfigA(lpszname : ::windows_sys::core::PCSTR, lpcc : *const COMMCONFIG, dwsize : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetDefaultCommConfigW(lpszname : ::windows_sys::core::PCWSTR, lpcc : *const COMMCONFIG, dwsize : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupComm(hfile : super::super::Foundation:: HANDLE, dwinqueue : u32, dwoutqueue : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TransmitCommChar(hfile : super::super::Foundation:: HANDLE, cchar : u8) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn WaitCommEvent(hfile : super::super::Foundation:: HANDLE, lpevtmask : *mut COMM_EVENT_MASK, lpoverlapped : *mut super::super::System::IO:: OVERLAPPED) -> super::super::Foundation:: BOOL);
pub const CE_BREAK: CLEAR_COMM_ERROR_FLAGS = 16u32;
pub const CE_FRAME: CLEAR_COMM_ERROR_FLAGS = 8u32;
pub const CE_OVERRUN: CLEAR_COMM_ERROR_FLAGS = 2u32;
pub const CE_RXOVER: CLEAR_COMM_ERROR_FLAGS = 1u32;
pub const CE_RXPARITY: CLEAR_COMM_ERROR_FLAGS = 4u32;
pub const CLRBREAK: ESCAPE_COMM_FUNCTION = 9u32;
pub const CLRDTR: ESCAPE_COMM_FUNCTION = 6u32;
pub const CLRRTS: ESCAPE_COMM_FUNCTION = 4u32;
pub const DIALOPTION_BILLING: MODEMDEVCAPS_DIAL_OPTIONS = 64u32;
pub const DIALOPTION_DIALTONE: MODEMDEVCAPS_DIAL_OPTIONS = 256u32;
pub const DIALOPTION_QUIET: MODEMDEVCAPS_DIAL_OPTIONS = 128u32;
pub const EVENPARITY: DCB_PARITY = 2u8;
pub const EV_BREAK: COMM_EVENT_MASK = 64u32;
pub const EV_CTS: COMM_EVENT_MASK = 8u32;
pub const EV_DSR: COMM_EVENT_MASK = 16u32;
pub const EV_ERR: COMM_EVENT_MASK = 128u32;
pub const EV_EVENT1: COMM_EVENT_MASK = 2048u32;
pub const EV_EVENT2: COMM_EVENT_MASK = 4096u32;
pub const EV_PERR: COMM_EVENT_MASK = 512u32;
pub const EV_RING: COMM_EVENT_MASK = 256u32;
pub const EV_RLSD: COMM_EVENT_MASK = 32u32;
pub const EV_RX80FULL: COMM_EVENT_MASK = 1024u32;
pub const EV_RXCHAR: COMM_EVENT_MASK = 1u32;
pub const EV_RXFLAG: COMM_EVENT_MASK = 2u32;
pub const EV_TXEMPTY: COMM_EVENT_MASK = 4u32;
pub const MARKPARITY: DCB_PARITY = 3u8;
pub const MAXLENGTH_NAI: u32 = 72u32;
pub const MAXLENGTH_UICCDATASTORE: u32 = 10u32;
pub const MDMSPKRFLAG_CALLSETUP: MODEMDEVCAPS_SPEAKER_MODE = 8u32;
pub const MDMSPKRFLAG_DIAL: MODEMDEVCAPS_SPEAKER_MODE = 2u32;
pub const MDMSPKRFLAG_OFF: MODEMDEVCAPS_SPEAKER_MODE = 1u32;
pub const MDMSPKRFLAG_ON: MODEMDEVCAPS_SPEAKER_MODE = 4u32;
pub const MDMSPKR_CALLSETUP: MODEMSETTINGS_SPEAKER_MODE = 8u32;
pub const MDMSPKR_DIAL: MODEMSETTINGS_SPEAKER_MODE = 2u32;
pub const MDMSPKR_OFF: MODEMSETTINGS_SPEAKER_MODE = 1u32;
pub const MDMSPKR_ON: MODEMSETTINGS_SPEAKER_MODE = 4u32;
pub const MDMVOLFLAG_HIGH: MODEMDEVCAPS_SPEAKER_VOLUME = 4u32;
pub const MDMVOLFLAG_LOW: MODEMDEVCAPS_SPEAKER_VOLUME = 1u32;
pub const MDMVOLFLAG_MEDIUM: MODEMDEVCAPS_SPEAKER_VOLUME = 2u32;
pub const MDMVOL_HIGH: MODEM_SPEAKER_VOLUME = 2u32;
pub const MDMVOL_LOW: MODEM_SPEAKER_VOLUME = 0u32;
pub const MDMVOL_MEDIUM: MODEM_SPEAKER_VOLUME = 1u32;
pub const MDM_ANALOG_RLP_OFF: u32 = 1u32;
pub const MDM_ANALOG_RLP_ON: u32 = 0u32;
pub const MDM_ANALOG_V34: u32 = 2u32;
pub const MDM_AUTO_ML_2: u32 = 2u32;
pub const MDM_AUTO_ML_DEFAULT: u32 = 0u32;
pub const MDM_AUTO_ML_NONE: u32 = 1u32;
pub const MDM_AUTO_SPEED_DEFAULT: u32 = 0u32;
pub const MDM_BEARERMODE_ANALOG: u32 = 0u32;
pub const MDM_BEARERMODE_GSM: u32 = 2u32;
pub const MDM_BEARERMODE_ISDN: u32 = 1u32;
pub const MDM_BLIND_DIAL: u32 = 512u32;
pub const MDM_CCITT_OVERRIDE: u32 = 64u32;
pub const MDM_CELLULAR: u32 = 8u32;
pub const MDM_COMPRESSION: u32 = 1u32;
pub const MDM_DIAGNOSTICS: u32 = 2048u32;
pub const MDM_ERROR_CONTROL: u32 = 2u32;
pub const MDM_FLOWCONTROL_HARD: u32 = 16u32;
pub const MDM_FLOWCONTROL_SOFT: u32 = 32u32;
pub const MDM_FORCED_EC: u32 = 4u32;
pub const MDM_HDLCPPP_AUTH_CHAP: u32 = 3u32;
pub const MDM_HDLCPPP_AUTH_DEFAULT: u32 = 0u32;
pub const MDM_HDLCPPP_AUTH_MSCHAP: u32 = 4u32;
pub const MDM_HDLCPPP_AUTH_NONE: u32 = 1u32;
pub const MDM_HDLCPPP_AUTH_PAP: u32 = 2u32;
pub const MDM_HDLCPPP_ML_2: u32 = 2u32;
pub const MDM_HDLCPPP_ML_DEFAULT: u32 = 0u32;
pub const MDM_HDLCPPP_ML_NONE: u32 = 1u32;
pub const MDM_HDLCPPP_SPEED_56K: u32 = 2u32;
pub const MDM_HDLCPPP_SPEED_64K: u32 = 1u32;
pub const MDM_HDLCPPP_SPEED_DEFAULT: u32 = 0u32;
pub const MDM_MASK_AUTO_SPEED: u32 = 7u32;
pub const MDM_MASK_BEARERMODE: u32 = 61440u32;
pub const MDM_MASK_HDLCPPP_SPEED: u32 = 7u32;
pub const MDM_MASK_PROTOCOLDATA: u32 = 267386880u32;
pub const MDM_MASK_PROTOCOLID: u32 = 983040u32;
pub const MDM_MASK_V110_SPEED: u32 = 15u32;
pub const MDM_MASK_V120_SPEED: u32 = 7u32;
pub const MDM_MASK_X75_DATA: u32 = 7u32;
pub const MDM_PIAFS_INCOMING: u32 = 0u32;
pub const MDM_PIAFS_OUTGOING: u32 = 1u32;
pub const MDM_PROTOCOLID_ANALOG: u32 = 7u32;
pub const MDM_PROTOCOLID_AUTO: u32 = 6u32;
pub const MDM_PROTOCOLID_DEFAULT: u32 = 0u32;
pub const MDM_PROTOCOLID_GPRS: u32 = 8u32;
pub const MDM_PROTOCOLID_HDLCPPP: u32 = 1u32;
pub const MDM_PROTOCOLID_PIAFS: u32 = 9u32;
pub const MDM_PROTOCOLID_V110: u32 = 4u32;
pub const MDM_PROTOCOLID_V120: u32 = 5u32;
pub const MDM_PROTOCOLID_V128: u32 = 2u32;
pub const MDM_PROTOCOLID_X75: u32 = 3u32;
pub const MDM_SHIFT_AUTO_ML: u32 = 6u32;
pub const MDM_SHIFT_AUTO_SPEED: u32 = 0u32;
pub const MDM_SHIFT_BEARERMODE: u32 = 12u32;
pub const MDM_SHIFT_EXTENDEDINFO: u32 = 12u32;
pub const MDM_SHIFT_HDLCPPP_AUTH: u32 = 3u32;
pub const MDM_SHIFT_HDLCPPP_ML: u32 = 6u32;
pub const MDM_SHIFT_HDLCPPP_SPEED: u32 = 0u32;
pub const MDM_SHIFT_PROTOCOLDATA: u32 = 20u32;
pub const MDM_SHIFT_PROTOCOLID: u32 = 16u32;
pub const MDM_SHIFT_PROTOCOLINFO: u32 = 16u32;
pub const MDM_SHIFT_V110_SPEED: u32 = 0u32;
pub const MDM_SHIFT_V120_ML: u32 = 6u32;
pub const MDM_SHIFT_V120_SPEED: u32 = 0u32;
pub const MDM_SHIFT_X75_DATA: u32 = 0u32;
pub const MDM_SPEED_ADJUST: u32 = 128u32;
pub const MDM_TONE_DIAL: u32 = 256u32;
pub const MDM_V110_SPEED_12DOT0K: u32 = 5u32;
pub const MDM_V110_SPEED_14DOT4K: u32 = 6u32;
pub const MDM_V110_SPEED_19DOT2K: u32 = 7u32;
pub const MDM_V110_SPEED_1DOT2K: u32 = 1u32;
pub const MDM_V110_SPEED_28DOT8K: u32 = 8u32;
pub const MDM_V110_SPEED_2DOT4K: u32 = 2u32;
pub const MDM_V110_SPEED_38DOT4K: u32 = 9u32;
pub const MDM_V110_SPEED_4DOT8K: u32 = 3u32;
pub const MDM_V110_SPEED_57DOT6K: u32 = 10u32;
pub const MDM_V110_SPEED_9DOT6K: u32 = 4u32;
pub const MDM_V110_SPEED_DEFAULT: u32 = 0u32;
pub const MDM_V120_ML_2: u32 = 2u32;
pub const MDM_V120_ML_DEFAULT: u32 = 0u32;
pub const MDM_V120_ML_NONE: u32 = 1u32;
pub const MDM_V120_SPEED_56K: u32 = 2u32;
pub const MDM_V120_SPEED_64K: u32 = 1u32;
pub const MDM_V120_SPEED_DEFAULT: u32 = 0u32;
pub const MDM_V23_OVERRIDE: u32 = 1024u32;
pub const MDM_X75_DATA_128K: u32 = 2u32;
pub const MDM_X75_DATA_64K: u32 = 1u32;
pub const MDM_X75_DATA_BTX: u32 = 4u32;
pub const MDM_X75_DATA_DEFAULT: u32 = 0u32;
pub const MDM_X75_DATA_T_70: u32 = 3u32;
pub const MS_CTS_ON: MODEM_STATUS_FLAGS = 16u32;
pub const MS_DSR_ON: MODEM_STATUS_FLAGS = 32u32;
pub const MS_RING_ON: MODEM_STATUS_FLAGS = 64u32;
pub const MS_RLSD_ON: MODEM_STATUS_FLAGS = 128u32;
pub const NOPARITY: DCB_PARITY = 0u8;
pub const ODDPARITY: DCB_PARITY = 1u8;
pub const ONE5STOPBITS: DCB_STOP_BITS = 1u8;
pub const ONESTOPBIT: DCB_STOP_BITS = 0u8;
pub const PARITY_EVEN: COMMPROP_STOP_PARITY = 1024u16;
pub const PARITY_MARK: COMMPROP_STOP_PARITY = 2048u16;
pub const PARITY_NONE: COMMPROP_STOP_PARITY = 256u16;
pub const PARITY_ODD: COMMPROP_STOP_PARITY = 512u16;
pub const PARITY_SPACE: COMMPROP_STOP_PARITY = 4096u16;
pub const PURGE_RXABORT: PURGE_COMM_FLAGS = 2u32;
pub const PURGE_RXCLEAR: PURGE_COMM_FLAGS = 8u32;
pub const PURGE_TXABORT: PURGE_COMM_FLAGS = 1u32;
pub const PURGE_TXCLEAR: PURGE_COMM_FLAGS = 4u32;
pub const SETBREAK: ESCAPE_COMM_FUNCTION = 8u32;
pub const SETDTR: ESCAPE_COMM_FUNCTION = 5u32;
pub const SETRTS: ESCAPE_COMM_FUNCTION = 3u32;
pub const SETXOFF: ESCAPE_COMM_FUNCTION = 1u32;
pub const SETXON: ESCAPE_COMM_FUNCTION = 2u32;
pub const SID_3GPP_SUPSVCMODEL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d08e07_d767_4478_b14a_eecc87ea12f7);
pub const SPACEPARITY: DCB_PARITY = 4u8;
pub const STOPBITS_10: COMMPROP_STOP_PARITY = 1u16;
pub const STOPBITS_15: COMMPROP_STOP_PARITY = 2u16;
pub const STOPBITS_20: COMMPROP_STOP_PARITY = 4u16;
pub const TWOSTOPBITS: DCB_STOP_BITS = 2u8;
pub type CLEAR_COMM_ERROR_FLAGS = u32;
pub type COMMPROP_STOP_PARITY = u16;
pub type COMM_EVENT_MASK = u32;
pub type DCB_PARITY = u8;
pub type DCB_STOP_BITS = u8;
pub type ESCAPE_COMM_FUNCTION = u32;
pub type MODEMDEVCAPS_DIAL_OPTIONS = u32;
pub type MODEMDEVCAPS_SPEAKER_MODE = u32;
pub type MODEMDEVCAPS_SPEAKER_VOLUME = u32;
pub type MODEMSETTINGS_SPEAKER_MODE = u32;
pub type MODEM_SPEAKER_VOLUME = u32;
pub type MODEM_STATUS_FLAGS = u32;
pub type PURGE_COMM_FLAGS = u32;
#[repr(C)]
pub struct COMMCONFIG {
    pub dwSize: u32,
    pub wVersion: u16,
    pub wReserved: u16,
    pub dcb: DCB,
    pub dwProviderSubType: u32,
    pub dwProviderOffset: u32,
    pub dwProviderSize: u32,
    pub wcProviderData: [u16; 1],
}
impl ::core::marker::Copy for COMMCONFIG {}
impl ::core::clone::Clone for COMMCONFIG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COMMPROP {
    pub wPacketLength: u16,
    pub wPacketVersion: u16,
    pub dwServiceMask: u32,
    pub dwReserved1: u32,
    pub dwMaxTxQueue: u32,
    pub dwMaxRxQueue: u32,
    pub dwMaxBaud: u32,
    pub dwProvSubType: u32,
    pub dwProvCapabilities: u32,
    pub dwSettableParams: u32,
    pub dwSettableBaud: u32,
    pub wSettableData: u16,
    pub wSettableStopParity: COMMPROP_STOP_PARITY,
    pub dwCurrentTxQueue: u32,
    pub dwCurrentRxQueue: u32,
    pub dwProvSpec1: u32,
    pub dwProvSpec2: u32,
    pub wcProvChar: [u16; 1],
}
impl ::core::marker::Copy for COMMPROP {}
impl ::core::clone::Clone for COMMPROP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COMMTIMEOUTS {
    pub ReadIntervalTimeout: u32,
    pub ReadTotalTimeoutMultiplier: u32,
    pub ReadTotalTimeoutConstant: u32,
    pub WriteTotalTimeoutMultiplier: u32,
    pub WriteTotalTimeoutConstant: u32,
}
impl ::core::marker::Copy for COMMTIMEOUTS {}
impl ::core::clone::Clone for COMMTIMEOUTS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COMSTAT {
    pub _bitfield: u32,
    pub cbInQue: u32,
    pub cbOutQue: u32,
}
impl ::core::marker::Copy for COMSTAT {}
impl ::core::clone::Clone for COMSTAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DCB {
    pub DCBlength: u32,
    pub BaudRate: u32,
    pub _bitfield: u32,
    pub wReserved: u16,
    pub XonLim: u16,
    pub XoffLim: u16,
    pub ByteSize: u8,
    pub Parity: DCB_PARITY,
    pub StopBits: DCB_STOP_BITS,
    pub XonChar: u8,
    pub XoffChar: u8,
    pub ErrorChar: u8,
    pub EofChar: u8,
    pub EvtChar: u8,
    pub wReserved1: u16,
}
impl ::core::marker::Copy for DCB {}
impl ::core::clone::Clone for DCB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MODEMDEVCAPS {
    pub dwActualSize: u32,
    pub dwRequiredSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwModemProviderVersion: u32,
    pub dwModemManufacturerOffset: u32,
    pub dwModemManufacturerSize: u32,
    pub dwModemModelOffset: u32,
    pub dwModemModelSize: u32,
    pub dwModemVersionOffset: u32,
    pub dwModemVersionSize: u32,
    pub dwDialOptions: MODEMDEVCAPS_DIAL_OPTIONS,
    pub dwCallSetupFailTimer: u32,
    pub dwInactivityTimeout: u32,
    pub dwSpeakerVolume: MODEMDEVCAPS_SPEAKER_VOLUME,
    pub dwSpeakerMode: MODEMDEVCAPS_SPEAKER_MODE,
    pub dwModemOptions: u32,
    pub dwMaxDTERate: u32,
    pub dwMaxDCERate: u32,
    pub abVariablePortion: [u8; 1],
}
impl ::core::marker::Copy for MODEMDEVCAPS {}
impl ::core::clone::Clone for MODEMDEVCAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MODEMSETTINGS {
    pub dwActualSize: u32,
    pub dwRequiredSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwCallSetupFailTimer: u32,
    pub dwInactivityTimeout: u32,
    pub dwSpeakerVolume: MODEM_SPEAKER_VOLUME,
    pub dwSpeakerMode: MODEMSETTINGS_SPEAKER_MODE,
    pub dwPreferredModemOptions: u32,
    pub dwNegotiatedModemOptions: u32,
    pub dwNegotiatedDCERate: u32,
    pub abVariablePortion: [u8; 1],
}
impl ::core::marker::Copy for MODEMSETTINGS {}
impl ::core::clone::Clone for MODEMSETTINGS {
    fn clone(&self) -> Self {
        *self
    }
}
