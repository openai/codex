// Copyright 2015 Google Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Scanners for fragments of CommonMark syntax

use std::char;

use crate::parse::HtmlScanGuard;
pub(crate) use crate::puncttable::{is_ascii_punctuation, is_punctuation};
use crate::strings::CowStr;
use crate::{entities, HeadingLevel};
use crate::{Alignment, LinkType};

use memchr::memchr;

// sorted for binary search
const HTML_TAGS: [&str; 62] = [
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

/// Analysis of the beginning of a line, including indentation and container
/// markers.
#[derive(Clone)]
pub(crate) struct LineStart<'a> {
    bytes: &'a [u8],
    ix: usize,

    // The index in `bytes` after the last tab we scanned; initially
    // zero.
    //
    // Thus, there are no tab characters between `ix` and here, and for
    // the purpose of defining block structure, this position can be
    // considered to fall on a tab stop.
    //
    // This is only valid while scanning the initial portion of the
    // line; methods that work with interior structure don't bother to
    // update it.
    tab_start: usize,

    // In contexts where spaces help to define block structure, tabs
    // behave as if they were replaced by spaces with a tab stop of 4
    // characters.
    //
    // If we have scanned past a tab character but not consumed all
    // the horizontal width it contributed, this is the number of
    // spaces logically remaining, before the character at `ix`.
    spaces_remaining: usize,

    // no thematic breaks can occur before this offset.
    // this prevents scanning over and over up to a certain point
    min_hrule_offset: usize,
}

