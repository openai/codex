use crate::prelude::*;

pub type clock_t = i32;
pub type wchar_t = i32;
pub type time_t = i64;
pub type suseconds_t = i64;
pub type register_t = i64;

s! {
    pub struct reg32 {
        pub r_fs: u32,
        pub r_es: u32,
        pub r_ds: u32,
        pub r_edi: u32,
        pub r_esi: u32,
        pub r_ebp: u32,
        pub r_isp: u32,
        pub r_ebx: u32,
        pub r_edx: u32,
        pub r_ecx: u32,
        pub r_eax: u32,
        pub r_trapno: u32,
        pub r_err: u32,
        pub r_eip: u32,
        pub r_cs: u32,
        pub r_eflags: u32,
        pub r_esp: u32,
        pub r_ss: u32,
        pub r_gs: u32,
    }

    pub struct reg {
        pub r_r15: i64,
        pub r_r14: i64,
        pub r_r13: i64,
        pub r_r12: i64,
        pub r_r11: i64,
        pub r_r10: i64,
        pub r_r9: i64,
        pub r_r8: i64,
        pub r_rdi: i64,
        pub r_rsi: i64,
        pub r_rbp: i64,
        pub r_rbx: i64,
        pub r_rdx: i64,
        pub r_rcx: i64,
        pub r_rax: i64,
        pub r_trapno: u32,
        pub r_fs: u16,
        pub r_gs: u16,
        pub r_err: u32,
        pub r_es: u16,
        pub r_ds: u16,
        pub r_rip: i64,
        pub r_cs: i64,
        pub r_rflags: i64,
        pub r_rsp: i64,
        pub r_ss: i64,
    }
}

