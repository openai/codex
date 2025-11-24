// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::compat::private::OptionCompatLevelMut;
use crate::{
    uapi, Access, AddRuleError, AddRulesError, CompatError, CompatLevel, CompatResult, CompatState,
    Compatible, HandleAccessError, HandleAccessesError, HandledAccess, PathBeneathError,
    PathFdError, PrivateHandledAccess, PrivateRule, Rule, Ruleset, RulesetCreated, RulesetError,
    TailoredCompatLevel, TryCompat, ABI,
};
use enumflags2::{bitflags, make_bitflags, BitFlags};
use std::fs::OpenOptions;
use std::io::Error;
use std::mem::zeroed;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::path::Path;

#[cfg(test)]
use crate::{RulesetAttr, RulesetCreatedAttr};
#[cfg(test)]
use strum::IntoEnumIterator;

/// File system access right.
///
/// Each variant of `AccessFs` is an [access right](https://www.kernel.org/doc/html/latest/userspace-api/landlock.html#access-rights)
/// for the file system.
/// A set of access rights can be created with [`BitFlags<AccessFs>`](BitFlags).
///
/// # Example
///
/// ```
/// use landlock::{ABI, Access, AccessFs, BitFlags, make_bitflags};
///
/// let exec = AccessFs::Execute;
///
/// let exec_set: BitFlags<AccessFs> = exec.into();
///
/// let file_content = make_bitflags!(AccessFs::{Execute | WriteFile | ReadFile});
///
/// let fs_v1 = AccessFs::from_all(ABI::V1);
///
/// let without_exec = fs_v1 & !AccessFs::Execute;
///
/// assert_eq!(fs_v1 | AccessFs::Refer, AccessFs::from_all(ABI::V2));
/// ```
///
/// # Warning
///
/// To avoid unknown restrictions **don't use `BitFlags::<AccessFs>::all()` nor `BitFlags::ALL`**,
/// but use a version you tested and vetted instead,
/// for instance [`AccessFs::from_all(ABI::V1)`](Access::from_all).
/// Direct use of **the [`BitFlags`] API is deprecated**.
/// See [`ABI`] for the rationale and help to test it.
#[bitflags]
#[repr(u64)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccessFs {
    /// Execute a file.
    Execute = uapi::LANDLOCK_ACCESS_FS_EXECUTE as u64,
    /// Open a file with write access.
    ///
    /// # Note
    ///
    /// Certain operations (such as [`std::fs::write`]) may also require [`AccessFs::Truncate`] since [`ABI::V3`].
    WriteFile = uapi::LANDLOCK_ACCESS_FS_WRITE_FILE as u64,
    /// Open a file with read access.
    ReadFile = uapi::LANDLOCK_ACCESS_FS_READ_FILE as u64,
    /// Open a directory or list its content.
    ReadDir = uapi::LANDLOCK_ACCESS_FS_READ_DIR as u64,
    /// Remove an empty directory or rename one.
    RemoveDir = uapi::LANDLOCK_ACCESS_FS_REMOVE_DIR as u64,
    /// Unlink (or rename) a file.
    RemoveFile = uapi::LANDLOCK_ACCESS_FS_REMOVE_FILE as u64,
    /// Create (or rename or link) a character device.
    MakeChar = uapi::LANDLOCK_ACCESS_FS_MAKE_CHAR as u64,
    /// Create (or rename) a directory.
    MakeDir = uapi::LANDLOCK_ACCESS_FS_MAKE_DIR as u64,
    /// Create (or rename or link) a regular file.
    MakeReg = uapi::LANDLOCK_ACCESS_FS_MAKE_REG as u64,
    /// Create (or rename or link) a UNIX domain socket.
    MakeSock = uapi::LANDLOCK_ACCESS_FS_MAKE_SOCK as u64,
    /// Create (or rename or link) a named pipe.
    MakeFifo = uapi::LANDLOCK_ACCESS_FS_MAKE_FIFO as u64,
    /// Create (or rename or link) a block device.
    MakeBlock = uapi::LANDLOCK_ACCESS_FS_MAKE_BLOCK as u64,
    /// Create (or rename or link) a symbolic link.
    MakeSym = uapi::LANDLOCK_ACCESS_FS_MAKE_SYM as u64,
    /// Link or rename a file from or to a different directory.
    Refer = uapi::LANDLOCK_ACCESS_FS_REFER as u64,
    /// Truncate a file with `truncate(2)`, `ftruncate(2)`, `creat(2)`, or `open(2)` with `O_TRUNC`.
    Truncate = uapi::LANDLOCK_ACCESS_FS_TRUNCATE as u64,
    /// Send IOCL commands to a device file.
    IoctlDev = uapi::LANDLOCK_ACCESS_FS_IOCTL_DEV as u64,
}

