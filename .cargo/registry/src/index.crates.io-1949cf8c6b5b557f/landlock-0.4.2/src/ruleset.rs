// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::compat::private::OptionCompatLevelMut;
use crate::{
    uapi, AccessFs, AccessNet, AddRuleError, AddRulesError, BitFlags, CompatLevel, CompatState,
    Compatibility, Compatible, CreateRulesetError, HandledAccess, PrivateHandledAccess,
    RestrictSelfError, RulesetError, Scope, ScopeError, TryCompat,
};
use std::io::Error;
use std::mem::size_of_val;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};

#[cfg(test)]
use crate::*;

// Public interface without methods and which is impossible to implement outside this crate.
pub trait Rule<T>: PrivateRule<T>
where
    T: HandledAccess,
{
}

// PrivateRule is not public outside this crate.
pub trait PrivateRule<T>
where
    Self: TryCompat<T> + Compatible,
    T: HandledAccess,
{
    const TYPE_ID: uapi::landlock_rule_type;

    /// Returns a raw pointer to the rule's inner attribute.
    ///
    /// The caller must ensure that the rule outlives the pointer this function returns, or else it
    /// will end up pointing to garbage.
    fn as_ptr(&mut self) -> *const libc::c_void;

    fn check_consistency(&self, ruleset: &RulesetCreated) -> Result<(), AddRulesError>;
}

/// Enforcement status of a ruleset.
#[derive(Debug, PartialEq, Eq)]
pub enum RulesetStatus {
    /// All requested restrictions are enforced.
    FullyEnforced,
    /// Some requested restrictions are enforced,
    /// following a best-effort approach.
    PartiallyEnforced,
    /// The running system doesn't support Landlock
    /// or a subset of the requested Landlock features.
    NotEnforced,
}

impl From<CompatState> for RulesetStatus {
    fn from(state: CompatState) -> Self {
        match state {
            CompatState::Init | CompatState::No | CompatState::Dummy => RulesetStatus::NotEnforced,
            CompatState::Full => RulesetStatus::FullyEnforced,
            CompatState::Partial => RulesetStatus::PartiallyEnforced,
        }
    }
}

// The Debug, PartialEq and Eq implementations are useful for crate users to debug and check the
// result of a Landlock ruleset enforcement.
/// Status of a [`RulesetCreated`]
/// after calling [`restrict_self()`](RulesetCreated::restrict_self).
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RestrictionStatus {
    /// Status of the Landlock ruleset enforcement.
    pub ruleset: RulesetStatus,
    /// Status of `prctl(2)`'s `PR_SET_NO_NEW_PRIVS` enforcement.
    pub no_new_privs: bool,
}

fn prctl_set_no_new_privs() -> Result<(), Error> {
    match unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } {
        0 => Ok(()),
        _ => Err(Error::last_os_error()),
    }
}

fn support_no_new_privs() -> bool {
    // Only Linux < 3.5 or kernel with seccomp filters should return an error.
    matches!(
        unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) },
        0 | 1
    )
}

/// Landlock ruleset builder.
///
/// `Ruleset` enables to create a Landlock ruleset in a flexible way
/// following the builder pattern.
/// Most build steps return a [`Result`] with [`RulesetError`].
///
/// You should probably not create more than one ruleset per application.
/// Creating multiple rulesets is only useful when gradually restricting an application
/// (e.g., a first set of generic restrictions before reading any file,
/// then a second set of tailored restrictions after reading the configuration).
///
/// # Simple example
///
/// Simple helper handling only Landlock-related errors.
///
/// ```
/// use landlock::{
///     Access, AccessFs, PathBeneath, PathFd, RestrictionStatus, Ruleset, RulesetAttr,
///     RulesetCreatedAttr, RulesetError, ABI,
/// };
/// use std::os::unix::io::AsFd;
///
/// fn restrict_fd<T>(hierarchy: T) -> Result<RestrictionStatus, RulesetError>
/// where
///     T: AsFd,
/// {
///     // The Landlock ABI should be incremented (and tested) regularly.
///     let abi = ABI::V1;
///     let access_all = AccessFs::from_all(abi);
///     let access_read = AccessFs::from_read(abi);
///     Ok(Ruleset::default()
///         .handle_access(access_all)?
///         .create()?
///         .add_rule(PathBeneath::new(hierarchy, access_read))?
///         .restrict_self()?)
/// }
///
/// let fd = PathFd::new("/home").expect("failed to open /home");
/// let status = restrict_fd(fd).expect("failed to build the ruleset");
/// ```
///
/// # Generic example
///
/// More generic helper handling a set of file hierarchies
/// and multiple types of error (i.e. [`RulesetError`](crate::RulesetError)
/// and [`PathFdError`](crate::PathFdError).
///
/// ```
/// use landlock::{
///     Access, AccessFs, PathBeneath, PathFd, PathFdError, RestrictionStatus, Ruleset,
///     RulesetAttr, RulesetCreatedAttr, RulesetError, ABI,
/// };
/// use thiserror::Error;
///
/// #[derive(Debug, Error)]
/// enum MyRestrictError {
///     #[error(transparent)]
///     Ruleset(#[from] RulesetError),
///     #[error(transparent)]
///     AddRule(#[from] PathFdError),
/// }
///
/// fn restrict_paths(hierarchies: &[&str]) -> Result<RestrictionStatus, MyRestrictError> {
///     // The Landlock ABI should be incremented (and tested) regularly.
///     let abi = ABI::V1;
///     let access_all = AccessFs::from_all(abi);
///     let access_read = AccessFs::from_read(abi);
///     Ok(Ruleset::default()
///         .handle_access(access_all)?
///         .create()?
///         .add_rules(
///             hierarchies
///                 .iter()
///                 .map::<Result<_, MyRestrictError>, _>(|p| {
///                     Ok(PathBeneath::new(PathFd::new(p)?, access_read))
///                 }),
///         )?
///         .restrict_self()?)
/// }
///
/// let status = restrict_paths(&["/usr", "/home"]).expect("failed to build the ruleset");
/// ```
#[cfg_attr(test, derive(Debug))]
pub struct Ruleset {
    pub(crate) requested_handled_fs: BitFlags<AccessFs>,
    pub(crate) requested_handled_net: BitFlags<AccessNet>,
    pub(crate) requested_scoped: BitFlags<Scope>,
    pub(crate) actual_handled_fs: BitFlags<AccessFs>,
    pub(crate) actual_handled_net: BitFlags<AccessNet>,
    pub(crate) actual_scoped: BitFlags<Scope>,
    pub(crate) compat: Compatibility,
}