impl<'a> LineStart<'a> {
    pub(crate) fn new(bytes: &[u8]) -> LineStart<'_> {
        LineStart {
            bytes,
            tab_start: 0,
            ix: 0,
            spaces_remaining: 0,
            min_hrule_offset: 0,
        }
    }

    /// Try to scan a number of spaces.
    ///
    /// Returns true if all spaces were consumed.
    ///
    /// Note: consumes some spaces even if not successful.
    pub(crate) fn scan_space(&mut self, n_space: usize) -> bool {
        self.scan_space_inner(n_space) == 0
    }

    /// Scan a number of spaces up to a maximum.
    ///
    /// Returns number of spaces scanned.
    pub(crate) fn scan_space_upto(&mut self, n_space: usize) -> usize {
        n_space - self.scan_space_inner(n_space)
    }

    /// Returns unused remainder of spaces.
    fn scan_space_inner(&mut self, mut n_space: usize) -> usize {
        // Consume any common prefix between the number of spaces we
        // want and the number of unscanned tab-introduced spaces.
        let n_from_remaining = self.spaces_remaining.min(n_space);
        self.spaces_remaining -= n_from_remaining;
        n_space -= n_from_remaining;

        while n_space > 0 && self.ix < self.bytes.len() {
            match self.bytes[self.ix] {
                b' ' => {
                    self.ix += 1;
                    n_space -= 1;
                }
                b'\t' => {
                    let spaces = 4 - (self.ix - self.tab_start) % 4;
                    self.ix += 1;
                    self.tab_start = self.ix;
                    let n = spaces.min(n_space);
                    n_space -= n;

                    // Record the unscanned portion of the tab.
                    self.spaces_remaining = spaces - n;
                }
                _ => break,
            }
        }
        n_space
    }

    /// Scan all available ASCII whitespace (not including eol).
    pub(crate) fn scan_all_space(&mut self) {
        self.spaces_remaining = 0;
        self.ix += self.bytes[self.ix..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();
    }

    /// Determine whether we're at end of line (includes end of file).
    pub(crate) fn is_at_eol(&self) -> bool {
        self.bytes
            .get(self.ix)
            .map(|&c| c == b'\r' || c == b'\n')
            .unwrap_or(true)
    }

    fn scan_ch(&mut self, c: u8) -> bool {
        if self.ix < self.bytes.len() && self.bytes[self.ix] == c {
            self.ix += 1;
            true
        } else {
            false
        }
    }

    pub(crate) fn scan_blockquote_marker(&mut self) -> bool {
        let save = self.clone();
        let _ = self.scan_space(3);
        if self.scan_ch(b'>') {
            let _ = self.scan_space(1);
            true
        } else {
            *self = save;
            false
        }
    }

    /// Scan a list marker.
    ///
    /// Return value is the character, the start index, and the indent in spaces.
    /// For ordered list markers, the character will be one of b'.' or b')'. For
    /// bullet list markers, it will be one of b'-', b'+', or b'*'.
    pub(crate) fn scan_list_marker(&mut self) -> Option<(u8, u64, usize)> {
        let save = self.clone();
        let indent = self.scan_space_upto(4);
        if indent < 4 && self.ix < self.bytes.len() {
            let c = self.bytes[self.ix];
            if c == b'-' || c == b'+' || c == b'*' {
                if self.ix >= self.min_hrule_offset {
                    // there could be an hrule here
                    if let Err(min_offset) = scan_hrule(&self.bytes[self.ix..]) {
                        self.min_hrule_offset = min_offset;
                    } else {
                        *self = save;
                        return None;
                    }
                }
                self.ix += 1;
                if self.scan_space(1) || self.is_at_eol() {
                    return self.finish_list_marker(c, 0, indent + 2);
                }
            } else if c.is_ascii_digit() {
                let start_ix = self.ix;
                let mut ix = self.ix + 1;
                let mut val = u64::from(c - b'0');
                while ix < self.bytes.len() && ix - start_ix < 10 {
                    let c = self.bytes[ix];
                    ix += 1;
                    if c.is_ascii_digit() {
                        val = val * 10 + u64::from(c - b'0');
                    } else if c == b')' || c == b'.' {
                        self.ix = ix;
                        if self.scan_space(1) || self.is_at_eol() {
                            return self.finish_list_marker(c, val, indent + 1 + ix - start_ix);
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }
        *self = save;
        None
    }

    fn finish_list_marker(
        &mut self,
        c: u8,
        start: u64,
        mut indent: usize,
    ) -> Option<(u8, u64, usize)> {
        let save = self.clone();

        // skip the rest of the line if it's blank
        if scan_blank_line(&self.bytes[self.ix..]).is_some() {
            return Some((c, start, indent));
        }

        let post_indent = self.scan_space_upto(4);
        if post_indent < 4 {
            indent += post_indent;
        } else {
            *self = save;
        }
        Some((c, start, indent))
    }

    /// Returns Some(is_checked) when a task list marker was found. Resets itself
    /// to original state otherwise.
    pub(crate) fn scan_task_list_marker(&mut self) -> Option<bool> {
        let save = self.clone();
        self.scan_space_upto(3);

        if !self.scan_ch(b'[') {
            *self = save;
            return None;
        }
        let is_checked = match self.bytes.get(self.ix) {
            Some(&c) if is_ascii_whitespace_no_nl(c) => {
                self.ix += 1;
                false
            }
            Some(b'x') | Some(b'X') => {
                self.ix += 1;
                true
            }
            _ => {
                *self = save;
                return None;
            }
        };
        if !self.scan_ch(b']') {
            *self = save;
            return None;
        }
        if !self
            .bytes
            .get(self.ix)
            .map(|&b| is_ascii_whitespace_no_nl(b))
            .unwrap_or(false)
        {
            *self = save;
            return None;
        }
        Some(is_checked)
    }

    pub(crate) fn bytes_scanned(&self) -> usize {
        self.ix
    }

    pub(crate) fn remaining_space(&self) -> usize {
        self.spaces_remaining
    }
}

pub(crate) fn is_ascii_whitespace(c: u8) -> bool {
    (0x09..=0x0d).contains(&c) || c == b' '
}

pub(crate) fn is_ascii_whitespace_no_nl(c: u8) -> bool {
    c == b'\t' || c == 0x0b || c == 0x0c || c == b' '
}

fn is_ascii_alpha(c: u8) -> bool {
    c.is_ascii_alphabetic()
}

fn is_ascii_alphanumeric(c: u8) -> bool {
    matches!(c, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z')
}

fn is_ascii_letterdigitdash(c: u8) -> bool {
    c == b'-' || is_ascii_alphanumeric(c)
}

fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

fn is_valid_unquoted_attr_value_char(c: u8) -> bool {
    !matches!(
        c,
        b'\'' | b'"' | b' ' | b'=' | b'>' | b'<' | b'`' | b'\n' | b'\r'
    )
}

// scan a single character
pub(crate) fn scan_ch(data: &[u8], c: u8) -> usize {
    if !data.is_empty() && data[0] == c {
        1
    } else {
        0
    }
}

pub(crate) fn scan_while<F>(data: &[u8], mut f: F) -> usize
where
    F: FnMut(u8) -> bool,
{
    data.iter().take_while(|&&c| f(c)).count()
}

pub(crate) fn scan_rev_while<F>(data: &[u8], mut f: F) -> usize
where
    F: FnMut(u8) -> bool,
{
    data.iter().rev().take_while(|&&c| f(c)).count()
}

pub(crate) fn scan_ch_repeat(data: &[u8], c: u8) -> usize {
    scan_while(data, |x| x == c)
}

// Note: this scans ASCII whitespace only, for Unicode whitespace use
// a different function.
pub(crate) fn scan_whitespace_no_nl(data: &[u8]) -> usize {
    scan_while(data, is_ascii_whitespace_no_nl)
}

fn scan_attr_value_chars(data: &[u8]) -> usize {
    scan_while(data, is_valid_unquoted_attr_value_char)
}

pub(crate) fn scan_eol(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() {
        return Some(0);
    }
    match bytes[0] {
        b'\n' => Some(1),
        b'\r' => Some(if bytes.get(1) == Some(&b'\n') { 2 } else { 1 }),
        _ => None,
    }
}

pub(crate) fn scan_blank_line(bytes: &[u8]) -> Option<usize> {
    let i = scan_whitespace_no_nl(bytes);
    scan_eol(&bytes[i..]).map(|n| i + n)
}

pub(crate) fn scan_nextline(bytes: &[u8]) -> usize {
    memchr(b'\n', bytes).map_or(bytes.len(), |x| x + 1)
}

// return: end byte for closing code fence, or None
// if the line is not a closing code fence
pub(crate) fn scan_closing_code_fence(
    bytes: &[u8],
    fence_char: u8,
    n_fence_char: usize,
) -> Option<usize> {
    if bytes.is_empty() {
        return Some(0);
    }
    let mut i = 0;
    let num_fence_chars_found = scan_ch_repeat(&bytes[i..], fence_char);
    if num_fence_chars_found < n_fence_char {
        return None;
    }
    i += num_fence_chars_found;
    let num_trailing_spaces = scan_ch_repeat(&bytes[i..], b' ');
    i += num_trailing_spaces;
    scan_eol(&bytes[i..]).map(|_| i)
}

// return: end byte for closing metadata block, or None
// if the line is not a closing metadata block
pub(crate) fn scan_closing_metadata_block(bytes: &[u8], fence_char: u8) -> Option<usize> {
    let mut i = 0;
    let mut num_fence_chars_found = scan_ch_repeat(&bytes[i..], fence_char);
    if num_fence_chars_found != 3 {
        // if YAML style metadata block the closing character can also be `.`
        if fence_char == b'-' {
            num_fence_chars_found = scan_ch_repeat(&bytes[i..], b'.');
            if num_fence_chars_found != 3 {
                return None;
            }
        } else {
            return None;
        }
    }
    i += num_fence_chars_found;
    let num_trailing_spaces = scan_ch_repeat(&bytes[i..], b' ');
    i += num_trailing_spaces;
    scan_eol(&bytes[i..]).map(|_| i)
}

// returned pair is (number of bytes, number of spaces)
pub(crate) fn calc_indent(text: &[u8], max: usize) -> (usize, usize) {
    let mut spaces = 0;
    let mut offset = 0;

    for (i, &b) in text.iter().enumerate() {
        offset = i;
        match b {
            b' ' => {
                spaces += 1;
                if spaces == max {
                    break;
                }
            }
            b'\t' => {
                let new_spaces = spaces + 4 - (spaces & 3);
                if new_spaces > max {
                    break;
                }
                spaces = new_spaces;
            }
            _ => break,
        }
    }

    (offset, spaces)
}

/// Scan hrule opening sequence.
///
/// Returns Ok(x) when it finds an hrule, where x is the
/// size of line containing the hrule, including the trailing newline.
///
/// Returns Err(x) when it does not find an hrule and x is
/// the offset in data before no hrule can appear.
pub(crate) fn scan_hrule(bytes: &[u8]) -> Result<usize, usize> {
    if bytes.len() < 3 {
        return Err(0);
    }
    let c = bytes[0];
    if !(c == b'*' || c == b'-' || c == b'_') {
        return Err(0);
    }
    let mut n = 0;
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\n' | b'\r' => {
                i += scan_eol(&bytes[i..]).unwrap_or(0);
                break;
            }
            c2 if c2 == c => {
                n += 1;
            }
            b' ' | b'\t' => (),
            _ => return Err(i),
        }
        i += 1;
    }
    if n >= 3 {
        Ok(i)
    } else {
        Err(i)
    }
}

/// Scan an ATX heading opening sequence.
///
/// Returns number of bytes in prefix and level.
pub(crate) fn scan_atx_heading(data: &[u8]) -> Option<HeadingLevel> {
    let level = scan_ch_repeat(data, b'#');
    if data.get(level).copied().map_or(true, is_ascii_whitespace) {
        HeadingLevel::try_from(level).ok()
    } else {
        None
    }
}

/// Scan a setext heading underline.
///
/// Returns number of bytes in line (including trailing newline) and level.
pub(crate) fn scan_setext_heading(data: &[u8]) -> Option<(usize, HeadingLevel)> {
    let c = *data.first()?;
    let level = if c == b'=' {
        HeadingLevel::H1
    } else if c == b'-' {
        HeadingLevel::H2
    } else {
        return None;
    };
    let mut i = 1 + scan_ch_repeat(&data[1..], c);
    i += scan_blank_line(&data[i..])?;
    Some((i, level))
}

// returns number of bytes in line (including trailing
// newline) and column alignments
pub(crate) fn scan_table_head(data: &[u8]) -> (usize, Vec<Alignment>) {
    let (mut i, spaces) = calc_indent(data, 4);
    if spaces > 3 || i == data.len() {
        return (0, vec![]);
    }
    let mut cols = vec![];
    let mut active_col = Alignment::None;
    let mut start_col = true;
    let mut found_pipe = false;
    let mut found_hyphen = false;
    let mut found_hyphen_in_col = false;
    if data[i] == b'|' {
        i += 1;
        found_pipe = true;
    }
    for c in &data[i..] {
        if let Some(n) = scan_eol(&data[i..]) {
            i += n;
            break;
        }
        match *c {
            b' ' => (),
            b':' => {
                active_col = match (start_col, active_col) {
                    (true, Alignment::None) => Alignment::Left,
                    (false, Alignment::Left) => Alignment::Center,
                    (false, Alignment::None) => Alignment::Right,
                    _ => active_col,
                };
                start_col = false;
            }
            b'-' => {
                start_col = false;
                found_hyphen = true;
                found_hyphen_in_col = true;
            }
            b'|' => {
                start_col = true;
                found_pipe = true;
                cols.push(active_col);
                active_col = Alignment::None;
                if !found_hyphen_in_col {
                    // It isn't a table head if it has back-to-back pipes.
                    return (0, vec![]);
                }
                found_hyphen_in_col = false;
            }
            _ => {
                // It isn't a table head if it has characters outside the allowed set.
                return (0, vec![]);
            }
        }
        i += 1;
    }

    if !start_col {
        cols.push(active_col);
    }
    if !found_pipe || !found_hyphen {
        // It isn't a table head if it doesn't have a least one pipe or hyphen.
        // It's a list, a header, or a thematic break.
        return (0, vec![]);
    }

    (i, cols)
}

/// Scan code fence.
///
/// Returns number of bytes scanned and the char that is repeated to make the code fence.
pub(crate) fn scan_code_fence(data: &[u8]) -> Option<(usize, u8)> {
    let c = *data.first()?;
    if !(c == b'`' || c == b'~') {
        return None;
    }
    let i = 1 + scan_ch_repeat(&data[1..], c);
    if i >= 3 {
        if c == b'`' {
            let suffix = &data[i..];
            let next_line = i + scan_nextline(suffix);
            // FIXME: make sure this is correct
            if suffix[..(next_line - i)].iter().any(|&b| b == b'`') {
                return None;
            }
        }
        Some((i, c))
    } else {
        None
    }
}

/// Scan metadata block, returning the number of delimiter bytes
/// (always 3 for now) and the delimiter character.
///
/// Differently to code blocks, metadata blocks must be closed with the closing
/// sequence not being a valid terminator the end of the file.
///
/// In addition, they cannot be empty (closing sequence in the next line) and
/// the next line cannot be an empty line.
pub(crate) fn scan_metadata_block(
    data: &[u8],
    yaml_style_enabled: bool,
    pluses_style_enabled: bool,
) -> Option<(usize, u8)> {
    // Only if metadata blocks are enabled
    if yaml_style_enabled || pluses_style_enabled {
        let c = *data.first()?;
        if !((c == b'-' && yaml_style_enabled) || (c == b'+' && pluses_style_enabled)) {
            return None;
        }
        let i = 1 + scan_ch_repeat(&data[1..], c);
        // Only trailing spaces after the delimiters in the line
        let next_line = scan_nextline(&data[i..]);
        for c in &data[i..i + next_line] {
            if !c.is_ascii_whitespace() {
                return None;
            }
        }
        if i == 3 {
            // Search the closing sequence
            let mut j = i;
            let mut first_line = true;
            while j < data.len() {
                j += scan_nextline(&data[j..]);
                let closed = scan_closing_metadata_block(&data[j..], c).is_some();
                // The first line of the metadata block cannot be an empty line
                // nor the end of the block
                if first_line {
                    if closed || scan_blank_line(&data[j..]).is_some() {
                        return None;
                    }
                    first_line = false;
                }
                if closed {
                    return Some((i, c));
                }
            }
            None
        } else {
            None
        }
    } else {
        None
    }
}

pub(crate) fn scan_blockquote_start(data: &[u8]) -> Option<usize> {
    if data.first().copied() == Some(b'>') {
        let space = if data.get(1).copied() == Some(b' ') {
            1
        } else {
            0
        };
        Some(1 + space)
    } else {
        None
    }
}

/// return number of bytes scanned, delimiter, start index, and indent
pub(crate) fn scan_listitem(bytes: &[u8]) -> Option<(usize, u8, usize, usize)> {
    let mut c = *bytes.first()?;
    let (w, start) = match c {
        b'-' | b'+' | b'*' => (1, 0),
        b'0'..=b'9' => {
            let (length, start) = parse_decimal(bytes, 9);
            c = *bytes.get(length)?;
            if !(c == b'.' || c == b')') {
                return None;
            }
            (length + 1, start)
        }
        _ => {
            return None;
        }
    };
    // TODO: replace calc_indent with scan_leading_whitespace, for tab correctness
    let (mut postn, mut postindent) = calc_indent(&bytes[w..], 5);
    if postindent == 0 {
        scan_eol(&bytes[w..])?;
        postindent += 1;
    } else if postindent > 4 {
        postn = 1;
        postindent = 1;
    }
    if scan_blank_line(&bytes[w..]).is_some() {
        postn = 0;
        postindent = 1;
    }
    Some((w + postn, c, start, w + postindent))
}

// returns (number of bytes, parsed decimal)
fn parse_decimal(bytes: &[u8], limit: usize) -> (usize, usize) {
    match bytes
        .iter()
        .take(limit)
        .take_while(|&&b| is_digit(b))
        .try_fold((0, 0usize), |(count, acc), c| {
            let digit = usize::from(c - b'0');
            match acc
                .checked_mul(10)
                .and_then(|ten_acc| ten_acc.checked_add(digit))
            {
                Some(number) => Ok((count + 1, number)),
                // stop early on overflow
                None => Err((count, acc)),
            }
        }) {
        Ok(p) | Err(p) => p,
    }
}

// returns (number of bytes, parsed hex)
fn parse_hex(bytes: &[u8], limit: usize) -> (usize, usize) {
    match bytes
        .iter()
        .take(limit)
        .try_fold((0, 0usize), |(count, acc), c| {
            let mut c = *c;
            let digit = if c.is_ascii_digit() {
                usize::from(c - b'0')
            } else {
                // make lower case
                c |= 0x20;
                if (b'a'..=b'f').contains(&c) {
                    usize::from(c - b'a' + 10)
                } else {
                    return Err((count, acc));
                }
            };
            match acc
                .checked_mul(16)
                .and_then(|sixteen_acc| sixteen_acc.checked_add(digit))
            {
                Some(number) => Ok((count + 1, number)),
                // stop early on overflow
                None => Err((count, acc)),
            }
        }) {
        Ok(p) | Err(p) => p,
    }
}

fn char_from_codepoint(input: usize) -> Option<char> {
    let codepoint = input.try_into().ok()?;
    if codepoint == 0 {
        return None;
    }
    char::from_u32(codepoint)
}

// doesn't bother to check data[0] == '&'
pub(crate) fn scan_entity(bytes: &[u8]) -> (usize, Option<CowStr<'static>>) {
    let mut end = 1;
    if scan_ch(&bytes[end..], b'#') == 1 {
        end += 1;
        let (bytecount, codepoint) = if end < bytes.len() && bytes[end] | 0x20 == b'x' {
            end += 1;
            parse_hex(&bytes[end..], 6)
        } else {
            parse_decimal(&bytes[end..], 7)
        };
        end += bytecount;
        return if bytecount == 0 || scan_ch(&bytes[end..], b';') == 0 {
            (0, None)
        } else {
            (
                end + 1,
                Some(char_from_codepoint(codepoint).unwrap_or('\u{FFFD}').into()),
            )
        };
    }
    end += scan_while(&bytes[end..], is_ascii_alphanumeric);
    if scan_ch(&bytes[end..], b';') == 1 {
        if let Some(value) = entities::get_entity(&bytes[1..end]) {
            return (end + 1, Some(value.into()));
        }
    }
    (0, None)
}

// note: dest returned is raw, still needs to be unescaped
// TODO: check that nested parens are really not allowed for refdefs
// TODO(performance): this func should probably its own unescaping
pub(crate) fn scan_link_dest(
    data: &str,
    start_ix: usize,
    max_next: usize,
) -> Option<(usize, &str)> {
    let bytes = &data.as_bytes()[start_ix..];
    let mut i = scan_ch(bytes, b'<');

    if i != 0 {
        // pointy links
        while i < bytes.len() {
            match bytes[i] {
                b'\n' | b'\r' | b'<' => return None,
                b'>' => return Some((i + 1, &data[(start_ix + 1)..(start_ix + i)])),
                b'\\' if i + 1 < bytes.len() && is_ascii_punctuation(bytes[i + 1]) => {
                    i += 1;
                }
                _ => {}
            }
            i += 1;
        }
        None
    } else {
        // non-pointy links
        let mut nest = 0;
        while i < bytes.len() {
            match bytes[i] {
                0x0..=0x20 => {
                    break;
                }
                b'(' => {
                    if nest > max_next {
                        return None;
                    }
                    nest += 1;
                }
                b')' => {
                    if nest == 0 {
                        break;
                    }
                    nest -= 1;
                }
                b'\\' if i + 1 < bytes.len() && is_ascii_punctuation(bytes[i + 1]) => {
                    i += 1;
                }
                _ => {}
            }
            i += 1;
        }
        if nest != 0 {
            return None;
        }
        Some((i, &data[start_ix..(start_ix + i)]))
    }
}

/// Returns bytes scanned
fn scan_attribute_name(data: &[u8]) -> Option<usize> {
    let (&c, tail) = data.split_first()?;
    if is_ascii_alpha(c) || c == b'_' || c == b':' {
        Some(
            1 + scan_while(tail, |c| {
                is_ascii_alphanumeric(c) || c == b'_' || c == b'.' || c == b':' || c == b'-'
            }),
        )
    } else {
        None
    }
}

/// Returns the index immediately following the attribute on success.
/// The argument `buffer_ix` refers to the index into `data` from which we
/// should copy into `buffer` when we find bytes to skip.
fn scan_attribute(
    data: &[u8],
    mut ix: usize,
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
    buffer: &mut Vec<u8>,
    buffer_ix: &mut usize,
) -> Option<usize> {
    ix += scan_attribute_name(&data[ix..])?;
    let ix_after_attribute = ix;
    ix = scan_whitespace_with_newline_handler_without_buffer(data, ix, newline_handler)?;
    if scan_ch(&data[ix..], b'=') == 1 {
        ix = scan_whitespace_with_newline_handler(data, ix_after_attribute, newline_handler, buffer, buffer_ix)?;
        ix += 1;
        ix = scan_whitespace_with_newline_handler(data, ix, newline_handler, buffer, buffer_ix)?;
        ix = scan_attribute_value(data, ix, newline_handler, buffer, buffer_ix)?;
        Some(ix)
    } else {
        // Leave whitespace for next attribute.
        Some(ix_after_attribute)
    }
}

/// Scans whitespace and possibly newlines according to the
/// behavior defined by the newline handler. When bytes are skipped,
/// all preceding non-skipped bytes are pushed to the buffer.
fn scan_whitespace_with_newline_handler(
    data: &[u8],
    mut i: usize,
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
    buffer: &mut Vec<u8>,
    buffer_ix: &mut usize,
) -> Option<usize> {
    while i < data.len() {
        if !is_ascii_whitespace(data[i]) {
            return Some(i);
        }
        if let Some(eol_bytes) = scan_eol(&data[i..]) {
            let handler = newline_handler?;
            i += eol_bytes;
            let skipped_bytes = handler(&data[i..]);

            if skipped_bytes > 0 {
                buffer.extend(&data[*buffer_ix..i]);
                *buffer_ix = i + skipped_bytes;
            }

            i += skipped_bytes;
        } else {
            i += 1;
        }
    }

    Some(i)
}

/// Scans whitespace and possible newlines according to the behavior defined
/// by the newline handler.
///
/// Unlike [`scan_whitespace_with_newline_handler`], this function doesn't
/// copy skipped data into a buffer. Typically, if this function
/// returns `Some`, a call to `scan_whitespace_with_newline_handler` will
/// soon follow.
fn scan_whitespace_with_newline_handler_without_buffer(
    data: &[u8],
    mut i: usize,
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
) -> Option<usize> {
    while i < data.len() {
        if !is_ascii_whitespace(data[i]) {
            return Some(i);
        }
        if let Some(eol_bytes) = scan_eol(&data[i..]) {
            let handler = newline_handler?;
            i += eol_bytes;
            let skipped_bytes = handler(&data[i..]);
            i += skipped_bytes;
        } else {
            i += 1;
        }
    }

    Some(i)
}

/// Returns the index immediately following the attribute value on success.
fn scan_attribute_value(
    data: &[u8],
    mut i: usize,
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
    buffer: &mut Vec<u8>,
    buffer_ix: &mut usize,
) -> Option<usize> {
    match *data.get(i)? {
        b @ b'"' | b @ b'\'' => {
            i += 1;
            while i < data.len() {
                if data[i] == b {
                    return Some(i + 1);
                }
                if let Some(eol_bytes) = scan_eol(&data[i..]) {
                    let handler = newline_handler?;
                    i += eol_bytes;
                    let skipped_bytes = handler(&data[i..]);

                    if skipped_bytes > 0 {
                        buffer.extend(&data[*buffer_ix..i]);
                        *buffer_ix = i + skipped_bytes;
                    }
                    i += skipped_bytes;
                } else {
                    i += 1;
                }
            }
            return None;
        }
        b' ' | b'=' | b'>' | b'<' | b'`' | b'\n' | b'\r' => {
            return None;
        }
        _ => {
            // unquoted attribute value
            i += scan_attr_value_chars(&data[i..]);
        }
    }

    Some(i)
}

// Remove backslash escapes and resolve entities
pub(crate) fn unescape<'a, I: Into<CowStr<'a>>>(input: I, is_in_table: bool) -> CowStr<'a> {
    let input = input.into();
    let mut result = String::new();
    let mut mark = 0;
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        match bytes[i] {
            // Tables are special, because they're parsed as-if the tables
            // were parsed in a discrete pass, changing `\|` to `|`, and then
            // passing the changed string to the inline parser.
            b'\\'
                if is_in_table
                    && i + 2 < bytes.len()
                    && bytes[i + 1] == b'\\'
                    && bytes[i + 2] == b'|' =>
            {
                // even number of `\`s before pipe
                // odd number is handled in the normal way below
                result.push_str(&input[mark..i]);
                mark = i + 2;
                i += 3;
            }
            b'\\' if i + 1 < bytes.len() && is_ascii_punctuation(bytes[i + 1]) => {
                result.push_str(&input[mark..i]);
                mark = i + 1;
                i += 2;
            }
            b'&' => match scan_entity(&bytes[i..]) {
                (n, Some(value)) => {
                    result.push_str(&input[mark..i]);
                    result.push_str(&value);
                    i += n;
                    mark = i;
                }
                _ => i += 1,
            },
            b'\r' => {
                result.push_str(&input[mark..i]);
                i += 1;
                mark = i;
            }
            _ => i += 1,
        }
    }
    if mark == 0 {
        input
    } else {
        result.push_str(&input[mark..]);
        result.into()
    }
}

