use crate::codex::Session;
use crate::models::FunctionCallOutputPayload;
use crate::models::ResponseInputItem;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::FileChange;
use crate::protocol::PatchApplyBeginEvent;
use crate::protocol::PatchApplyEndEvent;
use crate::protocol::ReviewDecision;
use crate::safety::SafetyCheck;
use crate::safety::assess_patch_safety;
use anyhow::Context;
use codex_apply_patch::AffectedPaths;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;
use codex_apply_patch::print_summary;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

pub(crate) async fn apply_patch(
    sess: &Session,
    sub_id: String,
    call_id: String,
    action: ApplyPatchAction,
) -> ResponseInputItem {
    let writable_roots_snapshot = {
        #[allow(clippy::unwrap_used)]
        let guard = sess.writable_roots.lock().unwrap();
        guard.clone()
    };

    let auto_approved = match assess_patch_safety(
        &action,
        sess.approval_policy,
        &writable_roots_snapshot,
        &sess.cwd,
    ) {
        SafetyCheck::AutoApprove { .. } => true,
        SafetyCheck::AskUser => {
            // Compute a readable summary of path changes to include in the
            // approval request so the user can make an informed decision.
            let rx_approve = sess
                .request_patch_approval(sub_id.clone(), call_id.clone(), &action, None, None)
                .await;
            match rx_approve.await.unwrap_or_default() {
                ReviewDecision::Approved | ReviewDecision::ApprovedForSession => false,
                ReviewDecision::Denied | ReviewDecision::Abort => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload {
                            content: "patch rejected by user".to_string(),
                            success: Some(false),
                        },
                    };
                }
            }
        }
        SafetyCheck::Reject { reason } => {
            return ResponseInputItem::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload {
                    content: format!("patch rejected: {reason}"),
                    success: Some(false),
                },
            };
        }
    };

    // Verify write permissions before touching the filesystem.
    let writable_snapshot = {
        #[allow(clippy::unwrap_used)]
        sess.writable_roots.lock().unwrap().clone()
    };

    if let Some(offending) = first_offending_path(&action, &writable_snapshot, &sess.cwd) {
        let root = offending.parent().unwrap_or(&offending).to_path_buf();

        let reason = Some(format!(
            "grant write access to {} for this session",
            root.display()
        ));

        let rx = sess
            .request_patch_approval(
                sub_id.clone(),
                call_id.clone(),
                &action,
                reason.clone(),
                Some(root.clone()),
            )
            .await;

        if !matches!(
            rx.await.unwrap_or_default(),
            ReviewDecision::Approved | ReviewDecision::ApprovedForSession
        ) {
            return ResponseInputItem::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload {
                    content: "patch rejected by user".to_string(),
                    success: Some(false),
                },
            };
        }

        // user approved, extend writable roots for this session
        #[allow(clippy::unwrap_used)]
        sess.writable_roots.lock().unwrap().push(root);
    }

    let _ = sess
        .tx_event
        .send(Event {
            id: sub_id.clone(),
            msg: EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                call_id: call_id.clone(),
                auto_approved,
                changes: convert_apply_patch_to_protocol(&action),
            }),
        })
        .await;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    // Enforce writable roots. If a write is blocked, collect offending root
    // and prompt the user to extend permissions.
    let mut result = apply_changes_from_apply_patch_and_report(&action, &mut stdout, &mut stderr);

    if let Err(err) = &result {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            // Determine first offending path.
            let offending_opt = action
                .changes()
                .iter()
                .flat_map(|(path, change)| match change {
                    ApplyPatchFileChange::Add { .. } => vec![path.as_ref()],
                    ApplyPatchFileChange::Delete => vec![path.as_ref()],
                    ApplyPatchFileChange::Update {
                        move_path: Some(move_path),
                        ..
                    } => {
                        vec![path.as_ref(), move_path.as_ref()]
                    }
                    ApplyPatchFileChange::Update {
                        move_path: None, ..
                    } => vec![path.as_ref()],
                })
                .find_map(|path: &Path| {
                    // ApplyPatchAction promises to guarantee absolute paths.
                    if !path.is_absolute() {
                        panic!("apply_patch invariant failed: path is not absolute: {path:?}");
                    }

                    let writable = {
                        #[allow(clippy::unwrap_used)]
                        let roots = sess.writable_roots.lock().unwrap();
                        roots.iter().any(|root| path.starts_with(root))
                    };
                    if writable {
                        None
                    } else {
                        Some(path.to_path_buf())
                    }
                });

            if let Some(offending) = offending_opt {
                let root = offending.parent().unwrap_or(&offending).to_path_buf();

                let reason = Some(format!(
                    "grant write access to {} for this session",
                    root.display()
                ));
                let rx = sess
                    .request_patch_approval(
                        sub_id.clone(),
                        call_id.clone(),
                        &action,
                        reason.clone(),
                        Some(root.clone()),
                    )
                    .await;
                if matches!(
                    rx.await.unwrap_or_default(),
                    ReviewDecision::Approved | ReviewDecision::ApprovedForSession
                ) {
                    // Extend writable roots.
                    #[allow(clippy::unwrap_used)]
                    sess.writable_roots.lock().unwrap().push(root);
                    stdout.clear();
                    stderr.clear();
                    result = apply_changes_from_apply_patch_and_report(
                        &action,
                        &mut stdout,
                        &mut stderr,
                    );
                }
            }
        }
    }

    // Emit PatchApplyEnd event.
    let success_flag = result.is_ok();
    let _ = sess
        .tx_event
        .send(Event {
            id: sub_id.clone(),
            msg: EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                call_id: call_id.clone(),
                stdout: String::from_utf8_lossy(&stdout).to_string(),
                stderr: String::from_utf8_lossy(&stderr).to_string(),
                success: success_flag,
            }),
        })
        .await;

    match result {
        Ok(_) => ResponseInputItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                content: String::from_utf8_lossy(&stdout).to_string(),
                success: None,
            },
        },
        Err(e) => ResponseInputItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                content: format!("error: {e:#}, stderr: {}", String::from_utf8_lossy(&stderr)),
                success: Some(false),
            },
        },
    }
}