impl From<Compatibility> for Ruleset {
    fn from(compat: Compatibility) -> Self {
        Ruleset {
            // Non-working default handled FS accesses to force users to set them explicitely.
            requested_handled_fs: Default::default(),
            requested_handled_net: Default::default(),
            requested_scoped: Default::default(),
            actual_handled_fs: Default::default(),
            actual_handled_net: Default::default(),
            actual_scoped: Default::default(),
            compat,
        }
    }
}

#[cfg(test)]
impl From<ABI> for Ruleset {
    fn from(abi: ABI) -> Self {
        Ruleset::from(Compatibility::from(abi))
    }
}

#[test]
fn ruleset_add_rule_iter() {
    assert!(matches!(
        Ruleset::from(ABI::Unsupported)
            .handle_access(AccessFs::Execute)
            .unwrap()
            .create()
            .unwrap()
            .add_rule(PathBeneath::new(
                PathFd::new("/").unwrap(),
                AccessFs::ReadFile
            ))
            .unwrap_err(),
        RulesetError::AddRules(AddRulesError::Fs(AddRuleError::UnhandledAccess { .. }))
    ));
}

impl Default for Ruleset {
    /// Returns a new `Ruleset`.
    /// This call automatically probes the running kernel to know if it supports Landlock.
    ///
    /// To be able to successfully call [`create()`](Ruleset::create),
    /// it is required to set the handled accesses with
    /// [`handle_access()`](Ruleset::handle_access).
    fn default() -> Self {
        // The API should be future-proof: one Rust program or library should have the same
        // behavior if built with an old or a newer crate (e.g. with an extended ruleset_attr
        // enum).  It should then not be possible to give an "all-possible-handled-accesses" to the
        // Ruleset builder because this value would be relative to the running kernel.
        Compatibility::new().into()
    }
}

impl Ruleset {
    #[allow(clippy::new_without_default)]
    #[deprecated(note = "Use Ruleset::default() instead")]
    pub fn new() -> Self {
        Ruleset::default()
    }

    /// Attempts to create a real Landlock ruleset (if supported by the running kernel).
    /// The returned [`RulesetCreated`] is also a builder.
    ///
    /// On error, returns a wrapped [`CreateRulesetError`].
    pub fn create(mut self) -> Result<RulesetCreated, RulesetError> {
        let body = || -> Result<RulesetCreated, CreateRulesetError> {
            match self.compat.state {
                CompatState::Init => {
                    // Checks that there is at least one requested access (e.g.
                    // requested_handled_fs): one call to handle_access().
                    Err(CreateRulesetError::MissingHandledAccess)
                }
                CompatState::No | CompatState::Dummy => {
                    // There is at least one requested access.
                    #[cfg(test)]
                    assert!(
                        !self.requested_handled_fs.is_empty()
                            || !self.requested_handled_net.is_empty()
                            || !self.requested_scoped.is_empty()
                    );

                    // CompatState::No should be handled as CompatState::Dummy because it is not
                    // possible to create an actual ruleset.
                    self.compat.update(CompatState::Dummy);
                    match self.compat.level.into() {
                        CompatLevel::HardRequirement => {
                            Err(CreateRulesetError::MissingHandledAccess)
                        }
                        _ => Ok(RulesetCreated::new(self, None)),
                    }
                }
                CompatState::Full | CompatState::Partial => {
                    // There is at least one actual handled access.
                    #[cfg(test)]
                    assert!(
                        !self.actual_handled_fs.is_empty()
                            || !self.actual_handled_net.is_empty()
                            || !self.actual_scoped.is_empty()
                    );

                    let attr = uapi::landlock_ruleset_attr {
                        handled_access_fs: self.actual_handled_fs.bits(),
                        handled_access_net: self.actual_handled_net.bits(),
                        scoped: self.actual_scoped.bits(),
                    };
                    match unsafe { uapi::landlock_create_ruleset(&attr, size_of_val(&attr), 0) } {
                        fd if fd >= 0 => Ok(RulesetCreated::new(
                            self,
                            Some(unsafe { OwnedFd::from_raw_fd(fd) }),
                        )),
                        _ => Err(CreateRulesetError::CreateRulesetCall {
                            source: Error::last_os_error(),
                        }),
                    }
                }
            }
        };
        Ok(body()?)
    }
}

