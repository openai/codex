//! Reboot/shutdown
//!
//! On Linux, This can also be used to enable/disable Ctrl-Alt-Delete.

use crate::errno::Errno;
use crate::Result;
use cfg_if::cfg_if;
use std::convert::Infallible;

cfg_if! {
    if #[cfg(target_os = "linux")] {
        use std::mem::drop;

        libc_enum! {
            /// How exactly should the system be rebooted.
            ///
            /// See [`set_cad_enabled()`](fn.set_cad_enabled.html) for
            /// enabling/disabling Ctrl-Alt-Delete.
            #[repr(i32)]
            #[non_exhaustive]
            pub enum RebootMode {
                /// Halt the system.
                RB_HALT_SYSTEM,
                /// Execute a kernel that has been loaded earlier with
                /// [`kexec_load(2)`](https://man7.org/linux/man-pages/man2/kexec_load.2.html).
                RB_KEXEC,
                /// Stop the system and switch off power, if possible.
                RB_POWER_OFF,
                /// Restart the system.
                RB_AUTOBOOT,
                // we do not support Restart2.
                /// Suspend the system using software suspend.
                RB_SW_SUSPEND,
            }
        }

        /// Reboots or shuts down the system.
        pub fn reboot(how: RebootMode) -> Result<Infallible> {
            unsafe { libc::reboot(how as libc::c_int) };
            Err(Errno::last())
        }

        /// Enable or disable the reboot keystroke (Ctrl-Alt-Delete).
        ///
        /// Corresponds to calling `reboot(RB_ENABLE_CAD)` or `reboot(RB_DISABLE_CAD)` in C.
        pub fn set_cad_enabled(enable: bool) -> Result<()> {
            let cmd = if enable {
                libc::RB_ENABLE_CAD
            } else {
                libc::RB_DISABLE_CAD
            };
            let res = unsafe { libc::reboot(cmd) };
            Errno::result(res).map(drop)
        }
    } else if #[cfg(netbsdlike)] {
        use libc::c_int;

        libc_bitflags! {
            /// How exactly should the system be rebooted.
            pub struct RebootMode: c_int {
                /// The default, causing the system to reboot in its usual fashion.
                RB_AUTOBOOT;
                /// Interpreted by the bootstrap program itself, causing it to
                /// prompt on the console as to what file should be booted.
                /// Normally, the system is booted from the file “xx(0,0)bsd”,
                /// where xx is the default disk name, without prompting for
                /// the file name.
                RB_ASKNAME;
                /// Dump kernel memory before rebooting; see `savecore(8)` for
                /// more information.
                RB_DUMP;
                /// The processor is simply halted; no reboot takes place.
                RB_HALT;
                /// Power off the system if the system hardware supports the
                /// function, otherwise it has no effect.
                ///
                /// Should be used in conjunction with `RB_HALT`.
                RB_POWERDOWN;
                /// By default, the system will halt if `reboot()` is called during
                /// startup (before the system has finished autoconfiguration), even
                /// if `RB_HALT` is not specified. This is because `panic(9)`s
                /// during startup will probably just repeat on the next boot.
                /// Use of this option implies that the user has requested the
                /// action specified (for example, using the `ddb(4)` boot reboot
                /// command), so the system will reboot if a halt is not explicitly
                /// requested.
                #[cfg(target_os = "openbsd")]
                RB_USERREQ;
                /// Load the symbol table and enable a built-in debugger in the
                /// system. This option will have no useful function if the kernel
                /// is not configured for debugging. Several other options have
                /// different meaning if combined with this option, although their
                /// use may not be possible via the `reboot()` call. See `ddb(4)` for
                /// more information.
                RB_KDB;
                /// Normally, the disks are sync'd (see `sync(8)`) before the
                /// processor is halted or rebooted. This option may be useful
                /// if file system changes have been made manually or if the
                /// processor is on fire.
                RB_NOSYNC;
                /// Normally, the reboot procedure involves an automatic disk
                /// consistency check and then multi-user operations. `RB_SINGLE`
                /// prevents this, booting the system with a single-user shell on
                /// the console. `RB_SINGLE` is actually interpreted by the `init(8)`
                /// program in the newly booted system.
                ///
                /// When no options are given (i.e., `RB_AUTOBOOT` is used), the
                /// system is rebooted from file /bsd in the root file system of
                /// unit 0 of a disk chosen in a processor specific way. An automatic
                /// consistency check of the disks is normally performed (see `fsck(8)`).
                RB_SINGLE;
                /// Initially invoke the `userconf(4)` facility when the system
                /// starts up again, if it has been compiled into the kernel
                /// that is loaded.
                #[cfg(target_os = "netbsd")]
                RB_USERCONF;
                /// Don't update the hardware clock from the system clock, presumably
                /// because the system clock is suspect.
                #[cfg(target_os = "openbsd")]
                RB_TIMEBAD;
            }
        }

        /// Reboot system or halt processor
        ///
        /// For more information, see the man pages:
        ///
        /// * [NetBSD](https://man.netbsd.org/reboot.2)
        /// * [OpenBSD](https://man.openbsd.org/reboot.2)
        #[cfg(netbsdlike)]
        pub fn reboot(how: RebootMode) -> Result<Infallible> {
            #[cfg(target_os = "openbsd")]
            unsafe { libc::reboot(how.bits()) };
            #[cfg(target_os = "netbsd")]
            unsafe { libc::reboot(how.bits(), std::ptr::null_mut()) };

            Err(Errno::last())
        }
    }
}

