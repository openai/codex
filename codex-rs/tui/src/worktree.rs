use std::path::Path;
use std::time::Instant;

use codex_app_server_protocol::WorktreeDirtyPolicy as DirtyPolicy;
use codex_app_server_protocol::WorktreeDirtyState;
use codex_app_server_protocol::WorktreeInfo;
use codex_app_server_protocol::WorktreeSource;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionRowDisplay;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::motion::ACTIVITY_SPINNER_INTERVAL;
use crate::motion::activity_spinner_frame_at;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::tui::FrameRequester;

const WORKTREE_USAGE: &str =
    "Usage: /worktree [list|new <branch>|switch <branch>|path <branch>|remove <branch>]";
pub(crate) const WORKTREE_SELECTION_VIEW_ID: &str = "worktree-selection";

struct WorktreeLoadingHeader {
    started_at: Instant,
    frame_requester: FrameRequester,
    animations_enabled: bool,
    status: String,
    note: String,
}

impl WorktreeLoadingHeader {
    fn new(
        frame_requester: FrameRequester,
        animations_enabled: bool,
        status: String,
        note: String,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            frame_requester,
            animations_enabled,
            status,
            note,
        }
    }
}

impl Renderable for WorktreeLoadingHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        if self.animations_enabled {
            self.frame_requester
                .schedule_frame_in(ACTIVITY_SPINNER_INTERVAL);
        }

        let mut loading_spans = Vec::new();
        if self.animations_enabled {
            loading_spans.push(activity_spinner_frame_at(self.started_at, Instant::now()).into());
            loading_spans.push(" ".into());
        } else {
            loading_spans.push("•".dim());
            loading_spans.push(" ".into());
        }
        loading_spans.push(self.status.clone().dim());

        Paragraph::new(vec![
            Line::from("Worktrees".bold()),
            Line::from(loading_spans),
            Line::from(self.note.clone().dim()),
        ])
        .render_ref(area, buf);
    }

    fn desired_height(&self, _width: u16) -> u16 {
        3
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorktreeSlashAction {
    OpenPicker,
    Create {
        branch: String,
        base_ref: Option<String>,
        dirty_policy: Option<DirtyPolicy>,
    },
    Switch {
        target: String,
    },
    ShowPath {
        target: String,
    },
    Remove {
        target: String,
        force: bool,
        delete_branch: bool,
    },
}

impl WorktreeSlashAction {
    pub(crate) fn dispatch(self, tx: &AppEventSender) {
        match self {
            WorktreeSlashAction::OpenPicker => tx.send(AppEvent::OpenWorktreePicker),
            WorktreeSlashAction::Create {
                branch,
                base_ref,
                dirty_policy,
            } => tx.send(AppEvent::CreateWorktreeAndSwitch {
                branch,
                base_ref,
                dirty_policy,
            }),
            WorktreeSlashAction::Switch { target } => {
                tx.send(AppEvent::SwitchToWorktree { target });
            }
            WorktreeSlashAction::ShowPath { target } => {
                tx.send(AppEvent::ShowWorktreePath { target });
            }
            WorktreeSlashAction::Remove {
                target,
                force,
                delete_branch,
            } => tx.send(AppEvent::RemoveWorktree {
                target,
                force,
                delete_branch,
                confirmed: force,
            }),
        }
    }
}

pub(crate) fn parse_worktree_slash_args(args: &str) -> Result<WorktreeSlashAction, String> {
    let mut parts = args.split_whitespace();
    let Some(command) = parts.next() else {
        return Ok(WorktreeSlashAction::OpenPicker);
    };
    match command {
        "list" => Ok(WorktreeSlashAction::OpenPicker),
        "new" => parse_new(parts),
        "switch" | "move" => {
            let target = required_target(parts, command)?;
            Ok(WorktreeSlashAction::Switch { target })
        }
        "path" => {
            let target = required_target(parts, command)?;
            Ok(WorktreeSlashAction::ShowPath { target })
        }
        "remove" => parse_remove(parts),
        _ => Err(WORKTREE_USAGE.to_string()),
    }
}

fn parse_new<'a>(mut parts: impl Iterator<Item = &'a str>) -> Result<WorktreeSlashAction, String> {
    let Some(branch) = parts.next() else {
        return Err("Usage: /worktree new <branch> [--base <ref>] [--dirty <mode>]".to_string());
    };
    let mut base_ref = None;
    let mut dirty_policy = None;
    while let Some(flag) = parts.next() {
        match flag {
            "--base" => {
                let Some(value) = parts.next() else {
                    return Err("Usage: /worktree new <branch> --base <ref>".to_string());
                };
                base_ref = Some(value.to_string());
            }
            "--dirty" => {
                let Some(value) = parts.next() else {
                    return Err("Usage: /worktree new <branch> --dirty <mode>".to_string());
                };
                dirty_policy = Some(parse_dirty_policy(value)?);
            }
            _ => return Err(format!("Unknown /worktree new option '{flag}'.")),
        }
    }
    Ok(WorktreeSlashAction::Create {
        branch: branch.to_string(),
        base_ref,
        dirty_policy,
    })
}