impl Access for AccessFs {
    /// Union of [`from_read()`](AccessFs::from_read) and [`from_write()`](AccessFs::from_write).
    fn from_all(abi: ABI) -> BitFlags<Self> {
        // An empty access-right would be an error if passed to the kernel, but because the kernel
        // doesn't support Landlock, no Landlock syscall should be called.  try_compat() should
        // also return RestrictionStatus::Unrestricted when called with unsupported/empty
        // access-rights.
        Self::from_read(abi) | Self::from_write(abi)
    }
}

impl AccessFs {
    // Roughly read (i.e. not all FS actions are handled).
    /// Gets the access rights identified as read-only according to a specific ABI.
    /// Exclusive with [`from_write()`](AccessFs::from_write).
    pub fn from_read(abi: ABI) -> BitFlags<Self> {
        match abi {
            ABI::Unsupported => BitFlags::EMPTY,
            ABI::V1 | ABI::V2 | ABI::V3 | ABI::V4 | ABI::V5 | ABI::V6 => make_bitflags!(AccessFs::{
                Execute
                | ReadFile
                | ReadDir
            }),
        }
    }

    // Roughly write (i.e. not all FS actions are handled).
    /// Gets the access rights identified as write-only according to a specific ABI.
    /// Exclusive with [`from_read()`](AccessFs::from_read).
    pub fn from_write(abi: ABI) -> BitFlags<Self> {
        match abi {
            ABI::Unsupported => BitFlags::EMPTY,
            ABI::V1 => make_bitflags!(AccessFs::{
                WriteFile
                | RemoveDir
                | RemoveFile
                | MakeChar
                | MakeDir
                | MakeReg
                | MakeSock
                | MakeFifo
                | MakeBlock
                | MakeSym
            }),
            ABI::V2 => Self::from_write(ABI::V1) | AccessFs::Refer,
            ABI::V3 | ABI::V4 => Self::from_write(ABI::V2) | AccessFs::Truncate,
            ABI::V5 | ABI::V6 => Self::from_write(ABI::V4) | AccessFs::IoctlDev,
        }
    }

    /// Gets the access rights legitimate for non-directory files.
    pub fn from_file(abi: ABI) -> BitFlags<Self> {
        Self::from_all(abi) & ACCESS_FILE
    }
}

#[test]
fn consistent_access_fs_rw() {
    for abi in ABI::iter() {
        let access_all = AccessFs::from_all(abi);
        let access_read = AccessFs::from_read(abi);
        let access_write = AccessFs::from_write(abi);
        assert_eq!(access_read, !access_write & access_all);
        assert_eq!(access_read | access_write, access_all);
    }
}

impl HandledAccess for AccessFs {}

impl PrivateHandledAccess for AccessFs {
    fn ruleset_handle_access(
        ruleset: &mut Ruleset,
        access: BitFlags<Self>,
    ) -> Result<(), HandleAccessesError> {
        // We need to record the requested accesses for PrivateRule::check_consistency().
        ruleset.requested_handled_fs |= access;
        ruleset.actual_handled_fs |= match access
            .try_compat(
                ruleset.compat.abi(),
                ruleset.compat.level,
                &mut ruleset.compat.state,
            )
            .map_err(HandleAccessError::Compat)?
        {
            Some(a) => a,
            None => return Ok(()),
        };
        Ok(())
    }

