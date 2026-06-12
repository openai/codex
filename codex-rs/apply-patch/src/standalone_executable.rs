use std::io::Read;
use std::io::Write;

pub fn main() -> ! {
    let exit_code = run_main();
    std::process::exit(exit_code);
}

/// We would prefer to return `std::process::ExitCode`, but its `exit_process()`
/// method is still a nightly API and we want main() to return !.
pub fn run_main() -> i32 {
    // Expect either one argument (the full apply_patch payload), optionally prefixed
    // by --preserve-crlf, or read it from stdin.
    let mut args = std::env::args_os();
    let _argv0 = args.next();
    let first_arg = args.next();
    let (options, patch_arg) = match first_arg {
        Some(arg) if arg == crate::APPLY_PATCH_PRESERVE_CRLF_ARG => {
            (crate::ApplyPatchOptions::preserve_crlf(), args.next())
        }
        patch_arg => (crate::ApplyPatchOptions::default(), patch_arg),
    };

    let patch_arg = match patch_arg {
        Some(arg) => match arg.into_string() {
            Ok(s) => s,
            Err(_) => {
                eprintln!("Error: apply_patch requires a UTF-8 PATCH argument.");
                return 1;
            }
        },
        None => {
            // No argument provided; attempt to read the patch from stdin.
            let mut buf = String::new();
            match std::io::stdin().read_to_string(&mut buf) {
                Ok(_) => {
                    if buf.is_empty() {
                        eprintln!(
                            "Usage: apply_patch [--preserve-crlf] 'PATCH'\n       echo 'PATCH' | apply_patch [--preserve-crlf]"
                        );
                        return 2;
                    }
                    buf
                }
                Err(err) => {
                    eprintln!("Error: Failed to read PATCH from stdin.\n{err}");
                    return 1;
                }
            }
        }
    };

    // Refuse extra args to avoid ambiguity.
    if args.next().is_some() {
        eprintln!("Error: apply_patch accepts exactly one PATCH argument.");
        return 2;
    }

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let cwd = match codex_utils_absolute_path::AbsolutePathBuf::current_dir() {
        Ok(cwd) => cwd,
        Err(err) => {
            eprintln!("Error: Failed to determine current directory.\n{err}");
            return 1;
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("Error: Failed to initialize runtime.\n{err}");
            return 1;
        }
    };
    match runtime.block_on(crate::apply_patch_with_options(
        &patch_arg,
        options,
        &cwd,
        &mut stdout,
        &mut stderr,
        codex_exec_server::LOCAL_FS.as_ref(),
        /*sandbox*/ None,
    )) {
        Ok(_) => {
            // Flush to ensure output ordering when used in pipelines.
            let _ = stdout.flush();
            0
        }
        Err(_) => 1,
    }
}
