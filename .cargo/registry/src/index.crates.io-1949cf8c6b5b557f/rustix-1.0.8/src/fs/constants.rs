//! Filesystem API constants, translated into `bitflags` constants.

use crate::backend;

pub use crate::timespec::{Nsecs, Secs, Timespec};
pub use backend::fs::types::*;

impl FileType {
    /// Returns `true` if this `FileType` is a regular file.
    pub fn is_file(self) -> bool {
        self == Self::RegularFile
    }

    /// Returns `true` if this `FileType` is a directory.
    pub fn is_dir(self) -> bool {
        self == Self::Directory
    }

    /// Returns `true` if this `FileType` is a symlink.
    pub fn is_symlink(self) -> bool {
        self == Self::Symlink
    }

    /// Returns `true` if this `FileType` is a fifo.
    #[cfg(not(target_os = "wasi"))]
    pub fn is_fifo(self) -> bool {
        self == Self::Fifo
    }

    /// Returns `true` if this `FileType` is a socket.
    #[cfg(not(target_os = "wasi"))]
    pub fn is_socket(self) -> bool {
        self == Self::Socket
    }

    /// Returns `true` if this `FileType` is a character device.
    pub fn is_char_device(self) -> bool {
        self == Self::CharacterDevice
    }

    /// Returns `true` if this `FileType` is a block device.
    pub fn is_block_device(self) -> bool {
        self == Self::BlockDevice
    }
}

