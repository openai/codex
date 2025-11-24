#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn Netbios(pncb : *mut NCB) -> u8);
pub const ALL_TRANSPORTS: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("M\u{0}\u{0}\u{0}");
pub const ASYNCH: u32 = 128u32;
pub const CALL_PENDING: u32 = 2u32;
pub const DEREGISTERED: u32 = 5u32;
pub const DUPLICATE: u32 = 6u32;
pub const DUPLICATE_DEREG: u32 = 7u32;
pub const GROUP_NAME: u32 = 128u32;
pub const HANGUP_COMPLETE: u32 = 5u32;
pub const HANGUP_PENDING: u32 = 4u32;
pub const LISTEN_OUTSTANDING: u32 = 1u32;
pub const MAX_LANA: u32 = 254u32;
pub const MS_NBF: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("MNBF");
pub const NAME_FLAGS_MASK: u32 = 135u32;
pub const NCBACTION: u32 = 119u32;
pub const NCBADDGRNAME: u32 = 54u32;
pub const NCBADDNAME: u32 = 48u32;
pub const NCBASTAT: u32 = 51u32;
pub const NCBCALL: u32 = 16u32;
pub const NCBCANCEL: u32 = 53u32;
pub const NCBCHAINSEND: u32 = 23u32;
pub const NCBCHAINSENDNA: u32 = 114u32;
pub const NCBDELNAME: u32 = 49u32;
pub const NCBDGRECV: u32 = 33u32;
pub const NCBDGRECVBC: u32 = 35u32;
pub const NCBDGSEND: u32 = 32u32;
pub const NCBDGSENDBC: u32 = 34u32;
pub const NCBENUM: u32 = 55u32;
pub const NCBFINDNAME: u32 = 120u32;
pub const NCBHANGUP: u32 = 18u32;
pub const NCBLANSTALERT: u32 = 115u32;
pub const NCBLISTEN: u32 = 17u32;
pub const NCBNAMSZ: u32 = 16u32;
pub const NCBRECV: u32 = 21u32;
pub const NCBRECVANY: u32 = 22u32;
pub const NCBRESET: u32 = 50u32;
pub const NCBSEND: u32 = 20u32;
pub const NCBSENDNA: u32 = 113u32;
pub const NCBSSTAT: u32 = 52u32;
pub const NCBTRACE: u32 = 121u32;
pub const NCBUNLINK: u32 = 112u32;
pub const NRC_ACTSES: u32 = 15u32;
pub const NRC_BADDR: u32 = 7u32;
pub const NRC_BRIDGE: u32 = 35u32;
pub const NRC_BUFLEN: u32 = 1u32;
pub const NRC_CANCEL: u32 = 38u32;
pub const NRC_CANOCCR: u32 = 36u32;
pub const NRC_CMDCAN: u32 = 11u32;
pub const NRC_CMDTMO: u32 = 5u32;
pub const NRC_DUPENV: u32 = 48u32;
pub const NRC_DUPNAME: u32 = 13u32;
pub const NRC_ENVNOTDEF: u32 = 52u32;
pub const NRC_GOODRET: u32 = 0u32;
pub const NRC_IFBUSY: u32 = 33u32;
pub const NRC_ILLCMD: u32 = 3u32;
pub const NRC_ILLNN: u32 = 19u32;
pub const NRC_INCOMP: u32 = 6u32;
pub const NRC_INUSE: u32 = 22u32;
pub const NRC_INVADDRESS: u32 = 57u32;
pub const NRC_INVDDID: u32 = 59u32;
pub const NRC_LOCKFAIL: u32 = 60u32;
pub const NRC_LOCTFUL: u32 = 17u32;
pub const NRC_MAXAPPS: u32 = 54u32;
pub const NRC_NAMCONF: u32 = 25u32;
pub const NRC_NAMERR: u32 = 23u32;
pub const NRC_NAMTFUL: u32 = 14u32;
pub const NRC_NOCALL: u32 = 20u32;
pub const NRC_NORES: u32 = 9u32;
pub const NRC_NORESOURCES: u32 = 56u32;
pub const NRC_NOSAPS: u32 = 55u32;
pub const NRC_NOWILD: u32 = 21u32;
pub const NRC_OPENERR: u32 = 63u32;
pub const NRC_OSRESNOTAV: u32 = 53u32;
pub const NRC_PENDING: u32 = 255u32;
pub const NRC_REMTFUL: u32 = 18u32;
pub const NRC_SABORT: u32 = 24u32;
pub const NRC_SCLOSED: u32 = 10u32;
pub const NRC_SNUMOUT: u32 = 8u32;
pub const NRC_SYSTEM: u32 = 64u32;
pub const NRC_TOOMANY: u32 = 34u32;
pub const REGISTERED: u32 = 4u32;
pub const REGISTERING: u32 = 0u32;
pub const SESSION_ABORTED: u32 = 6u32;
pub const SESSION_ESTABLISHED: u32 = 3u32;
pub const UNIQUE_NAME: u32 = 0u32;
#[repr(C)]
pub struct ACTION_HEADER {
    pub transport_id: u32,
    pub action_code: u16,
    pub reserved: u16,
}
impl ::core::marker::Copy for ACTION_HEADER {}
impl ::core::clone::Clone for ACTION_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADAPTER_STATUS {
    pub adapter_address: [u8; 6],
    pub rev_major: u8,
    pub reserved0: u8,
    pub adapter_type: u8,
    pub rev_minor: u8,
    pub duration: u16,
    pub frmr_recv: u16,
    pub frmr_xmit: u16,
    pub iframe_recv_err: u16,
    pub xmit_aborts: u16,
    pub xmit_success: u32,
    pub recv_success: u32,
    pub iframe_xmit_err: u16,
    pub recv_buff_unavail: u16,
    pub t1_timeouts: u16,
    pub ti_timeouts: u16,
    pub reserved1: u32,
    pub free_ncbs: u16,
    pub max_cfg_ncbs: u16,
    pub max_ncbs: u16,
    pub xmit_buf_unavail: u16,
    pub max_dgram_size: u16,
    pub pending_sess: u16,
    pub max_cfg_sess: u16,
    pub max_sess: u16,
    pub max_sess_pkt_size: u16,
    pub name_count: u16,
}
impl ::core::marker::Copy for ADAPTER_STATUS {}
impl ::core::clone::Clone for ADAPTER_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FIND_NAME_BUFFER {
    pub length: u8,
    pub access_control: u8,
    pub frame_control: u8,
    pub destination_addr: [u8; 6],
    pub source_addr: [u8; 6],
    pub routing_info: [u8; 18],
}
impl ::core::marker::Copy for FIND_NAME_BUFFER {}
impl ::core::clone::Clone for FIND_NAME_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FIND_NAME_HEADER {
    pub node_count: u16,
    pub reserved: u8,
    pub unique_group: u8,
}
impl ::core::marker::Copy for FIND_NAME_HEADER {}
impl ::core::clone::Clone for FIND_NAME_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LANA_ENUM {
    pub length: u8,
    pub lana: [u8; 255],
}
impl ::core::marker::Copy for LANA_ENUM {}
impl ::core::clone::Clone for LANA_ENUM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NAME_BUFFER {
    pub name: [u8; 16],
    pub name_num: u8,
    pub name_flags: u8,
}
impl ::core::marker::Copy for NAME_BUFFER {}
impl ::core::clone::Clone for NAME_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct NCB {
    pub ncb_command: u8,
    pub ncb_retcode: u8,
    pub ncb_lsn: u8,
    pub ncb_num: u8,
    pub ncb_buffer: *mut u8,
    pub ncb_length: u16,
    pub ncb_callname: [u8; 16],
    pub ncb_name: [u8; 16],
    pub ncb_rto: u8,
    pub ncb_sto: u8,
    pub ncb_post: isize,
    pub ncb_lana_num: u8,
    pub ncb_cmd_cplt: u8,
    pub ncb_reserve: [u8; 18],
    pub ncb_event: super::super::Foundation::HANDLE,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for NCB {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for NCB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct NCB {
    pub ncb_command: u8,
    pub ncb_retcode: u8,
    pub ncb_lsn: u8,
    pub ncb_num: u8,
    pub ncb_buffer: *mut u8,
    pub ncb_length: u16,
    pub ncb_callname: [u8; 16],
    pub ncb_name: [u8; 16],
    pub ncb_rto: u8,
    pub ncb_sto: u8,
    pub ncb_post: isize,
    pub ncb_lana_num: u8,
    pub ncb_cmd_cplt: u8,
    pub ncb_reserve: [u8; 10],
    pub ncb_event: super::super::Foundation::HANDLE,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for NCB {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for NCB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SESSION_BUFFER {
    pub lsn: u8,
    pub state: u8,
    pub local_name: [u8; 16],
    pub remote_name: [u8; 16],
    pub rcvs_outstanding: u8,
    pub sends_outstanding: u8,
}
impl ::core::marker::Copy for SESSION_BUFFER {}
impl ::core::clone::Clone for SESSION_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SESSION_HEADER {
    pub sess_name: u8,
    pub num_sess: u8,
    pub rcv_dg_outstanding: u8,
    pub rcv_any_outstanding: u8,
}
impl ::core::marker::Copy for SESSION_HEADER {}
impl ::core::clone::Clone for SESSION_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
