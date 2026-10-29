//! Builds the configured status line for both the footer and its live picker preview.

use super::STATUS_LINE_SEPARATOR;
use super::footer_content::FooterContent;
use super::status_line_from_segments;
use super::status_line_setup::StatusLineItem;
use crate::ui_tree::Key;
use crate::ui_tree::LinkHref;
use crate::ui_tree::LinkProps;
use crate::ui_tree::StackProps;
use crate::ui_tree::UiNode;
use codex_protocol::ThreadId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StatusLineRenderer {
    Native,
    Extension,
}

impl StatusLineRenderer {
    pub(crate) fn content(
        self,
        segments: impl IntoIterator<Item = (StatusLineItem, String)>,
        use_theme_colors: bool,
        thread_id: Option<ThreadId>,
        pull_request_url: Option<&str>,
    ) -> Option<FooterContent> {
        if self == Self::Native {
            return status_line_from_segments(
                segments,
                use_theme_colors,
                thread_id,
                pull_request_url,
            )
            .map(FooterContent::HyperlinkLine);
        }

        let mut children = Vec::new();
        for (item, text) in segments {
            if !children.is_empty() {
                children.push(Some(UiNode::text(/*key*/ None, STATUS_LINE_SEPARATOR)));
            }
            let key = Some(Key::String(item.to_string()));
            let node = if item == StatusLineItem::PullRequestNumber
                && let Some(href) = pull_request_url.and_then(|url| LinkHref::try_from(url).ok())
            {
                UiNode::Link {
                    key,
                    props: LinkProps { href, title: None },
                    children: vec![Some(UiNode::text(/*key*/ None, text))],
                }
            } else {
                UiNode::text(key, text)
            };
            children.push(Some(node));
        }
        let tree = UiNode::Row {
            key: Some(Key::String("status-line".to_string())),
            props: StackProps::default(),
            children,
        };
        Some(FooterContent::Extension { content: tree })
    }
}
