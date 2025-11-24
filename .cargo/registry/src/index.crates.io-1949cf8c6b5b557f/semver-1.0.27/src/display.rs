use crate::{BuildMetadata, Comparator, Op, Prerelease, Version, VersionReq};
use core::fmt::{self, Alignment, Debug, Display, Write};

impl Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let do_display = |formatter: &mut fmt::Formatter| -> fmt::Result {
            write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
            if !self.pre.is_empty() {
                write!(formatter, "-{}", self.pre)?;
            }
            if !self.build.is_empty() {
                write!(formatter, "+{}", self.build)?;
            }
            Ok(())
        };

        let do_len = || -> usize {
            digits(self.major)
                + 1
                + digits(self.minor)
                + 1
                + digits(self.patch)
                + !self.pre.is_empty() as usize
                + self.pre.len()
                + !self.build.is_empty() as usize
                + self.build.len()
        };

        pad(formatter, do_display, do_len)
    }
}

impl Display for VersionReq {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if self.comparators.is_empty() {
            return formatter.write_str("*");
        }
        for (i, comparator) in self.comparators.iter().enumerate() {
            if i > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{}", comparator)?;
        }
        Ok(())
    }
}

impl Display for Comparator {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let op = match self.op {
            Op::Exact => "=",
            Op::Greater => ">",
            Op::GreaterEq => ">=",
            Op::Less => "<",
            Op::LessEq => "<=",
            Op::Tilde => "~",
            Op::Caret => "^",
            Op::Wildcard => "",
        };
        formatter.write_str(op)?;
        write!(formatter, "{}", self.major)?;
        if let Some(minor) = &self.minor {
            write!(formatter, ".{}", minor)?;
            if let Some(patch) = &self.patch {
                write!(formatter, ".{}", patch)?;
                if !self.pre.is_empty() {
                    write!(formatter, "-{}", self.pre)?;
                }
            } else if self.op == Op::Wildcard {
                formatter.write_str(".*")?;
            }
        } else if self.op == Op::Wildcard {
            formatter.write_str(".*")?;
        }
        Ok(())
    }
}

impl Display for Prerelease {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Display for BuildMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Debug for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = formatter.debug_struct("Version");
        debug
            .field("major", &self.major)
            .field("minor", &self.minor)
            .field("patch", &self.patch);
        if !self.pre.is_empty() {
            debug.field("pre", &self.pre);
        }
        if !self.build.is_empty() {
            debug.field("build", &self.build);
        }
        debug.finish()
    }
}

impl Debug for Prerelease {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Prerelease(\"{}\")", self)
    }
}

impl Debug for BuildMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "BuildMetadata(\"{}\")", self)
    }
}

fn pad(
    formatter: &mut fmt::Formatter,
    do_display: impl FnOnce(&mut fmt::Formatter) -> fmt::Result,
    do_len: impl FnOnce() -> usize,
) -> fmt::Result {
    let min_width = match formatter.width() {
        Some(min_width) => min_width,
        None => return do_display(formatter),
    };

    let len = do_len();
    if len >= min_width {
        return do_display(formatter);
    }

    let default_align = Alignment::Left;
    let align = formatter.align().unwrap_or(default_align);
    let padding = min_width - len;
    let (pre_pad, post_pad) = match align {
        Alignment::Left => (0, padding),
        Alignment::Right => (padding, 0),
        Alignment::Center => (padding / 2, (padding + 1) / 2),
    };

    let fill = formatter.fill();
    for _ in 0..pre_pad {
        formatter.write_char(fill)?;
    }

    do_display(formatter)?;

    for _ in 0..post_pad {
        formatter.write_char(fill)?;
    }
    Ok(())
}

fn digits(val: u64) -> usize {
    if val < 10 {
        1
    } else {
        1 + digits(val / 10)
    }
}