/// Assumes `data` is preceded by `<`.
pub(crate) fn starts_html_block_type_6(data: &[u8]) -> bool {
    let i = scan_ch(data, b'/');
    let tail = &data[i..];
    let n = scan_while(tail, is_ascii_alphanumeric);
    if !is_html_tag(&tail[..n]) {
        return false;
    }
    // Starting condition says the next byte must be either a space, a tab,
    // the end of the line, the string >, or the string />
    let tail = &tail[n..];
    tail.is_empty()
        || tail[0] == b' '
        || tail[0] == b'\t'
        || tail[0] == b'\r'
        || tail[0] == b'\n'
        || tail[0] == b'>'
        || tail.len() >= 2 && &tail[..2] == b"/>"
}

fn is_html_tag(tag: &[u8]) -> bool {
    HTML_TAGS
        .binary_search_by(|probe| {
            let probe_bytes_iter = probe.as_bytes().iter();
            let tag_bytes_iter = tag.iter();

            probe_bytes_iter
                .zip(tag_bytes_iter)
                .find_map(|(&a, &b)| {
                    // We can compare case insensitively because the probes are
                    // all lower case alpha strings.
                    match a.cmp(&(b | 0x20)) {
                        std::cmp::Ordering::Equal => None,
                        inequality => Some(inequality),
                    }
                })
                .unwrap_or_else(|| probe.len().cmp(&tag.len()))
        })
        .is_ok()
}

