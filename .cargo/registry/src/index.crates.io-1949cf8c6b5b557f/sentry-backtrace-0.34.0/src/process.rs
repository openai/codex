use std::borrow::Cow;

use backtrace::Backtrace;
use sentry_core::ClientOptions;

use crate::trim::{is_sys_function, trim_stacktrace};
use crate::utils::{
    demangle_symbol, filename, function_starts_with, parse_crate_name, strip_symbol,
};
use crate::{Frame, Stacktrace};

/// Processes a `Stacktrace`.
///
/// Trims a `Stacktrace` and marks frames as in-app based on the provided
/// `ClientOptions`.
pub fn process_event_stacktrace(stacktrace: &mut Stacktrace, options: &ClientOptions) {
    // automatically trim backtraces
    if options.trim_backtraces {
        trim_stacktrace(stacktrace, |frame, _| {
            if let Some(ref func) = frame.function {
                options.extra_border_frames.contains(&func.as_str())
            } else {
                false
            }
        })
    }

    // automatically prime in_app and set package
    let mut any_in_app = false;
    for frame in &mut stacktrace.frames {
        let func_name = match frame.function {
            Some(ref func) => func,
            None => continue,
        };

        // set package if missing to crate prefix
        if frame.package.is_none() {
            frame.package = parse_crate_name(func_name);
        }

        match frame.in_app {
            Some(true) => {
                any_in_app = true;
                continue;
            }
            Some(false) => {
                continue;
            }
            None => {}
        }

        for m in &options.in_app_include {
            if function_starts_with(func_name, m) {
                frame.in_app = Some(true);
                any_in_app = true;
                break;
            }
        }

        if frame.in_app.is_some() {
            continue;
        }

        for m in &options.in_app_exclude {
            if function_starts_with(func_name, m) {
                frame.in_app = Some(false);
                break;
            }
        }

        if frame.in_app.is_some() {
            continue;
        }

        if is_sys_function(func_name) {
            frame.in_app = Some(false);
        }
    }

    if !any_in_app {
        for frame in &mut stacktrace.frames {
            if frame.in_app.is_none() {
                frame.in_app = Some(true);
            }
        }
    }
}

/// Convert a `backtrace::Backtrace` into a Rust `Stacktrace`
pub fn backtrace_to_stacktrace(bt: &Backtrace) -> Option<Stacktrace> {
    let frames = bt
        .frames()
        .iter()
        .flat_map(|frame| {
            // For each frame, there may be multiple symbols if a function was inlined, so
            // add an entry for each symbol.
            let symbols = frame.symbols();
            symbols
                .iter()
                .map(move |sym| {
                    let abs_path = sym.filename().map(|m| m.to_string_lossy().to_string());
                    let filename = abs_path.as_ref().map(|p| filename(p).to_string());
                    let real_symbol = sym
                        .name()
                        .map_or(Cow::Borrowed("<unknown>"), |n| Cow::Owned(n.to_string()));
                    let symbol = strip_symbol(&real_symbol);
                    let function = demangle_symbol(&symbol);
                    Frame {
                        symbol: if symbol != function {
                            Some(symbol.into())
                        } else {
                            None
                        },
                        function: Some(function),
                        instruction_addr: Some(frame.ip().into()),
                        abs_path,
                        filename,
                        lineno: sym.lineno().map(u64::from),
                        colno: None,
                        ..Default::default()
                    }

                    // If there were no symbols at all, make sure to add at least one frame, as we
                    // may be able to symbolicate it on the server.
                })
                .chain(if symbols.is_empty() {
                    Some(Frame {
                        instruction_addr: Some(frame.ip().into()),
                        function: Some("<unknown>".into()),
                        ..Default::default()
                    })
                } else {
                    None
                })
        })
        .collect();
    Stacktrace::from_frames_reversed(frames)
}
