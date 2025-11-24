use crate::backend::c;

/// `sysinfo`
#[cfg(linux_kernel)]
pub type Sysinfo = c::sysinfo;

#[cfg(not(target_os = "wasi"))]
pub(crate) type RawUname = c::utsname;
