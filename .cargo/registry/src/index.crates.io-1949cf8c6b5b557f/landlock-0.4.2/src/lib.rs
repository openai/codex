// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Landlock is a security feature available since Linux 5.13.
//! The goal is to enable to restrict ambient rights
//! (e.g., global filesystem access)
//! for a set of processes by creating safe security sandboxes as new security layers
//! in addition to the existing system-wide access-controls.
//! This kind of sandbox is expected to help mitigate the security impact of bugs,
//! unexpected or malicious behaviors in applications.
//! Landlock empowers any process, including unprivileged ones, to securely restrict themselves.
//! More information about Landlock can be found in the [official website](https://landlock.io).
//!
//! This crate provides a safe abstraction for the Landlock system calls, along with some helpers.
//!
//! Minimum Supported Rust Version (MSRV): 1.63
//!
//! # Use cases
//!
//! This crate is especially useful to protect users' data by sandboxing:
//! * trusted applications dealing with potentially malicious data
//!   (e.g., complex file format, network request) that could exploit security vulnerabilities;
//! * sandbox managers, container runtimes or shells launching untrusted applications.
//!
//! # Examples
//!
//! A simple example can be found with the [`path_beneath_rules()`] helper.
//! More complex examples can be found with the [`Ruleset` documentation](Ruleset)
//! and the [sandboxer example](https://github.com/landlock-lsm/rust-landlock/blob/master/examples/sandboxer.rs).
//!
//! # Current limitations
//!
//! This crate exposes the Landlock features available as of Linux 5.19
//! and then inherits some [kernel limitations](https://www.kernel.org/doc/html/latest/userspace-api/landlock.html#current-limitations)
//! that will be addressed with future kernel releases
//! (e.g., arbitrary mounts are always denied).
//!
//! # Compatibility
//!
//! Types defined in this crate are designed to enable the strictest Landlock configuration
//! for the given kernel on which the program runs.
//! In the default [best-effort](CompatLevel::BestEffort) mode,
//! [`Ruleset`] will determine compatibility
//! with the intersection of the currently running kernel's features
//! and those required by the caller.
//! This way, callers can distinguish between
//! Landlock compatibility issues inherent to the current system
//! (e.g., file names that don't exist)
//! and misconfiguration that should be fixed in the program
//! (e.g., empty or inconsistent access rights).
//! [`RulesetError`] identifies such kind of errors.
//!
//! With [`set_compatibility(CompatLevel::BestEffort)`](Compatible::set_compatibility),
//! users of the crate may mark Landlock features that are deemed required
//! and other features that may be downgraded to use lower security on systems
//! where they can't be enforced.
//! It is discouraged to compare the system's provided [Landlock ABI](ABI) version directly,
//! as it is difficult to track detailed ABI differences
//! which are handled thanks to the [`Compatible`] trait.
//!
//! To make it easier to migrate to a new version of this library,
//! we use the builder pattern
//! and designed objects to require the minimal set of method arguments.
//! Most `enum` are marked as `non_exhaustive` to enable backward-compatible evolutions.
//!
//! ## Test strategy
//!
//! Developers should test their sandboxed applications
//! with a kernel that supports all requested Landlock features
//! and check that [`RulesetCreated::restrict_self()`] returns a status matching
//! [`Ok(RestrictionStatus { ruleset: RulesetStatus::FullyEnforced, no_new_privs: true, })`](RestrictionStatus)
//! to make sure everything works as expected in an enforced sandbox.
//! Alternatively, using [`set_compatibility(CompatLevel::HardRequirement)`](Compatible::set_compatibility)
//! will immediately inform about unsupported Landlock features.
//! These configurations should only depend on the test environment
//! (e.g. [by checking an environment variable](https://github.com/landlock-lsm/rust-landlock/search?q=LANDLOCK_CRATE_TEST_ABI)).
//! However, applications should only check that no error is returned (i.e. `Ok(_)`)
//! and optionally log and inform users that the application is not fully sandboxed
//! because of missing features from the running kernel.

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

pub use access::{Access, HandledAccess};
pub use compat::{CompatLevel, Compatible, ABI};
pub use enumflags2::{make_bitflags, BitFlags};
pub use errors::{
    AccessError, AddRuleError, AddRulesError, CompatError, CreateRulesetError, Errno,
    HandleAccessError, HandleAccessesError, PathBeneathError, PathFdError, RestrictSelfError,
    RulesetError, ScopeError,
};
pub use fs::{path_beneath_rules, AccessFs, PathBeneath, PathFd};
pub use net::{AccessNet, NetPort};
pub use ruleset::{
    RestrictionStatus, Rule, Ruleset, RulesetAttr, RulesetCreated, RulesetCreatedAttr,
    RulesetStatus,
};
pub use scope::Scope;