impl OptionCompatLevelMut for Ruleset {
    fn as_option_compat_level_mut(&mut self) -> &mut Option<CompatLevel> {
        &mut self.compat.level
    }
}

impl OptionCompatLevelMut for &mut Ruleset {
    fn as_option_compat_level_mut(&mut self) -> &mut Option<CompatLevel> {
        &mut self.compat.level
    }
}

impl Compatible for Ruleset {}

impl Compatible for &mut Ruleset {}

impl AsMut<Ruleset> for Ruleset {
    fn as_mut(&mut self) -> &mut Ruleset {
        self
    }
}

// Tests unambiguous type.
#[test]
fn ruleset_as_mut() {
    let mut ruleset = Ruleset::from(ABI::Unsupported);
    let _ = ruleset.as_mut();

    let mut ruleset_created = Ruleset::from(ABI::Unsupported)
        .handle_access(AccessFs::Execute)
        .unwrap()
        .create()
        .unwrap();
    let _ = ruleset_created.as_mut();
}

pub trait RulesetAttr: Sized + AsMut<Ruleset> + Compatible {
    /// Attempts to add a set of access rights that will be supported by this ruleset.
    /// By default, all actions requiring these access rights will be denied.
    /// Consecutive calls to `handle_access()` will be interpreted as logical ORs
    /// with the previous handled accesses.
    ///
    /// On error, returns a wrapped [`HandleAccessesError`](crate::HandleAccessesError).
    /// E.g., `RulesetError::HandleAccesses(HandleAccessesError::Fs(HandleAccessError<AccessFs>))`
    fn handle_access<T, U>(mut self, access: T) -> Result<Self, RulesetError>
    where
        T: Into<BitFlags<U>>,
        U: HandledAccess + PrivateHandledAccess,
    {
        U::ruleset_handle_access(self.as_mut(), access.into())?;
        Ok(self)
    }

    /// Attempts to add a set of scopes that will be supported by this ruleset.
    /// Consecutive calls to `scope()` will be interpreted as logical ORs
    /// with the previous scopes.
    ///
    /// On error, returns a wrapped [`ScopeError`](crate::ScopeError).
    /// E.g., `RulesetError::Scope(ScopeError)`
    fn scope<T>(mut self, scope: T) -> Result<Self, RulesetError>
    where
        T: Into<BitFlags<Scope>>,
    {
        let scope = scope.into();
        let ruleset = self.as_mut();
        ruleset.requested_scoped |= scope;
        if let Some(a) = scope
            .try_compat(
                ruleset.compat.abi(),
                ruleset.compat.level,
                &mut ruleset.compat.state,
            )
            .map_err(ScopeError::Compat)?
        {
            ruleset.actual_scoped |= a;
        }
        Ok(self)
    }
}

impl RulesetAttr for Ruleset {}

impl RulesetAttr for &mut Ruleset {}

#[test]
fn ruleset_attr() {
    let mut ruleset = Ruleset::from(ABI::Unsupported);
    let ruleset_ref = &mut ruleset;

    // Can pass this reference to prepare the ruleset...
    ruleset_ref
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(AccessFs::Execute)
        .unwrap()
        .handle_access(AccessFs::ReadFile)
        .unwrap();

    // ...and finally create the ruleset (thanks to non-lexical lifetimes).
    ruleset
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(AccessFs::Execute)
        .unwrap()
        .handle_access(AccessFs::WriteFile)
        .unwrap()
        .create()
        .unwrap();
}

#[test]
fn ruleset_created_handle_access_fs() {
    let access = make_bitflags!(AccessFs::{Execute | ReadDir});

    // Tests AccessFs::ruleset_handle_access()
    let ruleset = Ruleset::from(ABI::V1).handle_access(access).unwrap();
    assert_eq!(ruleset.requested_handled_fs, access);
    assert_eq!(ruleset.actual_handled_fs, access);

    // Tests composition (binary OR) of handled accesses.
    let ruleset = Ruleset::from(ABI::V1)
        .handle_access(AccessFs::Execute)
        .unwrap()
        .handle_access(AccessFs::ReadDir)
        .unwrap()
        .handle_access(AccessFs::Execute)
        .unwrap();
    assert_eq!(ruleset.requested_handled_fs, access);
    assert_eq!(ruleset.actual_handled_fs, access);

    // Tests that only the required handled accesses are reported as incompatible:
    // access should not contains AccessFs::Execute.
    assert!(matches!(Ruleset::from(ABI::Unsupported)
        .handle_access(AccessFs::Execute)
        .unwrap()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::ReadDir)
        .unwrap_err(),
        RulesetError::HandleAccesses(HandleAccessesError::Fs(HandleAccessError::Compat(
            CompatError::Access(AccessError::Incompatible { access })
        ))) if access == AccessFs::ReadDir
    ));
}

