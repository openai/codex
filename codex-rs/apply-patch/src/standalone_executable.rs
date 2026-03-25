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
    let mut preserve_crlf = false;

    let first_arg = args.next();
    let patch_arg = match first_arg {
        Some(arg) => match arg.into_string() {
            Ok(arg) if arg == crate::PRESERVE_CRLF_FLAG => {
                preserve_crlf = true;
                match args.next() {
                    Some(arg) => match arg.into_string() {
                        Ok(s) => s,
                        Err(_) => {
                            eprintln!("Error: apply_patch requires a UTF-8 PATCH argument.");
                            return 1;
                        }
                    },
                    None => {
                        // No patch argument after flag; attempt to read patch from stdin.
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
                }
            }
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
    let options = crate::ApplyPatchOptions { preserve_crlf };
    match crate::apply_patch_with_options(&patch_arg, options, &mut stdout, &mut stderr) {
        Ok(()) => {
            // Flush to ensure output ordering when used in pipelines.
            let _ = stdout.flush();
            0
        }
        Err(_) => 1,
    }
}
