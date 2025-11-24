use crate::prelude::*;
use crate::PT_FIRSTMACH;

pub type c___greg_t = u64;
pub type __cpu_simple_lock_nv_t = c_uchar;

s! {
    pub struct mcontext_t {
        pub __gregs: [c___greg_t; 26],
        pub _mc_tlsbase: c___greg_t,
        pub __fpregs: [[c_char; 32]; 16],
    }

    pub struct ucontext_t {
        pub uc_flags: c_uint,
        pub uc_link: *mut crate::ucontext_t,
        pub uc_sigmask: crate::sigset_t,
        pub uc_stack: crate::stack_t,
        pub uc_mcontext: crate::mcontext_t,
    }
}

pub(crate) const _ALIGNBYTES: usize = size_of::<c_long>() - 1;

pub const PT_STEP: c_int = PT_FIRSTMACH + 0;
pub const PT_GETREGS: c_int = PT_FIRSTMACH + 1;
pub const PT_SETREGS: c_int = PT_FIRSTMACH + 2;
pub const PT_GETFPREGS: c_int = PT_FIRSTMACH + 3;
pub const PT_SETFPREGS: c_int = PT_FIRSTMACH + 4;

pub const _REG_RDI: c_int = 0;
pub const _REG_RSI: c_int = 1;
pub const _REG_RDX: c_int = 2;
pub const _REG_RCX: c_int = 3;
pub const _REG_R8: c_int = 4;
pub const _REG_R9: c_int = 5;
pub const _REG_R10: c_int = 6;
pub const _REG_R11: c_int = 7;
pub const _REG_R12: c_int = 8;
pub const _REG_R13: c_int = 9;
pub const _REG_R14: c_int = 10;
pub const _REG_R15: c_int = 11;
pub const _REG_RBP: c_int = 12;
pub const _REG_RBX: c_int = 13;
pub const _REG_RAX: c_int = 14;
pub const _REG_GS: c_int = 15;
pub const _REG_FS: c_int = 16;
pub const _REG_ES: c_int = 17;
pub const _REG_DS: c_int = 18;
pub const _REG_TRAPNO: c_int = 19;
pub const _REG_ERR: c_int = 20;
pub const _REG_RIP: c_int = 21;
pub const _REG_CS: c_int = 22;
pub const _REG_RFLAGS: c_int = 23;
pub const _REG_RSP: c_int = 24;
pub const _REG_SS: c_int = 25;
