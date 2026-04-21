use crate::compact::content_items_to_text;
use crate::event_mapping::is_contextual_user_message_content;
use crate::session::session::Session;
use codex_protocol::models::ResponseItem;
use codex_realtime::RealtimeStartupContext;
use codex_realtime::RealtimeStartupContextTurn;
use codex_thread_store::ListThreadsParams;
use codex_thread_store::SortDirection;
use codex_thread_store::StoredThread;
use codex_thread_store::ThreadSortKey;
use codex_thread_store::ThreadStore;
use dirs::home_dir;
use std::mem::take;
use tracing::warn;

const MAX_RECENT_THREADS: usize = 40;

pub(crate) async fn build_realtime_startup_context(
    sess: &Session,
    budget_tokens: usize,
) -> Option<String> {
    let config = sess.get_config().await;
    let history = sess.clone_history().await;
    let current_thread_turns = current_thread_turns(history.raw_items());
    let recent_threads = load_recent_threads(sess).await;
    let context = RealtimeStartupContext {
        cwd: config.cwd.clone(),
        current_thread_turns,
        recent_threads,
        user_root: home_dir(),
    };
    codex_realtime::build_realtime_startup_context(&context, budget_tokens).await
}

async fn load_recent_threads(sess: &Session) -> Vec<StoredThread> {
    match sess
        .services
        .thread_store
        .list_threads(ListThreadsParams {
            page_size: MAX_RECENT_THREADS,
            cursor: None,
            sort_key: ThreadSortKey::UpdatedAt,
            sort_direction: SortDirection::Desc,
            allowed_sources: Vec::new(),
            model_providers: None,
            archived: false,
            search_term: None,
        })
        .await
    {
        Ok(page) => page.items,
        Err(err) => {
            warn!("failed to load realtime startup threads from thread store: {err}");
            Vec::new()
        }
    }
}

fn current_thread_turns(items: &[ResponseItem]) -> Vec<RealtimeStartupContextTurn> {
    let mut turns = Vec::new();
    let mut current_user = Vec::new();
    let mut current_assistant = Vec::new();

    for item in items {
        match item {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                if is_contextual_user_message_content(content) {
                    continue;
                }
                let Some(text) = content_items_to_text(content)
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                else {
                    continue;
                };
                if !current_user.is_empty() || !current_assistant.is_empty() {
                    turns.push(RealtimeStartupContextTurn {
                        user_messages: take(&mut current_user),
                        assistant_messages: take(&mut current_assistant),
                    });
                }
                current_user.push(text);
            }
            ResponseItem::Message { role, content, .. } if role == "assistant" => {
                let Some(text) = content_items_to_text(content)
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                else {
                    continue;
                };
                if current_user.is_empty() && current_assistant.is_empty() {
                    continue;
                }
                current_assistant.push(text);
            }
            _ => {}
        }
    }

    if !current_user.is_empty() || !current_assistant.is_empty() {
        turns.push(RealtimeStartupContextTurn {
            user_messages: current_user,
            assistant_messages: current_assistant,
        });
    }

    turns
}

#[cfg(test)]
fn build_current_thread_section(items: &[ResponseItem]) -> Option<String> {
    codex_realtime::build_current_thread_section(&current_thread_turns(items))
}

#[cfg(test)]
pub(crate) use codex_realtime::CURRENT_THREAD_SECTION_TOKEN_BUDGET;
#[cfg(test)]
pub(crate) use codex_realtime::NOTES_SECTION_TOKEN_BUDGET;
pub(crate) use codex_realtime::REALTIME_TURN_TOKEN_BUDGET;
#[cfg(test)]
pub(crate) use codex_realtime::RECENT_WORK_SECTION_TOKEN_BUDGET;
#[cfg(test)]
pub(crate) use codex_realtime::STARTUP_CONTEXT_HEADER;
#[cfg(test)]
pub(crate) use codex_realtime::WORKSPACE_SECTION_TOKEN_BUDGET;
#[cfg(test)]
use codex_realtime::build_recent_work_section;
#[cfg(test)]
use codex_realtime::build_workspace_section_with_user_root;
#[cfg(test)]
use codex_realtime::format_section;
#[cfg(test)]
use codex_realtime::format_startup_context_blob;
pub(crate) use codex_realtime::truncate_realtime_text_to_token_budget;

#[cfg(test)]
#[path = "realtime_context_tests.rs"]
mod tests;
