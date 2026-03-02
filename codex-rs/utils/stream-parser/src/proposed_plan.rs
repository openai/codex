use crate::StreamTextChunk;
use crate::StreamTextParser;
use crate::tagged_line_parser::TagSpec;
use crate::tagged_line_parser::TaggedLineParser;
use crate::tagged_line_parser::TaggedLineSegment;

const OPEN_TAG: &str = "<proposed_plan>";
const CLOSE_TAG: &str = "</proposed_plan>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanTag {
    ProposedPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposedPlanSegment {
    Normal(String),
    ProposedPlanStart,
    ProposedPlanDelta(String),
    ProposedPlanEnd,
}

/// Parser for `<proposed_plan>` blocks emitted in plan mode.
///
/// Implements [`StreamTextParser`] so callers can consume:
/// - `visible_text`: normal assistant text with plan blocks removed
/// - `extracted`: ordered plan segments (includes `Normal(...)` segments for ordering fidelity)
#[derive(Debug)]
pub struct ProposedPlanParser {
    parser: TaggedLineParser<PlanTag>,
}

impl ProposedPlanParser {
    pub fn new() -> Self {
        Self {
            parser: TaggedLineParser::new(vec![TagSpec {
                open: OPEN_TAG,
                close: CLOSE_TAG,
                tag: PlanTag::ProposedPlan,
            }]),
        }
    }
}

impl Default for ProposedPlanParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamTextParser for ProposedPlanParser {
    type Extracted = ProposedPlanSegment;

    fn push_str(&mut self, chunk: &str) -> StreamTextChunk<Self::Extracted> {
        map_segments(self.parser.parse(chunk))
    }

    fn finish(&mut self) -> StreamTextChunk<Self::Extracted> {
        map_segments(self.parser.finish())
    }
}

fn map_segments(segments: Vec<TaggedLineSegment<PlanTag>>) -> StreamTextChunk<ProposedPlanSegment> {
    let mut out = StreamTextChunk::default();
    for segment in segments {
        let mapped = match segment {
            TaggedLineSegment::Normal(text) => ProposedPlanSegment::Normal(text),
            TaggedLineSegment::TagStart(PlanTag::ProposedPlan) => {
                ProposedPlanSegment::ProposedPlanStart
            }
            TaggedLineSegment::TagDelta(PlanTag::ProposedPlan, text) => {
                ProposedPlanSegment::ProposedPlanDelta(text)
            }
            TaggedLineSegment::TagEnd(PlanTag::ProposedPlan) => {
                ProposedPlanSegment::ProposedPlanEnd
            }
        };
        if let ProposedPlanSegment::Normal(text) = &mapped {
            out.visible_text.push_str(text);
        }
        out.extracted.push(mapped);
    }
    out
}

pub fn strip_proposed_plan_blocks(text: &str) -> String {
    let mut parser = ProposedPlanParser::new();
    let mut out = parser.push_str(text).visible_text;
    out.push_str(&parser.finish().visible_text);
    out
}

pub fn extract_proposed_plan_text(text: &str) -> Option<String> {
    let mut parser = ProposedPlanParser::new();
    let mut plan_text = String::new();
    let mut saw_plan_block = false;
    for segment in parser
        .push_str(text)
        .extracted
        .into_iter()
        .chain(parser.finish().extracted)
    {
        match segment {
            ProposedPlanSegment::ProposedPlanStart => {
                saw_plan_block = true;
                plan_text.clear();
            }
            ProposedPlanSegment::ProposedPlanDelta(delta) => {
                plan_text.push_str(&delta);
            }
            ProposedPlanSegment::ProposedPlanEnd | ProposedPlanSegment::Normal(_) => {}
        }
    }
    saw_plan_block.then_some(plan_text)
}

#[cfg(test)]
mod tests {
    use super::ProposedPlanParser;
    use super::ProposedPlanSegment;
    use super::extract_proposed_plan_text;
    use super::strip_proposed_plan_blocks;
    use crate::StreamTextChunk;
    use crate::StreamTextParser;
    use pretty_assertions::assert_eq;

    fn collect_chunks<P>(parser: &mut P, chunks: &[&str]) -> StreamTextChunk<P::Extracted>
    where
        P: StreamTextParser,
    {
        let mut all = StreamTextChunk::default();
        for chunk in chunks {
            let next = parser.push_str(chunk);
            all.visible_text.push_str(&next.visible_text);
            all.extracted.extend(next.extracted);
        }
        let tail = parser.finish();
        all.visible_text.push_str(&tail.visible_text);
        all.extracted.extend(tail.extracted);
        all
    }

    #[test]
    fn streams_proposed_plan_segments_and_visible_text() {
        let mut parser = ProposedPlanParser::new();
        let out = collect_chunks(
            &mut parser,
            &[
                "Intro text\n<prop",
                "osed_plan>\n- step 1\n",
                "</proposed_plan>\nOutro",
            ],
        );

        assert_eq!(out.visible_text, "Intro text\nOutro");
        assert_eq!(
            out.extracted,
            vec![
                ProposedPlanSegment::Normal("Intro text\n".to_string()),
                ProposedPlanSegment::ProposedPlanStart,
                ProposedPlanSegment::ProposedPlanDelta("- step 1\n".to_string()),
                ProposedPlanSegment::ProposedPlanEnd,
                ProposedPlanSegment::Normal("Outro".to_string()),
            ]
        );
    }

    #[test]
    fn preserves_non_tag_lines() {
        let mut parser = ProposedPlanParser::new();
        let out = collect_chunks(&mut parser, &["  <proposed_plan> extra\n"]);

        assert_eq!(out.visible_text, "  <proposed_plan> extra\n");
        assert_eq!(
            out.extracted,
            vec![ProposedPlanSegment::Normal(
                "  <proposed_plan> extra\n".to_string()
            )]
        );
    }

    #[test]
    fn closes_unterminated_plan_block_on_finish() {
        let mut parser = ProposedPlanParser::new();
        let out = collect_chunks(&mut parser, &["<proposed_plan>\n- step 1\n"]);

        assert_eq!(out.visible_text, "");
        assert_eq!(
            out.extracted,
            vec![
                ProposedPlanSegment::ProposedPlanStart,
                ProposedPlanSegment::ProposedPlanDelta("- step 1\n".to_string()),
                ProposedPlanSegment::ProposedPlanEnd,
            ]
        );
    }

    #[test]
    fn strips_proposed_plan_blocks_from_text() {
        let text = "before\n<proposed_plan>\n- step\n</proposed_plan>\nafter";
        assert_eq!(strip_proposed_plan_blocks(text), "before\nafter");
    }

    #[test]
    fn extracts_proposed_plan_text() {
        let text = "before\n<proposed_plan>\n- step\n</proposed_plan>\nafter";
        assert_eq!(
            extract_proposed_plan_text(text),
            Some("- step\n".to_string())
        );
    }
}
