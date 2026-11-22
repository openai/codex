//! Bounded, sandboxed reads for model-requested previews on the embedded backend.

use crate::app_server_session::fs::AppServerFileSystem;
use crate::app_server_session::fs::resolve_preview_path;
use codex_app_server_client::AppServerPath;
use codex_app_server_client::ThreadPermissions;
use codex_exec_server::Environment;
use codex_exec_server::FileSystemSandboxContext;
use codex_protocol::ThreadId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_path_uri::PathUri;
use color_eyre::eyre::Result;
use color_eyre::eyre::ensure;
use color_eyre::eyre::eyre;
use futures::StreamExt;

const MAX_PREVIEW_BYTES: usize = 8 * 1024 * 1024;

pub(in crate::app) async fn read(
    fs: &AppServerFileSystem,
    environment: &Environment,
    source: ThreadId,
    original_path: &AppServerPath,
    path: &PathUri,
    permissions: &ThreadPermissions,
) -> Result<Vec<u8>> {
    let cwd_uri = fs.preview_workspace(source).await?;
    let cwd = cwd_uri.to_abs_path()?;
    let roots = if permissions.workspace_roots.is_empty() {
        std::slice::from_ref(&cwd_uri)
    } else {
        &permissions.workspace_roots
    };
    ensure!(
        *path == resolve_preview_path(original_path, &cwd_uri)?
            && roots.iter().any(|root| path.starts_with(root)),
        "The preview path is outside the calling workspace or the workspace changed"
    );
    ensure!(
        !matches!(permissions.profile, PermissionProfile::External { .. }),
        "The thread's external sandbox cannot authorize a preview"
    );
    let thread_policy = permissions.profile.file_system_sandbox_policy();
    let deny_matcher = ReadDenyMatcher::try_new_for_local_paths(&thread_policy, cwd.as_path())
        .map_err(|error| eyre!("{error}"))?;
    let requested = path.to_abs_path()?;
    let canonical = tokio::fs::canonicalize(requested.as_path()).await?;
    let mut within_workspace = false;
    for root in roots {
        if let Ok(root) = root.to_abs_path()
            && let Ok(root) = tokio::fs::canonicalize(root.as_path()).await
            && canonical.starts_with(root)
        {
            within_workspace = true;
            break;
        }
    }
    ensure!(
        within_workspace
            && [requested.as_path(), canonical.as_path()]
                .into_iter()
                .all(|path| {
                    thread_policy.can_read_local_path_with_cwd(path, cwd.as_path())
                        && !deny_matcher
                            .as_ref()
                            .is_some_and(|matcher| matcher.is_local_path_read_denied(path))
                }),
        "The calling workspace permissions do not allow reading this preview"
    );
    let path = PathUri::from_host_native_path(&canonical)?;
    // Keep the actual open within the file's own allow rule and the thread's deny rules.
    let mut file_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
        path.clone().into(),
        FileSystemAccessMode::Read,
    )]);
    file_policy.preserve_deny_read_restrictions_from(&thread_policy);
    let profile =
        PermissionProfile::from_runtime_permissions(&file_policy, NetworkSandboxPolicy::Restricted);
    let info = environment.info().await?;
    let mut sandbox = FileSystemSandboxContext::from_permission_profile(profile, cwd_uri.clone());
    sandbox.workspace_roots = roots.to_vec();
    sandbox.user_home_dir = info.user_home_dir;
    sandbox.temporary_directories = info.temporary_directories;
    ensure!(
        info.capabilities.sandboxed_file_streaming && sandbox.should_read_from_sandbox(),
        "The executor does not support sandboxed file previews"
    );
    let mut stream = environment
        .get_filesystem()
        .read_file_stream(&path, Some(&sandbox))
        .await?;
    let mut contents = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        ensure!(
            chunk.len() <= MAX_PREVIEW_BYTES - contents.len(),
            "The file exceeds the 8 MiB model preview limit"
        );
        contents.extend_from_slice(&chunk);
    }
    ensure!(
        fs.preview_workspace(source).await? == cwd_uri,
        "Workspace changed while loading the preview"
    );
    Ok(contents)
}
