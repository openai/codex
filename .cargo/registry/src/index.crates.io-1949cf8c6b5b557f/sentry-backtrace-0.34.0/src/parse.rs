use once_cell::sync::Lazy;
use regex::Regex;

use crate::utils::{demangle_symbol, filename, strip_symbol};
use crate::{Frame, Stacktrace};

static FRAME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?xm)
        ^
            \s*(?:\d+:)?\s*                      # frame number (missing for inline)

            (?:
                (?P<addr_old>0x[a-f0-9]+)        # old style address prefix
                \s-\s
            )?

            (?P<symbol>[^\r\n\(]+)               # symbol name

            (?:
                \s\((?P<addr_new>0x[a-f0-9]+)\)  # new style address in parens
            )?

            (?:
                \r?\n
                \s+at\s                          # padded "at" in new line
                (?P<path>[^\r\n]+?)              # path to source file
                (?::(?P<lineno>\d+))?            # optional source line
                (?::(?P<colno>\d+))?             # optional source column
            )?
        $
    "#,
    )
    .unwrap()
});

/// Parses a backtrace string into a Sentry `Stacktrace`.
pub fn parse_stacktrace(bt: &str) -> Option<Stacktrace> {
    let mut last_address = None;

    let frames = FRAME_RE
        .captures_iter(bt)
        .map(|captures| {
            let abs_path = captures.name("path").map(|m| m.as_str().to_string());
            let filename = abs_path.as_ref().map(|p| filename(p).to_string());
            let real_symbol = captures["symbol"].to_string();
            let symbol = strip_symbol(&real_symbol);
            let function = demangle_symbol(&symbol);

            // Obtain the instruction address. A missing address usually indicates an inlined stack
            // frame, in which case the previous address needs to be used.
            last_address = captures
                .name("addr_new")
                .or_else(|| captures.name("addr_old"))
                .and_then(|m| m.as_str().parse().ok())
                .or(last_address);

            Frame {
                symbol: if symbol != function {
                    Some(symbol.into())
                } else {
                    None
                },
                function: Some(function),
                instruction_addr: last_address,
                abs_path,
                filename,
                lineno: captures
                    .name("lineno")
                    .map(|x| x.as_str().parse::<u64>().unwrap()),
                colno: captures
                    .name("colno")
                    .map(|x| x.as_str().parse::<u64>().unwrap()),
                ..Default::default()
            }
        })
        .collect();

    Stacktrace::from_frames_reversed(frames)
}
