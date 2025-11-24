cfg_if! {
    if #[cfg(any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    ))] {
        mod mips;
        pub use self::mips::*;
    } else if #[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))] {
        mod powerpc;
        pub use self::powerpc::*;
    } else if #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))] {
        mod sparc;
        pub use self::sparc::*;
    } else {
        mod generic;
        pub use self::generic::*;
    }
}
