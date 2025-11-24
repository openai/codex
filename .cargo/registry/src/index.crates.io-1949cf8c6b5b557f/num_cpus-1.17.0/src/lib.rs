//! A crate with utilities to determine the number of CPUs available on the
//! current system.
//!
//! Sometimes the CPU will exaggerate the number of CPUs it contains, because it can use
//! [processor tricks] to deliver increased performance when there are more threads. This 
//! crate provides methods to get both the logical and physical numbers of cores.
//!
//! This information can be used as a guide to how many tasks can be run in parallel.
//! There are many properties of the system architecture that will affect parallelism,
//! for example memory access speeds (for all the caches and RAM) and the physical
//! architecture of the processor, so the number of CPUs should be used as a rough guide
//! only.
//!
//!
//! ## Examples
//!
//! Fetch the number of logical CPUs.
//!
//! ```
//! let cpus = num_cpus::get();
//! ```
//!
//! See [`rayon::Threadpool`] for an example of where the number of CPUs could be
//! used when setting up parallel jobs (Where the threadpool example uses a fixed
//! number 8, it could use the number of CPUs).
//!
//! [processor tricks]: https://en.wikipedia.org/wiki/Simultaneous_multithreading
//! [`rayon::ThreadPool`]: https://docs.rs/rayon/1.*/rayon/struct.ThreadPool.html
#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]
#![allow(non_snake_case)]

#[cfg(not(windows))]
extern crate libc;

#[cfg(target_os = "hermit")]
extern crate hermit_abi;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::{get_num_cpus, get_num_physical_cpus};

/// Returns the number of available CPUs of the current system.
///
/// This function will get the number of logical cores. Sometimes this is different from the number
/// of physical cores (See [Simultaneous multithreading on Wikipedia][smt]).
///
/// This will always return at least `1`.
///
/// # Examples
///
/// ```
/// let cpus = num_cpus::get();
/// if cpus > 1 {
///     println!("We are on a multicore system with {} CPUs", cpus);
/// } else {
///     println!("We are on a single core system");
/// }
/// ```
///
/// # Note
///
/// This will check [sched affinity] on Linux, showing a lower number of CPUs if the current
/// thread does not have access to all the computer's CPUs.
///
/// This will also check [cgroups], frequently used in containers to constrain CPU usage.
///
/// [smt]: https://en.wikipedia.org/wiki/Simultaneous_multithreading
/// [sched affinity]: http://www.gnu.org/software/libc/manual/html_node/CPU-Affinity.html
/// [cgroups]: https://www.kernel.org/doc/Documentation/cgroup-v1/cgroups.txt
#[inline]
pub fn get() -> usize {
    get_num_cpus()
}

/// Returns the number of physical cores of the current system.
///
/// This will always return at least `1`.
///
/// # Note
///
/// Physical count is supported only on Linux, mac OS and Windows platforms.
/// On other platforms, or if the physical count fails on supported platforms,
/// this function returns the same as [`get()`], which is the number of logical
/// CPUS.
///
/// # Examples
///
/// ```
/// let logical_cpus = num_cpus::get();
/// let physical_cpus = num_cpus::get_physical();
/// if logical_cpus > physical_cpus {
///     println!("We have simultaneous multithreading with about {:.2} \
///               logical cores to 1 physical core.", 
///               (logical_cpus as f64) / (physical_cpus as f64));
/// } else if logical_cpus == physical_cpus {
///     println!("Either we don't have simultaneous multithreading, or our \
///               system doesn't support getting the number of physical CPUs.");
/// } else {
///     println!("We have less logical CPUs than physical CPUs, maybe we only have access to \
///               some of the CPUs on our system.");
/// }
/// ```
///
/// [`get()`]: fn.get.html
#[inline]
pub fn get_physical() -> usize {
    get_num_physical_cpus()
}


#[cfg(not(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos",
    target_os = "openbsd",
    target_os = "aix")))]
#[inline]
fn get_num_physical_cpus() -> usize {
    // Not implemented, fall back
    get_num_cpus()
}

#[cfg(target_os = "windows")]
fn get_num_physical_cpus() -> usize {
    match get_num_physical_cpus_windows() {
        Some(num) => num,
        None => get_num_cpus()
    }
}

