use std::path::Path;

use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListCwdFilter;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;

use super::*;
use crate::resume_picker::picker_cwd_filter;

#[test]
fn local_picker_thread_list_params_include_cwd_filter() {
    let cwd_filter = picker_cwd_filter(
        Path::new("/tmp/project"),
        /*show_all*/ false,
        /*uses_remote_workspace*/ false,
        /*remote_cwd_override*/ None,
    );
    let params = thread_list_params(
        Some(String::from("cursor-1")),
        cwd_filter.as_deref(),
        ProviderFilter::MatchDefault(String::from("openai")),
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ false,
    );

    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::One(String::from("/tmp/project")))
    );
}

#[test]
fn remote_thread_list_params_omit_provider_filter() {
    let params = thread_list_params(
        Some(String::from("cursor-1")),
        Some(Path::new("repo/on/server")),
        ProviderFilter::Any,
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ false,
    );

    assert_eq!(params.cursor, Some(String::from("cursor-1")));
    assert_eq!(params.model_providers, None);
    assert_eq!(
        params.source_kinds,
        Some(vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode])
    );
    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::One(String::from("repo/on/server")))
    );
}

#[test]
fn remote_thread_list_params_can_include_non_interactive_sources() {
    let params = thread_list_params(
        Some(String::from("cursor-1")),
        /*cwd_filter*/ None,
        ProviderFilter::Any,
        ThreadSortKey::UpdatedAt,
        /*include_non_interactive*/ true,
    );

    assert_eq!(params.cursor, Some(String::from("cursor-1")));
    assert_eq!(params.model_providers, None);
    let source_kinds = crate::resume_source_kinds(/*include_non_interactive*/ true);
    assert_eq!(params.source_kinds, Some(source_kinds));
}

#[test]
fn app_server_row_keeps_pathless_threads() {
    let thread_id = ThreadId::new();
    let thread = Thread {
        id: thread_id.to_string(),
        session_id: thread_id.to_string(),
        forked_from_id: None,
        parent_thread_id: None,
        preview: String::from("remote thread"),
        ephemeral: false,
        model_provider: String::from("openai"),
        created_at: 1,
        updated_at: 2,
        status: codex_app_server_protocol::ThreadStatus::Idle,
        path: None,
        cwd: test_path_buf("/tmp").abs(),
        cli_version: String::from("0.0.0"),
        source: codex_app_server_protocol::SessionSource::Cli,
        thread_source: None,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: Some(String::from("Named thread")),
        turns: Vec::new(),
    };

    let row = row_from_app_server_thread(thread).expect("row should be preserved");

    assert_eq!(row.path, None);
    assert_eq!(row.thread_id, Some(thread_id));
    assert_eq!(row.thread_name, Some(String::from("Named thread")));
}
