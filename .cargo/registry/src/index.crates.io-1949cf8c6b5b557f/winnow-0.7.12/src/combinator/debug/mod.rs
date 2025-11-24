#![cfg_attr(feature = "debug", allow(clippy::std_instead_of_core))]

#[cfg(feature = "debug")]
mod internals;

use crate::error::ParserError;
use crate::stream::Stream;
use crate::Parser;

/// Trace the execution of the parser
///
/// Note that [`Parser::context`] also provides high level trace information.
///
/// See [tutorial][crate::_tutorial::chapter_8] for more details.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::token::take_while;
/// # use winnow::stream::AsChar;
/// # use winnow::prelude::*;
/// use winnow::combinator::trace;
///
/// fn short_alpha<'s>(s: &mut &'s [u8]) -> ModalResult<&'s [u8]> {
///   trace("short_alpha",
///     take_while(3..=6, AsChar::is_alpha)
///   ).parse_next(s)
/// }
///
/// assert_eq!(short_alpha.parse_peek(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(short_alpha.parse_peek(b"lengthy"), Ok((&b"y"[..], &b"length"[..])));
/// assert_eq!(short_alpha.parse_peek(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert!(short_alpha.parse_peek(b"ed").is_err());
/// assert!(short_alpha.parse_peek(b"12345").is_err());
/// ```
#[cfg_attr(not(feature = "debug"), allow(unused_variables))]
#[cfg_attr(not(feature = "debug"), allow(unused_mut))]
#[cfg_attr(not(feature = "debug"), inline(always))]
pub fn trace<I: Stream, O, E: ParserError<I>>(
    name: impl crate::lib::std::fmt::Display,
    parser: impl Parser<I, O, E>,
) -> impl Parser<I, O, E> {
    #[cfg(feature = "debug")]
    {
        internals::Trace::new(parser, name)
    }
    #[cfg(not(feature = "debug"))]
    {
        parser
    }
}

#[cfg_attr(not(feature = "debug"), allow(unused_variables))]
pub(crate) fn trace_result<T, I: Stream, E: ParserError<I>>(
    name: impl crate::lib::std::fmt::Display,
    res: &Result<T, E>,
) {
    #[cfg(feature = "debug")]
    {
        let depth = internals::Depth::existing();
        let severity = internals::Severity::with_result(res);
        internals::result(*depth, &name, severity);
    }
}

pub(crate) struct DisplayDebug<D>(pub(crate) D);

impl<D: crate::lib::std::fmt::Debug> crate::lib::std::fmt::Display for DisplayDebug<D> {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[test]
#[cfg(feature = "std")]
#[cfg_attr(miri, ignore)]
#[cfg(unix)]
#[cfg(feature = "debug")]
fn example() {
    use term_transcript::{test::TestConfig, ShellOptions};

    let path = snapbox::cmd::compile_example("string", ["--features=debug"]).unwrap();

    let current_dir = path.parent().unwrap();
    let cmd = path.file_name().unwrap();
    // HACK: term_transcript doesn't allow non-UTF8 paths
    let cmd = format!("./{}", cmd.to_string_lossy());

    TestConfig::new(
        ShellOptions::default()
            .with_current_dir(current_dir)
            .with_env("CLICOLOR_FORCE", "1"),
    )
    .test("assets/trace.svg", [format!(r#"{cmd} '"abc"'"#).as_str()]);
}