use access::PrivateHandledAccess;
use compat::{CompatResult, CompatState, Compatibility, TailoredCompatLevel, TryCompat};
use ruleset::PrivateRule;

#[cfg(test)]
use compat::{can_emulate, get_errno_from_landlock_status};
#[cfg(test)]
use errors::TestRulesetError;
#[cfg(test)]
use strum::IntoEnumIterator;

mod access;
mod compat;
mod errors;
mod fs;
mod net;
mod ruleset;
mod scope;
mod uapi;

// Makes sure private traits cannot be implemented outside of this crate.
mod private {
    pub trait Sealed {}

    impl Sealed for crate::AccessFs {}
    impl Sealed for crate::AccessNet {}
    impl Sealed for crate::Scope {}
}

#[cfg(test)]
mod tests {
    use crate::*;

    // Emulate old kernel supports.
    fn check_ruleset_support<F>(
        partial: ABI,
        full: Option<ABI>,
        check: F,
        error_if_abi_lt_partial: bool,
    ) where
        F: Fn(Ruleset) -> Result<RestrictionStatus, TestRulesetError> + Send + Copy + 'static,
    {
        // If there is no partial support, it means that `full == partial`.
        assert!(partial <= full.unwrap_or(partial));
        for abi in ABI::iter() {
            // Ensures restrict_self() is called on a dedicated thread to avoid inconsistent tests.
            let ret = std::thread::spawn(move || check(Ruleset::from(abi)))
                .join()
                .unwrap();

            // Useful for failed tests and with cargo test -- --show-output
            println!("Checking ABI {abi:?}: received {ret:#?}");
            if can_emulate(abi, partial, full) {
                if abi < partial && error_if_abi_lt_partial {
                    // TODO: Check exact error type; this may require better error types.
                    assert!(matches!(ret, Err(TestRulesetError::Ruleset(_))));
                } else {
                    let full_support = if let Some(full_inner) = full {
                        abi >= full_inner
                    } else {
                        false
                    };
                    let ruleset_status = if full_support {
                        RulesetStatus::FullyEnforced
                    } else if abi >= partial {
                        RulesetStatus::PartiallyEnforced
                    } else {
                        RulesetStatus::NotEnforced
                    };
                    println!("Expecting ruleset status {ruleset_status:?}");
                    assert!(matches!(
                        ret,
                        Ok(RestrictionStatus {
                            ruleset,
                            no_new_privs: true,
                        }) if ruleset == ruleset_status
                    ))
                }
            } else {
                // The errno value should be ENOSYS, EOPNOTSUPP, EINVAL (e.g. when an unknown
                // access right is provided), or E2BIG (e.g. when there is an unknown field in a
                // Landlock syscall attribute).
                let errno = get_errno_from_landlock_status();
                println!("Expecting error {errno:?}");
                match ret {
                    Err(
                        ref error @ TestRulesetError::Ruleset(RulesetError::CreateRuleset(
                            CreateRulesetError::CreateRulesetCall { ref source },
                        )),
                    ) => {
                        assert_eq!(source.raw_os_error(), Some(*Errno::from(error)));
                        match (source.raw_os_error(), errno) {
                            (Some(e1), Some(e2)) => assert_eq!(e1, e2),
                            (Some(e1), None) => assert!(matches!(e1, libc::EINVAL | libc::E2BIG)),
                            _ => unreachable!(),
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    #[test]
    fn allow_root_compat() {
        let abi = ABI::V1;

        check_ruleset_support(
            abi,
            Some(abi),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::from_all(abi))?
                    .create()?
                    .add_rule(PathBeneath::new(PathFd::new("/")?, AccessFs::from_all(abi)))?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn too_much_access_rights_for_a_file() {
        let abi = ABI::V1;

        check_ruleset_support(
            abi,
            Some(abi),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::from_all(abi))?
                    .create()?
                    // Same code as allow_root_compat() but with /etc/passwd instead of /
                    .add_rule(PathBeneath::new(
                        PathFd::new("/etc/passwd")?,
                        // Only allow legitimate access rights on a file.
                        AccessFs::from_file(abi),
                    ))?
                    .restrict_self()?)
            },
            false,
        );

        check_ruleset_support(
            abi,
            None,
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::from_all(abi))?
                    .create()?
                    // Same code as allow_root_compat() but with /etc/passwd instead of /
                    .add_rule(PathBeneath::new(
                        PathFd::new("/etc/passwd")?,
                        // Tries to allow all access rights on a file.
                        AccessFs::from_all(abi),
                    ))?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn path_beneath_rules_with_too_much_access_rights_for_a_file() {
        let abi = ABI::V1;

        check_ruleset_support(
            abi,
            Some(abi),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::from_all(ABI::V1))?
                    .create()?
                    // Same code as too_much_access_rights_for_a_file() but using path_beneath_rules()
                    .add_rules(path_beneath_rules(["/etc/passwd"], AccessFs::from_all(abi)))?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn allow_root_fragile() {
        let abi = ABI::V1;

        check_ruleset_support(
            abi,
            Some(abi),
            move |ruleset: Ruleset| -> _ {
                // Sets default support requirement: abort the whole sandboxing for any Landlock error.
                Ok(ruleset
                    // Must have at least the execute check…
                    .set_compatibility(CompatLevel::HardRequirement)
                    .handle_access(AccessFs::Execute)?
                    // …and possibly others.
                    .set_compatibility(CompatLevel::BestEffort)
                    .handle_access(AccessFs::from_all(abi))?
                    .create()?
                    .set_no_new_privs(true)
                    .add_rule(PathBeneath::new(PathFd::new("/")?, AccessFs::from_all(abi)))?
                    .restrict_self()?)
            },
            true,
        );
    }

    #[test]
    fn ruleset_enforced() {
        let abi = ABI::V1;

        check_ruleset_support(
            abi,
            Some(abi),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    // Restricting without rule exceptions is legitimate to forbid a set of actions.
                    .handle_access(AccessFs::Execute)?
                    .create()?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn abi_v2_exec_refer() {
        check_ruleset_support(
            ABI::V1,
            Some(ABI::V2),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::Execute)?
                    // AccessFs::Refer is not supported by ABI::V1 (best-effort).
                    .handle_access(AccessFs::Refer)?
                    .create()?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn abi_v2_refer_only() {
        // When no access is handled, do not try to create a ruleset without access.
        check_ruleset_support(
            ABI::V2,
            Some(ABI::V2),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::Refer)?
                    .create()?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn abi_v3_truncate() {
        check_ruleset_support(
            ABI::V2,
            Some(ABI::V3),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::Refer)?
                    .handle_access(AccessFs::Truncate)?
                    .create()?
                    .add_rule(PathBeneath::new(PathFd::new("/")?, AccessFs::Refer))?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn ruleset_created_try_clone() {
        check_ruleset_support(
            ABI::V1,
            Some(ABI::V1),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::Execute)?
                    .create()?
                    .add_rule(PathBeneath::new(PathFd::new("/")?, AccessFs::Execute))?
                    .try_clone()?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn abi_v4_tcp() {
        check_ruleset_support(
            ABI::V3,
            Some(ABI::V4),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::Truncate)?
                    .handle_access(AccessNet::BindTcp | AccessNet::ConnectTcp)?
                    .create()?
                    .add_rule(NetPort::new(1, AccessNet::ConnectTcp))?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn abi_v5_ioctl_dev() {
        check_ruleset_support(
            ABI::V4,
            Some(ABI::V5),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessNet::BindTcp)?
                    .handle_access(AccessFs::IoctlDev)?
                    .create()?
                    .add_rule(PathBeneath::new(PathFd::new("/")?, AccessFs::IoctlDev))?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn abi_v6_scope_mix() {
        check_ruleset_support(
            ABI::V5,
            Some(ABI::V6),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .handle_access(AccessFs::IoctlDev)?
                    .scope(Scope::AbstractUnixSocket | Scope::Signal)?
                    .create()?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn abi_v6_scope_only() {
        check_ruleset_support(
            ABI::V6,
            Some(ABI::V6),
            move |ruleset: Ruleset| -> _ {
                Ok(ruleset
                    .scope(Scope::AbstractUnixSocket | Scope::Signal)?
                    .create()?
                    .restrict_self()?)
            },
            false,
        );
    }

    #[test]
    fn ruleset_created_try_clone_ownedfd() {
        use std::os::unix::io::{AsRawFd, OwnedFd};

        let abi = ABI::V1;
        check_ruleset_support(
            abi,
            Some(abi),
            move |ruleset: Ruleset| -> _ {
                let ruleset1 = ruleset.handle_access(AccessFs::from_all(abi))?.create()?;
                let ruleset2 = ruleset1.try_clone().unwrap();
                let ruleset3 = ruleset2.try_clone().unwrap();

                let some1: Option<OwnedFd> = ruleset1.into();
                if let Some(fd1) = some1 {
                    assert!(fd1.as_raw_fd() >= 0);

                    let some2: Option<OwnedFd> = ruleset2.into();
                    let fd2 = some2.unwrap();
                    assert!(fd2.as_raw_fd() >= 0);

                    assert_ne!(fd1.as_raw_fd(), fd2.as_raw_fd());
                }
                Ok(ruleset3.restrict_self()?)
            },
            false,
        );
    }
}
