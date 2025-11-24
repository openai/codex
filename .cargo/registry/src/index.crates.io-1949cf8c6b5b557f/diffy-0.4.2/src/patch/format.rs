use super::{Hunk, Line, Patch, NO_NEWLINE_AT_EOF};
use nu_ansi_term::{Color, Style};
use std::{
    fmt::{Display, Formatter, Result},
    io,
};

/// Struct used to adjust the formatting of a `Patch`
#[derive(Debug)]
pub struct PatchFormatter {
    with_color: bool,
    with_missing_newline_message: bool,
    suppress_blank_empty: bool,

    context: Style,
    delete: Style,
    insert: Style,
    hunk_header: Style,
    patch_header: Style,
    function_context: Style,
}

impl PatchFormatter {
    /// Construct a new formatter
    pub fn new() -> Self {
        Self {
            with_color: false,
            with_missing_newline_message: true,

            // TODO the default in git-diff and GNU diff is to have this set to false, on the next
            // semver breaking release we should contemplate switching this to be false by default
            suppress_blank_empty: true,

            context: Style::new(),
            delete: Color::Red.normal(),
            insert: Color::Green.normal(),
            hunk_header: Color::Cyan.normal(),
            patch_header: Style::new().bold(),
            function_context: Style::new(),
        }
    }

    /// Enable formatting a patch with color
    pub fn with_color(mut self) -> Self {
        self.with_color = true;
        self
    }

    /// Sets whether to format a patch with a "No newline at end of file" message.
    ///
    /// Default is `true`.
    ///
    /// Note: If this is disabled by setting to `false`, formatted patches will no longer contain
    /// sufficient information to determine if a file ended with a newline character (`\n`) or not
    /// and the patch will be formatted as if both the original and modified files ended with a
    /// newline character (`\n`).
    pub fn missing_newline_message(mut self, enable: bool) -> Self {
        self.with_missing_newline_message = enable;
        self
    }

    /// Sets whether to suppress printing of a space before empty lines.
    ///
    /// Defaults to `true`.
    ///
    /// For more information you can refer to the [Omitting trailing blanks] manual page of GNU
    /// diff or the [diff.suppressBlankEmpty] config for `git-diff`.
    ///
    /// [Omitting trailing blanks]: https://www.gnu.org/software/diffutils/manual/html_node/Trailing-Blanks.html
    /// [diff.suppressBlankEmpty]: https://git-scm.com/docs/git-diff#Documentation/git-diff.txt-codediffsuppressBlankEmptycode
    pub fn suppress_blank_empty(mut self, enable: bool) -> Self {
        self.suppress_blank_empty = enable;
        self
    }

    /// Returns a `Display` impl which can be used to print a Patch
    pub fn fmt_patch<'a>(&'a self, patch: &'a Patch<'a, str>) -> impl Display + 'a {
        PatchDisplay { f: self, patch }
    }

    pub fn write_patch_into<T: ToOwned + AsRef<[u8]> + ?Sized, W: io::Write>(
        &self,
        patch: &Patch<'_, T>,
        w: W,
    ) -> io::Result<()> {
        PatchDisplay { f: self, patch }.write_into(w)
    }

    fn fmt_hunk<'a>(&'a self, hunk: &'a Hunk<'a, str>) -> impl Display + 'a {
        HunkDisplay { f: self, hunk }
    }

    fn write_hunk_into<T: AsRef<[u8]> + ?Sized, W: io::Write>(
        &self,
        hunk: &Hunk<'_, T>,
        w: W,
    ) -> io::Result<()> {
        HunkDisplay { f: self, hunk }.write_into(w)
    }

    fn fmt_line<'a>(&'a self, line: &'a Line<'a, str>) -> impl Display + 'a {
        LineDisplay { f: self, line }
    }

    fn write_line_into<T: AsRef<[u8]> + ?Sized, W: io::Write>(
        &self,
        line: &Line<'_, T>,
        w: W,
    ) -> io::Result<()> {
        LineDisplay { f: self, line }.write_into(w)
    }
}

impl Default for PatchFormatter {
    fn default() -> Self {
        Self::new()
    }
}

struct PatchDisplay<'a, T: ToOwned + ?Sized> {
    f: &'a PatchFormatter,
    patch: &'a Patch<'a, T>,
}

impl<T: ToOwned + AsRef<[u8]> + ?Sized> PatchDisplay<'_, T> {
    fn write_into<W: io::Write>(&self, mut w: W) -> io::Result<()> {
        if self.patch.original.is_some() || self.patch.modified.is_some() {
            if self.f.with_color {
                write!(w, "{}", self.f.patch_header.prefix())?;
            }
            if let Some(original) = &self.patch.original {
                write!(w, "--- ")?;
                original.write_into(&mut w)?;
                writeln!(w)?;
            }
            if let Some(modified) = &self.patch.modified {
                write!(w, "+++ ")?;
                modified.write_into(&mut w)?;
                writeln!(w)?;
            }
            if self.f.with_color {
                write!(w, "{}", self.f.patch_header.suffix())?;
            }
        }

        for hunk in &self.patch.hunks {
            self.f.write_hunk_into(hunk, &mut w)?;
        }

        Ok(())
    }
}