#[test]
fn ruleset_created_handle_access_net_tcp() {
    let access = make_bitflags!(AccessNet::{BindTcp | ConnectTcp});

    // Tests AccessNet::ruleset_handle_access() with ABI that doesn't support TCP rights.
    let ruleset = Ruleset::from(ABI::V3).handle_access(access).unwrap();
    assert_eq!(ruleset.requested_handled_net, access);
    assert_eq!(ruleset.actual_handled_net, BitFlags::<AccessNet>::EMPTY);

    // Tests AccessNet::ruleset_handle_access() with ABI that supports TCP rights.
    let ruleset = Ruleset::from(ABI::V4).handle_access(access).unwrap();
    assert_eq!(ruleset.requested_handled_net, access);
    assert_eq!(ruleset.actual_handled_net, access);

    // Tests composition (binary OR) of handled accesses.
    let ruleset = Ruleset::from(ABI::V4)
        .handle_access(AccessNet::BindTcp)
        .unwrap()
        .handle_access(AccessNet::ConnectTcp)
        .unwrap()
        .handle_access(AccessNet::BindTcp)
        .unwrap();
    assert_eq!(ruleset.requested_handled_net, access);
    assert_eq!(ruleset.actual_handled_net, access);

    // Tests that only the required handled accesses are reported as incompatible:
    // access should not contains AccessNet::BindTcp.
    assert!(matches!(Ruleset::from(ABI::Unsupported)
        .handle_access(AccessNet::BindTcp)
        .unwrap()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessNet::ConnectTcp)
        .unwrap_err(),
        RulesetError::HandleAccesses(HandleAccessesError::Net(HandleAccessError::Compat(
            CompatError::Access(AccessError::Incompatible { access })
        ))) if access == AccessNet::ConnectTcp
    ));
}

#[test]
fn ruleset_created_scope() {
    let scopes = make_bitflags!(Scope::{AbstractUnixSocket | Signal});

    // Tests Ruleset::scope() with ABI that doesn't support scopes.
    let ruleset = Ruleset::from(ABI::V5).scope(scopes).unwrap();
    assert_eq!(ruleset.requested_scoped, scopes);
    assert_eq!(ruleset.actual_scoped, BitFlags::<Scope>::EMPTY);

    // Tests Ruleset::scope() with ABI that supports scopes.
    let ruleset = Ruleset::from(ABI::V6).scope(scopes).unwrap();
    assert_eq!(ruleset.requested_scoped, scopes);
    assert_eq!(ruleset.actual_scoped, scopes);

    // Tests composition (binary OR) of scopes.
    let ruleset = Ruleset::from(ABI::V6)
        .scope(Scope::AbstractUnixSocket)
        .unwrap()
        .scope(Scope::Signal)
        .unwrap()
        .scope(Scope::AbstractUnixSocket)
        .unwrap();
    assert_eq!(ruleset.requested_scoped, scopes);
    assert_eq!(ruleset.actual_scoped, scopes);

    // Tests that only the required scopes are reported as incompatible:
    // scope should not contain Scope::AbstractUnixSocket.
    assert!(matches!(Ruleset::from(ABI::Unsupported)
        .scope(Scope::AbstractUnixSocket)
        .unwrap()
        .set_compatibility(CompatLevel::HardRequirement)
        .scope(Scope::Signal)
        .unwrap_err(),
        RulesetError::Scope(ScopeError::Compat(
            CompatError::Access(AccessError::Incompatible { access })
        )) if access == Scope::Signal
    ));
}

#[test]
fn ruleset_created_fs_net_scope() {
    let access_fs = make_bitflags!(AccessFs::{Execute | ReadDir});
    let access_net = make_bitflags!(AccessNet::{BindTcp | ConnectTcp});
    let scopes = make_bitflags!(Scope::{AbstractUnixSocket | Signal});

    // Tests composition (binary OR) of handled accesses.
    let ruleset = Ruleset::from(ABI::V5)
        .handle_access(access_fs)
        .unwrap()
        .scope(scopes)
        .unwrap()
        .handle_access(access_net)
        .unwrap();
    assert_eq!(ruleset.requested_handled_fs, access_fs);
    assert_eq!(ruleset.actual_handled_fs, access_fs);
    assert_eq!(ruleset.requested_handled_net, access_net);
    assert_eq!(ruleset.actual_handled_net, access_net);
    assert_eq!(ruleset.requested_scoped, scopes);
    assert_eq!(ruleset.actual_scoped, BitFlags::<Scope>::EMPTY);

    // Tests composition (binary OR) of handled accesses and scopes.
    let ruleset = Ruleset::from(ABI::V6)
        .handle_access(access_fs)
        .unwrap()
        .scope(scopes)
        .unwrap()
        .handle_access(access_net)
        .unwrap();
    assert_eq!(ruleset.requested_handled_fs, access_fs);
    assert_eq!(ruleset.actual_handled_fs, access_fs);
    assert_eq!(ruleset.requested_handled_net, access_net);
    assert_eq!(ruleset.actual_handled_net, access_net);
    assert_eq!(ruleset.requested_scoped, scopes);
    assert_eq!(ruleset.actual_scoped, scopes);
}

impl OptionCompatLevelMut for RulesetCreated {
    fn as_option_compat_level_mut(&mut self) -> &mut Option<CompatLevel> {
        &mut self.compat.level
    }
}

impl OptionCompatLevelMut for &mut RulesetCreated {
    fn as_option_compat_level_mut(&mut self) -> &mut Option<CompatLevel> {
        &mut self.compat.level
    }
}

impl Compatible for RulesetCreated {}

impl Compatible for &mut RulesetCreated {}