#[cfg(test)]
#[allow(unused_imports)]
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use crate::backend::c;
    // Rust's libc crate lacks statx for Non-glibc targets.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    use crate::fs::{Statx, StatxTimestamp};

    #[test]
    fn test_layouts() {
        #[cfg(linux_kernel)]
        assert_eq_size!(FsWord, linux_raw_sys::general::__fsword_t);

        // Don't test against `__kernel_mode_t` on platforms where it's a
        // `u16`.
        #[cfg(linux_kernel)]
        #[cfg(not(any(
            target_arch = "x86",
            target_arch = "sparc",
            target_arch = "avr",
            target_arch = "arm",
        )))]
        assert_eq_size!(RawMode, linux_raw_sys::general::__kernel_mode_t);

        #[cfg(linux_kernel)]
        #[cfg(any(
            target_arch = "x86",
            target_arch = "sparc",
            target_arch = "avr",
            target_arch = "arm",
        ))]
        assert_eq_size!(u16, linux_raw_sys::general::__kernel_mode_t);

        let some_stat: Stat = unsafe { core::mem::zeroed() };

        // Ensure that seconds fields are 64-bit on non-y2038-bug platforms, and
        // on Linux where we use statx.
        #[cfg(any(linux_kernel, not(fix_y2038)))]
        {
            assert_eq!(some_stat.st_atime, 0_i64);
            assert_eq!(some_stat.st_mtime, 0_i64);
            assert_eq!(some_stat.st_ctime, 0_i64);
        }

        // Ensure that file offsets are 64-bit.
        assert_eq!(some_stat.st_size, 0_i64);

        // Check that various fields match expected types.
        assert_eq!(some_stat.st_mode, 0 as RawMode);
        assert_eq!(some_stat.st_dev, 0 as Dev);
        assert_eq!(some_stat.st_rdev, 0 as Dev);
        assert_eq!(some_stat.st_uid, 0 as crate::ugid::RawUid);
        assert_eq!(some_stat.st_gid, 0 as crate::ugid::RawGid);

        // `Stat` should match `c::stat` or `c::stat64` unless we need y2038
        // fixes and are using a different layout.
        #[cfg(not(any(
            all(libc, linux_kernel, target_pointer_width = "32"),
            all(
                linux_raw,
                any(
                    target_pointer_width = "32",
                    target_arch = "mips64",
                    target_arch = "mips64r6"
                )
            )
        )))]
        {
            // Check that `Stat` matches `c::stat`.
            #[cfg(not(all(
                libc,
                any(
                    all(linux_kernel, target_pointer_width = "64"),
                    target_os = "hurd",
                    target_os = "emscripten",
                    target_os = "l4re",
                )
            )))]
            {
                check_renamed_type!(Stat, stat);
                check_renamed_struct_field!(Stat, stat, st_dev);
                check_renamed_struct_field!(Stat, stat, st_ino);
                check_renamed_struct_field!(Stat, stat, st_nlink);
                check_renamed_struct_field!(Stat, stat, st_mode);
                check_renamed_struct_field!(Stat, stat, st_uid);
                check_renamed_struct_field!(Stat, stat, st_gid);
                #[cfg(all(
                    linux_raw,
                    not(any(
                        target_arch = "aarch64",
                        target_arch = "powerpc64",
                        target_arch = "riscv64",
                        target_arch = "s390x"
                    ))
                ))]
                check_renamed_struct_field!(Stat, stat, __pad0);
                check_renamed_struct_field!(Stat, stat, st_rdev);
                #[cfg(all(linux_raw, not(any(target_arch = "powerpc64", target_arch = "x86_64"))))]
                check_renamed_struct_field!(Stat, stat, __pad1);
                check_renamed_struct_field!(Stat, stat, st_size);
                check_renamed_struct_field!(Stat, stat, st_blksize);
                #[cfg(all(
                    linux_raw,
                    not(any(
                        target_arch = "powerpc64",
                        target_arch = "s390x",
                        target_arch = "x86_64"
                    ))
                ))]
                check_renamed_struct_field!(Stat, stat, __pad2);
                check_renamed_struct_field!(Stat, stat, st_blocks);
                check_renamed_struct_field!(Stat, stat, st_atime);
                #[cfg(not(target_os = "netbsd"))]
                check_renamed_struct_field!(Stat, stat, st_atime_nsec);
                #[cfg(target_os = "netbsd")]
                check_renamed_struct_renamed_field!(Stat, stat, st_atime_nsec, st_atimensec);
                check_renamed_struct_field!(Stat, stat, st_mtime);
                #[cfg(not(target_os = "netbsd"))]
                check_renamed_struct_field!(Stat, stat, st_mtime_nsec);
                #[cfg(target_os = "netbsd")]
                check_renamed_struct_renamed_field!(Stat, stat, st_mtime_nsec, st_mtimensec);
                check_renamed_struct_field!(Stat, stat, st_ctime);
                #[cfg(not(target_os = "netbsd"))]
                check_renamed_struct_field!(Stat, stat, st_ctime_nsec);
                #[cfg(target_os = "netbsd")]
                check_renamed_struct_renamed_field!(Stat, stat, st_ctime_nsec, st_ctimensec);
                #[cfg(all(
                    linux_raw,
                    not(any(
                        target_arch = "aarch64",
                        target_arch = "powerpc64",
                        target_arch = "riscv64"
                    ))
                ))]
                check_renamed_struct_field!(Stat, stat, __unused);
                #[cfg(all(linux_raw, not(any(target_arch = "s390x", target_arch = "x86_64"))))]
                check_renamed_struct_field!(Stat, stat, __unused4);
                #[cfg(all(linux_raw, not(any(target_arch = "s390x", target_arch = "x86_64"))))]
                check_renamed_struct_field!(Stat, stat, __unused5);
                #[cfg(all(
                    linux_raw,
                    not(any(
                        target_arch = "aarch64",
                        target_arch = "riscv64",
                        target_arch = "s390x",
                        target_arch = "x86_64"
                    ))
                ))]
                check_renamed_struct_field!(Stat, stat, __unused6);
            }

            // Check that `Stat` matches `c::stat64`.
            #[cfg(all(
                libc,
                any(
                    all(linux_kernel, target_pointer_width = "64"),
                    target_os = "hurd",
                    target_os = "emscripten",
                    target_os = "l4re",
                )
            ))]
            {
                check_renamed_type!(Stat, stat64);
                check_renamed_struct_field!(Stat, stat64, st_dev);
                check_renamed_struct_field!(Stat, stat64, st_ino);
                check_renamed_struct_field!(Stat, stat64, st_nlink);
                check_renamed_struct_field!(Stat, stat64, st_mode);
                check_renamed_struct_field!(Stat, stat64, st_uid);
                check_renamed_struct_field!(Stat, stat64, st_gid);
                #[cfg(all(
                    linux_raw,
                    not(any(
                        target_arch = "aarch64",
                        target_arch = "powerpc64",
                        target_arch = "riscv64",
                        target_arch = "s390x"
                    ))
                ))]
                check_renamed_struct_field!(Stat, stat64, __pad0);
                check_renamed_struct_field!(Stat, stat64, st_rdev);
                #[cfg(all(linux_raw, not(any(target_arch = "powerpc64", target_arch = "x86_64"))))]
                check_renamed_struct_field!(Stat, stat64, __pad1);
                check_renamed_struct_field!(Stat, stat64, st_size);
                check_renamed_struct_field!(Stat, stat64, st_blksize);
                #[cfg(all(
                    linux_raw,
                    not(any(
                        target_arch = "powerpc64",
                        target_arch = "s390x",
                        target_arch = "x86_64"
                    ))
                ))]
                check_renamed_struct_field!(Stat, stat64, __pad2);
                check_renamed_struct_field!(Stat, stat64, st_blocks);
                check_renamed_struct_field!(Stat, stat64, st_atime);
                check_renamed_struct_field!(Stat, stat64, st_atime_nsec);
                check_renamed_struct_field!(Stat, stat64, st_mtime);
                check_renamed_struct_field!(Stat, stat64, st_mtime_nsec);
                check_renamed_struct_field!(Stat, stat64, st_ctime);
                check_renamed_struct_field!(Stat, stat64, st_ctime_nsec);
                #[cfg(all(
                    linux_raw,
                    not(any(
                        target_arch = "aarch64",
                        target_arch = "powerpc64",
                        target_arch = "riscv64"
                    ))
                ))]
                check_renamed_struct_field!(Stat, stat64, __unused);
                #[cfg(all(linux_raw, not(any(target_arch = "s390x", target_arch = "x86_64"))))]
                check_renamed_struct_field!(Stat, stat64, __unused4);
                #[cfg(all(linux_raw, not(any(target_arch = "s390x", target_arch = "x86_64"))))]
                check_renamed_struct_field!(Stat, stat64, __unused5);
                #[cfg(all(
                    linux_raw,
                    not(any(
                        target_arch = "aarch64",
                        target_arch = "riscv64",
                        target_arch = "s390x",
                        target_arch = "x86_64"
                    ))
                ))]
                check_renamed_struct_field!(Stat, stat64, __unused6);
            }
        }

        #[cfg(not(any(
            solarish,
            target_os = "cygwin",
            target_os = "haiku",
            target_os = "nto",
            target_os = "redox",
            target_os = "wasi",
        )))]
        {
            check_renamed_type!(Fsid, fsid_t);
            #[cfg(not(libc))] // libc hides the `val` field
            check_renamed_struct_field!(Fsid, fsid_t, val);
        }

        #[cfg(linux_like)]
        {
            check_renamed_type!(StatFs, statfs64);
            check_renamed_struct_field!(StatFs, statfs64, f_type);
            check_renamed_struct_field!(StatFs, statfs64, f_bsize);
            check_renamed_struct_field!(StatFs, statfs64, f_blocks);
            check_renamed_struct_field!(StatFs, statfs64, f_bfree);
            check_renamed_struct_field!(StatFs, statfs64, f_bavail);
            check_renamed_struct_field!(StatFs, statfs64, f_files);
            check_renamed_struct_field!(StatFs, statfs64, f_ffree);
            check_renamed_struct_field!(StatFs, statfs64, f_fsid);
            check_renamed_struct_field!(StatFs, statfs64, f_namelen);
            check_renamed_struct_field!(StatFs, statfs64, f_frsize);
            check_renamed_struct_field!(StatFs, statfs64, f_flags);
            #[cfg(linux_raw)]
            check_renamed_struct_field!(StatFs, statfs64, f_spare);
        }

        // Rust's libc crate lacks statx for Non-glibc targets.
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            check_renamed_type!(StatxTimestamp, statx_timestamp);
            check_renamed_struct_field!(StatxTimestamp, statx_timestamp, tv_sec);
            check_renamed_struct_field!(StatxTimestamp, statx_timestamp, tv_nsec);
            #[cfg(linux_raw)]
            check_renamed_struct_field!(StatxTimestamp, statx_timestamp, __reserved);

            check_renamed_type!(Statx, statx);
            check_renamed_struct_field!(Statx, statx, stx_mask);
            check_renamed_struct_field!(Statx, statx, stx_blksize);
            check_renamed_struct_field!(Statx, statx, stx_attributes);
            check_renamed_struct_field!(Statx, statx, stx_nlink);
            check_renamed_struct_field!(Statx, statx, stx_uid);
            check_renamed_struct_field!(Statx, statx, stx_gid);
            check_renamed_struct_field!(Statx, statx, stx_mode);
            #[cfg(linux_raw)]
            check_renamed_struct_field!(Statx, statx, __spare0);
            check_renamed_struct_field!(Statx, statx, stx_ino);
            check_renamed_struct_field!(Statx, statx, stx_size);
            check_renamed_struct_field!(Statx, statx, stx_blocks);
            check_renamed_struct_field!(Statx, statx, stx_attributes_mask);
            check_renamed_struct_field!(Statx, statx, stx_atime);
            check_renamed_struct_field!(Statx, statx, stx_btime);
            check_renamed_struct_field!(Statx, statx, stx_ctime);
            check_renamed_struct_field!(Statx, statx, stx_mtime);
            check_renamed_struct_field!(Statx, statx, stx_rdev_major);
            check_renamed_struct_field!(Statx, statx, stx_rdev_minor);
            check_renamed_struct_field!(Statx, statx, stx_dev_major);
            check_renamed_struct_field!(Statx, statx, stx_dev_minor);
            check_renamed_struct_field!(Statx, statx, stx_mnt_id);
            check_renamed_struct_field!(Statx, statx, stx_dio_mem_align);
            check_renamed_struct_field!(Statx, statx, stx_dio_offset_align);
            #[cfg(not(libc))] // not in libc yet
            check_renamed_struct_field!(Statx, statx, stx_subvol);
            #[cfg(not(libc))] // not in libc yet
            check_renamed_struct_field!(Statx, statx, stx_atomic_write_unit_min);
            #[cfg(not(libc))] // not in libc yet
            check_renamed_struct_field!(Statx, statx, stx_atomic_write_unit_max);
            #[cfg(not(libc))] // not in libc yet
            check_renamed_struct_field!(Statx, statx, stx_atomic_write_segments_max);
            #[cfg(linux_raw)]
            check_renamed_struct_field!(Statx, statx, __spare1);
            #[cfg(linux_raw)]
            check_renamed_struct_field!(Statx, statx, __spare3);
        }
    }
}