    fn into_add_rules_error(error: AddRuleError<Self>) -> AddRulesError {
        AddRulesError::Fs(error)
    }

    fn into_handle_accesses_error(error: HandleAccessError<Self>) -> HandleAccessesError {
        HandleAccessesError::Fs(error)
    }
}

// TODO: Make ACCESS_FILE a property of AccessFs.
// TODO: Add tests for ACCESS_FILE.
const ACCESS_FILE: BitFlags<AccessFs> = make_bitflags!(AccessFs::{
    ReadFile | WriteFile | Execute | Truncate | IoctlDev
});

// XXX: What should we do when a stat call failed?
fn is_file<F>(fd: F) -> Result<bool, Error>
where
    F: AsFd,
{
    unsafe {
        let mut stat = zeroed();
        match libc::fstat(fd.as_fd().as_raw_fd(), &mut stat) {
            0 => Ok((stat.st_mode & libc::S_IFMT) != libc::S_IFDIR),
            _ => Err(Error::last_os_error()),
        }
    }
}

/// Landlock rule for a file hierarchy.
///
/// # Example
///
/// ```
/// use landlock::{AccessFs, PathBeneath, PathFd, PathFdError};
///
/// fn home_dir() -> Result<PathBeneath<PathFd>, PathFdError> {
///     Ok(PathBeneath::new(PathFd::new("/home")?, AccessFs::ReadDir))
/// }
/// ```
#[cfg_attr(test, derive(Debug))]
pub struct PathBeneath<F> {
    attr: uapi::landlock_path_beneath_attr,
    // Ties the lifetime of a file descriptor to this object.
    parent_fd: F,
    allowed_access: BitFlags<AccessFs>,
    compat_level: Option<CompatLevel>,
}

impl<F> PathBeneath<F>
where
    F: AsFd,
{
    /// Creates a new `PathBeneath` rule identifying the `parent` directory of a file hierarchy,
    /// or just a file, and allows `access` on it.
    /// The `parent` file descriptor will be automatically closed with the returned `PathBeneath`.
    pub fn new<A>(parent: F, access: A) -> Self
    where
        A: Into<BitFlags<AccessFs>>,
    {
        PathBeneath {
            // Invalid access rights until as_ptr() is called.
            attr: unsafe { zeroed() },
            parent_fd: parent,
            allowed_access: access.into(),
            compat_level: None,
        }
    }
}

impl<F> TryCompat<AccessFs> for PathBeneath<F>
where
    F: AsFd,
{
    fn try_compat_children<L>(
        mut self,
        abi: ABI,
        parent_level: L,
        compat_state: &mut CompatState,
    ) -> Result<Option<Self>, CompatError<AccessFs>>
    where
        L: Into<CompatLevel>,
    {
        // Checks with our own compatibility level, if any.
        self.allowed_access = match self.allowed_access.try_compat(
            abi,
            self.tailored_compat_level(parent_level),
            compat_state,
        )? {
            Some(a) => a,
            None => return Ok(None),
        };
        Ok(Some(self))
    }

    fn try_compat_inner(
        &mut self,
        _abi: ABI,
    ) -> Result<CompatResult<AccessFs>, CompatError<AccessFs>> {
        // Gets subset of valid accesses according the FD type.
        let valid_access =
            if is_file(&self.parent_fd).map_err(|e| PathBeneathError::StatCall { source: e })? {
                self.allowed_access & ACCESS_FILE
            } else {
                self.allowed_access
            };

        if self.allowed_access != valid_access {
            let error = PathBeneathError::DirectoryAccess {
                access: self.allowed_access,
                incompatible: self.allowed_access ^ valid_access,
            }
            .into();
            self.allowed_access = valid_access;
            // Linux would return EINVAL.
            Ok(CompatResult::Partial(error))
        } else {
            Ok(CompatResult::Full)
        }
    }
}

