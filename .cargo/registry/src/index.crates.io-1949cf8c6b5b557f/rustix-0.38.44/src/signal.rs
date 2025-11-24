use crate::backend::c;

/// A signal number for use with [`kill_process`], [`kill_process_group`], and
/// [`kill_current_process_group`].
///
/// [`kill_process`]: crate::process::kill_process
/// [`kill_process_group`]: crate::process::kill_process_group
/// [`kill_current_process_group`]: crate::process::kill_current_process_group
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum Signal {
    /// `SIGHUP`
    Hup = c::SIGHUP,
    /// `SIGINT`
    Int = c::SIGINT,
    /// `SIGQUIT`
    Quit = c::SIGQUIT,
    /// `SIGILL`
    Ill = c::SIGILL,
    /// `SIGTRAP`
    Trap = c::SIGTRAP,
    /// `SIGABRT`, aka `SIGIOT`
    #[doc(alias = "Iot")]
    #[doc(alias = "Abrt")]
    Abort = c::SIGABRT,
    /// `SIGBUS`
    Bus = c::SIGBUS,
    /// `SIGFPE`
    Fpe = c::SIGFPE,
    /// `SIGKILL`
    Kill = c::SIGKILL,
    /// `SIGUSR1`
    #[cfg(not(target_os = "vita"))]
    Usr1 = c::SIGUSR1,
    /// `SIGSEGV`
    Segv = c::SIGSEGV,
    /// `SIGUSR2`
    #[cfg(not(target_os = "vita"))]
    Usr2 = c::SIGUSR2,
    /// `SIGPIPE`
    Pipe = c::SIGPIPE,
    /// `SIGALRM`
    #[doc(alias = "Alrm")]
    Alarm = c::SIGALRM,
    /// `SIGTERM`
    Term = c::SIGTERM,
    /// `SIGSTKFLT`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto",
        target_os = "vita",
        all(
            linux_kernel,
            any(
                target_arch = "mips",
                target_arch = "mips32r6",
                target_arch = "mips64",
                target_arch = "mips64r6",
                target_arch = "sparc",
                target_arch = "sparc64"
            ),
        )
    )))]
    Stkflt = c::SIGSTKFLT,
    /// `SIGCHLD`
    #[cfg(not(target_os = "vita"))]
    #[doc(alias = "Chld")]
    Child = c::SIGCHLD,
    /// `SIGCONT`
    #[cfg(not(target_os = "vita"))]
    Cont = c::SIGCONT,
    /// `SIGSTOP`
    #[cfg(not(target_os = "vita"))]
    Stop = c::SIGSTOP,
    /// `SIGTSTP`
    #[cfg(not(target_os = "vita"))]
    Tstp = c::SIGTSTP,
    /// `SIGTTIN`
    #[cfg(not(target_os = "vita"))]
    Ttin = c::SIGTTIN,
    /// `SIGTTOU`
    #[cfg(not(target_os = "vita"))]
    Ttou = c::SIGTTOU,
    /// `SIGURG`
    #[cfg(not(target_os = "vita"))]
    Urg = c::SIGURG,
    /// `SIGXCPU`
    #[cfg(not(target_os = "vita"))]
    Xcpu = c::SIGXCPU,
    /// `SIGXFSZ`
    #[cfg(not(target_os = "vita"))]
    Xfsz = c::SIGXFSZ,
    /// `SIGVTALRM`
    #[cfg(not(target_os = "vita"))]
    #[doc(alias = "Vtalrm")]
    Vtalarm = c::SIGVTALRM,
    /// `SIGPROF`
    #[cfg(not(target_os = "vita"))]
    Prof = c::SIGPROF,
    /// `SIGWINCH`
    #[cfg(not(target_os = "vita"))]
    Winch = c::SIGWINCH,
    /// `SIGIO`, aka `SIGPOLL`
    #[doc(alias = "Poll")]
    #[cfg(not(any(target_os = "haiku", target_os = "vita")))]
    Io = c::SIGIO,
    /// `SIGPWR`
    #[cfg(not(any(bsd, target_os = "haiku", target_os = "hurd", target_os = "vita")))]
    #[doc(alias = "Pwr")]
    Power = c::SIGPWR,
    /// `SIGSYS`, aka `SIGUNUSED`
    #[doc(alias = "Unused")]
    Sys = c::SIGSYS,
    /// `SIGEMT`
    #[cfg(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "hermit",
        all(
            linux_kernel,
            any(
                target_arch = "mips",
                target_arch = "mips32r6",
                target_arch = "mips64",
                target_arch = "mips64r6",
                target_arch = "sparc",
                target_arch = "sparc64"
            )
        )
    ))]
    Emt = c::SIGEMT,
    /// `SIGINFO`
    #[cfg(bsd)]
    Info = c::SIGINFO,
    /// `SIGTHR`
    #[cfg(target_os = "freebsd")]
    #[doc(alias = "Lwp")]
    Thr = c::SIGTHR,
    /// `SIGLIBRT`
    #[cfg(target_os = "freebsd")]
    Librt = c::SIGLIBRT,
}