/// Assumes that `data` starts with `<`.
/// Returns the index into data directly after the html tag on success.
pub(crate) fn scan_html_type_7(data: &[u8]) -> Option<usize> {
    // Block type html does not allow for newlines, so we
    // do not pass a newline handler.
    let (_span, i) = scan_html_block_inner(data, None)?;
    scan_blank_line(&data[i..])?;
    Some(i)
}

/// Assumes that `data` starts with `<`.
/// Returns the number of bytes scanned and the html in case of
/// success.
/// When some bytes were skipped, because the html was split over
/// multiple leafs (e.g. over multiple lines in a blockquote),
/// the html is returned as a vector of bytes.
/// If no bytes were skipped, the buffer will be empty.
pub(crate) fn scan_html_block_inner(
    data: &[u8],
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
) -> Option<(Vec<u8>, usize)> {
    let mut buffer = Vec::new();
    let mut last_buf_index = 0;

    let close_tag_bytes = scan_ch(&data[1..], b'/');
    let l = scan_while(&data[(1 + close_tag_bytes)..], is_ascii_alpha);
    if l == 0 {
        return None;
    }
    let mut i = 1 + close_tag_bytes + l;
    i += scan_while(&data[i..], is_ascii_letterdigitdash);

    if close_tag_bytes == 0 {
        loop {
            let old_i = i;
            loop {
                i += scan_whitespace_no_nl(&data[i..]);
                if let Some(eol_bytes) = scan_eol(&data[i..]) {
                    if eol_bytes == 0 {
                        return None;
                    }
                    let handler = newline_handler?;
                    i += eol_bytes;
                    let skipped_bytes = handler(&data[i..]);

                    let data_len = data.len() - i;

                    debug_assert!(
                        skipped_bytes <= data_len,
                        "Handler tried to skip too many bytes, fed {}, skipped {}",
                        data_len,
                        skipped_bytes
                    );

                    if skipped_bytes > 0 {
                        buffer.extend(&data[last_buf_index..i]);
                        i += skipped_bytes;
                        last_buf_index = i;
                    }
                } else {
                    break;
                }
            }
            if let Some(b'/') | Some(b'>') = data.get(i) {
                break;
            }
            if old_i == i {
                // No whitespace, which is mandatory.
                return None;
            }
            i = scan_attribute(data, i, newline_handler, &mut buffer, &mut last_buf_index)?;
        }
    }

    i += scan_whitespace_no_nl(&data[i..]);

    if close_tag_bytes == 0 {
        i += scan_ch(&data[i..], b'/');
    }

    if scan_ch(&data[i..], b'>') == 0 {
        None
    } else {
        i += 1;
        if !buffer.is_empty() {
            buffer.extend(&data[last_buf_index..i]);
        }
        Some((buffer, i))
    }
}

