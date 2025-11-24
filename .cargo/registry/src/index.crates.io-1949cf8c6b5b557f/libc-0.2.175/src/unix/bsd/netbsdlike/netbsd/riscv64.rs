use PT_FIRSTMACH;

use crate::prelude::*;

pub type __greg_t = u64;
pub type __cpu_simple_lock_nv_t = c_int;
pub type __gregset = [__greg_t; _NGREG];
pub type __fregset = [__freg; _NFREG];

s! {
    pub struct mcontext_t {
        pub __gregs: __gregset,
        pub __fregs: __fpregset,
        __spare: [crate::__greg_t; 7],
    }
}

s_no_extra_traits! {
    pub union __fpreg {
        pub u_u64: u64,
        pub u_d: c_double,
    }
}

pub(crate) const _ALIGNBYTES: usize = size_of::<c_long>() - 1;

pub const PT_GETREGS: c_int = PT_FIRSTMACH + 0;
pub const PT_SETREGS: c_int = PT_FIRSTMACH + 1;
pub const PT_GETFPREGS: c_int = PT_FIRSTMACH + 2;
pub const PT_SETFPREGS: c_int = PT_FIRSTMACH + 3;

pub const _NGREG: usize = 32;
pub const _NFREG: usize = 33;

pub const _REG_X1: c_int = 0;
pub const _REG_X2: c_int = 1;
pub const _REG_X3: c_int = 2;
pub const _REG_X4: c_int = 3;
pub const _REG_X5: c_int = 4;
pub const _REG_X6: c_int = 5;
pub const _REG_X7: c_int = 6;
pub const _REG_X8: c_int = 7;
pub const _REG_X9: c_int = 8;
pub const _REG_X10: c_int = 9;
pub const _REG_X11: c_int = 10;
pub const _REG_X12: c_int = 11;
pub const _REG_X13: c_int = 12;
pub const _REG_X14: c_int = 13;
pub const _REG_X15: c_int = 14;
pub const _REG_X16: c_int = 15;
pub const _REG_X17: c_int = 16;
pub const _REG_X18: c_int = 17;
pub const _REG_X19: c_int = 18;
pub const _REG_X20: c_int = 19;
pub const _REG_X21: c_int = 20;
pub const _REG_X22: c_int = 21;
pub const _REG_X23: c_int = 22;
pub const _REG_X24: c_int = 23;
pub const _REG_X25: c_int = 24;
pub const _REG_X26: c_int = 25;
pub const _REG_X27: c_int = 26;
pub const _REG_X28: c_int = 27;
pub const _REG_X29: c_int = 28;
pub const _REG_X30: c_int = 29;
pub const _REG_X31: c_int = 30;
pub const _REG_PC: c_int = 31;

pub const _REG_RA: c_int = _REG_X1;
pub const _REG_SP: c_int = _REG_X2;
pub const _REG_GP: c_int = _REG_X3;
pub const _REG_TP: c_int = _REG_X4;
pub const _REG_S0: c_int = _REG_X8;
pub const _REG_RV: c_int = _REG_X10;
pub const _REG_A0: c_int = _REG_X10;

pub const _REG_F0: c_int = 0;
pub const _REG_FPCSR: c_int = 32;