impl Signal {
    /// Convert a raw signal number into a `Signal`, if possible.
    pub fn from_raw(sig: c::c_int) -> Option<Self> {
        match sig {
            c::SIGHUP => Some(Self::Hup),
            c::SIGINT => Some(Self::Int),
            c::SIGQUIT => Some(Self::Quit),
            c::SIGILL => Some(Self::Ill),
            c::SIGTRAP => Some(Self::Trap),
            c::SIGABRT => Some(Self::Abort),
            c::SIGBUS => Some(Self::Bus),
            c::SIGFPE => Some(Self::Fpe),
            c::SIGKILL => Some(Self::Kill),
            #[cfg(not(target_os = "vita"))]
            c::SIGUSR1 => Some(Self::Usr1),
            c::SIGSEGV => Some(Self::Segv),
            #[cfg(not(target_os = "vita"))]
            c::SIGUSR2 => Some(Self::Usr2),
            c::SIGPIPE => Some(Self::Pipe),
            c::SIGALRM => Some(Self::Alarm),
            c::SIGTERM => Some(Self::Term),
            #[cfg(not(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "haiku",
                target_os = "hurd",
                target_os = "nto",
                target_os = "vita",
                all(
                    linux_kernel,
                    any(
                        target_arch = "mips",
                        target_arch = "mips32r6",
                        target_arch = "mips64",
                        target_arch = "mips64r6",
                        target_arch = "sparc",
                        target_arch = "sparc64"
                    ),
                )
            )))]
            c::SIGSTKFLT => Some(Self::Stkflt),
            #[cfg(not(target_os = "vita"))]
            c::SIGCHLD => Some(Self::Child),
            #[cfg(not(target_os = "vita"))]
            c::SIGCONT => Some(Self::Cont),
            #[cfg(not(target_os = "vita"))]
            c::SIGSTOP => Some(Self::Stop),
            #[cfg(not(target_os = "vita"))]
            c::SIGTSTP => Some(Self::Tstp),
            #[cfg(not(target_os = "vita"))]
            c::SIGTTIN => Some(Self::Ttin),
            #[cfg(not(target_os = "vita"))]
            c::SIGTTOU => Some(Self::Ttou),
            #[cfg(not(target_os = "vita"))]
            c::SIGURG => Some(Self::Urg),
            #[cfg(not(target_os = "vita"))]
            c::SIGXCPU => Some(Self::Xcpu),
            #[cfg(not(target_os = "vita"))]
            c::SIGXFSZ => Some(Self::Xfsz),
            #[cfg(not(target_os = "vita"))]
            c::SIGVTALRM => Some(Self::Vtalarm),
            #[cfg(not(target_os = "vita"))]
            c::SIGPROF => Some(Self::Prof),
            #[cfg(not(target_os = "vita"))]
            c::SIGWINCH => Some(Self::Winch),
            #[cfg(not(any(target_os = "haiku", target_os = "vita")))]
            c::SIGIO => Some(Self::Io),
            #[cfg(not(any(bsd, target_os = "haiku", target_os = "hurd", target_os = "vita")))]
            c::SIGPWR => Some(Self::Power),
            c::SIGSYS => Some(Self::Sys),
            #[cfg(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "hermit",
                all(
                    linux_kernel,
                    any(
                        target_arch = "mips",
                        target_arch = "mips32r6",
                        target_arch = "mips64",
                        target_arch = "mips64r6",
                        target_arch = "sparc",
                        target_arch = "sparc64"
                    )
                )
            ))]
            c::SIGEMT => Some(Self::Emt),
            #[cfg(bsd)]
            c::SIGINFO => Some(Self::Info),
            #[cfg(target_os = "freebsd")]
            c::SIGTHR => Some(Self::Thr),
            #[cfg(target_os = "freebsd")]
            c::SIGLIBRT => Some(Self::Librt),
            _ => None,
        }
    }
}

#[test]
fn test_sizes() {
    assert_eq_size!(Signal, c::c_int);
}
