use memchr::memmem;
use memchr::{Memchr, Memchr2};

use crate::chars::{AsciiChar, Char};
use crate::score::{BONUS_FIRST_CHAR_MULTIPLIER, SCORE_MATCH};
use crate::Matcher;

impl Matcher {
    pub(crate) fn substring_match_1_ascii<const INDICES: bool>(
        &mut self,
        haystack: &[u8],
        c: u8,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        let mut max_score = 0;
        let mut max_pos = 0;
        if self.config.ignore_case && c >= b'a' && c <= b'z' {
            for i in Memchr2::new(c, c - 32, haystack) {
                let prev_char_class = i
                    .checked_sub(1)
                    .map(|i| AsciiChar(haystack[i]).char_class(&self.config))
                    .unwrap_or(self.config.initial_char_class);
                let char_class = AsciiChar(haystack[i]).char_class(&self.config);
                let bonus = self.config.bonus_for(prev_char_class, char_class);
                let score = bonus * BONUS_FIRST_CHAR_MULTIPLIER + SCORE_MATCH;
                if score > max_score {
                    max_pos = i as u32;
                    max_score = score;
                    // can't get better than this
                    if bonus >= self.config.bonus_boundary_white {
                        break;
                    }
                }
            }
        } else {
            let char_class = AsciiChar(c).char_class(&self.config);
            for i in Memchr::new(c, haystack) {
                let prev_char_class = i
                    .checked_sub(1)
                    .map(|i| AsciiChar(haystack[i]).char_class(&self.config))
                    .unwrap_or(self.config.initial_char_class);
                let bonus = self.config.bonus_for(prev_char_class, char_class);
                let score = bonus * BONUS_FIRST_CHAR_MULTIPLIER + SCORE_MATCH;
                if score > max_score {
                    max_pos = i as u32;
                    max_score = score;
                    // can't get better than this
                    if bonus >= self.config.bonus_boundary_white {
                        break;
                    }
                }
            }
        }
        if max_score == 0 {
            return None;
        }

        if INDICES {
            indices.push(max_pos);
        }
        Some(max_score)
    }

    pub(crate) fn substring_match_ascii_with_prefilter(
        &mut self,
        haystack: &[u8],
        needle: &[u8],
        prefilter_len: usize,
        prefilter: impl Iterator<Item = usize>,
    ) -> (u16, usize) {
        let needle_without_prefilter = &needle[prefilter_len..];
        let mut max_score = 0;
        let mut max_pos = 0;
        for i in prefilter {
            let prev_char_class = i
                .checked_sub(1)
                .map(|i| AsciiChar(haystack[i]).char_class(&self.config))
                .unwrap_or(self.config.initial_char_class);
            let char_class = AsciiChar(haystack[i]).char_class(&self.config);
            let bonus = self.config.bonus_for(prev_char_class, char_class);
            let score = bonus * BONUS_FIRST_CHAR_MULTIPLIER + SCORE_MATCH;
            if score > max_score
                && haystack[i + prefilter_len..(i + needle.len()).min(haystack.len())]
                    .iter()
                    .map(|&c| AsciiChar(c).normalize(&self.config).0)
                    .eq(needle_without_prefilter.iter().copied())
            {
                max_pos = i;
                max_score = score;
                // can't get better than this
                if bonus >= self.config.bonus_boundary_white {
                    break;
                }
            }
        }
        (max_score, max_pos)
    }

