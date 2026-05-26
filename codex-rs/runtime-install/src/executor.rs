use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_exec_server::Environment;
use codex_exec_server::ExecBackend;
use codex_exec_server::ExecEnvPolicy;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcessEvent;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ProcessId;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::errors::internal_error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeArchiveFormat {
    TarXz,
    Zip,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetPlatform {
    Unix,
    Windows,
}

impl TargetPlatform {
    pub(crate) fn runtime_name(self) -> &'static str {
        match self {
            Self::Unix => "unix",
            Self::Windows => "win32",
        }
    }
}

pub(crate) struct InstallTarget {
    pub(crate) install_root: AbsolutePathBuf,
    pub(crate) platform: TargetPlatform,
}

/// Executes runtime installation operations in the selected environment through
/// the generic executor process and filesystem interfaces.
pub(crate) struct RuntimeExecutor {
    backend: Arc<dyn ExecBackend>,
    filesystem: Arc<dyn ExecutorFileSystem>,
    cwd: PathUri,
}

impl RuntimeExecutor {
    pub(crate) async fn new(environment: &Environment) -> Result<Self, JSONRPCErrorError> {
        let codex_home = environment.codex_home().await?;
        Ok(Self {
            backend: environment.get_exec_backend(),
            filesystem: environment.get_filesystem(),
            cwd: path_uri(&codex_home),
        })
    }

