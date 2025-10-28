use std::io::Read;
use std::io::Write;

pub fn main() -> ! {
    let exit_code = run_main();
    std::process::exit(exit_code);
}

/// We would prefer to return `std::process::ExitCode`, but its `exit_process()`
/// method is still a nightly API and we want main() to return !.
pub fn run_main() -> i32 {
    // Allow optional flags, then a single positional PATCH argument; otherwise read from stdin.
    let args = std::env::args().skip(1).collect::<Vec<String>>();
    let mut patch_arg: Option<String> = None;

    let mut i = 0usize;
    while i < args.len() {
        let a = &args[i];
        if a == "--assume-eol" {
            if i + 1 >= args.len() {
                eprintln!("Error: --assume-eol requires a value (lf|crlf|git|detect).");
                return 2;
            }
            let v = args[i + 1].clone();
            match crate::parse_assume_eol(&v) {
                Some(sel) => crate::set_assume_eol(sel),
                None => {
                    eprintln!("Error: invalid --assume-eol value: {v}");
                    return 2;
                }
            }
            i += 2;
            continue;
        } else if let Some(rest) = a.strip_prefix("--assume-eol=") {
            match crate::parse_assume_eol(rest) {
                Some(sel) => crate::set_assume_eol(sel),
                None => {
                    eprintln!("Error: invalid --assume-eol value: {rest}");
                    return 2;
                }
            }
            i += 1;
            continue;
        } else if a.starts_with("--") {
            eprintln!("Error: unrecognized option: {a}");
            return 2;
        } else {
            if patch_arg.is_some() {
                eprintln!("Error: apply_patch accepts a single PATCH argument.");
                return 2;
            }
            patch_arg = Some(a.clone());
            i += 1;
            continue;
        }
    }

    let patch_arg = match patch_arg {
        Some(s) => s,
        None => {
            // No positional provided; attempt to read the patch from stdin.
            let mut buf = String::new();
            match std::io::stdin().read_to_string(&mut buf) {
                Ok(_) => {
                    if buf.is_empty() {
                        eprintln!(
                            "Usage: apply_patch [--assume-eol=lf|crlf|git|detect] 'PATCH'\n       echo 'PATCH' | apply-patch"
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

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    match crate::apply_patch(&patch_arg, &mut stdout, &mut stderr) {
        Ok(()) => {
            // Flush to ensure output ordering when used in pipelines.
            let _ = stdout.flush();
            0
        }
        Err(_) => 1,
    }
}
