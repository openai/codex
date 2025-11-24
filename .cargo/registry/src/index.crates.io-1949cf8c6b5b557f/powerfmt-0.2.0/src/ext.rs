//! Extension traits.

use core::fmt::{Alignment, Arguments, Formatter, Result, Write};

mod sealed {
    pub trait Sealed {}

    impl Sealed for core::fmt::Formatter<'_> {}
}

/// An extension trait for [`core::fmt::Formatter`].
pub trait FormatterExt: sealed::Sealed {
    /// Writes the given arguments to the formatter, padding them with the given width. If `width`
    /// is incorrect, the resulting output will not be the requested width.
    fn pad_with_width(&mut self, width: usize, args: Arguments<'_>) -> Result;
}

impl FormatterExt for Formatter<'_> {
    fn pad_with_width(&mut self, args_width: usize, args: Arguments<'_>) -> Result {
        let Some(final_width) = self.width() else {
            // The caller has not requested a width. Write the arguments as-is.
            return self.write_fmt(args);
        };
        let Some(fill_width @ 1..) = final_width.checked_sub(args_width) else {
            // No padding will be present. Write the arguments as-is.
            return self.write_fmt(args);
        };

        let alignment = self.align().unwrap_or(Alignment::Left);
        let fill = self.fill();

        let left_fill_width = match alignment {
            Alignment::Left => 0,
            Alignment::Right => fill_width,
            Alignment::Center => fill_width / 2,
        };
        let right_fill_width = match alignment {
            Alignment::Left => fill_width,
            Alignment::Right => 0,
            // When the fill is not even on both sides, the extra fill goes on the right.
            Alignment::Center => (fill_width + 1) / 2,
        };

        for _ in 0..left_fill_width {
            self.write_char(fill)?;
        }
        self.write_fmt(args)?;
        for _ in 0..right_fill_width {
            self.write_char(fill)?;
        }

        Ok(())
    }
}
