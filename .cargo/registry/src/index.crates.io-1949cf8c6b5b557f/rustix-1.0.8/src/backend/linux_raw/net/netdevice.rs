//! Wrappers for netdevice ioctls.

#![allow(unsafe_code)]

use crate::backend::io::syscalls::ioctl;
use crate::fd::BorrowedFd;
use crate::io;
use core::ptr::addr_of_mut;
use core::{slice, str};
use linux_raw_sys::ctypes::c_char;
use linux_raw_sys::ioctl::SIOCGIFINDEX;
#[cfg(feature = "alloc")]
use linux_raw_sys::ioctl::SIOCGIFNAME;
use linux_raw_sys::net::{ifreq, ifreq__bindgen_ty_1, ifreq__bindgen_ty_2, IFNAMSIZ};
#[cfg(feature = "alloc")]
use {alloc::borrow::ToOwned, alloc::string::String};

pub(crate) fn name_to_index(fd: BorrowedFd<'_>, if_name: &str) -> io::Result<u32> {
    let if_name_bytes = if_name.as_bytes();
    if if_name_bytes.len() >= IFNAMSIZ as usize {
        return Err(io::Errno::NODEV);
    }
    if if_name_bytes.contains(&0) {
        return Err(io::Errno::NODEV);
    }

    // SAFETY: Convert `&[u8]` to `&[c_char]`.
    let if_name_bytes = unsafe {
        slice::from_raw_parts(if_name_bytes.as_ptr().cast::<c_char>(), if_name_bytes.len())
    };

    let mut ifreq = ifreq {
        ifr_ifrn: ifreq__bindgen_ty_1 { ifrn_name: [0; 16] },
        ifr_ifru: ifreq__bindgen_ty_2 { ifru_ivalue: 0 },
    };
    unsafe { ifreq.ifr_ifrn.ifrn_name[..if_name_bytes.len()].copy_from_slice(if_name_bytes) };

    unsafe { ioctl(fd, SIOCGIFINDEX, addr_of_mut!(ifreq).cast()) }?;
    let index = unsafe { ifreq.ifr_ifru.ifru_ivalue };
    Ok(index as u32)
}

#[cfg(feature = "alloc")]
pub(crate) fn index_to_name(fd: BorrowedFd<'_>, index: u32) -> io::Result<String> {
    let mut ifreq = ifreq {
        ifr_ifrn: ifreq__bindgen_ty_1 { ifrn_name: [0; 16] },
        ifr_ifru: ifreq__bindgen_ty_2 {
            ifru_ivalue: index as _,
        },
    };

    unsafe { ioctl(fd, SIOCGIFNAME, addr_of_mut!(ifreq).cast()) }?;

    if let Some(nul_byte) = unsafe { ifreq.ifr_ifrn.ifrn_name }
        .iter()
        .position(|ch| *ch == 0)
    {
        let ifrn_name = unsafe { &ifreq.ifr_ifrn.ifrn_name[..nul_byte] };

        // SAFETY: Convert `&[c_char]` to `&[u8]`.
        let ifrn_name =
            unsafe { slice::from_raw_parts(ifrn_name.as_ptr().cast::<u8>(), ifrn_name.len()) };

        str::from_utf8(ifrn_name)
            .map_err(|_| io::Errno::ILSEQ)
            .map(ToOwned::to_owned)
    } else {
        Err(io::Errno::INVAL)
    }
}