/// Returns (next_byte_offset, uri, type)
pub(crate) fn scan_autolink(text: &str, start_ix: usize) -> Option<(usize, CowStr<'_>, LinkType)> {
    scan_uri(text, start_ix)
        .map(|(bytes, uri)| (bytes, uri, LinkType::Autolink))
        .or_else(|| scan_email(text, start_ix).map(|(bytes, uri)| (bytes, uri, LinkType::Email)))
}

/// Returns (next_byte_offset, uri)
fn scan_uri(text: &str, start_ix: usize) -> Option<(usize, CowStr<'_>)> {
    let bytes = &text.as_bytes()[start_ix..];

    // scheme's first byte must be an ascii letter
    if bytes.is_empty() || !is_ascii_alpha(bytes[0]) {
        return None;
    }

    let mut i = 1;

    while i < bytes.len() {
        let c = bytes[i];
        i += 1;
        match c {
            c if is_ascii_alphanumeric(c) => (),
            b'.' | b'-' | b'+' => (),
            b':' => break,
            _ => return None,
        }
    }

    // scheme length must be between 2 and 32 characters long. scheme
    // must be followed by colon
    if !(3..=33).contains(&i) {
        return None;
    }

    while i < bytes.len() {
        match bytes[i] {
            b'>' => return Some((start_ix + i + 1, text[start_ix..(start_ix + i)].into())),
            b'\0'..=b' ' | b'<' => return None,
            _ => (),
        }
        i += 1;
    }

    None
}