/// Return the first path in `hunks` that is NOT under any of the
/// `writable_roots` (after normalising). If all paths are acceptable,
/// returns None.
fn first_offending_path(
    action: &ApplyPatchAction,
    writable_roots: &[PathBuf],
    cwd: &Path,
) -> Option<PathBuf> {
    let changes = action.changes();
    for (path, change) in changes {
        let candidate = match change {
            ApplyPatchFileChange::Add { .. } => path,
            ApplyPatchFileChange::Delete => path,
            ApplyPatchFileChange::Update { move_path, .. } => move_path.as_ref().unwrap_or(path),
        };

        let abs = if candidate.is_absolute() {
            candidate.clone()
        } else {
            cwd.join(candidate)
        };

        let mut allowed = false;
        for root in writable_roots {
            let root_abs = if root.is_absolute() {
                root.clone()
            } else {
                cwd.join(root)
            };
            if abs.starts_with(&root_abs) {
                allowed = true;
                break;
            }
        }

        if !allowed {
            return Some(candidate.clone());
        }
    }
    None
}

pub(crate) fn convert_apply_patch_to_protocol(
    action: &ApplyPatchAction,
) -> HashMap<PathBuf, FileChange> {
    let changes = action.changes();
    let mut result = HashMap::with_capacity(changes.len());
    for (path, change) in changes {
        let protocol_change = match change {
            ApplyPatchFileChange::Add { content } => FileChange::Add {
                content: content.clone(),
            },
            ApplyPatchFileChange::Delete => FileChange::Delete,
            ApplyPatchFileChange::Update {
                unified_diff,
                move_path,
                new_content: _new_content,
            } => FileChange::Update {
                unified_diff: unified_diff.clone(),
                move_path: move_path.clone(),
            },
        };
        result.insert(path.clone(), protocol_change);
    }
    result
}

fn apply_changes_from_apply_patch_and_report(
    action: &ApplyPatchAction,
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
) -> std::io::Result<()> {
    match apply_changes_from_apply_patch(action) {
        Ok(affected_paths) => {
            print_summary(&affected_paths, stdout)?;
        }
        Err(err) => {
            writeln!(stderr, "{err:?}")?;
        }
    }

    Ok(())
}

fn apply_changes_from_apply_patch(action: &ApplyPatchAction) -> anyhow::Result<AffectedPaths> {
    let mut added: Vec<PathBuf> = Vec::new();
    let mut modified: Vec<PathBuf> = Vec::new();
    let mut deleted: Vec<PathBuf> = Vec::new();

    let changes = action.changes();
    for (path, change) in changes {
        match change {
            ApplyPatchFileChange::Add { content } => {
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!("Failed to create parent directories for {}", path.display())
                        })?;
                    }
                }
                std::fs::write(path, content)
                    .with_context(|| format!("Failed to write file {}", path.display()))?;
                added.push(path.clone());
            }
            ApplyPatchFileChange::Delete => {
                std::fs::remove_file(path)
                    .with_context(|| format!("Failed to delete file {}", path.display()))?;
                deleted.push(path.clone());
            }
            ApplyPatchFileChange::Update {
                unified_diff: _unified_diff,
                move_path,
                new_content,
            } => {
                if let Some(move_path) = move_path {
                    if let Some(parent) = move_path.parent() {
                        if !parent.as_os_str().is_empty() {
                            std::fs::create_dir_all(parent).with_context(|| {
                                format!(
                                    "Failed to create parent directories for {}",
                                    move_path.display()
                                )
                            })?;
                        }
                    }

                    std::fs::rename(path, move_path)
                        .with_context(|| format!("Failed to rename file {}", path.display()))?;
                    std::fs::write(move_path, new_content)?;
                    modified.push(move_path.clone());
                    deleted.push(path.clone());
                } else {
                    std::fs::write(path, new_content)?;
                    modified.push(path.clone());
                }
            }
        }
    }

    Ok(AffectedPaths {
        added,
        modified,
        deleted,
    })
}

pub(crate) fn get_writable_roots(cwd: &Path) -> Vec<PathBuf> {
    let mut writable_roots = Vec::new();
    if cfg!(target_os = "macos") {
        // On macOS, $TMPDIR is private to the user.
        writable_roots.push(std::env::temp_dir());

        // Allow pyenv to update its shims directory. Without this, any tool
        // that happens to be managed by `pyenv` will fail with an error like:
        //
        //   pyenv: cannot rehash: $HOME/.pyenv/shims isn't writable
        //
        // which is emitted every time `pyenv` tries to run `rehash` (for
        // example, after installing a new Python package that drops an entry
        // point). Although the sandbox is intentionally read‑only by default,
        // writing to the user's local `pyenv` directory is safe because it
        // is already user‑writable and scoped to the current user account.
        if let Ok(home_dir) = std::env::var("HOME") {
            let pyenv_dir = PathBuf::from(home_dir).join(".pyenv");
            writable_roots.push(pyenv_dir);
        }
    }

    writable_roots.push(cwd.to_path_buf());

    writable_roots
}
