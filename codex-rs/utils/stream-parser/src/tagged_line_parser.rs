//! Line-based tag block parsing for streamed text.
//!
//! The parser buffers each line until it can disprove that the line is a tag,
//! which is required for tags that must appear alone on a line.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TagSpec<T> {
    pub(crate) open: &'static str,
    pub(crate) close: &'static str,
    pub(crate) tag: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaggedLineSegment<T> {
    Normal(String),
    TagStart(T),
    TagDelta(T, String),
    TagEnd(T),
}

/// Stateful line parser that splits input into normal text vs tag blocks.
#[derive(Debug, Default)]
pub(crate) struct TaggedLineParser<T>
where
    T: Copy + Eq,
{
    specs: Vec<TagSpec<T>>,
    active_tag: Option<T>,
    detect_tag: bool,
    line_buffer: String,
}

impl<T> TaggedLineParser<T>
where
    T: Copy + Eq,
{
    pub(crate) fn new(specs: Vec<TagSpec<T>>) -> Self {
        Self {
            specs,
            active_tag: None,
            detect_tag: true,
            line_buffer: String::new(),
        }
    }

    pub(crate) fn parse(&mut self, delta: &str) -> Vec<TaggedLineSegment<T>> {
        let mut segments = Vec::new();
        let mut run = String::new();

        for ch in delta.chars() {
            if self.detect_tag {
                if !run.is_empty() {
                    self.push_text(std::mem::take(&mut run), &mut segments);
                }
                self.line_buffer.push(ch);
                if ch == '\n' {
                    self.finish_line(&mut segments);
                    continue;
                }
                let slug = self.line_buffer.trim_start();
                if slug.is_empty() || self.is_tag_prefix(slug) {
                    continue;
                }
                let buffered = std::mem::take(&mut self.line_buffer);
                self.detect_tag = false;
                self.push_text(buffered, &mut segments);
                continue;
            }

            run.push(ch);
            if ch == '\n' {
                self.push_text(std::mem::take(&mut run), &mut segments);
                self.detect_tag = true;
            }
        }

        if !run.is_empty() {
            self.push_text(run, &mut segments);
        }

        segments
    }

    pub(crate) fn finish(&mut self) -> Vec<TaggedLineSegment<T>> {
        let mut segments = Vec::new();
        if !self.line_buffer.is_empty() {
            let buffered = std::mem::take(&mut self.line_buffer);
            let without_newline = buffered.strip_suffix('\n').unwrap_or(&buffered);
            let slug = without_newline.trim_start().trim_end();

            if let Some(tag) = self.match_open(slug)
                && self.active_tag.is_none()
            {
                push_segment(&mut segments, TaggedLineSegment::TagStart(tag));
                self.active_tag = Some(tag);
            } else if let Some(tag) = self.match_close(slug)
                && self.active_tag == Some(tag)
            {
                push_segment(&mut segments, TaggedLineSegment::TagEnd(tag));
                self.active_tag = None;
            } else {
                self.push_text(buffered, &mut segments);
            }
        }
        if let Some(tag) = self.active_tag.take() {
            push_segment(&mut segments, TaggedLineSegment::TagEnd(tag));
        }
        self.detect_tag = true;
        segments
    }

    fn finish_line(&mut self, segments: &mut Vec<TaggedLineSegment<T>>) {
        let line = std::mem::take(&mut self.line_buffer);
        let without_newline = line.strip_suffix('\n').unwrap_or(&line);
        let slug = without_newline.trim_start().trim_end();

        if let Some(tag) = self.match_open(slug)
            && self.active_tag.is_none()
        {
            push_segment(segments, TaggedLineSegment::TagStart(tag));
            self.active_tag = Some(tag);
            self.detect_tag = true;
            return;
        }

        if let Some(tag) = self.match_close(slug)
            && self.active_tag == Some(tag)
        {
            push_segment(segments, TaggedLineSegment::TagEnd(tag));
            self.active_tag = None;
            self.detect_tag = true;
            return;
        }

        self.detect_tag = true;
        self.push_text(line, segments);
    }

    fn push_text(&self, text: String, segments: &mut Vec<TaggedLineSegment<T>>) {
        if let Some(tag) = self.active_tag {
            push_segment(segments, TaggedLineSegment::TagDelta(tag, text));
        } else {
            push_segment(segments, TaggedLineSegment::Normal(text));
        }
    }

    fn is_tag_prefix(&self, slug: &str) -> bool {
        let slug = slug.trim_end();
        self.specs
            .iter()
            .any(|spec| spec.open.starts_with(slug) || spec.close.starts_with(slug))
    }

    fn match_open(&self, slug: &str) -> Option<T> {
        self.specs
            .iter()
            .find(|spec| spec.open == slug)
            .map(|spec| spec.tag)
    }

    fn match_close(&self, slug: &str) -> Option<T> {
        self.specs
            .iter()
            .find(|spec| spec.close == slug)
            .map(|spec| spec.tag)
    }
}

fn push_segment<T>(segments: &mut Vec<TaggedLineSegment<T>>, segment: TaggedLineSegment<T>)
where
    T: Copy + Eq,
{
    match segment {
        TaggedLineSegment::Normal(delta) => {
            if delta.is_empty() {
                return;
            }
            if let Some(TaggedLineSegment::Normal(existing)) = segments.last_mut() {
                existing.push_str(&delta);
                return;
            }
            segments.push(TaggedLineSegment::Normal(delta));
        }
        TaggedLineSegment::TagDelta(tag, delta) => {
            if delta.is_empty() {
                return;
            }
            if let Some(TaggedLineSegment::TagDelta(existing_tag, existing)) = segments.last_mut()
                && *existing_tag == tag
            {
                existing.push_str(&delta);
                return;
            }
            segments.push(TaggedLineSegment::TagDelta(tag, delta));
        }
        TaggedLineSegment::TagStart(tag) => segments.push(TaggedLineSegment::TagStart(tag)),
        TaggedLineSegment::TagEnd(tag) => segments.push(TaggedLineSegment::TagEnd(tag)),
    }
}

#[cfg(test)]
mod tests {
    use super::TagSpec;
    use super::TaggedLineParser;
    use super::TaggedLineSegment;
    use pretty_assertions::assert_eq;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Tag {
        Block,
    }

    fn parser() -> TaggedLineParser<Tag> {
        TaggedLineParser::new(vec![TagSpec {
            open: "<tag>",
            close: "</tag>",
            tag: Tag::Block,
        }])
    }

    #[test]
    fn buffers_prefix_until_tag_is_decided() {
        let mut parser = parser();
        let mut segments = parser.parse("<t");
        segments.extend(parser.parse("ag>\nline\n</tag>\n"));
        segments.extend(parser.finish());

        assert_eq!(
            segments,
            vec![
                TaggedLineSegment::TagStart(Tag::Block),
                TaggedLineSegment::TagDelta(Tag::Block, "line\n".to_string()),
                TaggedLineSegment::TagEnd(Tag::Block),
            ]
        );
    }

    #[test]
    fn rejects_tag_lines_with_extra_text() {
        let mut parser = parser();
        let mut segments = parser.parse("<tag> extra\n");
        segments.extend(parser.finish());

        assert_eq!(
            segments,
            vec![TaggedLineSegment::Normal("<tag> extra\n".to_string())]
        );
    }
}