/// Returns (next_byte_offset, email)
fn scan_email(text: &str, start_ix: usize) -> Option<(usize, CowStr<'_>)> {
    // using a regex library would be convenient, but doing it by hand is not too bad
    let bytes = &text.as_bytes()[start_ix..];
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];
        i += 1;
        match c {
            c if is_ascii_alphanumeric(c) => (),
            b'.' | b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'/' | b'=' | b'?'
            | b'^' | b'_' | b'`' | b'{' | b'|' | b'}' | b'~' | b'-' => (),
            b'@' if i > 1 => break,
            _ => return None,
        }
    }

    loop {
        let label_start_ix = i;
        let mut fresh_label = true;

        while i < bytes.len() {
            match bytes[i] {
                c if is_ascii_alphanumeric(c) => (),
                b'-' if fresh_label => {
                    return None;
                }
                b'-' => (),
                _ => break,
            }
            fresh_label = false;
            i += 1;
        }

        if i == label_start_ix || i - label_start_ix > 63 || bytes[i - 1] == b'-' {
            return None;
        }

        if scan_ch(&bytes[i..], b'.') == 0 {
            break;
        }
        i += 1;
    }

    if scan_ch(&bytes[i..], b'>') == 0 {
        return None;
    }

    Some((start_ix + i + 1, text[start_ix..(start_ix + i)].into()))
}

