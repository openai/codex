use crate::ffi::CStr;

use crate::backend::fs::syscalls::{open, unlink};
use crate::backend::fs::types::{Mode, OFlags};
use crate::fd::OwnedFd;
use crate::{io, shm};

const NAME_MAX: usize = 255;
const SHM_DIR: &[u8] = b"/dev/shm/";

fn get_shm_name(name: &CStr) -> io::Result<([u8; NAME_MAX + SHM_DIR.len() + 1], usize)> {
    let name = name.to_bytes();

    if name.len() > NAME_MAX {
        return Err(io::Errno::NAMETOOLONG);
    }

    let num_slashes = name.iter().take_while(|x| **x == b'/').count();
    let after_slashes = &name[num_slashes..];
    if after_slashes.is_empty()
        || after_slashes == b"."
        || after_slashes == b".."
        || after_slashes.contains(&b'/')
    {
        return Err(io::Errno::INVAL);
    }

    let mut path = [0; NAME_MAX + SHM_DIR.len() + 1];
    path[..SHM_DIR.len()].copy_from_slice(SHM_DIR);
    path[SHM_DIR.len()..SHM_DIR.len() + name.len()].copy_from_slice(name);
    Ok((path, SHM_DIR.len() + name.len() + 1))
}

pub(crate) fn shm_open(name: &CStr, oflags: shm::OFlags, mode: Mode) -> io::Result<OwnedFd> {
    let (path, len) = get_shm_name(name)?;
    open(
        CStr::from_bytes_with_nul(&path[..len]).unwrap(),
        OFlags::from_bits(oflags.bits()).unwrap() | OFlags::CLOEXEC,
        mode,
    )
}

pub(crate) fn shm_unlink(name: &CStr) -> io::Result<()> {
    let (path, len) = get_shm_name(name)?;
    unlink(CStr::from_bytes_with_nul(&path[..len]).unwrap())
}
