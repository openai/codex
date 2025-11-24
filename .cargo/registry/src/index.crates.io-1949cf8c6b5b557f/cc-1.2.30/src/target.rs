//! Parsing of `rustc` target names to match the values exposed to Cargo
//! build scripts (`CARGO_CFG_*`).

mod apple;
mod generated;
mod llvm;
mod parser;

pub(crate) use parser::TargetInfoParser;

/// Information specific to a `rustc` target.
///
/// See <https://doc.rust-lang.org/cargo/appendix/glossary.html#target>.
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct TargetInfo<'a> {
    /// The full architecture, including the subarchitecture.
    ///
    /// This differs from `cfg!(target_arch)`, which only specifies the
    /// overall architecture, which is too coarse for certain cases.
    pub full_arch: &'a str,
    /// The overall target architecture.
    ///
    /// This is the same as the value of `cfg!(target_arch)`.
    pub arch: &'a str,
    /// The target vendor.
    ///
    /// This is the same as the value of `cfg!(target_vendor)`.
    pub vendor: &'a str,
    /// The operating system, or `none` on bare-metal targets.
    ///
    /// This is the same as the value of `cfg!(target_os)`.
    pub os: &'a str,
    /// The environment on top of the operating system.
    ///
    /// This is the same as the value of `cfg!(target_env)`.
    pub env: &'a str,
    /// The ABI on top of the operating system.
    ///
    /// This is the same as the value of `cfg!(target_abi)`.
    pub abi: &'a str,
}