fn parse_remove<'a>(
    mut parts: impl Iterator<Item = &'a str>,
) -> Result<WorktreeSlashAction, String> {
    let Some(target) = parts.next() else {
        return Err(
            "Usage: /worktree remove <branch-or-name> [--force] [--delete-branch]".to_string(),
        );
    };
    let mut force = false;
    let mut delete_branch = false;
    for flag in parts {
        match flag {
            "--force" => force = true,
            "--delete-branch" => delete_branch = true,
            _ => return Err(format!("Unknown /worktree remove option '{flag}'.")),
        }
    }
    Ok(WorktreeSlashAction::Remove {
        target: target.to_string(),
        force,
        delete_branch,
    })
}

fn required_target<'a>(
    mut parts: impl Iterator<Item = &'a str>,
    command: &str,
) -> Result<String, String> {
    let Some(target) = parts.next() else {
        return Err(format!("Usage: /worktree {command} <branch-or-name>"));
    };
    if parts.next().is_some() {
        return Err(format!("Usage: /worktree {command} <branch-or-name>"));
    }
    Ok(target.to_string())
}

fn parse_dirty_policy(value: &str) -> Result<DirtyPolicy, String> {
    match value {
        "fail" => Ok(DirtyPolicy::Fail),
        "ignore" => Ok(DirtyPolicy::Ignore),
        "copy-tracked" => Ok(DirtyPolicy::CopyTracked),
        "copy-all" => Ok(DirtyPolicy::CopyAll),
        "move-tracked" => Ok(DirtyPolicy::MoveTracked),
        "move-all" => Ok(DirtyPolicy::MoveAll),
        _ => Err(
            "Dirty mode must be one of: fail, ignore, copy-tracked, copy-all, move-tracked, move-all."
                .to_string(),
        ),
    }
}

pub(crate) fn dispatch_worktree_slash_args(args: &str, tx: &AppEventSender) -> Result<(), String> {
    parse_worktree_slash_args(args)?.dispatch(tx);
    Ok(())
}

pub(crate) fn loading_params(
    frame_requester: FrameRequester,
    animations_enabled: bool,
) -> SelectionViewParams {
    let status = "Loading worktrees...".to_string();
    let note =
        "This can take a moment when Codex is checking app, CLI, and Git worktrees.".to_string();
    SelectionViewParams {
        view_id: Some(WORKTREE_SELECTION_VIEW_ID),
        header: Box::new(WorktreeLoadingHeader::new(
            frame_requester,
            animations_enabled,
            status.clone(),
            note.clone(),
        )),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![SelectionItem {
            name: status,
            description: Some(note),
            is_disabled: true,
            ..Default::default()
        }],
        ..Default::default()
    }
}

pub(crate) fn switching_params(
    target: String,
    frame_requester: FrameRequester,
    animations_enabled: bool,
) -> SelectionViewParams {
    let status = format!("Switching to {target}...");
    let note =
        "Codex is rebuilding configuration and starting the chat in that workspace.".to_string();
    SelectionViewParams {
        view_id: Some(WORKTREE_SELECTION_VIEW_ID),
        header: Box::new(WorktreeLoadingHeader::new(
            frame_requester,
            animations_enabled,
            status,
            note.clone(),
        )),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![SelectionItem {
            name: "Preparing worktree session...".to_string(),
            description: Some(note),
            is_disabled: true,
            ..Default::default()
        }],
        ..Default::default()
    }
}

