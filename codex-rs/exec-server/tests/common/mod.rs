use std::env;
use std::path::PathBuf;

use codex_exec_server::CODEX_FS_HELPER_ARG1;
use codex_exec_server::ExecServerRuntimePaths;
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
use codex_test_binary_support::TestBinaryDispatchGuard;
use codex_test_binary_support::TestBinaryDispatchMode;
use codex_test_binary_support::configure_test_binary_dispatch;
use ctor::ctor;

pub(crate) mod exec_server;

#[ctor]
pub static TEST_BINARY_DISPATCH_GUARD: Option<TestBinaryDispatchGuard> = {
    let guard = configure_test_binary_dispatch("codex-exec-server-tests", |exe_name, argv1| {
        if argv1 == Some(CODEX_FS_HELPER_ARG1) {
            return TestBinaryDispatchMode::DispatchArg0Only;
        }
        if exe_name == CODEX_LINUX_SANDBOX_ARG0 {
            return TestBinaryDispatchMode::DispatchArg0Only;
        }
        TestBinaryDispatchMode::InstallAliases
    });
    maybe_run_exec_server_from_test_binary(guard.as_ref());
    guard
};

pub(crate) fn current_test_binary_helper_paths() -> anyhow::Result<(PathBuf, Option<PathBuf>)> {
    let current_exe = env::current_exe()?;
    let codex_linux_sandbox_exe = if cfg!(target_os = "linux") {
        TEST_BINARY_DISPATCH_GUARD
            .as_ref()
            .and_then(|guard| guard.paths().codex_linux_sandbox_exe.clone())
            .or_else(|| Some(current_exe.clone()))
    } else {
        None
    };
    Ok((current_exe, codex_linux_sandbox_exe))
}

fn maybe_run_exec_server_from_test_binary(guard: Option<&TestBinaryDispatchGuard>) {
    let mut args = env::args();
    let _program = args.next();
    let Some(command) = args.next() else {
        return;
    };
    if command != "exec-server" {
        return;
    }

    let Some(flag) = args.next() else {
        eprintln!("expected --listen");
        std::process::exit(1);
    };
    if flag != "--listen" {
        eprintln!("expected --listen, got `{flag}`");
        std::process::exit(1);
    }
    let Some(listen_url) = args.next() else {
        eprintln!("expected listen URL");
        std::process::exit(1);
    };
    if args.next().is_some() {
        eprintln!("unexpected extra arguments");
        std::process::exit(1);
    }

    let current_exe = match env::current_exe() {
        Ok(current_exe) => current_exe,
        Err(error) => {
            eprintln!("failed to resolve current test binary: {error}");
            std::process::exit(1);
        }
    };
    let runtime_paths = match ExecServerRuntimePaths::new(
        current_exe.clone(),
        linux_sandbox_exe(guard, &current_exe),
    ) {
        Ok(runtime_paths) => runtime_paths,
        Err(error) => {
            eprintln!("failed to configure exec-server runtime paths: {error}");
            std::process::exit(1);
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to build Tokio runtime: {error}");
            std::process::exit(1);
        }
    };
    let exit_code = match runtime.block_on(codex_exec_server::run_main(&listen_url, runtime_paths))
    {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("exec-server failed: {error}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn linux_sandbox_exe(
    guard: Option<&TestBinaryDispatchGuard>,
    current_exe: &std::path::Path,
) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        guard
            .and_then(|guard| guard.paths().codex_linux_sandbox_exe.clone())
            .or_else(|| Some(current_exe.to_path_buf()))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = guard;
        let _ = current_exe;
        None
    }
}