#[cfg(target_os = "windows")]
fn get_num_physical_cpus_windows() -> Option<usize> {
    // Inspired by https://msdn.microsoft.com/en-us/library/ms683194

    use std::ptr;
    use std::mem;

    #[allow(non_upper_case_globals)]
    const RelationProcessorCore: u32 = 0;

    #[repr(C)]
    #[allow(non_camel_case_types)]
    struct SYSTEM_LOGICAL_PROCESSOR_INFORMATION {
        mask: usize,
        relationship: u32,
        _unused: [u64; 2]
    }

    extern "system" {
        fn GetLogicalProcessorInformation(
            info: *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
            length: &mut u32
        ) -> u32;
    }

    // First we need to determine how much space to reserve.

    // The required size of the buffer, in bytes.
    let mut needed_size = 0;

    unsafe {
        GetLogicalProcessorInformation(ptr::null_mut(), &mut needed_size);
    }

    let struct_size = mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>() as u32;

    // Could be 0, or some other bogus size.
    if needed_size == 0 || needed_size < struct_size || needed_size % struct_size != 0 {
        return None;
    }

    let count = needed_size / struct_size;

    // Allocate some memory where we will store the processor info.
    let mut buf = Vec::with_capacity(count as usize);

    let result;

    unsafe {
        result = GetLogicalProcessorInformation(buf.as_mut_ptr(), &mut needed_size);
    }

    // Failed for any reason.
    if result == 0 {
        return None;
    }

    let count = needed_size / struct_size;

    unsafe {
        buf.set_len(count as usize);
    }

    let phys_proc_count = buf.iter()
        // Only interested in processor packages (physical processors.)
        .filter(|proc_info| proc_info.relationship == RelationProcessorCore)
        .count();

    if phys_proc_count == 0 {
        None
    } else {
        Some(phys_proc_count)
    }
}

#[cfg(windows)]
fn get_num_cpus() -> usize {
    #[repr(C)]
    struct SYSTEM_INFO {
        wProcessorArchitecture: u16,
        wReserved: u16,
        dwPageSize: u32,
        lpMinimumApplicationAddress: *mut u8,
        lpMaximumApplicationAddress: *mut u8,
        dwActiveProcessorMask: *mut u8,
        dwNumberOfProcessors: u32,
        dwProcessorType: u32,
        dwAllocationGranularity: u32,
        wProcessorLevel: u16,
        wProcessorRevision: u16,
    }

    extern "system" {
        fn GetSystemInfo(lpSystemInfo: *mut SYSTEM_INFO);
    }

    unsafe {
        let mut sysinfo: SYSTEM_INFO = std::mem::zeroed();
        GetSystemInfo(&mut sysinfo);
        sysinfo.dwNumberOfProcessors as usize
    }
}

#[cfg(any(target_os = "freebsd",
          target_os = "dragonfly",
          target_os = "netbsd"))]
fn get_num_cpus() -> usize {
    use std::ptr;

    let mut cpus: libc::c_uint = 0;
    let mut cpus_size = std::mem::size_of_val(&cpus);

    unsafe {
        cpus = libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as libc::c_uint;
    }
    if cpus < 1 {
        let mut mib = [libc::CTL_HW, libc::HW_NCPU, 0, 0];
        unsafe {
            libc::sysctl(mib.as_mut_ptr(),
                         2,
                         &mut cpus as *mut _ as *mut _,
                         &mut cpus_size as *mut _ as *mut _,
                         ptr::null_mut(),
                         0);
        }
        if cpus < 1 {
            cpus = 1;
        }
    }
    cpus as usize
}

#[cfg(target_os = "openbsd")]
fn get_num_cpus() -> usize {
    use std::ptr;

    let mut cpus: libc::c_uint = 0;
    let mut cpus_size = std::mem::size_of_val(&cpus);
    let mut mib = [libc::CTL_HW, libc::HW_NCPUONLINE, 0, 0];
    let rc: libc::c_int;

    unsafe {
        rc = libc::sysctl(mib.as_mut_ptr(),
                          2,
                          &mut cpus as *mut _ as *mut _,
                          &mut cpus_size as *mut _ as *mut _,
                          ptr::null_mut(),
                          0);
    }
    if rc < 0 {
        cpus = 1;
    }
    cpus as usize
}

#[cfg(target_os = "openbsd")]
fn get_num_physical_cpus() -> usize {
    use std::ptr;

    let mut cpus: libc::c_uint = 0;
    let mut cpus_size = std::mem::size_of_val(&cpus);
    let mut mib = [libc::CTL_HW, libc::HW_NCPU, 0, 0];
    let rc: libc::c_int;

    unsafe {
        rc = libc::sysctl(mib.as_mut_ptr(),
                          2,
                          &mut cpus as *mut _ as *mut _,
                          &mut cpus_size as *mut _ as *mut _,
                          ptr::null_mut(),
                          0);
    }
    if rc < 0 {
        cpus = 1;
    }
    cpus as usize
}