    pub(crate) fn substring_match_ascii<const INDICES: bool>(
        &mut self,
        haystack: &[u8],
        needle: &[u8],
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        let mut max_score = 0;
        let mut max_pos = 0;
        if self.config.ignore_case {
            match needle.iter().position(|&c| c >= b'a' && c <= b'z') {
                // start with char do case insensitive search
                Some(0) => {
                    (max_score, max_pos) = self.substring_match_ascii_with_prefilter(
                        haystack,
                        needle,
                        1,
                        Memchr2::new(
                            needle[0],
                            needle[0] - 32,
                            &haystack[..haystack.len() - needle.len() + 1],
                        ),
                    );
                    if max_score == 0 {
                        return None;
                    }
                }
                Some(1) => {
                    (max_score, max_pos) = self.substring_match_ascii_with_prefilter(
                        haystack,
                        needle,
                        1,
                        Memchr::new(needle[0], &haystack[..haystack.len() - needle.len() + 1]),
                    );
                    if max_score == 0 {
                        return None;
                    }
                }
                Some(len) => {
                    (max_score, max_pos) = self.substring_match_ascii_with_prefilter(
                        haystack,
                        needle,
                        1,
                        memmem::find_iter(&haystack[..haystack.len() - needle.len() + len], needle),
                    );
                    if max_score == 0 {
                        return None;
                    }
                }
                // in case we don't have any letter in the needle
                // we can treat the search as case sensitive and use memmem directly which is way faster
                None => (),
            }
        }

        if max_score == 0 {
            let char_class = AsciiChar(needle[0]).char_class(&self.config);
            for i in memmem::find_iter(haystack, needle) {
                let prev_char_class = i
                    .checked_sub(1)
                    .map(|i| AsciiChar(haystack[i]).char_class(&self.config))
                    .unwrap_or(self.config.initial_char_class);
                let bonus = self.config.bonus_for(prev_char_class, char_class);
                let score = bonus * BONUS_FIRST_CHAR_MULTIPLIER + SCORE_MATCH;
                if score > max_score {
                    max_pos = i;
                    max_score = score;
                    // can't get better than this
                    if bonus >= self.config.bonus_boundary_white {
                        break;
                    }
                }
            }
            if max_score == 0 {
                return None;
            }
        }
        let score = self.calculate_score::<INDICES, _, _>(
            AsciiChar::cast(haystack),
            AsciiChar::cast(needle),
            max_pos,
            max_pos + needle.len(),
            indices,
        );
        Some(score)
    }

    pub(crate) fn substring_match_1_non_ascii<const INDICES: bool>(
        &mut self,
        haystack: &[char],
        needle: char,
        start: usize,
        indices: &mut Vec<u32>,
    ) -> u16 {
        let mut max_score = 0;
        let mut max_pos = 0;
        let mut prev_class = start
            .checked_sub(1)
            .map(|i| haystack[i].char_class(&self.config))
            .unwrap_or(self.config.initial_char_class);
        for (i, &c) in haystack[start..].iter().enumerate() {
            let (c, char_class) = c.char_class_and_normalize(&self.config);
            if c != needle {
                continue;
            }
            let bonus = self.config.bonus_for(prev_class, char_class);
            prev_class = char_class;
            let score = bonus * BONUS_FIRST_CHAR_MULTIPLIER + SCORE_MATCH;
            if score > max_score {
                max_pos = i as u32;
                max_score = score;
                // can't get better than this
                if bonus >= self.config.bonus_boundary_white {
                    break;
                }
            }
        }

        if INDICES {
            indices.push(max_pos + start as u32);
        }
        max_score
    }

    pub(crate) fn substring_match_non_ascii<const INDICES: bool, N>(
        &mut self,
        haystack: &[char],
        needle: &[N],
        start: usize,
        indices: &mut Vec<u32>,
    ) -> Option<u16>
    where
        N: Char,
        char: PartialEq<N>,
    {
        let mut max_score = 0;
        let mut max_pos = 0;
        let mut prev_class = start
            .checked_sub(1)
            .map(|i| haystack[i].char_class(&self.config))
            .unwrap_or(self.config.initial_char_class);
        let end = haystack.len() - needle.len();
        for (i, &c) in haystack[start..end].iter().enumerate() {
            let (c, char_class) = c.char_class_and_normalize(&self.config);
            if c != needle[0] {
                continue;
            }
            let bonus = self.config.bonus_for(prev_class, char_class);
            prev_class = char_class;
            let score = bonus * BONUS_FIRST_CHAR_MULTIPLIER + SCORE_MATCH;
            if score > max_score
                && haystack[start + i + 1..start + i + needle.len()]
                    .iter()
                    .map(|c| c.normalize(&self.config))
                    .eq(needle[1..].iter().copied())
            {
                max_pos = i;
                max_score = score;
                // can't get better than this
                if bonus >= self.config.bonus_boundary_white {
                    break;
                }
            }
        }
        if max_score == 0 {
            return None;
        }

        let score = self.calculate_score::<INDICES, _, _>(
            haystack,
            needle,
            start + max_pos,
            start + max_pos + needle.len(),
            indices,
        );
        Some(score)
    }
}