/// Scan comment, declaration, or CDATA section, with initial "<!" already consumed.
/// Returns byte offset on match.
pub(crate) fn scan_inline_html_comment(
    bytes: &[u8],
    mut ix: usize,
    scan_guard: &mut HtmlScanGuard,
) -> Option<usize> {
    let c = *bytes.get(ix)?;
    ix += 1;
    match c {
        // An HTML comment consists of `<!-->`, `<!--->`, or  `<!--`, a string of characters not
        // including the string `-->`, and `-->`.
        b'-' => {
            // HTML comment needs two hyphens after the !.
            if *bytes.get(ix)? != b'-' {
                return None;
            }
            // Yes, we're intentionally going backwards.
            // We want the cursor to point here:
            //
            //     <!--
            //       ^
            //
            // This way, the `<!-->` case is covered by the loop below.
            ix -= 1;

            while let Some(x) = memchr(b'-', &bytes[ix..]) {
                ix += x + 1;
                if scan_ch(&bytes[ix..], b'-') == 1 && scan_ch(&bytes[ix + 1..], b'>') == 1 {
                    return Some(ix + 2);
                }
            }
            None
        }
        // A CDATA section consists of the string `<![CDATA[`, a string of characters not
        // including the string `]]>`, and the string `]]>`.
        b'[' if bytes[ix..].starts_with(b"CDATA[") && ix > scan_guard.cdata => {
            ix += b"CDATA[".len();
            ix = memchr(b']', &bytes[ix..]).map_or(bytes.len(), |x| ix + x);
            let close_brackets = scan_ch_repeat(&bytes[ix..], b']');
            ix += close_brackets;

            if close_brackets == 0 || scan_ch(&bytes[ix..], b'>') == 0 {
                scan_guard.cdata = ix;
                None
            } else {
                Some(ix + 1)
            }
        }
        // A declaration consists of the string `<!`, an ASCII letter, zero or more characters not
        // including the character >, and the character >.
        _ if c.is_ascii_alphabetic() && ix > scan_guard.declaration => {
            ix = memchr(b'>', &bytes[ix..]).map_or(bytes.len(), |x| ix + x);
            if scan_ch(&bytes[ix..], b'>') == 0 {
                scan_guard.declaration = ix;
                None
            } else {
                Some(ix + 1)
            }
        }
        _ => None,
    }
}

