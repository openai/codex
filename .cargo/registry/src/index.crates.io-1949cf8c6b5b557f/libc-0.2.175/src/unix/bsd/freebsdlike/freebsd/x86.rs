use crate::prelude::*;

pub type clock_t = c_ulong;
pub type wchar_t = i32;
pub type time_t = i32;
pub type suseconds_t = i32;
pub type register_t = i32;

s_no_extra_traits! {
    #[repr(align(16))]
    pub struct mcontext_t {
        pub mc_onstack: register_t,
        pub mc_gs: register_t,
        pub mc_fs: register_t,
        pub mc_es: register_t,
        pub mc_ds: register_t,
        pub mc_edi: register_t,
        pub mc_esi: register_t,
        pub mc_ebp: register_t,
        pub mc_isp: register_t,
        pub mc_ebx: register_t,
        pub mc_edx: register_t,
        pub mc_ecx: register_t,
        pub mc_eax: register_t,
        pub mc_trapno: register_t,
        pub mc_err: register_t,
        pub mc_eip: register_t,
        pub mc_cs: register_t,
        pub mc_eflags: register_t,
        pub mc_esp: register_t,
        pub mc_ss: register_t,
        pub mc_len: c_int,
        pub mc_fpformat: c_int,
        pub mc_ownedfp: c_int,
        pub mc_flags: register_t,
        pub mc_fpstate: [c_int; 128],
        pub mc_fsbase: register_t,
        pub mc_gsbase: register_t,
        pub mc_xfpustate: register_t,
        pub mc_xfpustate_len: register_t,
        pub mc_spare2: [c_int; 4],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for mcontext_t {
            fn eq(&self, other: &mcontext_t) -> bool {
                self.mc_onstack == other.mc_onstack
                    && self.mc_gs == other.mc_gs
                    && self.mc_fs == other.mc_fs
                    && self.mc_es == other.mc_es
                    && self.mc_ds == other.mc_ds
                    && self.mc_edi == other.mc_edi
                    && self.mc_esi == other.mc_esi
                    && self.mc_ebp == other.mc_ebp
                    && self.mc_isp == other.mc_isp
                    && self.mc_ebx == other.mc_ebx
                    && self.mc_edx == other.mc_edx
                    && self.mc_ecx == other.mc_ecx
                    && self.mc_eax == other.mc_eax
                    && self.mc_trapno == other.mc_trapno
                    && self.mc_err == other.mc_err
                    && self.mc_eip == other.mc_eip
                    && self.mc_cs == other.mc_cs
                    && self.mc_eflags == other.mc_eflags
                    && self.mc_esp == other.mc_esp
                    && self.mc_ss == other.mc_ss
                    && self.mc_len == other.mc_len
                    && self.mc_fpformat == other.mc_fpformat
                    && self.mc_ownedfp == other.mc_ownedfp
                    && self.mc_flags == other.mc_flags
                    && self
                        .mc_fpstate
                        .iter()
                        .zip(other.mc_fpstate.iter())
                        .all(|(a, b)| a == b)
                    && self.mc_fsbase == other.mc_fsbase
                    && self.mc_gsbase == other.mc_gsbase
                    && self.mc_xfpustate == other.mc_xfpustate
                    && self.mc_xfpustate_len == other.mc_xfpustate_len
                    && self
                        .mc_spare2
                        .iter()
                        .zip(other.mc_spare2.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for mcontext_t {}
        impl hash::Hash for mcontext_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.mc_onstack.hash(state);
                self.mc_gs.hash(state);
                self.mc_fs.hash(state);
                self.mc_es.hash(state);
                self.mc_ds.hash(state);
                self.mc_edi.hash(state);
                self.mc_esi.hash(state);
                self.mc_ebp.hash(state);
                self.mc_isp.hash(state);
                self.mc_ebx.hash(state);
                self.mc_edx.hash(state);
                self.mc_ecx.hash(state);
                self.mc_eax.hash(state);
                self.mc_trapno.hash(state);
                self.mc_err.hash(state);
                self.mc_eip.hash(state);
                self.mc_cs.hash(state);
                self.mc_eflags.hash(state);
                self.mc_esp.hash(state);
                self.mc_ss.hash(state);
                self.mc_len.hash(state);
                self.mc_fpformat.hash(state);
                self.mc_ownedfp.hash(state);
                self.mc_flags.hash(state);
                self.mc_fpstate.hash(state);
                self.mc_fsbase.hash(state);
                self.mc_gsbase.hash(state);
                self.mc_xfpustate.hash(state);
                self.mc_xfpustate_len.hash(state);
                self.mc_spare2.hash(state);
            }
        }
    }
}

pub(crate) const _ALIGNBYTES: usize = size_of::<c_long>() - 1;

pub const MINSIGSTKSZ: size_t = 2048; // 512 * 4

pub const BIOCSRTIMEOUT: c_ulong = 0x8008426d;
pub const BIOCGRTIMEOUT: c_ulong = 0x4008426e;
pub const KINFO_FILE_SIZE: c_int = 1392;
pub const TIOCTIMESTAMP: c_ulong = 0x40087459;