pub trait RulesetCreatedAttr: Sized + AsMut<RulesetCreated> + Compatible {
    /// Attempts to add a new rule to the ruleset.
    ///
    /// On error, returns a wrapped [`AddRulesError`].
    fn add_rule<T, U>(mut self, rule: T) -> Result<Self, RulesetError>
    where
        T: Rule<U>,
        U: HandledAccess + PrivateHandledAccess,
    {
        let body = || -> Result<Self, AddRulesError> {
            let self_ref = self.as_mut();
            rule.check_consistency(self_ref)?;
            let mut compat_rule = match rule
                .try_compat(
                    self_ref.compat.abi(),
                    self_ref.compat.level,
                    &mut self_ref.compat.state,
                )
                .map_err(AddRuleError::Compat)?
            {
                Some(r) => r,
                None => return Ok(self),
            };
            match self_ref.compat.state {
                CompatState::Init | CompatState::No | CompatState::Dummy => Ok(self),
                CompatState::Full | CompatState::Partial => {
                    #[cfg(test)]
                    assert!(self_ref.fd.is_some());
                    let fd = self_ref.fd.as_ref().map(|f| f.as_raw_fd()).unwrap_or(-1);
                    match unsafe {
                        uapi::landlock_add_rule(fd, T::TYPE_ID, compat_rule.as_ptr(), 0)
                    } {
                        0 => Ok(self),
                        _ => Err(AddRuleError::<U>::AddRuleCall {
                            source: Error::last_os_error(),
                        }
                        .into()),
                    }
                }
            }
        };
        Ok(body()?)
    }

    /// Attempts to add a set of new rules to the ruleset.
    ///
    /// On error, returns a (double) wrapped [`AddRulesError`].
    ///
    /// # Example
    ///
    /// Create a custom iterator to read paths from environment variable.
    ///
    /// ```
    /// use landlock::{
    ///     Access, AccessFs, BitFlags, PathBeneath, PathFd, PathFdError, RestrictionStatus, Ruleset,
    ///     RulesetAttr, RulesetCreatedAttr, RulesetError, ABI,
    /// };
    /// use std::env;
    /// use std::ffi::OsStr;
    /// use std::os::unix::ffi::{OsStrExt, OsStringExt};
    /// use thiserror::Error;
    ///
    /// #[derive(Debug, Error)]
    /// enum PathEnvError<'a> {
    ///     #[error(transparent)]
    ///     Ruleset(#[from] RulesetError),
    ///     #[error(transparent)]
    ///     AddRuleIter(#[from] PathFdError),
    ///     #[error("missing environment variable {0}")]
    ///     MissingVar(&'a str),
    /// }
    ///
    /// struct PathEnv {
    ///     paths: Vec<u8>,
    ///     access: BitFlags<AccessFs>,
    /// }
    ///
    /// impl PathEnv {
    ///     // env_var is the name of an environment variable
    ///     // containing paths requested to be allowed.
    ///     // Paths are separated with ":", e.g. "/bin:/lib:/usr:/proc".
    ///     // In case an empty string is provided,
    ///     // no restrictions are applied.
    ///     // `access` is the set of access rights allowed for each of the parsed paths.
    ///     fn new<'a>(
    ///         env_var: &'a str, access: BitFlags<AccessFs>
    ///     ) -> Result<Self, PathEnvError<'a>> {
    ///         Ok(Self {
    ///             paths: env::var_os(env_var)
    ///                 .ok_or(PathEnvError::MissingVar(env_var))?
    ///                 .into_vec(),
    ///             access,
    ///         })
    ///     }
    ///
    ///     fn iter(
    ///         &self,
    ///     ) -> impl Iterator<Item = Result<PathBeneath<PathFd>, PathEnvError<'static>>> + '_ {
    ///         let is_empty = self.paths.is_empty();
    ///         self.paths
    ///             .split(|b| *b == b':')
    ///             // Skips the first empty element from of an empty string.
    ///             .skip_while(move |_| is_empty)
    ///             .map(OsStr::from_bytes)
    ///             .map(move |path|
    ///                 Ok(PathBeneath::new(PathFd::new(path)?, self.access)))
    ///     }
    /// }
    ///
    /// fn restrict_env() -> Result<RestrictionStatus, PathEnvError<'static>> {
    ///     Ok(Ruleset::default()
    ///         .handle_access(AccessFs::from_all(ABI::V1))?
    ///         .create()?
    ///         // In the shell: export EXECUTABLE_PATH="/usr:/bin:/sbin"
    ///         .add_rules(PathEnv::new("EXECUTABLE_PATH", AccessFs::Execute.into())?.iter())?
    ///         .restrict_self()?)
    /// }
    /// ```
    fn add_rules<I, T, U, E>(mut self, rules: I) -> Result<Self, E>
    where
        I: IntoIterator<Item = Result<T, E>>,
        T: Rule<U>,
        U: HandledAccess + PrivateHandledAccess,
        E: From<RulesetError>,
    {
        for rule in rules {
            self = self.add_rule(rule?)?;
        }
        Ok(self)
    }

    /// Configures the ruleset to call `prctl(2)` with the `PR_SET_NO_NEW_PRIVS` command
    /// in [`restrict_self()`](RulesetCreated::restrict_self).
    ///
    /// This `prctl(2)` call is never ignored, even if an error was encountered on a [`Ruleset`] or
    /// [`RulesetCreated`] method call while [`CompatLevel::SoftRequirement`] was set.
    fn set_no_new_privs(mut self, no_new_privs: bool) -> Self {
        <Self as AsMut<RulesetCreated>>::as_mut(&mut self).no_new_privs = no_new_privs;
        self
    }
}

/// Ruleset created with [`Ruleset::create()`].
#[cfg_attr(test, derive(Debug))]
pub struct RulesetCreated {
    fd: Option<OwnedFd>,
    no_new_privs: bool,
    pub(crate) requested_handled_fs: BitFlags<AccessFs>,
    pub(crate) requested_handled_net: BitFlags<AccessNet>,
    compat: Compatibility,
}

