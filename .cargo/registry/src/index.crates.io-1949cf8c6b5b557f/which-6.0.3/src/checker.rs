use crate::finder::Checker;
use std::fs;
use std::path::Path;

pub struct ExecutableChecker;

impl ExecutableChecker {
    pub fn new() -> ExecutableChecker {
        ExecutableChecker
    }
}

impl Checker for ExecutableChecker {
    #[cfg(any(unix, target_os = "wasi", target_os = "redox"))]
    fn is_valid(&self, path: &Path) -> bool {
        use rustix::fs as rfs;
        let ret = rfs::access(path, rfs::Access::EXEC_OK).is_ok();
        #[cfg(feature = "tracing")]
        tracing::trace!("{} EXEC_OK = {ret}", path.display());
        ret
    }

    #[cfg(windows)]
    fn is_valid(&self, _path: &Path) -> bool {
        true
    }
}

pub struct ExistedChecker;

impl ExistedChecker {
    pub fn new() -> ExistedChecker {
        ExistedChecker
    }
}

impl Checker for ExistedChecker {
    #[cfg(target_os = "windows")]
    fn is_valid(&self, path: &Path) -> bool {
        let ret = fs::symlink_metadata(path)
            .map(|metadata| {
                let file_type = metadata.file_type();
                #[cfg(feature = "tracing")]
                tracing::trace!(
                    "{} is_file() = {}, is_symlink() = {}",
                    path.display(),
                    file_type.is_file(),
                    file_type.is_symlink()
                );
                file_type.is_file() || file_type.is_symlink()
            })
            .unwrap_or(false)
            && (path.extension().is_some() || matches_arch(path));
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "{} has_extension = {}, ExistedChecker::is_valid() = {ret}",
            path.display(),
            path.extension().is_some()
        );
        ret
    }

    #[cfg(not(target_os = "windows"))]
    fn is_valid(&self, path: &Path) -> bool {
        let ret = fs::metadata(path)
            .map(|metadata| metadata.is_file())
            .unwrap_or(false);
        #[cfg(feature = "tracing")]
        tracing::trace!("{} is_file() = {ret}", path.display());
        ret
    }
}

#[cfg(target_os = "windows")]
fn matches_arch(path: &Path) -> bool {
    let ret = winsafe::GetBinaryType(&path.display().to_string()).is_ok();
    #[cfg(feature = "tracing")]
    tracing::trace!("{} matches_arch() = {ret}", path.display());
    ret
}

pub struct CompositeChecker {
    checkers: Vec<Box<dyn Checker>>,
}

impl CompositeChecker {
    pub fn new() -> CompositeChecker {
        CompositeChecker {
            checkers: Vec::new(),
        }
    }

    pub fn add_checker(mut self, checker: Box<dyn Checker>) -> CompositeChecker {
        self.checkers.push(checker);
        self
    }
}

impl Checker for CompositeChecker {
    fn is_valid(&self, path: &Path) -> bool {
        self.checkers.iter().all(|checker| checker.is_valid(path))
    }
}
