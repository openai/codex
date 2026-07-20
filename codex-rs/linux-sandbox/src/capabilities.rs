use std::io;

const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
const MAX_CAPABILITY_INDEX: libc::c_int = 63;
const SECBIT_KEEP_CAPS: libc::c_int = 1 << 4;
const SECUREBITS_NO_CAPABILITY_REGAIN: libc::c_int =
    (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 5) | (1 << 6) | (1 << 7);

#[repr(C)]
struct CapabilityHeader {
    version: u32,
    pid: libc::c_int,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
#[repr(C)]
struct CapabilityData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

/// Permanently remove setup-only capabilities from the calling thread.
pub(crate) fn drop_all_capabilities() -> io::Result<()> {
    prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0)?;
    lock_securebits()?;
    drop_bounding_set()?;
    clear_and_verify_ambient_set()?;
    clear_and_verify_thread_capabilities()
}

fn prctl(option: libc::c_int, arg: libc::c_ulong, arg2: libc::c_ulong) -> io::Result<libc::c_int> {
    let result = unsafe { libc::prctl(option, arg, arg2, 0, 0) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(result)
}

fn lock_securebits() -> io::Result<()> {
    let current = prctl(libc::PR_GET_SECUREBITS, 0, 0)?;
    let desired = (current & !SECBIT_KEEP_CAPS) | SECUREBITS_NO_CAPABILITY_REGAIN;
    prctl(libc::PR_SET_SECUREBITS, desired as libc::c_ulong, 0)?;
    let actual = prctl(libc::PR_GET_SECUREBITS, 0, 0)?;
    if actual & SECUREBITS_NO_CAPABILITY_REGAIN != SECUREBITS_NO_CAPABILITY_REGAIN
        || actual & SECBIT_KEEP_CAPS != 0
    {
        return Err(io::Error::other("capability regain paths remain open"));
    }
    Ok(())
}

fn drop_bounding_set() -> io::Result<()> {
    for capability in 0..=MAX_CAPABILITY_INDEX {
        match prctl(libc::PR_CAPBSET_DROP, capability as libc::c_ulong, 0) {
            Ok(_) => {}
            Err(err) if err.raw_os_error() == Some(libc::EINVAL) && capability > 0 => break,
            Err(err) => return Err(err),
        }
        if prctl(libc::PR_CAPBSET_READ, capability as libc::c_ulong, 0)? != 0 {
            let message = format!("capability {capability} remained in bounding set");
            return Err(io::Error::other(message));
        }
    }
    Ok(())
}

fn clear_and_verify_ambient_set() -> io::Result<()> {
    prctl(
        libc::PR_CAP_AMBIENT,
        libc::PR_CAP_AMBIENT_CLEAR_ALL as libc::c_ulong,
        0,
    )?;
    for capability in 0..=MAX_CAPABILITY_INDEX {
        match prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_IS_SET as libc::c_ulong,
            capability as libc::c_ulong,
        ) {
            Ok(0) => {}
            Ok(_) => return Err(io::Error::other("ambient capabilities remained set")),
            Err(err) if err.raw_os_error() == Some(libc::EINVAL) && capability > 0 => break,
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn clear_and_verify_thread_capabilities() -> io::Result<()> {
    let mut header = CapabilityHeader {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let mut capabilities = [CapabilityData::default(); 2];
    for operation in [libc::SYS_capset, libc::SYS_capget] {
        let result = unsafe {
            libc::syscall(
                operation,
                &mut header as *mut CapabilityHeader,
                capabilities.as_mut_ptr(),
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    if capabilities != [CapabilityData::default(); 2] {
        return Err(io::Error::other("thread capabilities remained set"));
    }
    Ok(())
}
