//! Wrappers for netdevice ioctls.

#![allow(unsafe_code)]

#[cfg(feature = "alloc")]
use crate::alloc::string::String;
use crate::backend::c;
use crate::backend::io::syscalls::ioctl;
use crate::fd::BorrowedFd;
use crate::io;
#[cfg(feature = "alloc")]
use c::SIOCGIFNAME;
use c::{__c_anonymous_ifr_ifru, c_char, ifreq, IFNAMSIZ, SIOCGIFINDEX};

pub(crate) fn name_to_index(fd: BorrowedFd<'_>, if_name: &str) -> io::Result<u32> {
    let if_name_bytes = if_name.as_bytes();
    if if_name_bytes.len() >= IFNAMSIZ as usize {
        return Err(io::Errno::NODEV);
    }

    let mut ifreq = ifreq {
        ifr_name: [0; 16],
        ifr_ifru: __c_anonymous_ifr_ifru { ifru_ifindex: 0 },
    };

    let mut if_name_c_char_iter = if_name_bytes.iter().map(|byte| *byte as c_char);
    ifreq.ifr_name[..if_name_bytes.len()].fill_with(|| if_name_c_char_iter.next().unwrap());

    unsafe { ioctl(fd, SIOCGIFINDEX as _, &mut ifreq as *mut ifreq as _) }?;
    let index = unsafe { ifreq.ifr_ifru.ifru_ifindex };
    Ok(index as u32)
}

#[cfg(feature = "alloc")]
pub(crate) fn index_to_name(fd: BorrowedFd<'_>, index: u32) -> io::Result<String> {
    let mut ifreq = ifreq {
        ifr_name: [0; 16],
        ifr_ifru: __c_anonymous_ifr_ifru {
            ifru_ifindex: index as _,
        },
    };

    unsafe { ioctl(fd, SIOCGIFNAME as _, &mut ifreq as *mut ifreq as _) }?;

    if let Some(nul_byte) = ifreq.ifr_name.iter().position(|char| *char == 0) {
        let name: String = ifreq.ifr_name[..nul_byte]
            .iter()
            .map(|v| *v as u8 as char)
            .collect();

        Ok(name)
    } else {
        Err(io::Errno::INVAL)
    }
}
