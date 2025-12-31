//! Extension module for app.rs to minimize upstream merge conflicts.
//!
//! Contains Plan/EnterPlanMode/UserQuestion overlay construction logic.
//! Separated to keep app.rs modifications minimal during upstream syncs.

use codex_protocol::protocol_ext::UserQuestion;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

/// Build paragraph for Plan approval overlay.
pub fn build_plan_overlay_paragraph(
    plan_content: &str,
    plan_file_path: &str,
) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(vec![
            "Plan file: ".into(),
            plan_file_path.to_string().bold(),
        ]),
        Line::from(""),
        Line::from(plan_content.to_string()),
    ])
    .wrap(Wrap { trim: false })
}

/// Build paragraph for EnterPlanMode approval overlay.
pub fn build_enter_plan_mode_paragraph() -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(
            "The LLM is requesting to enter plan mode."
                .to_string()
                .bold(),
        ),
        Line::from(""),
        Line::from("In plan mode, the LLM will:"),
        Line::from("- Explore the codebase using read-only tools"),
        Line::from("- Design an implementation approach"),
        Line::from("- Write a plan file for your review"),
        Line::from("- Ask for approval before implementing"),
    ])
    .wrap(Wrap { trim: false })
}

/// Build paragraph for UserQuestion approval overlay.
pub fn build_user_question_paragraph(questions: &[UserQuestion]) -> Paragraph<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(
        "The LLM is asking for your input:".to_string().bold(),
    ));
    lines.push(Line::from(""));

    for (i, q) in questions.iter().enumerate() {
        lines.push(Line::from(format!("{}. {}", i + 1, q.question).bold()));
        lines.push(Line::from(format!("   [{}]", q.header)));
        for opt in &q.options {
            lines.push(Line::from(format!(
                "   • {} - {}",
                opt.label, opt.description
            )));
        }
        if q.multi_select {
            lines.push(Line::from("   (Multiple selections allowed)".to_string()));
        }
        lines.push(Line::from(""));
    }

    Paragraph::new(lines).wrap(Wrap { trim: false })
}
