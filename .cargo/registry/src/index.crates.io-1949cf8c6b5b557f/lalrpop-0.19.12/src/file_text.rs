use crate::grammar::parse_tree as pt;
use std::fmt::{Display, Error, Formatter};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;

pub struct FileText {
    path: PathBuf,
    input_str: String,
    newlines: Vec<usize>,
}

impl FileText {
    pub fn from_path(path: PathBuf) -> io::Result<FileText> {
        let mut input_str = String::new();
        let mut f = File::open(&path)?;
        f.read_to_string(&mut input_str)?;
        Ok(FileText::new(path, input_str))
    }

    pub fn new(path: PathBuf, input_str: String) -> FileText {
        let newline_indices: Vec<usize> = {
            let input_indices = input_str
                .as_bytes()
                .iter()
                .enumerate()
                .filter(|&(_, &b)| b == b'\n')
                .map(|(i, _)| i + 1); // index of first char in the line
            Some(0).into_iter().chain(input_indices).collect()
        };

        FileText {
            path,
            input_str,
            newlines: newline_indices,
        }
    }

    #[cfg(test)]
    pub fn test() -> FileText {
        Self::new(PathBuf::from("test.lalrpop"), String::from(""))
    }

    pub fn text(&self) -> &String {
        &self.input_str
    }

    pub fn span_str(&self, span: pt::Span) -> String {
        let (start_line, start_col) = self.line_col(span.0);
        let (end_line, end_col) = self.line_col(span.1);
        format!(
            "{}:{}:{}: {}:{}",
            self.path.display(),
            start_line + 1,
            start_col + 1,
            end_line + 1,
            end_col
        )
    }

    fn line_col(&self, pos: usize) -> (usize, usize) {
        let num_lines = self.newlines.len();
        let line = (0..num_lines)
            .filter(|&i| self.newlines[i] > pos)
            .map(|i| i - 1)
            .next()
            .unwrap_or(num_lines - 1);

        // offset of the first character in `line`
        let line_offset = self.newlines[line];

        // find the column; use `saturating_sub` in case `pos` is the
        // newline itself, which we'll call column 0
        let col = pos - line_offset;

        (line, col)
    }

    fn line_text(&self, line_num: usize) -> &str {
        let start_offset = self.newlines[line_num];
        if line_num == self.newlines.len() - 1 {
            &self.input_str[start_offset..]
        } else {
            let end_offset = self.newlines[line_num + 1];
            &self.input_str[start_offset..end_offset - 1]
        }
    }

    pub fn highlight(&self, span: pt::Span, out: &mut dyn Write) -> io::Result<()> {
        let (start_line, start_col) = self.line_col(span.0);
        let (end_line, end_col) = self.line_col(span.1);

        // (*) use `saturating_sub` since the start line could be the newline
        // itself, in which case we'll call it column zero

        // span is within one line:
        if start_line == end_line {
            let text = self.line_text(start_line);
            writeln!(out, "  {}", text)?;

            if end_col - start_col <= 1 {
                writeln!(out, "  {}^", Repeat(' ', start_col))?;
            } else {
                let width = end_col - start_col;
                writeln!(
                    out,
                    "  {}~{}~",
                    Repeat(' ', start_col),
                    Repeat('~', width.saturating_sub(2))
                )?;
            }
        } else {
            // span is across many lines, find the maximal width of any of those
            let line_strs: Vec<_> = (start_line..=end_line).map(|i| self.line_text(i)).collect();
            let max_len = line_strs.iter().map(|l| l.len()).max().unwrap();
            writeln!(
                out,
                "  {}{}~+",
                Repeat(' ', start_col),
                Repeat('~', max_len - start_col)
            )?;
            for line in &line_strs[..line_strs.len() - 1] {
                writeln!(out, "| {0:<1$} |", line, max_len)?;
            }
            writeln!(out, "| {}", line_strs[line_strs.len() - 1])?;
            writeln!(out, "+~{}", Repeat('~', end_col))?;
        }

        Ok(())
    }
}

struct Repeat(char, usize);

impl Display for Repeat {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        for _ in 0..self.1 {
            write!(fmt, "{}", self.0)?;
        }
        Ok(())
    }
}
