/*!
Utilities for working with I/O using byte strings.

This module currently only exports a single trait, `BufReadExt`, which provides
facilities for conveniently and efficiently working with lines as byte strings.

More APIs may be added in the future.
*/

use alloc::{vec, vec::Vec};

use std::io;

use crate::{ext_slice::ByteSlice, ext_vec::ByteVec};

/// An extension trait for
/// [`std::io::BufRead`](https://doc.rust-lang.org/std/io/trait.BufRead.html)
/// which provides convenience APIs for dealing with byte strings.
pub trait BufReadExt: io::BufRead {
    /// Returns an iterator over the lines of this reader, where each line
    /// is represented as a byte string.
    ///
    /// Each item yielded by this iterator is a `io::Result<Vec<u8>>`, where
    /// an error is yielded if there was a problem reading from the underlying
    /// reader.
    ///
    /// On success, the next line in the iterator is returned. The line does
    /// *not* contain a trailing `\n` or `\r\n`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::io;
    ///
    /// use bstr::io::BufReadExt;
    ///
    /// # fn example() -> Result<(), io::Error> {
    /// let mut cursor = io::Cursor::new(b"lorem\nipsum\r\ndolor");
    ///
    /// let mut lines = vec![];
    /// for result in cursor.byte_lines() {
    ///     let line = result?;
    ///     lines.push(line);
    /// }
    /// assert_eq!(lines.len(), 3);
    /// assert_eq!(lines[0], "lorem".as_bytes());
    /// assert_eq!(lines[1], "ipsum".as_bytes());
    /// assert_eq!(lines[2], "dolor".as_bytes());
    /// # Ok(()) }; example().unwrap()
    /// ```
    fn byte_lines(self) -> ByteLines<Self>
    where
        Self: Sized,
    {
        ByteLines { buf: self }
    }

    /// Returns an iterator over byte-terminated records of this reader, where
    /// each record is represented as a byte string.
    ///
    /// Each item yielded by this iterator is a `io::Result<Vec<u8>>`, where
    /// an error is yielded if there was a problem reading from the underlying
    /// reader.
    ///
    /// On success, the next record in the iterator is returned. The record
    /// does *not* contain its trailing terminator.
    ///
    /// Note that calling `byte_records(b'\n')` differs from `byte_lines()` in
    /// that it has no special handling for `\r`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::io;
    ///
    /// use bstr::io::BufReadExt;
    ///
    /// # fn example() -> Result<(), io::Error> {
    /// let mut cursor = io::Cursor::new(b"lorem\x00ipsum\x00dolor");
    ///
    /// let mut records = vec![];
    /// for result in cursor.byte_records(b'\x00') {
    ///     let record = result?;
    ///     records.push(record);
    /// }
    /// assert_eq!(records.len(), 3);
    /// assert_eq!(records[0], "lorem".as_bytes());
    /// assert_eq!(records[1], "ipsum".as_bytes());
    /// assert_eq!(records[2], "dolor".as_bytes());
    /// # Ok(()) }; example().unwrap()
    /// ```
    fn byte_records(self, terminator: u8) -> ByteRecords<Self>
    where
        Self: Sized,
    {
        ByteRecords { terminator, buf: self }
    }

    /// Executes the given closure on each line in the underlying reader.
    ///
    /// If the closure returns an error (or if the underlying reader returns an
    /// error), then iteration is stopped and the error is returned. If false
    /// is returned, then iteration is stopped and no error is returned.
    ///
    /// The closure given is called on exactly the same values as yielded by
    /// the [`byte_lines`](trait.BufReadExt.html#method.byte_lines)
    /// iterator. Namely, lines do _not_ contain trailing `\n` or `\r\n` bytes.
    ///
    /// This routine is useful for iterating over lines as quickly as
    /// possible. Namely, a single allocation is reused for each line.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::io;
    ///
    /// use bstr::io::BufReadExt;
    ///
    /// # fn example() -> Result<(), io::Error> {
    /// let mut cursor = io::Cursor::new(b"lorem\nipsum\r\ndolor");
    ///
    /// let mut lines = vec![];
    /// cursor.for_byte_line(|line| {
    ///     lines.push(line.to_vec());
    ///     Ok(true)
    /// })?;
    /// assert_eq!(lines.len(), 3);
    /// assert_eq!(lines[0], "lorem".as_bytes());
    /// assert_eq!(lines[1], "ipsum".as_bytes());
    /// assert_eq!(lines[2], "dolor".as_bytes());
    /// # Ok(()) }; example().unwrap()
    /// ```
    fn for_byte_line<F>(&mut self, mut for_each_line: F) -> io::Result<()>
    where
        Self: Sized,
        F: FnMut(&[u8]) -> io::Result<bool>,
    {
        self.for_byte_line_with_terminator(|line| {
            for_each_line(trim_line_slice(line))
        })
    }