impl RulesetCreated {
    pub(crate) fn new(ruleset: Ruleset, fd: Option<OwnedFd>) -> Self {
        // The compatibility state is initialized by Ruleset::create().
        #[cfg(test)]
        assert!(!matches!(ruleset.compat.state, CompatState::Init));

        RulesetCreated {
            fd,
            no_new_privs: true,
            requested_handled_fs: ruleset.requested_handled_fs,
            requested_handled_net: ruleset.requested_handled_net,
            compat: ruleset.compat,
        }
    }

    /// Attempts to restrict the calling thread with the ruleset
    /// according to the best-effort configuration
    /// (see [`RulesetCreated::set_compatibility()`] and [`CompatLevel::BestEffort`]).
    /// Call `prctl(2)` with the `PR_SET_NO_NEW_PRIVS`
    /// according to the ruleset configuration.
    ///
    /// On error, returns a wrapped [`RestrictSelfError`].
    pub fn restrict_self(mut self) -> Result<RestrictionStatus, RulesetError> {
        let mut body = || -> Result<RestrictionStatus, RestrictSelfError> {
            // Enforce no_new_privs even if something failed with SoftRequirement. The rationale is
            // that no_new_privs should not be an issue on its own if it is not explicitly
            // deactivated.
            let enforced_nnp = if self.no_new_privs {
                if let Err(e) = prctl_set_no_new_privs() {
                    match self.compat.level.into() {
                        CompatLevel::BestEffort => {}
                        CompatLevel::SoftRequirement => {
                            self.compat.update(CompatState::Dummy);
                        }
                        CompatLevel::HardRequirement => {
                            return Err(RestrictSelfError::SetNoNewPrivsCall { source: e });
                        }
                    }
                    // To get a consistent behavior, calls this prctl whether or not
                    // Landlock is supported by the running kernel.
                    let support_nnp = support_no_new_privs();
                    match self.compat.state {
                        // It should not be an error for kernel (older than 3.5) not supporting
                        // no_new_privs.
                        CompatState::Init | CompatState::No | CompatState::Dummy => {
                            if support_nnp {
                                // The kernel seems to be between 3.5 (included) and 5.13 (excluded),
                                // or Landlock is not enabled; no_new_privs should be supported anyway.
                                return Err(RestrictSelfError::SetNoNewPrivsCall { source: e });
                            }
                        }
                        // A kernel supporting Landlock should also support no_new_privs (unless
                        // filtered by seccomp).
                        CompatState::Full | CompatState::Partial => {
                            return Err(RestrictSelfError::SetNoNewPrivsCall { source: e })
                        }
                    }
                    false
                } else {
                    true
                }
            } else {
                false
            };

            match self.compat.state {
                CompatState::Init | CompatState::No | CompatState::Dummy => Ok(RestrictionStatus {
                    ruleset: self.compat.state.into(),
                    no_new_privs: enforced_nnp,
                }),
                CompatState::Full | CompatState::Partial => {
                    #[cfg(test)]
                    assert!(self.fd.is_some());
                    // Does not consume ruleset FD, which will be automatically closed after this block.
                    let fd = self.fd.as_ref().map(|f| f.as_raw_fd()).unwrap_or(-1);
                    match unsafe { uapi::landlock_restrict_self(fd, 0) } {
                        0 => {
                            self.compat.update(CompatState::Full);
                            Ok(RestrictionStatus {
                                ruleset: self.compat.state.into(),
                                no_new_privs: enforced_nnp,
                            })
                        }
                        // TODO: match specific Landlock restrict self errors
                        _ => Err(RestrictSelfError::RestrictSelfCall {
                            source: Error::last_os_error(),
                        }),
                    }
                }
            }
        };
        Ok(body()?)
    }

    /// Creates a new `RulesetCreated` instance by duplicating the underlying file descriptor.
    /// Rule modification will affect both `RulesetCreated` instances simultaneously.
    ///
    /// On error, returns [`std::io::Error`].
    pub fn try_clone(&self) -> std::io::Result<Self> {
        Ok(RulesetCreated {
            fd: self.fd.as_ref().map(|f| f.try_clone()).transpose()?,
            no_new_privs: self.no_new_privs,
            requested_handled_fs: self.requested_handled_fs,
            requested_handled_net: self.requested_handled_net,
            compat: self.compat,
        })
    }
}

impl From<RulesetCreated> for Option<OwnedFd> {
    fn from(ruleset: RulesetCreated) -> Self {
        ruleset.fd
    }
}

#[test]
fn ruleset_created_ownedfd_none() {
    let ruleset = Ruleset::from(ABI::Unsupported)
        .handle_access(AccessFs::Execute)
        .unwrap()
        .create()
        .unwrap();
    let fd: Option<OwnedFd> = ruleset.into();
    assert!(fd.is_none());
}

impl AsMut<RulesetCreated> for RulesetCreated {
    fn as_mut(&mut self) -> &mut RulesetCreated {
        self
    }
}

impl RulesetCreatedAttr for RulesetCreated {}

impl RulesetCreatedAttr for &mut RulesetCreated {}