impl Display for PatchDisplay<'_, str> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if self.patch.original.is_some() || self.patch.modified.is_some() {
            if self.f.with_color {
                write!(f, "{}", self.f.patch_header.prefix())?;
            }
            if let Some(original) = &self.patch.original {
                writeln!(f, "--- {}", original)?;
            }
            if let Some(modified) = &self.patch.modified {
                writeln!(f, "+++ {}", modified)?;
            }
            if self.f.with_color {
                write!(f, "{}", self.f.patch_header.suffix())?;
            }
        }

        for hunk in &self.patch.hunks {
            write!(f, "{}", self.f.fmt_hunk(hunk))?;
        }

        Ok(())
    }
}

struct HunkDisplay<'a, T: ?Sized> {
    f: &'a PatchFormatter,
    hunk: &'a Hunk<'a, T>,
}

impl<T: AsRef<[u8]> + ?Sized> HunkDisplay<'_, T> {
    fn write_into<W: io::Write>(&self, mut w: W) -> io::Result<()> {
        if self.f.with_color {
            write!(w, "{}", self.f.hunk_header.prefix())?;
        }
        write!(w, "@@ -{} +{} @@", self.hunk.old_range, self.hunk.new_range)?;
        if self.f.with_color {
            write!(w, "{}", self.f.hunk_header.suffix())?;
        }

        if let Some(ctx) = self.hunk.function_context {
            write!(w, " ")?;
            if self.f.with_color {
                write!(w, "{}", self.f.function_context.prefix())?;
            }
            write!(w, " ")?;
            w.write_all(ctx.as_ref())?;
            if self.f.with_color {
                write!(w, "{}", self.f.function_context.suffix())?;
            }
        }
        writeln!(w)?;

        for line in &self.hunk.lines {
            self.f.write_line_into(line, &mut w)?;
        }

        Ok(())
    }
}

impl Display for HunkDisplay<'_, str> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if self.f.with_color {
            write!(f, "{}", self.f.hunk_header.prefix())?;
        }
        write!(f, "@@ -{} +{} @@", self.hunk.old_range, self.hunk.new_range)?;
        if self.f.with_color {
            write!(f, "{}", self.f.hunk_header.suffix())?;
        }

        if let Some(ctx) = self.hunk.function_context {
            write!(f, " ")?;
            if self.f.with_color {
                write!(f, "{}", self.f.function_context.prefix())?;
            }
            write!(f, " {}", ctx)?;
            if self.f.with_color {
                write!(f, "{}", self.f.function_context.suffix())?;
            }
        }
        writeln!(f)?;

        for line in &self.hunk.lines {
            write!(f, "{}", self.f.fmt_line(line))?;
        }

        Ok(())
    }
}

struct LineDisplay<'a, T: ?Sized> {
    f: &'a PatchFormatter,
    line: &'a Line<'a, T>,
}

impl<T: AsRef<[u8]> + ?Sized> LineDisplay<'_, T> {
    fn write_into<W: io::Write>(&self, mut w: W) -> io::Result<()> {
        let (sign, line, style) = match self.line {
            Line::Context(line) => (' ', line.as_ref(), self.f.context),
            Line::Delete(line) => ('-', line.as_ref(), self.f.delete),
            Line::Insert(line) => ('+', line.as_ref(), self.f.insert),
        };

        if self.f.with_color {
            write!(w, "{}", style.prefix())?;
        }

        if self.f.suppress_blank_empty && sign == ' ' && line == b"\n" {
            w.write_all(line)?;
        } else {
            write!(w, "{}", sign)?;
            w.write_all(line)?;
        }

        if self.f.with_color {
            write!(w, "{}", style.suffix())?;
        }

        if !line.ends_with(b"\n") {
            writeln!(w)?;
            if self.f.with_missing_newline_message {
                writeln!(w, "{}", NO_NEWLINE_AT_EOF)?;
            }
        }

        Ok(())
    }
}

impl Display for LineDisplay<'_, str> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let (sign, line, style) = match self.line {
            Line::Context(line) => (' ', line, self.f.context),
            Line::Delete(line) => ('-', line, self.f.delete),
            Line::Insert(line) => ('+', line, self.f.insert),
        };

        if self.f.with_color {
            write!(f, "{}", style.prefix())?;
        }

        if self.f.suppress_blank_empty && sign == ' ' && *line == "\n" {
            write!(f, "{}", line)?;
        } else {
            write!(f, "{}{}", sign, line)?;
        }

        if self.f.with_color {
            write!(f, "{}", style.suffix())?;
        }

        if !line.ends_with('\n') {
            writeln!(f)?;
            if self.f.with_missing_newline_message {
                writeln!(f, "{}", NO_NEWLINE_AT_EOF)?;
            }
        }

        Ok(())
    }
}
