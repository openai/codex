use crate::prelude::*;
use crate::PT_FIRSTMACH;

pub type greg_t = u64;
pub type __cpu_simple_lock_nv_t = c_uchar;

s! {
    pub struct __fregset {
        pub __qregs: [__c_anonymous__freg; 32],
        pub __fpcr: u32,
        pub __fpsr: u32,
    }

    pub struct mcontext_t {
        pub __gregs: [crate::greg_t; 32],
        pub __fregs: __fregset,
        __spare: [crate::greg_t; 8],
    }

    pub struct ucontext_t {
        pub uc_flags: c_uint,
        pub uc_link: *mut ucontext_t,
        pub uc_sigmask: crate::sigset_t,
        pub uc_stack: crate::stack_t,
        pub uc_mcontext: mcontext_t,
    }
}

s_no_extra_traits! {
    #[repr(align(16))]
    pub union __c_anonymous__freg {
        pub __b8: [u8; 16],
        pub __h16: [u16; 8],
        pub __s32: [u32; 4],
        pub __d64: [u64; 2],
        pub __q128: [u128; 1],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for __c_anonymous__freg {
            fn eq(&self, other: &__c_anonymous__freg) -> bool {
                unsafe {
                    self.__b8 == other.__b8
                        || self.__h16 == other.__h16
                        || self.__s32 == other.__s32
                        || self.__d64 == other.__d64
                        || self.__q128 == other.__q128
                }
            }
        }
        impl Eq for __c_anonymous__freg {}
        impl hash::Hash for __c_anonymous__freg {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    self.__b8.hash(state);
                    self.__h16.hash(state);
                    self.__s32.hash(state);
                    self.__d64.hash(state);
                    self.__q128.hash(state);
                }
            }
        }
    }
}

pub(crate) const _ALIGNBYTES: usize = size_of::<c_int>() - 1;

pub const PT_GETREGS: c_int = PT_FIRSTMACH + 0;
pub const PT_SETREGS: c_int = PT_FIRSTMACH + 1;
pub const PT_GETFPREGS: c_int = PT_FIRSTMACH + 2;
pub const PT_SETFPREGS: c_int = PT_FIRSTMACH + 3;

pub const _REG_R0: c_int = 0;
pub const _REG_R1: c_int = 1;
pub const _REG_R2: c_int = 2;
pub const _REG_R3: c_int = 3;
pub const _REG_R4: c_int = 4;
pub const _REG_R5: c_int = 5;
pub const _REG_R6: c_int = 6;
pub const _REG_R7: c_int = 7;
pub const _REG_R8: c_int = 8;
pub const _REG_R9: c_int = 9;
pub const _REG_R10: c_int = 10;
pub const _REG_R11: c_int = 11;
pub const _REG_R12: c_int = 12;
pub const _REG_R13: c_int = 13;
pub const _REG_R14: c_int = 14;
pub const _REG_R15: c_int = 15;
pub const _REG_CPSR: c_int = 16;
pub const _REG_X0: c_int = 0;
pub const _REG_X1: c_int = 1;
pub const _REG_X2: c_int = 2;
pub const _REG_X3: c_int = 3;
pub const _REG_X4: c_int = 4;
pub const _REG_X5: c_int = 5;
pub const _REG_X6: c_int = 6;
pub const _REG_X7: c_int = 7;
pub const _REG_X8: c_int = 8;
pub const _REG_X9: c_int = 9;
pub const _REG_X10: c_int = 10;
pub const _REG_X11: c_int = 11;
pub const _REG_X12: c_int = 12;
pub const _REG_X13: c_int = 13;
pub const _REG_X14: c_int = 14;
pub const _REG_X15: c_int = 15;
pub const _REG_X16: c_int = 16;
pub const _REG_X17: c_int = 17;
pub const _REG_X18: c_int = 18;
pub const _REG_X19: c_int = 19;
pub const _REG_X20: c_int = 20;
pub const _REG_X21: c_int = 21;
pub const _REG_X22: c_int = 22;
pub const _REG_X23: c_int = 23;
pub const _REG_X24: c_int = 24;
pub const _REG_X25: c_int = 25;
pub const _REG_X26: c_int = 26;
pub const _REG_X27: c_int = 27;
pub const _REG_X28: c_int = 28;
pub const _REG_X29: c_int = 29;
pub const _REG_X30: c_int = 30;
pub const _REG_X31: c_int = 31;
pub const _REG_ELR: c_int = 32;
pub const _REG_SPSR: c_int = 33;
pub const _REG_TIPDR: c_int = 34;

pub const _REG_RV: c_int = _REG_X0;
pub const _REG_FP: c_int = _REG_X29;
pub const _REG_LR: c_int = _REG_X30;
pub const _REG_SP: c_int = _REG_X31;
pub const _REG_PC: c_int = _REG_ELR;