#[test]
fn ruleset_created_attr() {
    let mut ruleset_created = Ruleset::from(ABI::Unsupported)
        .handle_access(AccessFs::Execute)
        .unwrap()
        .create()
        .unwrap();
    let ruleset_created_ref = &mut ruleset_created;

    // Can pass this reference to populate the ruleset...
    ruleset_created_ref
        .set_compatibility(CompatLevel::BestEffort)
        .add_rule(PathBeneath::new(
            PathFd::new("/usr").unwrap(),
            AccessFs::Execute,
        ))
        .unwrap()
        .add_rule(PathBeneath::new(
            PathFd::new("/etc").unwrap(),
            AccessFs::Execute,
        ))
        .unwrap();

    // ...and finally restrict with the last rules (thanks to non-lexical lifetimes).
    assert_eq!(
        ruleset_created
            .set_compatibility(CompatLevel::BestEffort)
            .add_rule(PathBeneath::new(
                PathFd::new("/tmp").unwrap(),
                AccessFs::Execute,
            ))
            .unwrap()
            .add_rule(PathBeneath::new(
                PathFd::new("/var").unwrap(),
                AccessFs::Execute,
            ))
            .unwrap()
            .restrict_self()
            .unwrap(),
        RestrictionStatus {
            ruleset: RulesetStatus::NotEnforced,
            no_new_privs: true,
        }
    );
}

#[test]
fn ruleset_compat_dummy() {
    for level in [CompatLevel::BestEffort, CompatLevel::SoftRequirement] {
        println!("level: {:?}", level);

        // ABI:Unsupported does not support AccessFs::Execute.
        let ruleset = Ruleset::from(ABI::Unsupported);
        assert_eq!(ruleset.compat.state, CompatState::Init);

        let ruleset = ruleset.set_compatibility(level);
        assert_eq!(ruleset.compat.state, CompatState::Init);

        let ruleset = ruleset.handle_access(AccessFs::Execute).unwrap();
        assert_eq!(
            ruleset.compat.state,
            match level {
                CompatLevel::BestEffort => CompatState::No,
                CompatLevel::SoftRequirement => CompatState::Dummy,
                _ => unreachable!(),
            }
        );

        let ruleset_created = ruleset.create().unwrap();
        // Because the compatibility state was either No or Dummy, calling create() updates it to
        // Dummy.
        assert_eq!(ruleset_created.compat.state, CompatState::Dummy);

        let ruleset_created = ruleset_created
            .add_rule(PathBeneath::new(
                PathFd::new("/usr").unwrap(),
                AccessFs::Execute,
            ))
            .unwrap();
        assert_eq!(ruleset_created.compat.state, CompatState::Dummy);
    }
}

#[test]
fn ruleset_compat_partial() {
    // CompatLevel::BestEffort
    let ruleset = Ruleset::from(ABI::V1);
    assert_eq!(ruleset.compat.state, CompatState::Init);

    // ABI::V1 does not support AccessFs::Refer.
    let ruleset = ruleset.handle_access(AccessFs::Refer).unwrap();
    assert_eq!(ruleset.compat.state, CompatState::No);

    let ruleset = ruleset.handle_access(AccessFs::Execute).unwrap();
    assert_eq!(ruleset.compat.state, CompatState::Partial);

    // Requesting to handle another unsupported handled access does not change anything.
    let ruleset = ruleset.handle_access(AccessFs::Refer).unwrap();
    assert_eq!(ruleset.compat.state, CompatState::Partial);
}