    /// Executes the given closure on each byte-terminated record in the
    /// underlying reader.
    ///
    /// If the closure returns an error (or if the underlying reader returns an
    /// error), then iteration is stopped and the error is returned. If false
    /// is returned, then iteration is stopped and no error is returned.
    ///
    /// The closure given is called on exactly the same values as yielded by
    /// the [`byte_records`](trait.BufReadExt.html#method.byte_records)
    /// iterator. Namely, records do _not_ contain a trailing terminator byte.
    ///
    /// This routine is useful for iterating over records as quickly as
    /// possible. Namely, a single allocation is reused for each record.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::io;
    ///
    /// use bstr::io::BufReadExt;
    ///
    /// # fn example() -> Result<(), io::Error> {
    /// let mut cursor = io::Cursor::new(b"lorem\x00ipsum\x00dolor");
    ///
    /// let mut records = vec![];
    /// cursor.for_byte_record(b'\x00', |record| {
    ///     records.push(record.to_vec());
    ///     Ok(true)
    /// })?;
    /// assert_eq!(records.len(), 3);
    /// assert_eq!(records[0], "lorem".as_bytes());
    /// assert_eq!(records[1], "ipsum".as_bytes());
    /// assert_eq!(records[2], "dolor".as_bytes());
    /// # Ok(()) }; example().unwrap()
    /// ```
    fn for_byte_record<F>(
        &mut self,
        terminator: u8,
        mut for_each_record: F,
    ) -> io::Result<()>
    where
        Self: Sized,
        F: FnMut(&[u8]) -> io::Result<bool>,
    {
        self.for_byte_record_with_terminator(terminator, |chunk| {
            for_each_record(trim_record_slice(chunk, terminator))
        })
    }

    /// Executes the given closure on each line in the underlying reader.
    ///
    /// If the closure returns an error (or if the underlying reader returns an
    /// error), then iteration is stopped and the error is returned. If false
    /// is returned, then iteration is stopped and no error is returned.
    ///
    /// Unlike
    /// [`for_byte_line`](trait.BufReadExt.html#method.for_byte_line),
    /// the lines given to the closure *do* include the line terminator, if one
    /// exists.
    ///
    /// This routine is useful for iterating over lines as quickly as
    /// possible. Namely, a single allocation is reused for each line.
    ///
    /// This is identical to `for_byte_record_with_terminator` with a
    /// terminator of `\n`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::io;
    ///
    /// use bstr::io::BufReadExt;
    ///
    /// # fn example() -> Result<(), io::Error> {
    /// let mut cursor = io::Cursor::new(b"lorem\nipsum\r\ndolor");
    ///
    /// let mut lines = vec![];
    /// cursor.for_byte_line_with_terminator(|line| {
    ///     lines.push(line.to_vec());
    ///     Ok(true)
    /// })?;
    /// assert_eq!(lines.len(), 3);
    /// assert_eq!(lines[0], "lorem\n".as_bytes());
    /// assert_eq!(lines[1], "ipsum\r\n".as_bytes());
    /// assert_eq!(lines[2], "dolor".as_bytes());
    /// # Ok(()) }; example().unwrap()
    /// ```
    fn for_byte_line_with_terminator<F>(
        &mut self,
        for_each_line: F,
    ) -> io::Result<()>
    where
        Self: Sized,
        F: FnMut(&[u8]) -> io::Result<bool>,
    {
        self.for_byte_record_with_terminator(b'\n', for_each_line)
    }