    pub(crate) fn filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        Arc::clone(&self.filesystem)
    }

    pub(crate) async fn discover_target(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<InstallTarget, JSONRPCErrorError> {
        let unix_output = self
            .run_command(
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "case \"$(uname -s)\" in CYGWIN*|MINGW*|MSYS*) exit 1;; esac; test -n \"$HOME\" || exit 1; printf 'unix\\n%s/.cache/codex-runtimes\\n' \"$HOME\"".to_string(),
                ],
                cancellation,
                "inspect Unix runtime install environment",
            )
            .await;
        if let Ok(output) = unix_output {
            return parse_install_target(&output);
        }
        ensure_not_cancelled(cancellation)?;

        let windows_output = self
            .run_command(
                vec![
                    "powershell".to_string(),
                    "-NoProfile".to_string(),
                    "-NonInteractive".to_string(),
                    "-Command".to_string(),
                    "if (-not $env:USERPROFILE) { exit 1 }; [Console]::Out.WriteLine('win32'); [Console]::Out.WriteLine([IO.Path]::Combine($env:USERPROFILE, '.cache', 'codex-runtimes'))".to_string(),
                ],
                cancellation,
                "inspect Windows runtime install environment",
            )
            .await?;
        parse_install_target(&windows_output)
    }

    pub(crate) async fn download_archive(
        &self,
        platform: TargetPlatform,
        url: &str,
        destination: &AbsolutePathBuf,
        cancellation: &CancellationToken,
    ) -> Result<(), JSONRPCErrorError> {
        let argv = match platform {
            TargetPlatform::Unix => vec![
                "curl".to_string(),
                "--fail".to_string(),
                "--location".to_string(),
                "--silent".to_string(),
                "--show-error".to_string(),
                "--output".to_string(),
                destination.display().to_string(),
                url.to_string(),
            ],
            TargetPlatform::Windows => vec![
                "powershell".to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                "$ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri $args[0] -OutFile $args[1]".to_string(),
                url.to_string(),
                destination.display().to_string(),
            ],
        };
        self.run_command(argv, cancellation, "download runtime archive")
            .await
            .map(|_| ())
    }

    pub(crate) async fn archive_checksum(
        &self,
        platform: TargetPlatform,
        archive_path: &AbsolutePathBuf,
        cancellation: &CancellationToken,
    ) -> Result<String, JSONRPCErrorError> {
        let argv = match platform {
            TargetPlatform::Unix => vec![
                "sh".to_string(),
                "-c".to_string(),
                "if command -v sha256sum >/dev/null 2>&1; then sha256sum \"$1\"; else shasum -a 256 \"$1\"; fi".to_string(),
                "runtime-checksum".to_string(),
                archive_path.display().to_string(),
            ],
            TargetPlatform::Windows => vec![
                "powershell".to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                "[Console]::Out.WriteLine((Get-FileHash -LiteralPath $args[0] -Algorithm SHA256).Hash)".to_string(),
                archive_path.display().to_string(),
            ],
        };
        let output = self
            .run_command(argv, cancellation, "checksum runtime archive")
            .await?;
        output
            .split_whitespace()
            .next()
            .map(str::to_string)
            .ok_or_else(|| internal_error("checksum runtime archive returned no digest"))
    }

    pub(crate) async fn list_archive_entries(
        &self,
        format: RuntimeArchiveFormat,
        platform: TargetPlatform,
        archive_path: &AbsolutePathBuf,
        cancellation: &CancellationToken,
    ) -> Result<Vec<String>, JSONRPCErrorError> {
        let argv = match (format, platform) {
            (RuntimeArchiveFormat::TarXz, _) => vec![
                "tar".to_string(),
                "-tf".to_string(),
                archive_path.display().to_string(),
            ],
            (RuntimeArchiveFormat::Zip, TargetPlatform::Unix) => vec![
                "unzip".to_string(),
                "-Z1".to_string(),
                archive_path.display().to_string(),
            ],
            (RuntimeArchiveFormat::Zip, TargetPlatform::Windows) => vec![
                "powershell".to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                "Add-Type -AssemblyName System.IO.Compression.FileSystem; $archive = [IO.Compression.ZipFile]::OpenRead($args[0]); try { $archive.Entries | ForEach-Object { [Console]::Out.WriteLine($_.FullName) } } finally { $archive.Dispose() }".to_string(),
                archive_path.display().to_string(),
            ],
        };
        let output = self
            .run_command(argv, cancellation, "list runtime archive")
            .await?;
        Ok(output
            .lines()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string)
            .collect())
    }

    pub(crate) async fn extract_archive(
        &self,
        format: RuntimeArchiveFormat,
        platform: TargetPlatform,
        archive_path: &AbsolutePathBuf,
        extract_dir: &AbsolutePathBuf,
        cancellation: &CancellationToken,
    ) -> Result<(), JSONRPCErrorError> {
        let argv = match (format, platform) {
            (RuntimeArchiveFormat::TarXz, _) => vec![
                "tar".to_string(),
                "-xJf".to_string(),
                archive_path.display().to_string(),
                "-C".to_string(),
                extract_dir.display().to_string(),
            ],
            (RuntimeArchiveFormat::Zip, TargetPlatform::Unix) => vec![
                "unzip".to_string(),
                "-q".to_string(),
                archive_path.display().to_string(),
                "-d".to_string(),
                extract_dir.display().to_string(),
            ],
            (RuntimeArchiveFormat::Zip, TargetPlatform::Windows) => vec![
                "powershell".to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                "Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force".to_string(),
                archive_path.display().to_string(),
                extract_dir.display().to_string(),
            ],
        };
        self.run_command(argv, cancellation, "extract runtime archive")
            .await
            .map(|_| ())
    }

    pub(crate) async fn move_directory(
        &self,
        platform: TargetPlatform,
        source: &AbsolutePathBuf,
        destination: &AbsolutePathBuf,
        cancellation: &CancellationToken,
    ) -> Result<(), JSONRPCErrorError> {
        let argv = match platform {
            TargetPlatform::Unix => vec![
                "mv".to_string(),
                source.display().to_string(),
                destination.display().to_string(),
            ],
            TargetPlatform::Windows => vec![
                "powershell".to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                "Move-Item -LiteralPath $args[0] -Destination $args[1]".to_string(),
                source.display().to_string(),
                destination.display().to_string(),
            ],
        };
        self.run_command(argv, cancellation, "move runtime directory")
            .await
            .map(|_| ())
    }

    async fn run_command(
        &self,
        argv: Vec<String>,
        cancellation: &CancellationToken,
        operation: &str,
    ) -> Result<String, JSONRPCErrorError> {
        ensure_not_cancelled(cancellation)?;
        let started = self
            .backend
            .start(ExecParams {
                process_id: ProcessId::from(format!("runtime-install-{}", Uuid::now_v7())),
                argv,
                cwd: self.cwd.clone(),
                env_policy: Some(ExecEnvPolicy {
                    inherit: ShellEnvironmentPolicyInherit::All,
                    ignore_default_excludes: true,
                    exclude: Vec::new(),
                    r#set: HashMap::new(),
                    include_only: Vec::new(),
                }),
                env: HashMap::new(),
                tty: false,
                pipe_stdin: false,
                arg0: None,
            })
            .await
            .map_err(|err| internal_error(format!("failed to {operation}: {err}")))?;
        let mut events = started.process.subscribe_events();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = None;
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    let _ = started.process.terminate().await;
                    return Err(runtime_install_canceled());
                }
                event = events.recv() => {
                    let event = event.map_err(|err| internal_error(format!("{operation} output stream failed: {err}")))?;
                    match event {
                        ExecProcessEvent::Output(chunk) => match chunk.stream {
                            ExecOutputStream::Stdout => stdout.extend_from_slice(&chunk.chunk.0),
                            ExecOutputStream::Stderr | ExecOutputStream::Pty => {
                                stderr.extend_from_slice(&chunk.chunk.0);
                            }
                        },
                        ExecProcessEvent::Exited { exit_code: code, .. } => exit_code = Some(code),
                        ExecProcessEvent::Closed { .. } => {
                            if exit_code == Some(0) {
                                return String::from_utf8(stdout).map_err(|err| {
                                    internal_error(format!("{operation} returned invalid UTF-8: {err}"))
                                });
                            }
                            return Err(internal_error(format!(
                                "{operation} failed (exit code {}): {}",
                                exit_code.unwrap_or(-1),
                                String::from_utf8_lossy(&stderr).trim()
                            )));
                        }
                        ExecProcessEvent::Failed(message) => {
                            return Err(internal_error(format!("{operation} process failed: {message}")));
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn path_uri(path: &AbsolutePathBuf) -> PathUri {
    PathUri::from_abs_path(path)
}

fn parse_install_target(output: &str) -> Result<InstallTarget, JSONRPCErrorError> {
    let mut lines = output.lines();
    let platform = match lines.next().map(str::trim) {
        Some("unix") => TargetPlatform::Unix,
        Some("win32") => TargetPlatform::Windows,
        _ => {
            return Err(internal_error(
                "runtime install environment returned invalid platform",
            ));
        }
    };
    let install_root = lines
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .ok_or_else(|| internal_error("runtime install environment returned no install root"))?;
    let install_root = AbsolutePathBuf::from_absolute_path_checked(PathBuf::from(install_root))
        .map_err(|err| internal_error(format!("runtime install root is not absolute: {err}")))?;
    Ok(InstallTarget {
        install_root,
        platform,
    })
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), JSONRPCErrorError> {
    if cancellation.is_cancelled() {
        Err(runtime_install_canceled())
    } else {
        Ok(())
    }
}

pub(crate) fn runtime_install_canceled() -> JSONRPCErrorError {
    internal_error("runtime install canceled")
}