s_no_extra_traits! {
    pub struct fpreg32 {
        pub fpr_env: [u32; 7],
        pub fpr_acc: [[u8; 10]; 8],
        pub fpr_ex_sw: u32,
        pub fpr_pad: [u8; 64],
    }

    pub struct fpreg {
        pub fpr_env: [u64; 4],
        pub fpr_acc: [[u8; 16]; 8],
        pub fpr_xacc: [[u8; 16]; 16],
        pub fpr_spare: [u64; 12],
    }

    pub struct xmmreg {
        pub xmm_env: [u32; 8],
        pub xmm_acc: [[u8; 16]; 8],
        pub xmm_reg: [[u8; 16]; 8],
        pub xmm_pad: [u8; 224],
    }

    pub union __c_anonymous_elf64_auxv_union {
        pub a_val: c_long,
        pub a_ptr: *mut c_void,
        pub a_fcn: extern "C" fn(),
    }

    pub struct Elf64_Auxinfo {
        pub a_type: c_long,
        pub a_un: __c_anonymous_elf64_auxv_union,
    }

    #[repr(align(16))]
    pub struct max_align_t {
        priv_: [f64; 4],
    }

    #[repr(align(16))]
    pub struct mcontext_t {
        pub mc_onstack: register_t,
        pub mc_rdi: register_t,
        pub mc_rsi: register_t,
        pub mc_rdx: register_t,
        pub mc_rcx: register_t,
        pub mc_r8: register_t,
        pub mc_r9: register_t,
        pub mc_rax: register_t,
        pub mc_rbx: register_t,
        pub mc_rbp: register_t,
        pub mc_r10: register_t,
        pub mc_r11: register_t,
        pub mc_r12: register_t,
        pub mc_r13: register_t,
        pub mc_r14: register_t,
        pub mc_r15: register_t,
        pub mc_trapno: u32,
        pub mc_fs: u16,
        pub mc_gs: u16,
        pub mc_addr: register_t,
        pub mc_flags: u32,
        pub mc_es: u16,
        pub mc_ds: u16,
        pub mc_err: register_t,
        pub mc_rip: register_t,
        pub mc_cs: register_t,
        pub mc_rflags: register_t,
        pub mc_rsp: register_t,
        pub mc_ss: register_t,
        pub mc_len: c_long,
        pub mc_fpformat: c_long,
        pub mc_ownedfp: c_long,
        pub mc_fpstate: [c_long; 64],
        pub mc_fsbase: register_t,
        pub mc_gsbase: register_t,
        pub mc_xfpustate: register_t,
        pub mc_xfpustate_len: register_t,
        pub mc_tlsbase: register_t,
        pub mc_spare: [c_long; 3],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for fpreg32 {
            fn eq(&self, other: &fpreg32) -> bool {
                self.fpr_env == other.fpr_env
                    && self.fpr_acc == other.fpr_acc
                    && self.fpr_ex_sw == other.fpr_ex_sw
                    && self
                        .fpr_pad
                        .iter()
                        .zip(other.fpr_pad.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for fpreg32 {}
        impl hash::Hash for fpreg32 {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.fpr_env.hash(state);
                self.fpr_acc.hash(state);
                self.fpr_ex_sw.hash(state);
                self.fpr_pad.hash(state);
            }
        }

        impl PartialEq for fpreg {
            fn eq(&self, other: &fpreg) -> bool {
                self.fpr_env == other.fpr_env
                    && self.fpr_acc == other.fpr_acc
                    && self.fpr_xacc == other.fpr_xacc
                    && self.fpr_spare == other.fpr_spare
            }
        }
        impl Eq for fpreg {}
        impl hash::Hash for fpreg {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.fpr_env.hash(state);
                self.fpr_acc.hash(state);
                self.fpr_xacc.hash(state);
                self.fpr_spare.hash(state);
            }
        }

        impl PartialEq for xmmreg {
            fn eq(&self, other: &xmmreg) -> bool {
                self.xmm_env == other.xmm_env
                    && self.xmm_acc == other.xmm_acc
                    && self.xmm_reg == other.xmm_reg
                    && self
                        .xmm_pad
                        .iter()
                        .zip(other.xmm_pad.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for xmmreg {}
        impl hash::Hash for xmmreg {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.xmm_env.hash(state);
                self.xmm_acc.hash(state);
                self.xmm_reg.hash(state);
                self.xmm_pad.hash(state);
            }
        }

        // FIXME(msrv): suggested method was added in 1.85
        #[allow(unpredictable_function_pointer_comparisons)]
        impl PartialEq for __c_anonymous_elf64_auxv_union {
            fn eq(&self, other: &__c_anonymous_elf64_auxv_union) -> bool {
                unsafe {
                    self.a_val == other.a_val
                        || self.a_ptr == other.a_ptr
                        || self.a_fcn == other.a_fcn
                }
            }
        }
        impl Eq for __c_anonymous_elf64_auxv_union {}
        impl PartialEq for Elf64_Auxinfo {
            fn eq(&self, other: &Elf64_Auxinfo) -> bool {
                self.a_type == other.a_type && self.a_un == other.a_un
            }
        }
        impl Eq for Elf64_Auxinfo {}

        impl PartialEq for mcontext_t {
            fn eq(&self, other: &mcontext_t) -> bool {
                self.mc_onstack == other.mc_onstack
                    && self.mc_rdi == other.mc_rdi
                    && self.mc_rsi == other.mc_rsi
                    && self.mc_rdx == other.mc_rdx
                    && self.mc_rcx == other.mc_rcx
                    && self.mc_r8 == other.mc_r8
                    && self.mc_r9 == other.mc_r9
                    && self.mc_rax == other.mc_rax
                    && self.mc_rbx == other.mc_rbx
                    && self.mc_rbp == other.mc_rbp
                    && self.mc_r10 == other.mc_r10
                    && self.mc_r11 == other.mc_r11
                    && self.mc_r12 == other.mc_r12
                    && self.mc_r13 == other.mc_r13
                    && self.mc_r14 == other.mc_r14
                    && self.mc_r15 == other.mc_r15
                    && self.mc_trapno == other.mc_trapno
                    && self.mc_fs == other.mc_fs
                    && self.mc_gs == other.mc_gs
                    && self.mc_addr == other.mc_addr
                    && self.mc_flags == other.mc_flags
                    && self.mc_es == other.mc_es
                    && self.mc_ds == other.mc_ds
                    && self.mc_err == other.mc_err
                    && self.mc_rip == other.mc_rip
                    && self.mc_cs == other.mc_cs
                    && self.mc_rflags == other.mc_rflags
                    && self.mc_rsp == other.mc_rsp
                    && self.mc_ss == other.mc_ss
                    && self.mc_len == other.mc_len
                    && self.mc_fpformat == other.mc_fpformat
                    && self.mc_ownedfp == other.mc_ownedfp
                    && self
                        .mc_fpstate
                        .iter()
                        .zip(other.mc_fpstate.iter())
                        .all(|(a, b)| a == b)
                    && self.mc_fsbase == other.mc_fsbase
                    && self.mc_gsbase == other.mc_gsbase
                    && self.mc_xfpustate == other.mc_xfpustate
                    && self.mc_xfpustate_len == other.mc_xfpustate_len
                    && self.mc_spare == other.mc_spare
            }
        }
        impl Eq for mcontext_t {}
        impl hash::Hash for mcontext_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.mc_onstack.hash(state);
                self.mc_rdi.hash(state);
                self.mc_rsi.hash(state);
                self.mc_rdx.hash(state);
                self.mc_rcx.hash(state);
                self.mc_r8.hash(state);
                self.mc_r9.hash(state);
                self.mc_rax.hash(state);
                self.mc_rbx.hash(state);
                self.mc_rbp.hash(state);
                self.mc_r10.hash(state);
                self.mc_r11.hash(state);
                self.mc_r12.hash(state);
                self.mc_r13.hash(state);
                self.mc_r14.hash(state);
                self.mc_r15.hash(state);
                self.mc_trapno.hash(state);
                self.mc_fs.hash(state);
                self.mc_gs.hash(state);
                self.mc_addr.hash(state);
                self.mc_flags.hash(state);
                self.mc_es.hash(state);
                self.mc_ds.hash(state);
                self.mc_err.hash(state);
                self.mc_rip.hash(state);
                self.mc_cs.hash(state);
                self.mc_rflags.hash(state);
                self.mc_rsp.hash(state);
                self.mc_ss.hash(state);
                self.mc_len.hash(state);
                self.mc_fpformat.hash(state);
                self.mc_ownedfp.hash(state);
                self.mc_fpstate.hash(state);
                self.mc_fsbase.hash(state);
                self.mc_gsbase.hash(state);
                self.mc_xfpustate.hash(state);
                self.mc_xfpustate_len.hash(state);
                self.mc_spare.hash(state);
            }
        }
    }
}

pub(crate) const _ALIGNBYTES: usize = size_of::<c_long>() - 1;

pub const BIOCSRTIMEOUT: c_ulong = 0x8010426d;
pub const BIOCGRTIMEOUT: c_ulong = 0x4010426e;

pub const MAP_32BIT: c_int = 0x00080000;
pub const MINSIGSTKSZ: size_t = 2048; // 512 * 4

pub const _MC_HASSEGS: u32 = 0x1;
pub const _MC_HASBASES: u32 = 0x2;
pub const _MC_HASFPXSTATE: u32 = 0x4;
pub const _MC_FLAG_MASK: u32 = _MC_HASSEGS | _MC_HASBASES | _MC_HASFPXSTATE;

pub const _MC_FPFMT_NODEV: c_long = 0x10000;
pub const _MC_FPFMT_XMM: c_long = 0x10002;
pub const _MC_FPOWNED_NONE: c_long = 0x20000;
pub const _MC_FPOWNED_FPU: c_long = 0x20001;
pub const _MC_FPOWNED_PCB: c_long = 0x20002;

pub const KINFO_FILE_SIZE: c_int = 1392;

pub const TIOCTIMESTAMP: c_ulong = 0x40107459;