    /// Executes the given closure on each byte-terminated record in the
    /// underlying reader.
    ///
    /// If the closure returns an error (or if the underlying reader returns an
    /// error), then iteration is stopped and the error is returned. If false
    /// is returned, then iteration is stopped and no error is returned.
    ///
    /// Unlike
    /// [`for_byte_record`](trait.BufReadExt.html#method.for_byte_record),
    /// the lines given to the closure *do* include the record terminator, if
    /// one exists.
    ///
    /// This routine is useful for iterating over records as quickly as
    /// possible. Namely, a single allocation is reused for each record.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::io;
    ///
    /// use bstr::{io::BufReadExt, B};
    ///
    /// # fn example() -> Result<(), io::Error> {
    /// let mut cursor = io::Cursor::new(b"lorem\x00ipsum\x00dolor");
    ///
    /// let mut records = vec![];
    /// cursor.for_byte_record_with_terminator(b'\x00', |record| {
    ///     records.push(record.to_vec());
    ///     Ok(true)
    /// })?;
    /// assert_eq!(records.len(), 3);
    /// assert_eq!(records[0], B(b"lorem\x00"));
    /// assert_eq!(records[1], B("ipsum\x00"));
    /// assert_eq!(records[2], B("dolor"));
    /// # Ok(()) }; example().unwrap()
    /// ```
    fn for_byte_record_with_terminator<F>(
        &mut self,
        terminator: u8,
        mut for_each_record: F,
    ) -> io::Result<()>
    where
        Self: Sized,
        F: FnMut(&[u8]) -> io::Result<bool>,
    {
        let mut bytes = vec![];
        let mut res = Ok(());
        let mut consumed = 0;
        'outer: loop {
            // Lend out complete record slices from our buffer
            {
                let mut buf = self.fill_buf()?;
                if buf.is_empty() {
                    break;
                }
                while let Some(index) = buf.find_byte(terminator) {
                    let (record, rest) = buf.split_at(index + 1);
                    buf = rest;
                    consumed += record.len();
                    match for_each_record(record) {
                        Ok(false) => break 'outer,
                        Err(err) => {
                            res = Err(err);
                            break 'outer;
                        }
                        _ => (),
                    }
                }

                // Copy the final record fragment to our local buffer. This
                // saves read_until() from re-scanning a buffer we know
                // contains no remaining terminators.
                bytes.extend_from_slice(buf);
                consumed += buf.len();
            }

            self.consume(consumed);
            consumed = 0;

            // N.B. read_until uses a different version of memchr that may
            // be slower than the memchr crate that bstr uses. However, this
            // should only run for a fairly small number of records, assuming a
            // decent buffer size.
            self.read_until(terminator, &mut bytes)?;
            if bytes.is_empty() || !for_each_record(&bytes)? {
                break;
            }
            bytes.clear();
        }
        self.consume(consumed);
        res
    }
}

impl<B: io::BufRead> BufReadExt for B {}

/// An iterator over lines from an instance of
/// [`std::io::BufRead`](https://doc.rust-lang.org/std/io/trait.BufRead.html).
///
/// This iterator is generally created by calling the
/// [`byte_lines`](trait.BufReadExt.html#method.byte_lines)
/// method on the
/// [`BufReadExt`](trait.BufReadExt.html)
/// trait.
#[derive(Debug)]
pub struct ByteLines<B> {
    buf: B,
}

/// An iterator over records from an instance of
/// [`std::io::BufRead`](https://doc.rust-lang.org/std/io/trait.BufRead.html).
///
/// A byte record is any sequence of bytes terminated by a particular byte
/// chosen by the caller. For example, NUL separated byte strings are said to
/// be NUL-terminated byte records.
///
/// This iterator is generally created by calling the
/// [`byte_records`](trait.BufReadExt.html#method.byte_records)
/// method on the
/// [`BufReadExt`](trait.BufReadExt.html)
/// trait.
#[derive(Debug)]
pub struct ByteRecords<B> {
    buf: B,
    terminator: u8,
}

impl<B: io::BufRead> Iterator for ByteLines<B> {
    type Item = io::Result<Vec<u8>>;

    fn next(&mut self) -> Option<io::Result<Vec<u8>>> {
        let mut bytes = vec![];
        match self.buf.read_until(b'\n', &mut bytes) {
            Err(e) => Some(Err(e)),
            Ok(0) => None,
            Ok(_) => {
                trim_line(&mut bytes);
                Some(Ok(bytes))
            }
        }
    }
}

impl<B: io::BufRead> Iterator for ByteRecords<B> {
    type Item = io::Result<Vec<u8>>;

    fn next(&mut self) -> Option<io::Result<Vec<u8>>> {
        let mut bytes = vec![];
        match self.buf.read_until(self.terminator, &mut bytes) {
            Err(e) => Some(Err(e)),
            Ok(0) => None,
            Ok(_) => {
                trim_record(&mut bytes, self.terminator);
                Some(Ok(bytes))
            }
        }
    }
}

