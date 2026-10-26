mod answered_question;
pub use answered_question::AnsweredQuestion;
mod additional_context;
mod annotated_content;
mod fragment;
mod recap_prompt;

pub use additional_context::AdditionalContextDeveloperFragment;
pub use additional_context::AdditionalContextToolOutput;
pub use additional_context::AdditionalContextUserFragment;
pub use annotated_content::AnnotatedContent;
pub use annotated_content::message_from_parts;
pub use annotated_content::set_annotated_content;
pub use annotated_content::to_annotated_content;
pub use annotated_content::truncate_annotated_text;
pub use fragment::AttributedFragment;
pub use fragment::ContextualUserFragment;
pub use fragment::RenderedFragment;

pub use recap_prompt::RecapPrompt;

/// Shared classification for compaction summaries produced by core and filtered by Guardian.
pub const COMPACTION_SUMMARY_KIND: &str = "compaction.summary";