#[test]
fn path_beneath_try_compat_children() {
    use crate::*;

    // AccessFs::Refer is not handled by ABI::V1 and only for directories.
    let access_file = AccessFs::ReadFile | AccessFs::Refer;

    // Test error ordering with ABI::V1
    let mut ruleset = Ruleset::from(ABI::V1).handle_access(access_file).unwrap();
    // Do not actually perform any syscall.
    ruleset.compat.state = CompatState::Dummy;
    assert!(matches!(
        RulesetCreated::new(ruleset, None)
            .set_compatibility(CompatLevel::HardRequirement)
            .add_rule(PathBeneath::new(PathFd::new("/dev/null").unwrap(), access_file))
            .unwrap_err(),
        RulesetError::AddRules(AddRulesError::Fs(AddRuleError::Compat(
            CompatError::PathBeneath(PathBeneathError::DirectoryAccess { access, incompatible })
        ))) if access == access_file && incompatible == AccessFs::Refer
    ));

    // Test error ordering with ABI::V2
    let mut ruleset = Ruleset::from(ABI::V2).handle_access(access_file).unwrap();
    // Do not actually perform any syscall.
    ruleset.compat.state = CompatState::Dummy;
    assert!(matches!(
        RulesetCreated::new(ruleset, None)
            .set_compatibility(CompatLevel::HardRequirement)
            .add_rule(PathBeneath::new(PathFd::new("/dev/null").unwrap(), access_file))
            .unwrap_err(),
        RulesetError::AddRules(AddRulesError::Fs(AddRuleError::Compat(
            CompatError::PathBeneath(PathBeneathError::DirectoryAccess { access, incompatible })
        ))) if access == access_file && incompatible == AccessFs::Refer
    ));
}

#[test]
fn path_beneath_try_compat() {
    use crate::*;

    let abi = ABI::V1;

    for file in &["/etc/passwd", "/dev/null"] {
        let mut compat_state = CompatState::Init;
        let ro_access = AccessFs::ReadDir | AccessFs::ReadFile;
        assert!(matches!(
            PathBeneath::new(PathFd::new(file).unwrap(), ro_access)
                .try_compat(abi, CompatLevel::HardRequirement, &mut compat_state)
                .unwrap_err(),
            CompatError::PathBeneath(PathBeneathError::DirectoryAccess { access, incompatible })
                if access == ro_access && incompatible == AccessFs::ReadDir
        ));

        let mut compat_state = CompatState::Init;
        assert!(matches!(
            PathBeneath::new(PathFd::new(file).unwrap(), BitFlags::EMPTY)
                .try_compat(abi, CompatLevel::BestEffort, &mut compat_state)
                .unwrap_err(),
            CompatError::Access(AccessError::Empty)
        ));
    }

    let full_access = AccessFs::from_all(ABI::V1);
    for compat_level in &[
        CompatLevel::BestEffort,
        CompatLevel::SoftRequirement,
        CompatLevel::HardRequirement,
    ] {
        let mut compat_state = CompatState::Init;
        let mut path_beneath = PathBeneath::new(PathFd::new("/").unwrap(), full_access)
            .try_compat(abi, *compat_level, &mut compat_state)
            .unwrap()
            .unwrap();
        assert_eq!(compat_state, CompatState::Full);

        // Without synchronization.
        let raw_access = path_beneath.attr.allowed_access;
        assert_eq!(raw_access, 0);

        // Synchronize the inner attribute buffer.
        let _ = path_beneath.as_ptr();
        let raw_access = path_beneath.attr.allowed_access;
        assert_eq!(raw_access, full_access.bits());
    }
}