pub(crate) fn creating_params(
    branch: String,
    frame_requester: FrameRequester,
    animations_enabled: bool,
) -> SelectionViewParams {
    let status = format!("Creating {branch}...");
    let note =
        "Codex is creating the worktree before starting the chat in that workspace.".to_string();
    SelectionViewParams {
        view_id: Some(WORKTREE_SELECTION_VIEW_ID),
        header: Box::new(WorktreeLoadingHeader::new(
            frame_requester,
            animations_enabled,
            status,
            note.clone(),
        )),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![SelectionItem {
            name: "Preparing worktree...".to_string(),
            description: Some(note),
            is_disabled: true,
            ..Default::default()
        }],
        ..Default::default()
    }
}

pub(crate) fn empty_params() -> SelectionViewParams {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Worktrees".bold()));

    SelectionViewParams {
        view_id: Some(WORKTREE_SELECTION_VIEW_ID),
        header: Box::new(header),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![new_worktree_item()],
        ..Default::default()
    }
}

pub(crate) fn error_params(error: String) -> SelectionViewParams {
    error_with_summary_params("Failed to list worktrees.".to_string(), error)
}

pub(crate) fn error_with_summary_params(summary: String, error: String) -> SelectionViewParams {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Worktrees".bold()));

    SelectionViewParams {
        view_id: Some(WORKTREE_SELECTION_VIEW_ID),
        header: Box::new(header),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![SelectionItem {
            name: summary,
            description: Some(error),
            is_disabled: true,
            ..Default::default()
        }],
        ..Default::default()
    }
}

pub(crate) fn picker_params(entries: Vec<WorktreeInfo>, current_cwd: &Path) -> SelectionViewParams {
    let mut items = vec![new_worktree_item()];
    let mut initial_selected_idx = None;
    items.extend(entries.into_iter().enumerate().map(|(idx, entry)| {
        let target = entry.branch.clone().unwrap_or_else(|| entry.name.clone());
        let source = source_label(entry.source);
        let status = if dirty_state_is_dirty(&entry.dirty) {
            "dirty"
        } else {
            "clean"
        };
        let is_current = is_current_worktree(current_cwd, &entry);
        if is_current {
            initial_selected_idx = Some(idx + 1);
        }
        let description = format!("{status} · {source} · {}", entry.workspace_cwd);
        let selected_description = if is_current {
            "Already in this worktree".to_string()
        } else {
            format!("Fork this chat into {}", entry.workspace_cwd)
        };
        let search_value = Some(format!(
            "{} {} {} {}",
            target, entry.name, source, entry.workspace_cwd
        ));
        let target_for_action = target.clone();
        let actions: Vec<SelectionAction> = if is_current {
            vec![Box::new(move |tx| {
                tx.send(AppEvent::CurrentWorktreeSelected {
                    target: target_for_action.clone(),
                });
            })]
        } else {
            let info_for_action = entry;
            vec![Box::new(move |tx| {
                tx.send(AppEvent::SwitchToWorktreeInfo {
                    info: info_for_action.clone(),
                });
            })]
        };
        SelectionItem {
            name: target,
            description: Some(description),
            selected_description: Some(selected_description),
            actions,
            dismiss_on_select: true,
            search_value,
            is_current,
            ..Default::default()
        }
    }));

    let mut header = ColumnRenderable::new();
    header.push(Line::from("Worktrees".bold()));
    header.push(Line::from(
        "Create a worktree or fork this chat into an existing workspace.".dim(),
    ));

    SelectionViewParams {
        view_id: Some(WORKTREE_SELECTION_VIEW_ID),
        header: Box::new(header),
        footer_hint: Some(standard_popup_hint_line()),
        items,
        is_searchable: true,
        search_placeholder: Some("Search worktrees".to_string()),
        col_width_mode: ColumnWidthMode::AutoAllRows,
        row_display: SelectionRowDisplay::SingleLine,
        initial_selected_idx,
        ..Default::default()
    }
}

