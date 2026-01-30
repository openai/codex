use crate::tagged_block_parser::TagSpec;
use crate::tagged_block_parser::TaggedLineParser;
use crate::tagged_block_parser::TaggedLineSegment;

const OPEN_TAG: &str = "<proposed_plan>";
const CLOSE_TAG: &str = "</proposed_plan>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanTag {
    ProposedPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProposedPlanSegment {
    Normal(String),
    ProposedPlanStart,
    ProposedPlanDelta(String),
    ProposedPlanEnd,
}

/// Parser for `<proposed_plan>` blocks emitted in plan mode.
///
/// This is a thin wrapper around the generic line-based tag parser. It maps
/// tag-aware segments into plan-specific segments for downstream consumers.
#[derive(Debug)]
pub(crate) struct ProposedPlanParser {
    parser: TaggedLineParser<PlanTag>,
}

impl ProposedPlanParser {
    pub(crate) fn new() -> Self {
        Self {
            parser: TaggedLineParser::new(vec![TagSpec {
                open: OPEN_TAG,
                close: CLOSE_TAG,
                tag: PlanTag::ProposedPlan,
            }]),
        }
    }

    pub(crate) fn parse(&mut self, delta: &str) -> Vec<ProposedPlanSegment> {
        self.parser
            .parse(delta)
            .into_iter()
            .map(map_plan_segment)
            .collect()
    }

    pub(crate) fn finish(&mut self) -> Vec<ProposedPlanSegment> {
        self.parser
            .finish()
            .into_iter()
            .map(map_plan_segment)
            .collect()
    }
}

fn map_plan_segment(segment: TaggedLineSegment<PlanTag>) -> ProposedPlanSegment {
    match segment {
        TaggedLineSegment::Normal(text) => ProposedPlanSegment::Normal(text),
        TaggedLineSegment::TagStart(PlanTag::ProposedPlan) => {
            ProposedPlanSegment::ProposedPlanStart
        }
        TaggedLineSegment::TagDelta(PlanTag::ProposedPlan, text) => {
            ProposedPlanSegment::ProposedPlanDelta(text)
        }
        TaggedLineSegment::TagEnd(PlanTag::ProposedPlan) => ProposedPlanSegment::ProposedPlanEnd,
    }
}

pub(crate) fn strip_proposed_plan_blocks(text: &str) -> String {
    let mut parser = ProposedPlanParser::new();
    let mut out = String::new();
    for segment in parser.parse(text).into_iter().chain(parser.finish()) {
        if let ProposedPlanSegment::Normal(delta) = segment {
            out.push_str(&delta);
        }
    }
    out
}

pub(crate) fn extract_proposed_plan_text(text: &str) -> Option<String> {
    let mut parser = ProposedPlanParser::new();
    let mut plan_text = String::new();
    let mut saw_plan_block = false;
    for segment in parser.parse(text).into_iter().chain(parser.finish()) {
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
    use super::strip_proposed_plan_blocks;
    use pretty_assertions::assert_eq;

    #[test]
    fn streams_proposed_plan_segments() {
        let mut parser = ProposedPlanParser::new();
        let mut segments = Vec::new();

        for chunk in [
            "Intro text\n<prop",
            "osed_plan>\n- step 1\n",
            "</proposed_plan>\nOutro",
        ] {
            segments.extend(parser.parse(chunk));
        }
        segments.extend(parser.finish());

        assert_eq!(
            segments,
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
        let mut segments = parser.parse("  <proposed_plan> extra\n");
        segments.extend(parser.finish());

        assert_eq!(
            segments,
            vec![ProposedPlanSegment::Normal(
                "  <proposed_plan> extra\n".to_string()
            )]
        );
    }

    #[test]
    fn closes_unterminated_plan_block_on_finish() {
        let mut parser = ProposedPlanParser::new();
        let mut segments = parser.parse("<proposed_plan>\n- step 1\n");
        segments.extend(parser.finish());

        assert_eq!(
            segments,
            vec![
                ProposedPlanSegment::ProposedPlanStart,
                ProposedPlanSegment::ProposedPlanDelta("- step 1\n".to_string()),
                ProposedPlanSegment::ProposedPlanEnd,
            ]
        );
    }

    #[test]
    fn closes_tag_line_without_trailing_newline() {
        let mut parser = ProposedPlanParser::new();
        let mut segments = parser.parse("<proposed_plan>\n- step 1\n</proposed_plan>");
        segments.extend(parser.finish());

        assert_eq!(
            segments,
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
}