impl<F> OptionCompatLevelMut for PathBeneath<F> {
    fn as_option_compat_level_mut(&mut self) -> &mut Option<CompatLevel> {
        &mut self.compat_level
    }
}

impl<F> OptionCompatLevelMut for &mut PathBeneath<F> {
    fn as_option_compat_level_mut(&mut self) -> &mut Option<CompatLevel> {
        &mut self.compat_level
    }
}

impl<F> Compatible for PathBeneath<F> {}

impl<F> Compatible for &mut PathBeneath<F> {}

#[test]
fn path_beneath_compatibility() {
    let mut path = PathBeneath::new(PathFd::new("/").unwrap(), AccessFs::from_all(ABI::V1));
    let path_ref = &mut path;

    let level = path_ref.as_option_compat_level_mut();
    assert_eq!(level, &None);
    assert_eq!(
        <Option<CompatLevel> as Into<CompatLevel>>::into(*level),
        CompatLevel::BestEffort
    );

    path_ref.set_compatibility(CompatLevel::SoftRequirement);
    assert_eq!(
        path_ref.as_option_compat_level_mut(),
        &Some(CompatLevel::SoftRequirement)
    );

    path.set_compatibility(CompatLevel::HardRequirement);
}

// It is useful for documentation generation to explicitely implement Rule for every types, instead
// of doing it generically.
impl<F> Rule<AccessFs> for PathBeneath<F> where F: AsFd {}

impl<F> PrivateRule<AccessFs> for PathBeneath<F>
where
    F: AsFd,
{
    const TYPE_ID: uapi::landlock_rule_type = uapi::landlock_rule_type_LANDLOCK_RULE_PATH_BENEATH;

    fn as_ptr(&mut self) -> *const libc::c_void {
        self.attr.parent_fd = self.parent_fd.as_fd().as_raw_fd();
        self.attr.allowed_access = self.allowed_access.bits();
        &self.attr as *const _ as _
    }

    fn check_consistency(&self, ruleset: &RulesetCreated) -> Result<(), AddRulesError> {
        // Checks that this rule doesn't contain a superset of the access-rights handled by the
        // ruleset.  This check is about requested access-rights but not actual access-rights.
        // Indeed, we want to get a deterministic behavior, i.e. not based on the running kernel
        // (which is handled by Ruleset and RulesetCreated).
        if ruleset.requested_handled_fs.contains(self.allowed_access) {
            Ok(())
        } else {
            Err(AddRuleError::UnhandledAccess {
                access: self.allowed_access,
                incompatible: self.allowed_access & !ruleset.requested_handled_fs,
            }
            .into())
        }
    }
}

#[test]
fn path_beneath_check_consistency() {
    use crate::*;

    let ro_access = AccessFs::ReadDir | AccessFs::ReadFile;
    let rx_access = AccessFs::Execute | AccessFs::ReadFile;
    assert!(matches!(
        Ruleset::from(ABI::Unsupported)
            .handle_access(ro_access)
            .unwrap()
            .create()
            .unwrap()
            .add_rule(PathBeneath::new(PathFd::new("/").unwrap(), rx_access))
            .unwrap_err(),
        RulesetError::AddRules(AddRulesError::Fs(AddRuleError::UnhandledAccess { access, incompatible }))
            if access == rx_access && incompatible == AccessFs::Execute
    ));
}

/// Simple helper to open a file or a directory with the `O_PATH` flag.
///
/// This is the recommended way to identify a path
/// and manage the lifetime of the underlying opened file descriptor.
/// Indeed, using other [`AsFd`] implementations such as [`File`] brings more complexity
/// and may lead to unexpected errors (e.g., denied access).
///
/// [`File`]: std::fs::File
///
/// # Example
///
/// ```
/// use landlock::{AccessFs, PathBeneath, PathFd, PathFdError};
///
/// fn allowed_root_dir(access: AccessFs) -> Result<PathBeneath<PathFd>, PathFdError> {
///     let fd = PathFd::new("/")?;
///     Ok(PathBeneath::new(fd, access))
/// }
/// ```
#[cfg_attr(test, derive(Debug))]
pub struct PathFd {
    fd: OwnedFd,
}

