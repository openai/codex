use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_core::features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;

use crate::test_codex::TestCodex;
use crate::test_codex::test_codex;

#[derive(Clone)]
pub struct ZshForkRuntime {
    zsh_path: PathBuf,
    main_execve_wrapper_exe: PathBuf,
}

impl ZshForkRuntime {
    fn apply_to_config(
        &self,
        config: &mut Config,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
    ) {
        config.features.enable(Feature::ShellTool);
        config.features.enable(Feature::ShellZshFork);
        config.zsh_path = Some(self.zsh_path.clone());
        config.main_execve_wrapper_exe = Some(self.main_execve_wrapper_exe.clone());
        config.permissions.allow_login_shell = false;
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy);
    }
}

pub fn restrictive_workspace_write_policy() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

pub fn zsh_fork_runtime(test_name: &str) -> Result<Option<ZshForkRuntime>> {
    let Some(zsh_path) = find_test_zsh_path()? else {
        return Ok(None);
    };
    if !supports_exec_wrapper_intercept(&zsh_path) {
        eprintln!(
            "skipping {test_name}: zsh does not support EXEC_WRAPPER intercepts ({})",
            zsh_path.display()
        );
        return Ok(None);
    }
    let Ok(main_execve_wrapper_exe) = codex_utils_cargo_bin::cargo_bin("codex-execve-wrapper")
    else {
        eprintln!("skipping {test_name}: unable to resolve `codex-execve-wrapper` binary");
        return Ok(None);
    };

    Ok(Some(ZshForkRuntime {
        zsh_path,
        main_execve_wrapper_exe,
    }))
}

pub async fn build_zsh_fork_test<F>(
    server: &wiremock::MockServer,
    runtime: ZshForkRuntime,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    pre_build_hook: F,
) -> Result<TestCodex>
where
    F: FnOnce(&Path) + Send + 'static,
{
    let mut builder = test_codex()
        .with_pre_build_hook(pre_build_hook)
        .with_config(move |config| {
            runtime.apply_to_config(config, approval_policy, sandbox_policy);
        });
    builder.build(server).await
}

fn find_test_zsh_path() -> Result<Option<PathBuf>> {
    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let dotslash_zsh = repo_root.join("codex-rs/app-server/tests/suite/zsh");
    if !dotslash_zsh.is_file() {
        eprintln!(
            "skipping zsh-fork test: shared zsh DotSlash file not found at {}",
            dotslash_zsh.display()
        );
        return Ok(None);
    }

    match crate::fetch_dotslash_file(&dotslash_zsh, None) {
        Ok(path) => Ok(Some(path)),
        Err(error) => {
            eprintln!("skipping zsh-fork test: failed to fetch zsh via dotslash: {error:#}");
            Ok(None)
        }
    }
}

fn supports_exec_wrapper_intercept(zsh_path: &Path) -> bool {
    let status = std::process::Command::new(zsh_path)
        .arg("-fc")
        .arg("/usr/bin/true")
        .env("EXEC_WRAPPER", "/usr/bin/false")
        .status();
    match status {
        Ok(status) => !status.success(),
        Err(_) => false,
    }
}
