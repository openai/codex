use cfg_if::cfg_if;

#[macro_export]
macro_rules! skip {
    ($($reason: expr),+) => {{
        use ::std::io::{self, Write};

        let stderr = io::stderr();
        let mut handle = stderr.lock();
        writeln!(handle, $($reason),+).unwrap();
        return;
    }}
}

cfg_if! {
    if #[cfg(linux_android)] {
        #[macro_export] macro_rules! require_capability {
            ($name:expr, $capname:ident) => {
                use ::caps::{Capability, CapSet, has_cap};

                if !has_cap(None, CapSet::Effective, Capability::$capname)
                    .unwrap()
                {
                    skip!("{} requires capability {}. Skipping test.", $name, Capability::$capname);
                }
            }
        }
    } else if #[cfg(not(target_os = "redox"))] {
        #[macro_export] macro_rules! require_capability {
            ($name:expr, $capname:ident) => {}
        }
    }
}

/// Skip the test if we don't have the ability to mount file systems.
#[cfg(target_os = "freebsd")]
#[macro_export]
macro_rules! require_mount {
    ($name:expr) => {
        use nix::unistd::Uid;
        use sysctl::{CtlValue, Sysctl};

        let ctl = ::sysctl::Ctl::new("vfs.usermount").unwrap();
        if !Uid::current().is_root() && CtlValue::Int(0) == ctl.value().unwrap()
        {
            skip!(
                "{} requires the ability to mount file systems. Skipping test.",
                $name
            );
        }
    };
}

#[cfg(linux_android)]
#[macro_export]
macro_rules! skip_if_cirrus {
    ($reason:expr) => {
        if std::env::var_os("CIRRUS_CI").is_some() {
            skip!("{}", $reason);
        }
    };
}

#[cfg(target_os = "freebsd")]
#[macro_export]
macro_rules! skip_if_jailed {
    ($name:expr) => {
        use sysctl::{CtlValue, Sysctl};

        let ctl = ::sysctl::Ctl::new("security.jail.jailed").unwrap();
        if let CtlValue::Int(1) = ctl.value().unwrap() {
            skip!("{} cannot run in a jail. Skipping test.", $name);
        }
    };
}

#[cfg(not(any(target_os = "redox", target_os = "fuchsia")))]
#[macro_export]
macro_rules! skip_if_not_root {
    ($name:expr) => {
        use nix::unistd::Uid;

        if !Uid::current().is_root() {
            skip!("{} requires root privileges. Skipping test.", $name);
        }
    };
}

cfg_if! {
    if #[cfg(linux_android)] {
        #[macro_export] macro_rules! skip_if_seccomp {
            ($name:expr) => {
                if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
                    for l in s.lines() {
                        let mut fields = l.split_whitespace();
                        if fields.next() == Some("Seccomp:") &&
                            fields.next() != Some("0")
                        {
                            skip!("{} cannot be run in Seccomp mode.  Skipping test.",
                                stringify!($name));
                        }
                    }
                }
            }
        }
    } else if #[cfg(not(target_os = "redox"))] {
        #[macro_export] macro_rules! skip_if_seccomp {
            ($name:expr) => {}
        }
    }
}

cfg_if! {
    if #[cfg(target_os = "linux")] {
        #[macro_export] macro_rules! require_kernel_version {
            ($name:expr, $version_requirement:expr) => {
                use semver::{Version, VersionReq};

                let version_requirement = VersionReq::parse($version_requirement)
                        .expect("Bad match_version provided");

                let uname = nix::sys::utsname::uname().unwrap();
                println!("{}", uname.sysname().to_str().unwrap());
                println!("{}", uname.nodename().to_str().unwrap());
                println!("{}", uname.release().to_str().unwrap());
                println!("{}", uname.version().to_str().unwrap());
                println!("{}", uname.machine().to_str().unwrap());

                // Fix stuff that the semver parser can't handle
                let fixed_release = &uname.release().to_str().unwrap().to_string()
                    // Fedora 33 reports version as 4.18.el8_2.x86_64 or
                    // 5.18.200-fc33.x86_64.  Remove the underscore.
                    .replace("_", "-")
                    // Cirrus-CI reports version as 4.19.112+ .  Remove the +
                    .replace("+", "");
                let mut version = Version::parse(fixed_release).unwrap();

                //Keep only numeric parts
                version.pre = semver::Prerelease::EMPTY;
                version.build = semver::BuildMetadata::EMPTY;

                if !version_requirement.matches(&version) {
                    skip!("Skip {} because kernel version `{}` doesn't match the requirement `{}`",
                        stringify!($name), version, version_requirement);
                }
            }
        }
    }
}