fn new_worktree_item() -> SelectionItem {
    SelectionItem {
        name: "New worktree...".to_string(),
        description: Some("Create a sibling worktree and start this chat there.".to_string()),
        selected_description: Some("Type the branch name for the new worktree.".to_string()),
        actions: vec![Box::new(|tx| {
            tx.send(AppEvent::OpenWorktreeCreatePrompt);
        })],
        dismiss_on_select: false,
        search_value: Some("new worktree create branch".to_string()),
        ..Default::default()
    }
}

pub(crate) fn dirty_policy_prompt_params(
    branch: String,
    base_ref: Option<String>,
) -> SelectionViewParams {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Source checkout has uncommitted changes".bold()));
    header.push(Line::from(
        "Choose what to carry into the new worktree.".dim(),
    ));
    let item = |name: &str, description: &str, dirty_policy: DirtyPolicy| SelectionItem {
        name: name.to_string(),
        description: Some(description.to_string()),
        actions: vec![Box::new({
            let branch = branch.clone();
            let base_ref = base_ref.clone();
            move |tx| {
                tx.send(AppEvent::CreateWorktreeAndSwitch {
                    branch: branch.clone(),
                    base_ref: base_ref.clone(),
                    dirty_policy: Some(dirty_policy),
                });
            }
        })],
        dismiss_on_select: true,
        ..Default::default()
    };
    SelectionViewParams {
        header: Box::new(header),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![
            item(
                "Move all",
                "Move tracked changes and untracked files; leave the source checkout clean.",
                DirtyPolicy::MoveAll,
            ),
            item(
                "Copy all",
                "Copy tracked changes and untracked files.",
                DirtyPolicy::CopyAll,
            ),
            item(
                "Move tracked",
                "Move staged and unstaged tracked changes; leave untracked files behind.",
                DirtyPolicy::MoveTracked,
            ),
            item(
                "Copy tracked",
                "Copy staged and unstaged tracked changes.",
                DirtyPolicy::CopyTracked,
            ),
            item(
                "Ignore",
                "Create from the requested base without copying local changes.",
                DirtyPolicy::Ignore,
            ),
            item(
                "Fail",
                "Cancel creation and leave the source checkout unchanged.",
                DirtyPolicy::Fail,
            ),
        ],
        ..Default::default()
    }
}

