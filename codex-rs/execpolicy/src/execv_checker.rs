use std::borrow::Cow;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

use crate::ArgType;
use crate::Error::CannotCanonicalizePath;
use crate::Error::CannotCheckRelativePath;
use crate::Error::ReadablePathNotInReadableFolders;
use crate::Error::WriteablePathNotInWriteableFolders;
use crate::ExecCall;
use crate::MatchedExec;
use crate::Policy;
use crate::Result;
use crate::ValidExec;
use path_absolutize::*;

macro_rules! check_file_in_folders {
    ($file:expr, $folders:expr, $error:ident) => {
        if !$folders.iter().any(|folder| $file.starts_with(folder)) {
            return Err($error {
                file: $file.clone(),
                folders: $folders.to_vec(),
            });
        }
    };
}

pub struct ExecvChecker {
    execv_policy: Policy,
}

impl ExecvChecker {
    pub fn new(execv_policy: Policy) -> Self {
        Self { execv_policy }
    }

    pub fn r#match(&self, exec_call: &ExecCall) -> Result<MatchedExec> {
        self.execv_policy.check(exec_call)
    }

    /// The caller is responsible for ensuring readable_folders and
    /// writeable_folders are in canonical form.
    pub fn check(
        &self,
        valid_exec: ValidExec,
        cwd: &Option<OsString>,
        readable_folders: &[PathBuf],
        writeable_folders: &[PathBuf],
    ) -> Result<String> {
        for (arg_type, value) in valid_exec
            .args
            .into_iter()
            .map(|arg| (arg.r#type, arg.value))
            .chain(
                valid_exec
                    .opts
                    .into_iter()
                    .map(|opt| (opt.r#type, opt.value)),
            )
        {
            match arg_type {
                ArgType::ReadableFile => {
                    let readable_file = ensure_absolute_path(&value, cwd)?;
                    check_file_in_folders!(
                        readable_file,
                        readable_folders,
                        ReadablePathNotInReadableFolders
                    );
                }
                ArgType::WriteableFile => {
                    let writeable_file = ensure_absolute_path(&value, cwd)?;
                    check_file_in_folders!(
                        writeable_file,
                        writeable_folders,
                        WriteablePathNotInWriteableFolders
                    );
                }
                ArgType::OpaqueNonFile
                | ArgType::Unknown
                | ArgType::PositiveInteger
                | ArgType::SedCommand
                | ArgType::Literal(_) => {
                    continue;
                }
            }
        }

        let mut program = valid_exec.program.to_string();
        for system_path in valid_exec.system_path {
            if is_executable_file(&system_path) {
                program = system_path;
                break;
            }
        }

        Ok(program)
    }
}

fn ensure_absolute_path(path: &str, cwd: &Option<OsString>) -> Result<PathBuf> {
    let file = PathBuf::from(path);
    let result = if file.is_relative() {
        match cwd {
            Some(cwd) => file.absolutize_from(cwd),
            None => return Err(CannotCheckRelativePath { file }),
        }
    } else {
        file.absolutize()
    };
    result
        .map(Cow::into_owned)
        .map_err(|error| CannotCanonicalizePath {
            file: path.to_string(),
            error: error.kind(),
        })
}

fn is_executable_file(path: &str) -> bool {
    let file_path = Path::new(path);

    if let Ok(metadata) = std::fs::metadata(file_path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = metadata.permissions();

            // Check if the file is executable (by checking the executable bit for the owner)
            return metadata.is_file() && (permissions.mode() & 0o111 != 0);
        }

        #[cfg(windows)]
        {
            // TODO(mbolin): Check against PATHEXT environment variable.
            return metadata.is_file();
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::MatchedArg;
    use crate::PolicyParser;
    use anyhow::Result;
    use anyhow::anyhow;

    fn setup(fake_cp: &Path) -> ExecvChecker {
        let source = format!(
            r#"
define_program(
program="cp",
args=[ARG_RFILE, ARG_WFILE],
system_path=[{fake_cp:?}]
)
"#
        );
        let parser = PolicyParser::new("#test", &source);
        let policy = parser.parse().unwrap();
        ExecvChecker::new(policy)
    }

    #[test]
    fn test_check_valid_input_files() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create an executable file that can be used with the system_path arg.
        let fake_cp = temp_dir.path().join("cp");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let fake_cp_file = std::fs::File::create(&fake_cp)?;
            let mut permissions = fake_cp_file.metadata()?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&fake_cp, permissions)?;
        }
        #[cfg(windows)]
        {
            std::fs::File::create(&fake_cp)?;
        }

        // Create root_path and reference to files under the root.
        let root_path = temp_dir.path().to_path_buf();
        let source_path = root_path.join("source");
        let dest_path = root_path.join("dest");

        let cp = fake_cp.to_str().unwrap().to_string();
        let root = root_path.to_str().unwrap().to_string();
        let source = source_path.to_str().unwrap().to_string();
        let dest = dest_path.to_str().unwrap().to_string();

        let cwd = Some(root_path.clone().into());

        let checker = setup(&fake_cp);
        let exec_call = ExecCall {
            program: "cp".into(),
            args: vec![source, dest.clone()],
        };
        let valid_exec = match checker.r#match(&exec_call).map_err(|e| anyhow!("{e:?}"))? {
            MatchedExec::Match { exec } => exec,
            unexpected => panic!("Expected a safe exec but got {unexpected:?}"),
        };

        // No readable or writeable folders specified.
        assert_eq!(
            checker.check(valid_exec.clone(), &cwd, &[], &[]),
            Err(ReadablePathNotInReadableFolders {
                file: source_path,
                folders: vec![]
            }),
        );

        // Only readable folders specified.
        assert_eq!(
            checker.check(
                valid_exec.clone(),
                &cwd,
                std::slice::from_ref(&root_path),
                &[]
            ),
            Err(WriteablePathNotInWriteableFolders {
                file: dest_path.clone(),
                folders: vec![]
            }),
        );

        // Both readable and writeable folders specified.
        assert_eq!(
            checker.check(
                valid_exec,
                &cwd,
                std::slice::from_ref(&root_path),
                std::slice::from_ref(&root_path)
            ),
            Ok(cp.clone()),
        );

        // Args are the readable and writeable folders, not files within the
        // folders.
        let exec_call_folders_as_args = ExecCall {
            program: "cp".into(),
            args: vec![root.clone(), root],
        };
        let valid_exec_call_folders_as_args = match checker
            .r#match(&exec_call_folders_as_args)
            .map_err(|e| anyhow!("{e:?}"))?
        {
            MatchedExec::Match { exec } => exec,
            _ => panic!("Expected a safe exec"),
        };
        assert_eq!(
            checker.check(
                valid_exec_call_folders_as_args,
                &cwd,
                std::slice::from_ref(&root_path),
                std::slice::from_ref(&root_path)
            ),
            Ok(cp),
        );

        // Specify a parent of a readable folder as input.
        let exec_with_parent_of_readable_folder = ValidExec {
            program: "cp".into(),
            args: vec![
                MatchedArg::new(
                    0,
                    ArgType::ReadableFile,
                    root_path.parent().unwrap().to_str().unwrap(),
                )
                .map_err(|e| anyhow!("{e:?}"))?,
                MatchedArg::new(1, ArgType::WriteableFile, &dest).map_err(|e| anyhow!("{e:?}"))?,
            ],
            ..Default::default()
        };
        assert_eq!(
            checker.check(
                exec_with_parent_of_readable_folder,
                &cwd,
                std::slice::from_ref(&root_path),
                std::slice::from_ref(&dest_path)
            ),
            Err(ReadablePathNotInReadableFolders {
                file: root_path.parent().unwrap().to_path_buf(),
                folders: vec![root_path.clone()]
            }),
        );
        Ok(())
    }
}
