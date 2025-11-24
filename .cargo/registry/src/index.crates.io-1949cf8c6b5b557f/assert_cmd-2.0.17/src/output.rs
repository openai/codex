//! Simplify one-off runs of programs.

use bstr::ByteSlice;
use std::error::Error;
use std::fmt;
use std::process;

/// Converts a type to an [`OutputResult`].
///
/// This is for example implemented on [`std::process::Output`].
///
/// # Examples
///
/// ```rust
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// let result = Command::new("echo")
///     .args(&["42"])
///     .ok();
/// assert!(result.is_ok());
/// ```
///
pub trait OutputOkExt
where
    Self: ::std::marker::Sized,
{
    /// Convert an [`Output`] to an [`OutputResult`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// let result = Command::new("echo")
    ///     .args(&["42"])
    ///     .ok();
    /// assert!(result.is_ok());
    /// ```
    ///
    /// [`Output`]: std::process::Output
    fn ok(self) -> OutputResult;

    /// Unwrap a [`Output`] but with a prettier message than `.ok().unwrap()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// let output = Command::new("echo")
    ///     .args(&["42"])
    ///     .unwrap();
    /// ```
    ///
    /// [`Output`]: std::process::Output
    fn unwrap(self) -> process::Output {
        match self.ok() {
            Ok(output) => output,
            Err(err) => panic!("{}", err),
        }
    }

    /// Unwrap a [`Output`] but with a prettier message than `ok().err().unwrap()`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// let err = Command::new("a-command")
    ///     .args(&["--will-fail"])
    ///     .unwrap_err();
    /// ```
    ///
    /// [`Output`]: std::process::Output
    fn unwrap_err(self) -> OutputError {
        match self.ok() {
            Ok(output) => panic!(
                "Command completed successfully\nstdout=```{}```",
                DebugBytes::new(&output.stdout)
            ),
            Err(err) => err,
        }
    }
}

impl OutputOkExt for process::Output {
    fn ok(self) -> OutputResult {
        if self.status.success() {
            Ok(self)
        } else {
            let error = OutputError::new(self);
            Err(error)
        }
    }
}

impl OutputOkExt for &mut process::Command {
    fn ok(self) -> OutputResult {
        let output = self.output().map_err(OutputError::with_cause)?;
        if output.status.success() {
            Ok(output)
        } else {
            let error = OutputError::new(output).set_cmd(format!("{self:?}"));
            Err(error)
        }
    }

    fn unwrap_err(self) -> OutputError {
        match self.ok() {
            Ok(output) => panic!(
                "Completed successfully:\ncommand=`{:?}`\nstdout=```{}```",
                self,
                DebugBytes::new(&output.stdout)
            ),
            Err(err) => err,
        }
    }
}

/// [`Output`] represented as a [`Result`].
///
/// Generally produced by [`OutputOkExt`].
///
/// # Examples
///
/// ```rust
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// let result = Command::new("echo")
///     .args(&["42"])
///     .ok();
/// assert!(result.is_ok());
/// ```
///
/// [`Output`]: std::process::Output
/// [`Result`]: std::result::Result
pub type OutputResult = Result<process::Output, OutputError>;

/// [`Command`] error.
///
/// Generally produced by [`OutputOkExt`].
///
/// # Examples
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// let err = Command::new("a-command")
///     .args(&["--will-fail"])
///     .unwrap_err();
/// ```
///
/// [`Command`]: std::process::Command
#[derive(Debug)]
pub struct OutputError {
    cmd: Option<String>,
    stdin: Option<bstr::BString>,
    cause: OutputCause,
}

impl OutputError {
    /// Convert [`Output`] into an [`Error`].
    ///
    /// [`Output`]: std::process::Output
    /// [`Error`]: std::error::Error
    pub fn new(output: process::Output) -> Self {
        Self {
            cmd: None,
            stdin: None,
            cause: OutputCause::Expected(Output { output }),
        }
    }

    /// For errors that happen in creating a [`Output`].
    ///
    /// [`Output`]: std::process::Output
    pub fn with_cause<E>(cause: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            cmd: None,
            stdin: None,
            cause: OutputCause::Unexpected(Box::new(cause)),
        }
    }

    /// Add the command line for additional context.
    pub fn set_cmd(mut self, cmd: String) -> Self {
        self.cmd = Some(cmd);
        self
    }

    /// Add the `stdin` for additional context.
    pub fn set_stdin(mut self, stdin: Vec<u8>) -> Self {
        self.stdin = Some(bstr::BString::from(stdin));
        self
    }

    /// Access the contained [`Output`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// let err = Command::new("a-command")
    ///     .args(&["--will-fail"])
    ///     .unwrap_err();
    /// let output = err
    ///     .as_output()
    ///     .unwrap();
    /// assert_eq!(Some(42), output.status.code());
    /// ```
    ///
    /// [`Output`]: std::process::Output
    pub fn as_output(&self) -> Option<&process::Output> {
        match self.cause {
            OutputCause::Expected(ref e) => Some(&e.output),
            OutputCause::Unexpected(_) => None,
        }
    }
}

impl Error for OutputError {}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::color();
        if let Some(ref cmd) = self.cmd {
            writeln!(f, "{:#}={:#}", palette.key("command"), palette.value(cmd))?;
        }
        if let Some(ref stdin) = self.stdin {
            writeln!(
                f,
                "{:#}={:#}",
                palette.key("stdin"),
                palette.value(DebugBytes::new(stdin))
            )?;
        }
        write!(f, "{:#}", self.cause)
    }
}