#[cfg(target_os = "macos")]
fn get_num_physical_cpus() -> usize {
    use std::ffi::CStr;
    use std::ptr;

    let mut cpus: i32 = 0;
    let mut cpus_size = std::mem::size_of_val(&cpus);

    let sysctl_name = CStr::from_bytes_with_nul(b"hw.physicalcpu\0")
        .expect("byte literal is missing NUL");

    unsafe {
        if 0 != libc::sysctlbyname(sysctl_name.as_ptr(),
                                   &mut cpus as *mut _ as *mut _,
                                   &mut cpus_size as *mut _ as *mut _,
                                   ptr::null_mut(),
                                   0) {
            return get_num_cpus();
        }
    }
    cpus as usize
}

#[cfg(target_os = "aix")]
fn get_num_physical_cpus() -> usize {
    match get_smt_threads_aix() {
        Some(num) => get_num_cpus() / num,
        None => get_num_cpus(),
    }
}

#[cfg(target_os = "aix")]
fn get_smt_threads_aix() -> Option<usize> {
    let smt = unsafe {
        libc::getsystemcfg(libc::SC_SMT_TC)
    };
    if smt == u64::MAX {
        return None;
    }
    Some(smt as usize)
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_os = "aix",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "fuchsia")
)]
fn get_num_cpus() -> usize {
    // On ARM targets, processors could be turned off to save power.
    // Use `_SC_NPROCESSORS_CONF` to get the real number.
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    const CONF_NAME: libc::c_int = libc::_SC_NPROCESSORS_CONF;
    #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
    const CONF_NAME: libc::c_int = libc::_SC_NPROCESSORS_ONLN;

    let cpus = unsafe { libc::sysconf(CONF_NAME) };
    if cpus < 1 {
        1
    } else {
        cpus as usize
    }
}

#[cfg(target_os = "haiku")]
fn get_num_cpus() -> usize {
    use std::mem;

    #[allow(non_camel_case_types)]
    type bigtime_t = i64;
    #[allow(non_camel_case_types)]
    type status_t = i32;

    #[repr(C)]
    pub struct system_info {
        pub boot_time: bigtime_t,
        pub cpu_count: u32,
        pub max_pages: u64,
        pub used_pages: u64,
        pub cached_pages: u64,
        pub block_cache_pages: u64,
        pub ignored_pages: u64,
        pub needed_memory: u64,
        pub free_memory: u64,
        pub max_swap_pages: u64,
        pub free_swap_pages: u64,
        pub page_faults: u32,
        pub max_sems: u32,
        pub used_sems: u32,
        pub max_ports: u32,
        pub used_ports: u32,
        pub max_threads: u32,
        pub used_threads: u32,
        pub max_teams: u32,
        pub used_teams: u32,
        pub kernel_name: [::std::os::raw::c_char; 256usize],
        pub kernel_build_date: [::std::os::raw::c_char; 32usize],
        pub kernel_build_time: [::std::os::raw::c_char; 32usize],
        pub kernel_version: i64,
        pub abi: u32,
    }

    extern {
        fn get_system_info(info: *mut system_info) -> status_t;
    }

    let mut info: system_info = unsafe { mem::zeroed() };
    let status = unsafe { get_system_info(&mut info as *mut _) };
    if status == 0 {
        info.cpu_count as usize
    } else {
        1
    }
}

#[cfg(target_os = "hermit")]
fn get_num_cpus() -> usize {
    unsafe { hermit_abi::get_processor_count() }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_os = "aix",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "fuchsia",
    target_os = "linux",
    target_os = "openbsd",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "haiku",
    target_os = "hermit",
    windows,
)))]
fn get_num_cpus() -> usize {
    1
}

#[cfg(test)]
mod tests {
    fn env_var(name: &'static str) -> Option<usize> {
        ::std::env::var(name).ok().map(|val| val.parse().unwrap())
    }

    #[test]
    fn test_get() {
        let num = super::get();
        if let Some(n) = env_var("NUM_CPUS_TEST_GET") {
            assert_eq!(num, n);
        } else {
            assert!(num > 0);
            assert!(num < 236_451);
        }
    }

    #[test]
    fn test_get_physical() {
        let num = super::get_physical();
        if let Some(n) = env_var("NUM_CPUS_TEST_GET_PHYSICAL") {
            assert_eq!(num, n);
        } else {
            assert!(num > 0);
            assert!(num < 236_451);
        }
    }
}
