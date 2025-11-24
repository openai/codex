// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Access, AccessFs, AccessNet, BitFlags, HandledAccess, PrivateHandledAccess, Scope};
use libc::c_int;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// Maps to all errors that can be returned by a ruleset action.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RulesetError {
    #[error(transparent)]
    HandleAccesses(#[from] HandleAccessesError),
    #[error(transparent)]
    CreateRuleset(#[from] CreateRulesetError),
    #[error(transparent)]
    AddRules(#[from] AddRulesError),
    #[error(transparent)]
    RestrictSelf(#[from] RestrictSelfError),
    #[error(transparent)]
    Scope(#[from] ScopeError),
}

#[test]
fn ruleset_error_breaking_change() {
    use crate::*;

    // Generics are part of the API and modifying them can lead to a breaking change.
    let _: RulesetError = RulesetError::HandleAccesses(HandleAccessesError::Fs(
        HandleAccessError::Compat(CompatError::Access(AccessError::Empty)),
    ));
}

/// Identifies errors when updating the ruleset's handled access-rights.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HandleAccessError<T>
where
    T: HandledAccess,
{
    #[error(transparent)]
    Compat(#[from] CompatError<T>),
}

/// Identifies errors when updating the ruleset's scopes.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ScopeError {
    #[error(transparent)]
    Compat(#[from] CompatError<Scope>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HandleAccessesError {
    #[error(transparent)]
    Fs(HandleAccessError<AccessFs>),
    #[error(transparent)]
    Net(HandleAccessError<AccessNet>),
}

// Generically implement for all the handled access implementations rather than for the cases
// listed in HandleAccessesError (with #[from]).
impl<A> From<HandleAccessError<A>> for HandleAccessesError
where
    A: PrivateHandledAccess,
{
    fn from(error: HandleAccessError<A>) -> Self {
        A::into_handle_accesses_error(error)
    }
}

/// Identifies errors when creating a ruleset.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CreateRulesetError {
    /// The `landlock_create_ruleset()` system call failed.
    #[error("failed to create a ruleset: {source}")]
    #[non_exhaustive]
    CreateRulesetCall { source: io::Error },
    /// Missing call to [`RulesetAttr::handle_access()`](crate::RulesetAttr::handle_access)
    /// or [`RulesetAttr::scope()`](crate::RulesetAttr::scope).
    #[error("missing access")]
    MissingHandledAccess,
}

/// Identifies errors when adding a rule to a ruleset.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AddRuleError<T>
where
    T: HandledAccess,
{
    /// The `landlock_add_rule()` system call failed.
    #[error("failed to add a rule: {source}")]
    #[non_exhaustive]
    AddRuleCall { source: io::Error },
    /// The rule's access-rights are not all handled by the (requested) ruleset access-rights.
    #[error("access-rights not handled by the ruleset: {incompatible:?}")]
    UnhandledAccess {
        access: BitFlags<T>,
        incompatible: BitFlags<T>,
    },
    #[error(transparent)]
    Compat(#[from] CompatError<T>),
}

// Generically implement for all the handled access implementations rather than for the cases listed
// in AddRulesError (with #[from]).
impl<A> From<AddRuleError<A>> for AddRulesError
where
    A: PrivateHandledAccess,
{
    fn from(error: AddRuleError<A>) -> Self {
        A::into_add_rules_error(error)
    }
}

/// Identifies errors when adding rules to a ruleset thanks to an iterator returning
/// Result<Rule, E> items.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AddRulesError {
    #[error(transparent)]
    Fs(AddRuleError<AccessFs>),
    #[error(transparent)]
    Net(AddRuleError<AccessNet>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CompatError<T>
where
    T: Access,
{
    #[error(transparent)]
    PathBeneath(#[from] PathBeneathError),
    #[error(transparent)]
    Access(#[from] AccessError<T>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PathBeneathError {
    /// To check that access-rights are consistent with a file descriptor, a call to
    /// [`RulesetCreatedAttr::add_rule()`](crate::RulesetCreatedAttr::add_rule)
    /// looks at the file type with an `fstat()` system call.
    #[error("failed to check file descriptor type: {source}")]
    #[non_exhaustive]
    StatCall { source: io::Error },
    /// This error is returned by
    /// [`RulesetCreatedAttr::add_rule()`](crate::RulesetCreatedAttr::add_rule)
    /// if the related PathBeneath object is not set to best-effort,
    /// and if its allowed access-rights contain directory-only ones
    /// whereas the file descriptor doesn't point to a directory.
    #[error("incompatible directory-only access-rights: {incompatible:?}")]
    DirectoryAccess {
        access: BitFlags<AccessFs>,
        incompatible: BitFlags<AccessFs>,
    },
}

#[derive(Debug, Error)]
// Exhaustive enum
pub enum AccessError<T>
where
    T: Access,
{
    /// The access-rights set is empty, which doesn't make sense and would be rejected by the
    /// kernel.
    #[error("empty access-right")]
    Empty,
    /// The access-rights set was forged with the unsafe `BitFlags::from_bits_unchecked()` and it
    /// contains unknown bits.
    #[error("unknown access-rights (at build time): {unknown:?}")]
    Unknown {
        access: BitFlags<T>,
        unknown: BitFlags<T>,
    },
    /// The best-effort approach was (deliberately) disabled and the requested access-rights are
    /// fully incompatible with the running kernel.
    #[error("fully incompatible access-rights: {access:?}")]
    Incompatible { access: BitFlags<T> },
    /// The best-effort approach was (deliberately) disabled and the requested access-rights are
    /// partially incompatible with the running kernel.
    #[error("partially incompatible access-rights: {incompatible:?}")]
    PartiallyCompatible {
        access: BitFlags<T>,
        incompatible: BitFlags<T>,
    },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RestrictSelfError {
    /// The `prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)` system call failed.
    #[error("failed to set no_new_privs: {source}")]
    #[non_exhaustive]
    SetNoNewPrivsCall { source: io::Error },
    /// The `landlock_restrict_self() `system call failed.
    #[error("failed to restrict the calling thread: {source}")]
    #[non_exhaustive]
    RestrictSelfCall { source: io::Error },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PathFdError {
    /// The `open()` system call failed.
    #[error("failed to open \"{path}\": {source}")]
    #[non_exhaustive]
    OpenCall { source: io::Error, path: PathBuf },
}

#[cfg(test)]
#[derive(Debug, Error)]
pub(crate) enum TestRulesetError {
    #[error(transparent)]
    Ruleset(#[from] RulesetError),
    #[error(transparent)]
    PathFd(#[from] PathFdError),
    #[error(transparent)]
    File(#[from] std::io::Error),
}

/// Get the underlying errno value.
///
/// This helper is useful for FFI to easily translate a Landlock error into an
/// errno value.
#[derive(Debug, PartialEq, Eq)]
pub struct Errno(c_int);

impl Errno {
    pub fn new(value: c_int) -> Self {
        Self(value)
    }
}

impl<T> From<T> for Errno
where
    T: std::error::Error,
{
    fn from(error: T) -> Self {
        let default = libc::EINVAL;
        if let Some(e) = error.source() {
            if let Some(e) = e.downcast_ref::<std::io::Error>() {
                return Errno(e.raw_os_error().unwrap_or(default));
            }
        }
        Errno(default)
    }
}

impl std::ops::Deref for Errno {
    type Target = c_int;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
fn _test_ruleset_errno(expected_errno: c_int) {
    use std::io::Error;

    let handle_access_err = RulesetError::HandleAccesses(HandleAccessesError::Fs(
        HandleAccessError::Compat(CompatError::Access(AccessError::Empty)),
    ));
    assert_eq!(*Errno::from(handle_access_err), libc::EINVAL);

    let create_ruleset_err = RulesetError::CreateRuleset(CreateRulesetError::CreateRulesetCall {
        source: Error::from_raw_os_error(expected_errno),
    });
    assert_eq!(*Errno::from(create_ruleset_err), expected_errno);

    let add_rules_fs_err = RulesetError::AddRules(AddRulesError::Fs(AddRuleError::AddRuleCall {
        source: Error::from_raw_os_error(expected_errno),
    }));
    assert_eq!(*Errno::from(add_rules_fs_err), expected_errno);

    let add_rules_net_err = RulesetError::AddRules(AddRulesError::Net(AddRuleError::AddRuleCall {
        source: Error::from_raw_os_error(expected_errno),
    }));
    assert_eq!(*Errno::from(add_rules_net_err), expected_errno);

    let add_rules_other_err =
        RulesetError::AddRules(AddRulesError::Fs(AddRuleError::UnhandledAccess {
            access: AccessFs::Execute.into(),
            incompatible: BitFlags::<AccessFs>::EMPTY,
        }));
    assert_eq!(*Errno::from(add_rules_other_err), libc::EINVAL);

    let restrict_self_err = RulesetError::RestrictSelf(RestrictSelfError::RestrictSelfCall {
        source: Error::from_raw_os_error(expected_errno),
    });
    assert_eq!(*Errno::from(restrict_self_err), expected_errno);

    let set_no_new_privs_err = RulesetError::RestrictSelf(RestrictSelfError::SetNoNewPrivsCall {
        source: Error::from_raw_os_error(expected_errno),
    });
    assert_eq!(*Errno::from(set_no_new_privs_err), expected_errno);

    let create_ruleset_missing_err =
        RulesetError::CreateRuleset(CreateRulesetError::MissingHandledAccess);
    assert_eq!(*Errno::from(create_ruleset_missing_err), libc::EINVAL);
}

#[test]
fn test_ruleset_errno() {
    _test_ruleset_errno(libc::EACCES);
    _test_ruleset_errno(libc::EIO);
}