impl PathFd {
    pub fn new<T>(path: T) -> Result<Self, PathFdError>
    where
        T: AsRef<Path>,
    {
        Ok(PathFd {
            fd: OpenOptions::new()
                .read(true)
                // If the O_PATH is not supported, it is automatically ignored (Linux < 2.6.39).
                .custom_flags(libc::O_PATH | libc::O_CLOEXEC)
                .open(path.as_ref())
                .map_err(|e| PathFdError::OpenCall {
                    source: e,
                    path: path.as_ref().into(),
                })?
                .into(),
        })
    }
}

impl AsFd for PathFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

#[test]
fn path_fd() {
    use std::fs::File;
    use std::io::Read;

    PathBeneath::new(PathFd::new("/").unwrap(), AccessFs::Execute);
    PathBeneath::new(File::open("/").unwrap(), AccessFs::Execute);

    let mut buffer = [0; 1];
    // Checks that PathFd really returns an FD opened with O_PATH (Bad file descriptor error).
    File::from(PathFd::new("/etc/passwd").unwrap().fd)
        .read(&mut buffer)
        .unwrap_err();
}

/// Helper to quickly create an iterator of PathBeneath rules.
///
/// # Note
///
/// Landlock rules operate on file descriptors, not paths. This is a helper to create rules based on paths. **It
/// silently ignores paths that cannot be opened**, and automatically adjusts access rights according to file types when
/// possible.
///
/// # Example
///
/// ```
/// use landlock::{
///     ABI, Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus, RulesetError,
///     path_beneath_rules,
/// };
///
/// fn restrict_thread() -> Result<(), RulesetError> {
///     let abi = ABI::V1;
///     let status = Ruleset::default()
///         .handle_access(AccessFs::from_all(abi))?
///         .create()?
///         // Read-only access to /usr, /etc and /dev.
///         .add_rules(path_beneath_rules(&["/usr", "/etc", "/dev"], AccessFs::from_read(abi)))?
///         // Read-write access to /home and /tmp.
///         .add_rules(path_beneath_rules(&["/home", "/tmp"], AccessFs::from_all(abi)))?
///         .restrict_self()?;
///     match status.ruleset {
///         // The FullyEnforced case must be tested by the developer.
///         RulesetStatus::FullyEnforced => println!("Fully sandboxed."),
///         RulesetStatus::PartiallyEnforced => println!("Partially sandboxed."),
///         // Users should be warned that they are not protected.
///         RulesetStatus::NotEnforced => println!("Not sandboxed! Please update your kernel."),
///     }
///     Ok(())
/// }
/// ```
pub fn path_beneath_rules<I, P, A>(
    paths: I,
    access: A,
) -> impl Iterator<Item = Result<PathBeneath<PathFd>, RulesetError>>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
    A: Into<BitFlags<AccessFs>>,
{
    let access = access.into();
    paths.into_iter().filter_map(move |p| match PathFd::new(p) {
        Ok(f) => {
            let valid_access = match is_file(&f) {
                Ok(true) => access & ACCESS_FILE,
                // If the stat call failed, let's blindly rely on the requested access rights.
                Err(_) | Ok(false) => access,
            };
            Some(Ok(PathBeneath::new(f, valid_access)))
        }
        Err(_) => None,
    })
}

#[test]
fn path_beneath_rules_iter() {
    let _ = Ruleset::default()
        .handle_access(AccessFs::from_all(ABI::V1))
        .unwrap()
        .create()
        .unwrap()
        .add_rules(path_beneath_rules(
            &["/usr", "/opt", "/does-not-exist", "/root"],
            AccessFs::Execute,
        ))
        .unwrap();
}
