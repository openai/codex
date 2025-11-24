//! `linux/can/j1939.h`

pub use crate::linux::can::*;

pub const J1939_MAX_UNICAST_ADDR: c_uchar = 0xfd;
pub const J1939_IDLE_ADDR: c_uchar = 0xfe;
pub const J1939_NO_ADDR: c_uchar = 0xff;
pub const J1939_NO_NAME: c_ulong = 0;
pub const J1939_PGN_REQUEST: c_uint = 0x0ea00;
pub const J1939_PGN_ADDRESS_CLAIMED: c_uint = 0x0ee00;
pub const J1939_PGN_ADDRESS_COMMANDED: c_uint = 0x0fed8;
pub const J1939_PGN_PDU1_MAX: c_uint = 0x3ff00;
pub const J1939_PGN_MAX: c_uint = 0x3ffff;
pub const J1939_NO_PGN: c_uint = 0x40000;

pub type pgn_t = u32;
pub type priority_t = u8;
pub type name_t = u64;

pub const SOL_CAN_J1939: c_int = SOL_CAN_BASE + CAN_J1939;

// FIXME(cleanup): these could use c_enum if it can accept anonymous enums.

pub const SO_J1939_FILTER: c_int = 1;
pub const SO_J1939_PROMISC: c_int = 2;
pub const SO_J1939_SEND_PRIO: c_int = 3;
pub const SO_J1939_ERRQUEUE: c_int = 4;

pub const SCM_J1939_DEST_ADDR: c_int = 1;
pub const SCM_J1939_DEST_NAME: c_int = 2;
pub const SCM_J1939_PRIO: c_int = 3;
pub const SCM_J1939_ERRQUEUE: c_int = 4;

pub const J1939_NLA_PAD: c_int = 0;
pub const J1939_NLA_BYTES_ACKED: c_int = 1;
pub const J1939_NLA_TOTAL_SIZE: c_int = 2;
pub const J1939_NLA_PGN: c_int = 3;
pub const J1939_NLA_SRC_NAME: c_int = 4;
pub const J1939_NLA_DEST_NAME: c_int = 5;
pub const J1939_NLA_SRC_ADDR: c_int = 6;
pub const J1939_NLA_DEST_ADDR: c_int = 7;

pub const J1939_EE_INFO_NONE: c_int = 0;
pub const J1939_EE_INFO_TX_ABORT: c_int = 1;
pub const J1939_EE_INFO_RX_RTS: c_int = 2;
pub const J1939_EE_INFO_RX_DPO: c_int = 3;
pub const J1939_EE_INFO_RX_ABORT: c_int = 4;

s! {
    pub struct j1939_filter {
        pub name: name_t,
        pub name_mask: name_t,
        pub pgn: pgn_t,
        pub pgn_mask: pgn_t,
        pub addr: u8,
        pub addr_mask: u8,
    }
}

pub const J1939_FILTER_MAX: c_int = 512;
