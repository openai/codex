use super::super::TextArea;
use super::VimTextObjectScope;
use std::ops::Range;

#[derive(Debug)]
enum ParsedTag {
    Open { name: String, range: Range<usize> },
    Close { name: String, range: Range<usize> },
    Ignored { end: usize },
}

impl ParsedTag {
    fn end(&self) -> usize {
        match self {
            Self::Open { range, .. } | Self::Close { range, .. } => range.end,
            Self::Ignored { end } => *end,
        }
    }
}

impl TextArea {
    pub(in super::super) fn tag_text_object_range(
        &self,
        scope: VimTextObjectScope,
    ) -> Option<Range<usize>> {
        let mut open_tags: Vec<(String, Range<usize>)> = Vec::new();
        let mut best = None;
        let mut pos = 0;
        while let Some(offset) = self.text[pos..].find('<') {
            let start = pos + offset;
            if self.is_inside_element(start) {
                pos = start + '<'.len_utf8();
                continue;
            }
            let Some(tag) = self.parse_tag(start) else {
                pos = start + '<'.len_utf8();
                continue;
            };
            pos = tag.end();
            match tag {
                ParsedTag::Open { name, range } => open_tags.push((name, range)),
                ParsedTag::Close { name, range } => {
                    let Some((open_name, open_range)) = open_tags.last() else {
                        continue;
                    };
                    if *open_name != name {
                        continue;
                    }
                    let open_range = open_range.clone();
                    open_tags.pop();
                    if open_range.start <= self.cursor_pos && self.cursor_pos <= range.end {
                        let candidate = match scope {
                            VimTextObjectScope::Inner => open_range.end..range.start,
                            VimTextObjectScope::Around => open_range.start..range.end,
                        };
                        if best
                            .as_ref()
                            .is_none_or(|current: &Range<usize>| candidate.len() < current.len())
                        {
                            best = Some(candidate);
                        }
                    }
                }
                ParsedTag::Ignored { .. } => {}
            }
        }
        best
    }

    fn parse_tag(&self, start: usize) -> Option<ParsedTag> {
        let rest = &self.text[start..];
        if rest.starts_with("<!--") {
            return rest.find("-->").map(|offset| ParsedTag::Ignored {
                end: start + offset + "-->".len(),
            });
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            return self
                .tag_end(start + '<'.len_utf8())
                .map(|end| ParsedTag::Ignored { end });
        }

        let mut pos = start + '<'.len_utf8();
        let closing = self.text[pos..].starts_with('/');
        if closing {
            pos += '/'.len_utf8();
        }
        let name_start = pos;
        let first = self.text[pos..].chars().next()?;
        if !is_tag_name_start(first) {
            return None;
        }
        pos += first.len_utf8();
        while let Some(ch) = self.text[pos..].chars().next() {
            if !is_tag_name_char(ch) {
                break;
            }
            pos += ch.len_utf8();
        }
        let name = self.text[name_start..pos].to_string();
        let end = self.tag_end(pos)?;
        if closing {
            if !self.text[pos..end - '>'.len_utf8()].trim().is_empty() {
                return None;
            }
            return Some(ParsedTag::Close {
                name,
                range: start..end,
            });
        }
        let body = self.text[pos..end - '>'.len_utf8()].trim_end();
        if body.ends_with('/') {
            return Some(ParsedTag::Ignored { end });
        }
        Some(ParsedTag::Open {
            name,
            range: start..end,
        })
    }

    fn tag_end(&self, mut pos: usize) -> Option<usize> {
        let mut quote = None;
        while let Some(ch) = self.text[pos..].chars().next() {
            if self.is_inside_element(pos) {
                return None;
            }
            if let Some(open_quote) = quote {
                if ch == open_quote {
                    quote = None;
                }
            } else if matches!(ch, '"' | '\'') {
                quote = Some(ch);
            } else if ch == '>' {
                return Some(pos + ch.len_utf8());
            }
            pos += ch.len_utf8();
        }
        None
    }
}

fn is_tag_name_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_tag_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':')
}
