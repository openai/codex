use crate::prelude::*;

pub type ucontext_t = sigcontext;

s! {
    pub struct sigcontext {
        __sc_unused: c_int,
        pub sc_mask: c_int,
        pub sc_sp: c_ulong,
        pub sc_lr: c_ulong,
        pub sc_elr: c_ulong,
        pub sc_spsr: c_ulong,
        pub sc_x: [c_ulong; 30],
        pub sc_cookie: c_long,
    }
}

pub(crate) const _ALIGNBYTES: usize = size_of::<c_long>() - 1;

pub const _MAX_PAGE_SHIFT: u32 = 12;
