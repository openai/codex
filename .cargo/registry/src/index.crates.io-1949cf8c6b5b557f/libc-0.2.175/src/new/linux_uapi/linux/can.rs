//! Header: `uapi/linux/can.h`

// FIXME(ctest): we shouldn't have to specify the path but garando doesn't find modules otherwise
#[path = "can/j1939.rs"]
pub(crate) mod j1939;
#[path = "can/raw.rs"]
pub(crate) mod raw;

pub use j1939::*;
pub use raw::*;

use crate::prelude::*;

pub const CAN_EFF_FLAG: canid_t = 0x80000000;
pub const CAN_RTR_FLAG: canid_t = 0x40000000;
pub const CAN_ERR_FLAG: canid_t = 0x20000000;

pub const CAN_SFF_MASK: canid_t = 0x000007FF;
pub const CAN_EFF_MASK: canid_t = 0x1FFFFFFF;
pub const CAN_ERR_MASK: canid_t = 0x1FFFFFFF;
pub const CANXL_PRIO_MASK: crate::canid_t = CAN_SFF_MASK;

pub type canid_t = u32;

pub const CAN_SFF_ID_BITS: c_int = 11;
pub const CAN_EFF_ID_BITS: c_int = 29;
pub const CANXL_PRIO_BITS: c_int = CAN_SFF_ID_BITS;

pub type can_err_mask_t = u32;

pub const CAN_MAX_DLC: c_int = 8;
pub const CAN_MAX_DLEN: usize = 8;

pub const CANFD_MAX_DLC: c_int = 15;
pub const CANFD_MAX_DLEN: usize = 64;

pub const CANXL_MIN_DLC: c_int = 0;
pub const CANXL_MAX_DLC: c_int = 2047;
pub const CANXL_MAX_DLC_MASK: c_int = 0x07FF;
pub const CANXL_MIN_DLEN: usize = 1;
pub const CANXL_MAX_DLEN: usize = 2048;

s! {
    #[repr(align(8))]
    pub struct can_frame {
        pub can_id: canid_t,
        // FIXME(1.0): this field was renamed to `len` in Linux 5.11
        pub can_dlc: u8,
        __pad: u8,
        __res0: u8,
        pub len8_dlc: u8,
        pub data: [u8; CAN_MAX_DLEN],
    }
}

pub const CANFD_BRS: c_int = 0x01;
pub const CANFD_ESI: c_int = 0x02;
pub const CANFD_FDF: c_int = 0x04;

s! {
    #[repr(align(8))]
    pub struct canfd_frame {
        pub can_id: canid_t,
        pub len: u8,
        pub flags: u8,
        __res0: u8,
        __res1: u8,
        pub data: [u8; CANFD_MAX_DLEN],
    }
}

pub const CANXL_XLF: c_int = 0x80;
pub const CANXL_SEC: c_int = 0x01;

s! {
    #[repr(align(8))]
    pub struct canxl_frame {
        pub prio: canid_t,
        pub flags: u8,
        pub sdt: u8,
        pub len: u16,
        pub af: u32,
        pub data: [u8; CANXL_MAX_DLEN],
    }
}

pub const CAN_MTU: usize = size_of::<can_frame>();
pub const CANFD_MTU: usize = size_of::<canfd_frame>();
pub const CANXL_MTU: usize = size_of::<canxl_frame>();
// FIXME(offset_of): use `core::mem::offset_of!` once that is available
// https://github.com/rust-lang/rfcs/pull/3308
// pub const CANXL_HDR_SIZE: usize = core::mem::offset_of!(canxl_frame, data);
pub const CANXL_HDR_SIZE: usize = 12;
pub const CANXL_MIN_MTU: usize = CANXL_HDR_SIZE + 64;
pub const CANXL_MAX_MTU: usize = CANXL_MTU;

pub const CAN_RAW: c_int = 1;
pub const CAN_BCM: c_int = 2;
pub const CAN_TP16: c_int = 3;
pub const CAN_TP20: c_int = 4;
pub const CAN_MCNET: c_int = 5;
pub const CAN_ISOTP: c_int = 6;
pub const CAN_J1939: c_int = 7;
pub const CAN_NPROTO: c_int = 8;

pub const SOL_CAN_BASE: c_int = 100;

s_no_extra_traits! {
    pub struct sockaddr_can {
        pub can_family: crate::sa_family_t,
        pub can_ifindex: c_int,
        pub can_addr: __c_anonymous_sockaddr_can_can_addr,
    }

    pub union __c_anonymous_sockaddr_can_can_addr {
        pub tp: __c_anonymous_sockaddr_can_tp,
        pub j1939: __c_anonymous_sockaddr_can_j1939,
    }
}

s! {
    pub struct __c_anonymous_sockaddr_can_tp {
        pub rx_id: canid_t,
        pub tx_id: canid_t,
    }

    pub struct __c_anonymous_sockaddr_can_j1939 {
        pub name: u64,
        pub pgn: u32,
        pub addr: u8,
    }

    pub struct can_filter {
        pub can_id: canid_t,
        pub can_mask: canid_t,
    }
}

pub const CAN_INV_FILTER: canid_t = 0x20000000;