/// Scan processing directive, with initial "<?" already consumed.
/// Returns the next byte offset on success.
pub(crate) fn scan_inline_html_processing(
    bytes: &[u8],
    mut ix: usize,
    scan_guard: &mut HtmlScanGuard,
) -> Option<usize> {
    if ix <= scan_guard.processing {
        return None;
    }
    while let Some(offset) = memchr(b'?', &bytes[ix..]) {
        ix += offset + 1;
        if scan_ch(&bytes[ix..], b'>') == 1 {
            return Some(ix + 1);
        }
    }
    scan_guard.processing = ix;
    None
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn overflow_list() {
        assert!(
            scan_listitem(b"4444444444444444444444444444444444444444444444444444444444!").is_none()
        );
    }

    #[test]
    fn overflow_by_addition() {
        assert!(scan_listitem(b"1844674407370955161615!").is_none());
    }

    #[test]
    fn good_emails() {
        const EMAILS: &[&str] = &[
            "<a@b.c>",
            "<a@b>",
            "<a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-@example.com>",
            "<a@sixty-three-letters-in-this-identifier-----------------------63>",
        ];
        for email in EMAILS {
            assert!(scan_email(email, 1).is_some());
        }
    }

    #[test]
    fn bad_emails() {
        const EMAILS: &[&str] = &[
            "<@b.c>",
            "<foo@-example.com>",
            "<foo@example-.com>",
            "<a@notrailingperiod.>",
            "<a(noparens)@example.com>",
            "<\"noquotes\"@example.com>",
            "<a@sixty-four-letters-in-this-identifier-------------------------64>",
        ];
        for email in EMAILS {
            assert!(scan_email(email, 1).is_none());
        }
    }
}
