use ::memchr::{memchr, memchr2, memrchr, memrchr2};

use crate::chars::Char;
use crate::utf32_str::Utf32Str;
use crate::Matcher;

#[inline(always)]
fn find_ascii_ignore_case(c: u8, haystack: &[u8]) -> Option<usize> {
    if c >= b'a' && c <= b'z' {
        memchr2(c, c - 32, haystack)
    } else {
        memchr(c, haystack)
    }
}

#[inline(always)]
fn find_ascii_ignore_case_rev(c: u8, haystack: &[u8]) -> Option<usize> {
    if c >= b'a' && c <= b'z' {
        memrchr2(c, c - 32, haystack)
    } else {
        memrchr(c, haystack)
    }
}

impl Matcher {
    pub(crate) fn prefilter_ascii(
        &self,
        mut haystack: &[u8],
        needle: &[u8],
        only_greedy: bool,
    ) -> Option<(usize, usize, usize)> {
        if self.config.ignore_case {
            let start =
                find_ascii_ignore_case(needle[0], &haystack[..haystack.len() - needle.len() + 1])?;
            let mut greedy_end = start + 1;
            haystack = &haystack[greedy_end..];
            for &c in &needle[1..] {
                let idx = find_ascii_ignore_case(c, haystack)? + 1;
                greedy_end += idx;
                haystack = &haystack[idx..];
            }
            if only_greedy {
                Some((start, greedy_end, greedy_end))
            } else {
                let end = greedy_end
                    + find_ascii_ignore_case_rev(*needle.last().unwrap(), haystack)
                        .map_or(0, |i| i + 1);
                Some((start, greedy_end, end))
            }
        } else {
            let start = memchr(needle[0], &haystack[..haystack.len() - needle.len() + 1])?;
            let mut greedy_end = start + 1;
            haystack = &haystack[greedy_end..];
            for &c in &needle[1..] {
                let idx = memchr(c, haystack)? + 1;
                greedy_end += idx;
                haystack = &haystack[idx..];
            }
            if only_greedy {
                Some((start, greedy_end, greedy_end))
            } else {
                let end =
                    greedy_end + memrchr(*needle.last().unwrap(), haystack).map_or(0, |i| i + 1);
                Some((start, greedy_end, end))
            }
        }
    }

    pub(crate) fn prefilter_non_ascii(
        &self,
        haystack: &[char],
        needle: Utf32Str<'_>,
        only_greedy: bool,
    ) -> Option<(usize, usize)> {
        let needle_char = needle.get(0);
        let start = haystack[..haystack.len() - needle.len() + 1]
            .iter()
            .position(|c| c.normalize(&self.config) == needle_char)?;
        let needle_char = needle.last();
        if only_greedy {
            if haystack.len() - start < needle.len() {
                return None;
            }
            Some((start, start + 1))
        } else {
            let end = haystack.len()
                - haystack[start + 1..]
                    .iter()
                    .rev()
                    .position(|c| c.normalize(&self.config) == needle_char)?;
            if end - start < needle.len() {
                return None;
            }

            Some((start, end))
        }
    }
}
