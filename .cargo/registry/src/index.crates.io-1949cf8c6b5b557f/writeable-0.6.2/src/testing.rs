// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::*;
use alloc::string::String;
use alloc::vec::Vec;

pub(crate) struct TestWriter {
    pub(crate) string: String,
    pub(crate) parts: Vec<(usize, usize, Part)>,
}

impl TestWriter {
    pub(crate) fn finish(mut self) -> (String, Vec<(usize, usize, Part)>) {
        // Sort by first open and last closed
        self.parts
            .sort_unstable_by_key(|(begin, end, _)| (*begin, end.wrapping_neg()));
        (self.string, self.parts)
    }
}

impl fmt::Write for TestWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.string.write_str(s)
    }
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.string.write_char(c)
    }
}

impl PartsWrite for TestWriter {
    type SubPartsWrite = Self;
    fn with_part(
        &mut self,
        part: Part,
        mut f: impl FnMut(&mut Self::SubPartsWrite) -> fmt::Result,
    ) -> fmt::Result {
        let start = self.string.len();
        f(self)?;
        let end = self.string.len();
        if start < end {
            self.parts.push((start, end, part));
        }
        Ok(())
    }
}

pub fn writeable_to_parts_for_test<W: Writeable>(
    writeable: &W,
) -> (String, Vec<(usize, usize, Part)>) {
    let mut writer = TestWriter {
        string: alloc::string::String::new(),
        parts: Vec::new(),
    };
    #[expect(clippy::expect_used)] // for test code
    writeable
        .write_to_parts(&mut writer)
        .expect("String writer infallible");
    writer.finish()
}

#[expect(clippy::type_complexity)]
pub fn try_writeable_to_parts_for_test<W: TryWriteable>(
    writeable: &W,
) -> (String, Vec<(usize, usize, Part)>, Option<W::Error>) {
    let mut writer = TestWriter {
        string: alloc::string::String::new(),
        parts: Vec::new(),
    };
    #[expect(clippy::expect_used)] // for test code
    let result = writeable
        .try_write_to_parts(&mut writer)
        .expect("String writer infallible");
    let (actual_str, actual_parts) = writer.finish();
    (actual_str, actual_parts, result.err())
}