#[derive(Debug)]
enum OutputCause {
    Expected(Output),
    Unexpected(Box<dyn Error + Send + Sync + 'static>),
}

impl fmt::Display for OutputCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            OutputCause::Expected(ref e) => write!(f, "{e:#}"),
            OutputCause::Unexpected(ref e) => write!(f, "{e:#}"),
        }
    }
}

#[derive(Debug)]
struct Output {
    output: process::Output,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        output_fmt(&self.output, f)
    }
}

pub(crate) fn output_fmt(output: &process::Output, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let palette = crate::Palette::color();
    if let Some(code) = output.status.code() {
        writeln!(f, "{:#}={:#}", palette.key("code"), palette.value(code))?;
    } else {
        writeln!(
            f,
            "{:#}={:#}",
            palette.key("code"),
            palette.value("<interrupted>")
        )?;
    }

    write!(
        f,
        "{:#}={:#}\n{:#}={:#}\n",
        palette.key("stdout"),
        palette.value(DebugBytes::new(&output.stdout)),
        palette.key("stderr"),
        palette.value(DebugBytes::new(&output.stderr)),
    )?;
    Ok(())
}

#[derive(Debug)]
pub(crate) struct DebugBytes<'a> {
    bytes: &'a [u8],
}

impl<'a> DebugBytes<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        DebugBytes { bytes }
    }
}

impl fmt::Display for DebugBytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_bytes(self.bytes, f)
    }
}

#[derive(Debug)]
pub(crate) struct DebugBuffer {
    buffer: bstr::BString,
}

impl DebugBuffer {
    pub(crate) fn new(buffer: Vec<u8>) -> Self {
        DebugBuffer {
            buffer: buffer.into(),
        }
    }
}

impl fmt::Display for DebugBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_bytes(&self.buffer, f)
    }
}

fn format_bytes(data: &[u8], f: &mut impl fmt::Write) -> fmt::Result {
    #![allow(clippy::assertions_on_constants)]

    const LINES_MIN_OVERFLOW: usize = 80;
    const LINES_MAX_START: usize = 20;
    const LINES_MAX_END: usize = 40;
    const LINES_MAX_PRINTED: usize = LINES_MAX_START + LINES_MAX_END;

    const BYTES_MIN_OVERFLOW: usize = 8192;
    const BYTES_MAX_START: usize = 2048;
    const BYTES_MAX_END: usize = 2048;
    const BYTES_MAX_PRINTED: usize = BYTES_MAX_START + BYTES_MAX_END;

    assert!(LINES_MAX_PRINTED < LINES_MIN_OVERFLOW);
    assert!(BYTES_MAX_PRINTED < BYTES_MIN_OVERFLOW);

    let lines_total = data.as_bstr().lines_with_terminator().count();
    let multiline = 1 < lines_total;

    if LINES_MIN_OVERFLOW <= lines_total {
        let lines_omitted = lines_total - LINES_MAX_PRINTED;
        let start_lines = data.as_bstr().lines_with_terminator().take(LINES_MAX_START);
        let end_lines = data
            .as_bstr()
            .lines_with_terminator()
            .skip(LINES_MAX_START + lines_omitted);
        writeln!(f, "<{lines_total} lines total>")?;
        write_debug_bstrs(f, true, start_lines)?;
        writeln!(f, "<{lines_omitted} lines omitted>")?;
        write_debug_bstrs(f, true, end_lines)
    } else if BYTES_MIN_OVERFLOW <= data.len() {
        write!(
            f,
            "<{} bytes total>{}",
            data.len(),
            if multiline { "\n" } else { "" }
        )?;
        write_debug_bstrs(
            f,
            multiline,
            data[..BYTES_MAX_START].lines_with_terminator(),
        )?;
        write!(
            f,
            "<{} bytes omitted>{}",
            data.len() - BYTES_MAX_PRINTED,
            if multiline { "\n" } else { "" }
        )?;
        write_debug_bstrs(
            f,
            multiline,
            data[data.len() - BYTES_MAX_END..].lines_with_terminator(),
        )
    } else {
        write_debug_bstrs(f, multiline, data.lines_with_terminator())
    }
}

fn write_debug_bstrs<'a>(
    f: &mut impl fmt::Write,
    multiline: bool,
    mut lines: impl Iterator<Item = &'a [u8]>,
) -> fmt::Result {
    if multiline {
        writeln!(f, "```")?;
        for mut line in lines {
            let mut newline = false;
            if line.last() == Some(&b'\n') {
                line = &line[..line.len() - 1];
                newline = true;
            }
            let s = format!("{:?}", line.as_bstr());
            write!(
                f,
                "{}{}",
                &s[1..s.len() - 1],
                if newline { "\n" } else { "" }
            )?;
        }
        writeln!(f, "```")
    } else {
        write!(f, "{:?}", lines.next().unwrap_or(&[]).as_bstr())
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn format_bytes() {
        let mut s = String::new();
        for i in 0..80 {
            s.push_str(&format!("{i}\n"));
        }

        let mut buf = String::new();
        super::format_bytes(s.as_bytes(), &mut buf).unwrap();

        assert_eq!(
            "<80 lines total>
```
0
1
2
3
4
5
6
7
8
9
10
11
12
13
14
15
16
17
18
19
```
<20 lines omitted>
```
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62
63
64
65
66
67
68
69
70
71
72
73
74
75
76
77
78
79
```
",
            buf
        );
    }

    #[test]
    fn no_trailing_newline() {
        let s = "no\ntrailing\nnewline";

        let mut buf = String::new();
        super::format_bytes(s.as_bytes(), &mut buf).unwrap();

        assert_eq!(
            "```
no
trailing
newline```
",
            buf
        );
    }
}