#[test]
fn ruleset_unsupported() {
    assert_eq!(
        Ruleset::from(ABI::Unsupported)
            // BestEffort for Ruleset.
            .handle_access(AccessFs::Execute)
            .unwrap()
            .create()
            .unwrap()
            .restrict_self()
            .unwrap(),
        RestrictionStatus {
            ruleset: RulesetStatus::NotEnforced,
            // With BestEffort, no_new_privs is still enabled.
            no_new_privs: true,
        }
    );

    assert_eq!(
        Ruleset::from(ABI::Unsupported)
            // SoftRequirement for Ruleset.
            .set_compatibility(CompatLevel::SoftRequirement)
            .handle_access(AccessFs::Execute)
            .unwrap()
            .create()
            .unwrap()
            .restrict_self()
            .unwrap(),
        RestrictionStatus {
            ruleset: RulesetStatus::NotEnforced,
            // With SoftRequirement, no_new_privs is still enabled.
            no_new_privs: true,
        }
    );

    // Missing handled access because of the compatibility level.
    matches!(
        Ruleset::from(ABI::Unsupported)
            // HardRequirement for Ruleset.
            .set_compatibility(CompatLevel::HardRequirement)
            .handle_access(AccessFs::Execute)
            .unwrap_err(),
        RulesetError::CreateRuleset(CreateRulesetError::MissingHandledAccess)
    );

    // Missing scope access because of the compatibility level.
    matches!(
        Ruleset::from(ABI::Unsupported)
            // HardRequirement for Ruleset.
            .set_compatibility(CompatLevel::HardRequirement)
            .scope(Scope::Signal)
            .unwrap_err(),
        RulesetError::CreateRuleset(CreateRulesetError::MissingHandledAccess)
    );

    assert_eq!(
        Ruleset::from(ABI::Unsupported)
            .handle_access(AccessFs::Execute)
            .unwrap()
            .create()
            .unwrap()
            // SoftRequirement for RulesetCreated without any rule.
            .set_compatibility(CompatLevel::SoftRequirement)
            .restrict_self()
            .unwrap(),
        RestrictionStatus {
            ruleset: RulesetStatus::NotEnforced,
            // With SoftRequirement, no_new_privs is untouched if there is no error (e.g. no rule).
            no_new_privs: true,
        }
    );

    // Don't explicitly call create() on a CI that doesn't support Landlock.
    if compat::can_emulate(ABI::V1, ABI::V1, Some(ABI::V2)) {
        assert_eq!(
            Ruleset::from(ABI::V1)
                .handle_access(make_bitflags!(AccessFs::{Execute | Refer}))
                .unwrap()
                .create()
                .unwrap()
                // SoftRequirement for RulesetCreated with a rule.
                .set_compatibility(CompatLevel::SoftRequirement)
                .add_rule(PathBeneath::new(PathFd::new("/").unwrap(), AccessFs::Refer))
                .unwrap()
                .restrict_self()
                .unwrap(),
            RestrictionStatus {
                ruleset: RulesetStatus::NotEnforced,
                // With SoftRequirement, no_new_privs is still enabled, even if there is an error
                // (e.g. unsupported access right).
                no_new_privs: true,
            }
        );
    }

    assert_eq!(
        Ruleset::from(ABI::Unsupported)
            .handle_access(AccessFs::Execute)
            .unwrap()
            .create()
            .unwrap()
            .set_no_new_privs(false)
            .restrict_self()
            .unwrap(),
        RestrictionStatus {
            ruleset: RulesetStatus::NotEnforced,
            no_new_privs: false,
        }
    );

    // Checks empty handled access with moot ruleset.
    assert!(matches!(
        Ruleset::from(ABI::Unsupported)
            // Empty access-rights
            .handle_access(AccessFs::from_all(ABI::Unsupported))
            .unwrap_err(),
        RulesetError::HandleAccesses(HandleAccessesError::Fs(HandleAccessError::Compat(
            CompatError::Access(AccessError::Empty)
        )))
    ));

    assert!(matches!(
        Ruleset::from(ABI::Unsupported)
            // No handle_access() nor scope() call.
            .create()
            .unwrap_err(),
        RulesetError::CreateRuleset(CreateRulesetError::MissingHandledAccess)
    ));

    // Checks empty handled access with minimal ruleset.
    assert!(matches!(
        Ruleset::from(ABI::V1)
            // Empty access-rights
            .handle_access(AccessFs::from_all(ABI::Unsupported))
            .unwrap_err(),
        RulesetError::HandleAccesses(HandleAccessesError::Fs(HandleAccessError::Compat(
            CompatError::Access(AccessError::Empty)
        )))
    ));

    // Checks empty scope with moot ruleset.
    assert!(matches!(
        Ruleset::from(ABI::Unsupported)
            .scope(Scope::from_all(ABI::Unsupported))
            .unwrap_err(),
        RulesetError::Scope(ScopeError::Compat(CompatError::Access(AccessError::Empty)))
    ));

    // Checks empty scope with minimal ruleset.
    assert!(matches!(
        Ruleset::from(ABI::V1)
            .scope(Scope::from_all(ABI::Unsupported))
            .unwrap_err(),
        RulesetError::Scope(ScopeError::Compat(CompatError::Access(AccessError::Empty)))
    ));

    // Tests inconsistency between the ruleset handled access-rights and the rule access-rights.
    for handled_access in &[
        make_bitflags!(AccessFs::{Execute | WriteFile}),
        AccessFs::Execute.into(),
    ] {
        let ruleset = Ruleset::from(ABI::V1)
            .handle_access(*handled_access)
            .unwrap();
        // Fakes a call to create() to test without involving the kernel (i.e. no
        // landlock_ruleset_create() call).
        let ruleset_created = RulesetCreated::new(ruleset, None);
        assert!(matches!(
            ruleset_created
                .add_rule(PathBeneath::new(
                    PathFd::new("/").unwrap(),
                    AccessFs::ReadFile
                ))
                .unwrap_err(),
            RulesetError::AddRules(AddRulesError::Fs(AddRuleError::UnhandledAccess { .. }))
        ));
    }
}

#[test]
fn ignore_abi_v2_with_abi_v1() {
    // We don't need kernel/CI support for Landlock because no related syscalls should actually be
    // performed.
    assert_eq!(
        Ruleset::from(ABI::V1)
            .set_compatibility(CompatLevel::HardRequirement)
            .handle_access(AccessFs::from_all(ABI::V1))
            .unwrap()
            .set_compatibility(CompatLevel::SoftRequirement)
            // Because Ruleset only supports V1, Refer will be ignored.
            .handle_access(AccessFs::Refer)
            .unwrap()
            .create()
            .unwrap()
            .add_rule(PathBeneath::new(
                PathFd::new("/tmp").unwrap(),
                AccessFs::from_all(ABI::V2)
            ))
            .unwrap()
            .add_rule(PathBeneath::new(
                PathFd::new("/usr").unwrap(),
                make_bitflags!(AccessFs::{ReadFile | ReadDir})
            ))
            .unwrap()
            .restrict_self()
            .unwrap(),
        RestrictionStatus {
            ruleset: RulesetStatus::NotEnforced,
            no_new_privs: true,
        }
    );
}

#[test]
fn unsupported_handled_access() {
    matches!(
        Ruleset::from(ABI::V3)
            .handle_access(AccessNet::from_all(ABI::V3))
            .unwrap_err(),
        RulesetError::HandleAccesses(HandleAccessesError::Net(HandleAccessError::Compat(
            CompatError::Access(AccessError::Empty)
        )))
    );
}

#[test]
fn unsupported_handled_access_errno() {
    assert_eq!(
        Errno::from(
            Ruleset::from(ABI::V3)
                .handle_access(AccessNet::from_all(ABI::V3))
                .unwrap_err()
        ),
        Errno::new(libc::EINVAL)
    );
}