pub(crate) fn remove_confirmation_params(
    target: String,
    force: bool,
    delete_branch: bool,
) -> SelectionViewParams {
    let mut header = ColumnRenderable::new();
    header.push(Line::from(format!("Remove worktree {target}?").bold()));
    header.push(Line::from(
        "Only Codex-managed worktrees can be removed.".dim(),
    ));

    SelectionViewParams {
        header: Box::new(header),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![
            SelectionItem {
                name: "Remove".to_string(),
                description: Some("Remove the selected worktree.".to_string()),
                actions: vec![Box::new({
                    move |tx| {
                        tx.send(AppEvent::RemoveWorktree {
                            target: target.clone(),
                            force,
                            delete_branch,
                            confirmed: true,
                        });
                    }
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Cancel".to_string(),
                description: Some("Keep the worktree.".to_string()),
                dismiss_on_select: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

pub(crate) fn find_worktree<'a>(
    entries: &'a [WorktreeInfo],
    target: &str,
) -> Result<&'a WorktreeInfo, String> {
    let matches = entries
        .iter()
        .filter(|entry| {
            entry.branch.as_deref() == Some(target) || entry.name == target || entry.slug == target
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [entry] => Ok(entry),
        [] => Err(format!("No worktree found matching '{target}'.")),
        _ => Err(format!(
            "Multiple worktrees match '{target}'; use a more specific name."
        )),
    }
}

pub(crate) fn source_label(source: WorktreeSource) -> &'static str {
    match source {
        WorktreeSource::Cli => "cli",
        WorktreeSource::App => "app",
        WorktreeSource::Legacy => "legacy",
        WorktreeSource::Git => "git",
    }
}

fn paths_match(a: &Path, b: &Path) -> bool {
    let a = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
    let b = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
    a == b
}

fn is_current_worktree(current_cwd: &Path, entry: &WorktreeInfo) -> bool {
    let workspace_cwd = Path::new(&entry.workspace_cwd);
    if paths_match(current_cwd, workspace_cwd) {
        return true;
    }
    let current_cwd = current_cwd
        .canonicalize()
        .unwrap_or_else(|_| current_cwd.to_path_buf());
    let worktree_git_root = Path::new(&entry.worktree_git_root);
    let worktree_root = worktree_git_root
        .canonicalize()
        .unwrap_or_else(|_| worktree_git_root.to_path_buf());
    current_cwd.starts_with(worktree_root)
}

fn dirty_state_is_dirty(dirty: &WorktreeDirtyState) -> bool {
    dirty.has_staged_changes || dirty.has_unstaged_changes || dirty.has_untracked_files
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::app_event_sender::AppEventSender;
    use crate::bottom_pane::ListSelectionView;
    use crate::keymap::RuntimeKeymap;
    use crate::render::renderable::Renderable;
    use crate::tui::FrameRequester;
    use codex_app_server_protocol::WorktreeLocation;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn parse_new_with_flags() {
        assert_eq!(
            parse_worktree_slash_args("new fcoury/demo --base origin/main --dirty copy-tracked"),
            Ok(WorktreeSlashAction::Create {
                branch: "fcoury/demo".to_string(),
                base_ref: Some("origin/main".to_string()),
                dirty_policy: Some(DirtyPolicy::CopyTracked),
            })
        );
    }

    #[test]
    fn parse_new_with_move_all_dirty_policy() {
        assert_eq!(
            parse_worktree_slash_args("new fcoury/demo --dirty move-all"),
            Ok(WorktreeSlashAction::Create {
                branch: "fcoury/demo".to_string(),
                base_ref: /*base_ref*/ None,
                dirty_policy: Some(DirtyPolicy::MoveAll),
            })
        );
    }

    #[test]
    fn parse_new_with_move_tracked_dirty_policy() {
        assert_eq!(
            parse_worktree_slash_args("new fcoury/demo --dirty move-tracked"),
            Ok(WorktreeSlashAction::Create {
                branch: "fcoury/demo".to_string(),
                base_ref: /*base_ref*/ None,
                dirty_policy: Some(DirtyPolicy::MoveTracked),
            })
        );
    }

    #[test]
    fn parse_switch_aliases_move() {
        assert_eq!(
            parse_worktree_slash_args("move fcoury/demo"),
            Ok(WorktreeSlashAction::Switch {
                target: "fcoury/demo".to_string(),
            })
        );
    }

    #[test]
    fn parse_remove_with_flags() {
        assert_eq!(
            parse_worktree_slash_args("remove fcoury/demo --force --delete-branch"),
            Ok(WorktreeSlashAction::Remove {
                target: "fcoury/demo".to_string(),
                force: true,
                delete_branch: true,
            })
        );
    }

    #[test]
    fn worktree_picker_snapshot() {
        let params = picker_params(
            vec![
                sample_info("fcoury/demo", WorktreeSource::Cli, /*dirty*/ false),
                sample_info("codex", WorktreeSource::App, /*dirty*/ false),
                sample_info("main", WorktreeSource::Git, /*dirty*/ true),
            ],
            Path::new("/repo/codex.fcoury-demo"),
        );
        insta::assert_snapshot!("worktree_picker", render_selection(params, /*width*/ 86));
    }

    #[test]
    fn worktree_picker_preselects_current_worktree_from_subdirectory() {
        let params = picker_params(
            vec![
                sample_info("fcoury/demo", WorktreeSource::Cli, /*dirty*/ false),
                sample_info(
                    "fcoury/worktrees",
                    WorktreeSource::Git,
                    /*dirty*/ false,
                ),
            ],
            Path::new("/repo/codex.fcoury-worktrees/codex-rs"),
        );

        assert_eq!(params.initial_selected_idx, Some(2));
    }

    #[test]
    fn current_worktree_item_dispatches_current_selection_event() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let params = picker_params(
            vec![sample_info(
                "fcoury/worktrees",
                WorktreeSource::Git,
                /*dirty*/ false,
            )],
            Path::new("/repo/codex.fcoury-worktrees/codex-rs"),
        );

        (params.items[1].actions[0])(&tx);

        assert!(matches!(
            rx.try_recv(),
            Ok(AppEvent::CurrentWorktreeSelected { target }) if target == "fcoury/worktrees"
        ));
    }

    #[test]
    fn existing_worktree_item_dispatches_selected_worktree_info() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let params = picker_params(
            vec![sample_info(
                "fcoury/worktrees",
                WorktreeSource::Git,
                /*dirty*/ false,
            )],
            Path::new("/repo/codex"),
        );

        (params.items[1].actions[0])(&tx);

        assert!(matches!(
            rx.try_recv(),
            Ok(AppEvent::SwitchToWorktreeInfo { info }) if info.branch.as_deref() == Some("fcoury/worktrees")
        ));
    }

    #[test]
    fn worktree_loading_snapshot() {
        insta::assert_snapshot!(
            "worktree_loading",
            render_selection(
                loading_params(
                    FrameRequester::test_dummy(),
                    /*animations_enabled*/ false
                ),
                /*width*/ 92
            )
        );
    }

    #[test]
    fn worktree_switching_snapshot() {
        insta::assert_snapshot!(
            "worktree_switching",
            render_selection(
                switching_params(
                    "fcoury/demo".to_string(),
                    FrameRequester::test_dummy(),
                    /*animations_enabled*/ false
                ),
                /*width*/ 92
            )
        );
    }

    #[test]
    fn worktree_creating_snapshot() {
        insta::assert_snapshot!(
            "worktree_creating",
            render_selection(
                creating_params(
                    "fcoury/demo".to_string(),
                    FrameRequester::test_dummy(),
                    /*animations_enabled*/ false
                ),
                /*width*/ 92
            )
        );
    }

    #[test]
    fn worktree_empty_snapshot() {
        insta::assert_snapshot!(
            "worktree_empty",
            render_selection(empty_params(), /*width*/ 84)
        );
    }

    #[test]
    fn new_worktree_item_dispatches_create_prompt_event() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let item = new_worktree_item();

        assert!(
            !item.dismiss_on_select,
            "picker should stay behind the branch-name prompt"
        );
        (item.actions[0])(&tx);

        assert!(matches!(
            rx.try_recv(),
            Ok(AppEvent::OpenWorktreeCreatePrompt)
        ));
    }

    #[test]
    fn worktree_dirty_policy_prompt_snapshot() {
        insta::assert_snapshot!(
            "worktree_dirty_policy_prompt",
            render_selection(
                dirty_policy_prompt_params("fcoury/demo".to_string(), /*base_ref*/ None),
                /*width*/ 82
            )
        );
    }

    #[test]
    fn worktree_remove_confirmation_snapshot() {
        insta::assert_snapshot!(
            "worktree_remove_confirmation",
            render_selection(
                remove_confirmation_params(
                    "fcoury/demo".to_string(),
                    /*force*/ false,
                    /*delete_branch*/ false
                ),
                /*width*/ 80
            )
        );
    }

    fn sample_info(branch: &str, source: WorktreeSource, dirty: bool) -> WorktreeInfo {
        let path = format!("/repo/codex.{}", branch.replace('/', "-"));
        WorktreeInfo {
            id: "repo-id".to_string(),
            name: branch.to_string(),
            slug: branch.replace('/', "-"),
            source,
            location: match source {
                WorktreeSource::Cli => WorktreeLocation::Sibling,
                WorktreeSource::App | WorktreeSource::Legacy => WorktreeLocation::CodexHome,
                WorktreeSource::Git => WorktreeLocation::External,
            },
            repo_name: "codex".to_string(),
            repo_root: path.clone(),
            common_git_dir: "/repo/codex/.git".to_string(),
            worktree_git_root: path.clone(),
            workspace_cwd: path,
            original_relative_cwd: String::new(),
            branch: Some(branch.to_string()),
            head: Some("abcdef".to_string()),
            owner_thread_id: None,
            metadata_path: "/repo/codex/.git/codex-worktree.json".to_string(),
            dirty: WorktreeDirtyState {
                has_staged_changes: false,
                has_unstaged_changes: dirty,
                has_untracked_files: false,
            },
        }
    }

    fn render_selection(params: SelectionViewParams, width: u16) -> String {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = ListSelectionView::new(params, tx, RuntimeKeymap::defaults().list);
        let height = view.desired_height(width);
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        let lines: Vec<String> = (0..area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..area.width {
                    let symbol = buf[(area.x + col, area.y + row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line.trim_end().to_string()
            })
            .collect();
        lines.join("\n")
    }
}