fn trim_line(line: &mut Vec<u8>) {
    if line.last_byte() == Some(b'\n') {
        line.pop_byte();
        if line.last_byte() == Some(b'\r') {
            line.pop_byte();
        }
    }
}

fn trim_line_slice(mut line: &[u8]) -> &[u8] {
    if line.last_byte() == Some(b'\n') {
        line = &line[..line.len() - 1];
        if line.last_byte() == Some(b'\r') {
            line = &line[..line.len() - 1];
        }
    }
    line
}

fn trim_record(record: &mut Vec<u8>, terminator: u8) {
    if record.last_byte() == Some(terminator) {
        record.pop_byte();
    }
}

fn trim_record_slice(mut record: &[u8], terminator: u8) -> &[u8] {
    if record.last_byte() == Some(terminator) {
        record = &record[..record.len() - 1];
    }
    record
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::{vec, vec::Vec};

    use crate::bstring::BString;

    use super::BufReadExt;

    fn collect_lines<B: AsRef<[u8]>>(slice: B) -> Vec<BString> {
        let mut lines = vec![];
        slice
            .as_ref()
            .for_byte_line(|line| {
                lines.push(BString::from(line.to_vec()));
                Ok(true)
            })
            .unwrap();
        lines
    }

    fn collect_lines_term<B: AsRef<[u8]>>(slice: B) -> Vec<BString> {
        let mut lines = vec![];
        slice
            .as_ref()
            .for_byte_line_with_terminator(|line| {
                lines.push(BString::from(line.to_vec()));
                Ok(true)
            })
            .unwrap();
        lines
    }

    #[test]
    fn lines_without_terminator() {
        assert_eq!(collect_lines(""), Vec::<BString>::new());

        assert_eq!(collect_lines("\n"), vec![""]);
        assert_eq!(collect_lines("\n\n"), vec!["", ""]);
        assert_eq!(collect_lines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(collect_lines("a\nb"), vec!["a", "b"]);
        assert_eq!(collect_lines("abc\nxyz\n"), vec!["abc", "xyz"]);
        assert_eq!(collect_lines("abc\nxyz"), vec!["abc", "xyz"]);

        assert_eq!(collect_lines("\r\n"), vec![""]);
        assert_eq!(collect_lines("\r\n\r\n"), vec!["", ""]);
        assert_eq!(collect_lines("a\r\nb\r\n"), vec!["a", "b"]);
        assert_eq!(collect_lines("a\r\nb"), vec!["a", "b"]);
        assert_eq!(collect_lines("abc\r\nxyz\r\n"), vec!["abc", "xyz"]);
        assert_eq!(collect_lines("abc\r\nxyz"), vec!["abc", "xyz"]);

        assert_eq!(collect_lines("abc\rxyz"), vec!["abc\rxyz"]);
    }

    #[test]
    fn lines_with_terminator() {
        assert_eq!(collect_lines_term(""), Vec::<BString>::new());

        assert_eq!(collect_lines_term("\n"), vec!["\n"]);
        assert_eq!(collect_lines_term("\n\n"), vec!["\n", "\n"]);
        assert_eq!(collect_lines_term("a\nb\n"), vec!["a\n", "b\n"]);
        assert_eq!(collect_lines_term("a\nb"), vec!["a\n", "b"]);
        assert_eq!(collect_lines_term("abc\nxyz\n"), vec!["abc\n", "xyz\n"]);
        assert_eq!(collect_lines_term("abc\nxyz"), vec!["abc\n", "xyz"]);

        assert_eq!(collect_lines_term("\r\n"), vec!["\r\n"]);
        assert_eq!(collect_lines_term("\r\n\r\n"), vec!["\r\n", "\r\n"]);
        assert_eq!(collect_lines_term("a\r\nb\r\n"), vec!["a\r\n", "b\r\n"]);
        assert_eq!(collect_lines_term("a\r\nb"), vec!["a\r\n", "b"]);
        assert_eq!(
            collect_lines_term("abc\r\nxyz\r\n"),
            vec!["abc\r\n", "xyz\r\n"]
        );
        assert_eq!(collect_lines_term("abc\r\nxyz"), vec!["abc\r\n", "xyz"]);

        assert_eq!(collect_lines_term("abc\rxyz"), vec!["abc\rxyz"]);
    }
}
