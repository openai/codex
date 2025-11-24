use crate::prelude::*;

pub type boolean_t = c_uint;
pub type mcontext_t = *mut __darwin_mcontext64;

s! {
    pub struct ucontext_t {
        pub uc_onstack: c_int,
        pub uc_sigmask: crate::sigset_t,
        pub uc_stack: crate::stack_t,
        pub uc_link: *mut crate::ucontext_t,
        pub uc_mcsize: usize,
        pub uc_mcontext: mcontext_t,
    }

    pub struct __darwin_mcontext64 {
        pub __es: __darwin_x86_exception_state64,
        pub __ss: __darwin_x86_thread_state64,
        pub __fs: __darwin_x86_float_state64,
    }

    pub struct __darwin_x86_exception_state64 {
        pub __trapno: u16,
        pub __cpu: u16,
        pub __err: u32,
        pub __faultvaddr: u64,
    }

    pub struct __darwin_x86_thread_state64 {
        pub __rax: u64,
        pub __rbx: u64,
        pub __rcx: u64,
        pub __rdx: u64,
        pub __rdi: u64,
        pub __rsi: u64,
        pub __rbp: u64,
        pub __rsp: u64,
        pub __r8: u64,
        pub __r9: u64,
        pub __r10: u64,
        pub __r11: u64,
        pub __r12: u64,
        pub __r13: u64,
        pub __r14: u64,
        pub __r15: u64,
        pub __rip: u64,
        pub __rflags: u64,
        pub __cs: u64,
        pub __fs: u64,
        pub __gs: u64,
    }

    pub struct __darwin_x86_float_state64 {
        pub __fpu_reserved: [c_int; 2],
        __fpu_fcw: c_short,
        __fpu_fsw: c_short,
        pub __fpu_ftw: u8,
        pub __fpu_rsrv1: u8,
        pub __fpu_fop: u16,
        pub __fpu_ip: u32,
        pub __fpu_cs: u16,
        pub __fpu_rsrv2: u16,
        pub __fpu_dp: u32,
        pub __fpu_ds: u16,
        pub __fpu_rsrv3: u16,
        pub __fpu_mxcsr: u32,
        pub __fpu_mxcsrmask: u32,
        pub __fpu_stmm0: __darwin_mmst_reg,
        pub __fpu_stmm1: __darwin_mmst_reg,
        pub __fpu_stmm2: __darwin_mmst_reg,
        pub __fpu_stmm3: __darwin_mmst_reg,
        pub __fpu_stmm4: __darwin_mmst_reg,
        pub __fpu_stmm5: __darwin_mmst_reg,
        pub __fpu_stmm6: __darwin_mmst_reg,
        pub __fpu_stmm7: __darwin_mmst_reg,
        pub __fpu_xmm0: __darwin_xmm_reg,
        pub __fpu_xmm1: __darwin_xmm_reg,
        pub __fpu_xmm2: __darwin_xmm_reg,
        pub __fpu_xmm3: __darwin_xmm_reg,
        pub __fpu_xmm4: __darwin_xmm_reg,
        pub __fpu_xmm5: __darwin_xmm_reg,
        pub __fpu_xmm6: __darwin_xmm_reg,
        pub __fpu_xmm7: __darwin_xmm_reg,
        pub __fpu_xmm8: __darwin_xmm_reg,
        pub __fpu_xmm9: __darwin_xmm_reg,
        pub __fpu_xmm10: __darwin_xmm_reg,
        pub __fpu_xmm11: __darwin_xmm_reg,
        pub __fpu_xmm12: __darwin_xmm_reg,
        pub __fpu_xmm13: __darwin_xmm_reg,
        pub __fpu_xmm14: __darwin_xmm_reg,
        pub __fpu_xmm15: __darwin_xmm_reg,
        // this field is actually [u8; 96], but defining it with a bigger type
        // allows us to auto-implement traits for it since the length of the
        // array is less than 32
        __fpu_rsrv4: [u32; 24],
        pub __fpu_reserved1: c_int,
    }

    pub struct __darwin_mmst_reg {
        pub __mmst_reg: [c_char; 10],
        pub __mmst_rsrv: [c_char; 6],
    }

    pub struct __darwin_xmm_reg {
        pub __xmm_reg: [c_char; 16],
    }

    pub struct malloc_introspection_t {
        _private: [crate::uintptr_t; 16], // FIXME(macos): keeping private for now
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct malloc_zone_t {
        _reserved1: *mut c_void,
        _reserved2: *mut c_void,
        pub size:
            Option<unsafe extern "C" fn(zone: *mut malloc_zone_t, ptr: *const c_void) -> size_t>,
        pub malloc:
            Option<unsafe extern "C" fn(zone: *mut malloc_zone_t, size: size_t) -> *mut c_void>,
        pub calloc: Option<
            unsafe extern "C" fn(
                zone: *mut malloc_zone_t,
                num_items: size_t,
                size: size_t,
            ) -> *mut c_void,
        >,
        pub valloc:
            Option<unsafe extern "C" fn(zone: *mut malloc_zone_t, size: size_t) -> *mut c_void>,
        pub free: Option<unsafe extern "C" fn(zone: *mut malloc_zone_t, ptr: *mut c_void)>,
        pub realloc: Option<
            unsafe extern "C" fn(
                zone: *mut malloc_zone_t,
                ptr: *mut c_void,
                size: size_t,
            ) -> *mut c_void,
        >,
        pub destroy: Option<unsafe extern "C" fn(zone: *mut malloc_zone_t)>,
        pub zone_name: *const c_char,
        pub batch_malloc: Option<
            unsafe extern "C" fn(
                zone: *mut malloc_zone_t,
                size: size_t,
                results: *mut *mut c_void,
                num_requested: c_uint,
            ) -> c_uint,
        >,
        pub batch_free: Option<
            unsafe extern "C" fn(
                zone: *mut malloc_zone_t,
                to_be_freed: *mut *mut c_void,
                num_to_be_freed: c_uint,
            ),
        >,
        pub introspect: *mut malloc_introspection_t,
        pub version: c_uint,
        pub memalign: Option<
            unsafe extern "C" fn(
                zone: *mut malloc_zone_t,
                alignment: size_t,
                size: size_t,
            ) -> *mut c_void,
        >,
        pub free_definite_size:
            Option<unsafe extern "C" fn(zone: *mut malloc_zone_t, ptr: *mut c_void, size: size_t)>,
        pub pressure_relief:
            Option<unsafe extern "C" fn(zone: *mut malloc_zone_t, goal: size_t) -> size_t>,
        pub claimed_address: Option<
            unsafe extern "C" fn(zone: *mut malloc_zone_t, ptr: *mut c_void) -> crate::boolean_t,
        >,
    }
}

s_no_extra_traits! {
    #[repr(align(16))]
    pub struct max_align_t {
        priv_: [f64; 2],
    }
}
