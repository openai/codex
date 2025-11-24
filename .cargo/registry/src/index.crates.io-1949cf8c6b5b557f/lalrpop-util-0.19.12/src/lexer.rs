use std::{fmt, marker::PhantomData};

use crate::ParseError;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Token<'input>(pub usize, pub &'input str);
impl<'a> fmt::Display for Token<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt::Display::fmt(self.1, formatter)
    }
}

struct RegexEntry {
    regex: regex::Regex,
    skip: bool,
}

pub struct MatcherBuilder {
    regex_set: regex::RegexSet,
    regex_vec: Vec<RegexEntry>,
}

impl MatcherBuilder {
    pub fn new<S>(
        exprs: impl IntoIterator<Item = (S, bool)>,
    ) -> Result<MatcherBuilder, regex::Error>
    where
        S: AsRef<str>,
    {
        let exprs = exprs.into_iter();
        let mut regex_vec = Vec::with_capacity(exprs.size_hint().0);
        let mut first_error = None;
        let regex_set_result = regex::RegexSet::new(exprs.scan((), |_, (s, skip)| {
            regex_vec.push(match regex::Regex::new(s.as_ref()) {
                Ok(regex) => RegexEntry { regex, skip },
                Err(err) => {
                    first_error = Some(err);
                    return None;
                }
            });
            Some(s)
        }));

        if let Some(err) = first_error {
            return Err(err);
        }
        let regex_set = regex_set_result?;

        Ok(MatcherBuilder {
            regex_set,
            regex_vec,
        })
    }
    pub fn matcher<'input, 'builder, E>(
        &'builder self,
        s: &'input str,
    ) -> Matcher<'input, 'builder, E> {
        Matcher {
            text: s,
            consumed: 0,
            regex_set: &self.regex_set,
            regex_vec: &self.regex_vec,
            _marker: PhantomData,
        }
    }
}

pub struct Matcher<'input, 'builder, E> {
    text: &'input str,
    consumed: usize,
    regex_set: &'builder regex::RegexSet,
    regex_vec: &'builder Vec<RegexEntry>,
    _marker: PhantomData<fn() -> E>,
}

impl<'input, 'builder, E> Iterator for Matcher<'input, 'builder, E> {
    type Item = Result<(usize, Token<'input>, usize), ParseError<usize, Token<'input>, E>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let text = self.text;
            let start_offset = self.consumed;
            if text.is_empty() {
                self.consumed = start_offset;
                return None;
            } else {
                let matches = self.regex_set.matches(text);
                if !matches.matched_any() {
                    return Some(Err(ParseError::InvalidToken {
                        location: start_offset,
                    }));
                } else {
                    let mut longest_match = 0;
                    let mut index = 0;
                    let mut skip = false;
                    for i in matches.iter() {
                        let entry = &self.regex_vec[i];
                        let match_ = entry.regex.find(text).unwrap();
                        let len = match_.end();
                        if len >= longest_match {
                            longest_match = len;
                            index = i;
                            skip = entry.skip;
                        }
                    }

                    let result = &text[..longest_match];
                    let remaining = &text[longest_match..];
                    let end_offset = start_offset + longest_match;
                    self.text = remaining;
                    self.consumed = end_offset;

                    // Skip any whitespace matches
                    if skip {
                        if longest_match == 0 {
                            return Some(Err(ParseError::InvalidToken {
                                location: start_offset,
                            }));
                        }
                        continue;
                    }

                    return Some(Ok((start_offset, Token(index, result), end_offset)));
                }
            }
        }
    }
}
