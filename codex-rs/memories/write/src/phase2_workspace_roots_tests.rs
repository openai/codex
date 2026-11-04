use super::agent;
use crate::memory_root;
use codex_model_provider::create_model_provider;
use codex_protocol::permissions::PROTECTED_METADATA_PATH_NAMES;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn consolidation_rebinds_workspace_roots_to_memory_root() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let extra_root = TempDir::new()?;
    let parent_extra_root = extra_root.path().to_path_buf();
    let test = test_codex()
        .with_home(home)
        .with_config(move |config| {
            config.workspace_roots.push(
                codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(parent_extra_root)
                    .expect("temporary directory is absolute"),
            );
            config.workspace_roots_explicit = false;
            config
                .permissions
                .set_workspace_roots(config.workspace_roots.clone());
        })
        .build_with_auto_env(&server)
        .await?;
    let provider = create_model_provider(
        test.config.model_provider.clone(),
        Some(test.thread_manager.auth_manager()),
    );

    let parent_permission_profile = test.config.permissions.effective_permission_profile();
    let root = memory_root(&test.config.codex_home);

    for metadata_exists in [false, true] {
        if metadata_exists {
            for name in PROTECTED_METADATA_PATH_NAMES {
                std::fs::create_dir_all(root.join(name))?;
            }
        }
        let agent_config = agent::get_config(
            &test.config,
            parent_permission_profile.clone(),
            provider.as_ref(),
        )
        .expect("agent config should be created");

        assert_eq!(
            (
                &agent_config.cwd,
                &agent_config.workspace_roots,
                agent_config.permissions.workspace_roots(),
                agent_config.workspace_roots_explicit,
            ),
            (&root, &vec![root.clone()], [root.clone()].as_slice(), true),
        );
        assert_eq!(
            agent_config.legacy_sandbox_policy(),
            SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            }
        );
        let filesystem = agent_config.permissions.file_system_sandbox_policy();
        assert!(filesystem.can_write_local_path_with_cwd(root.join("memory.md").as_path(), &root));
        for path in [test.config.cwd.as_path(), extra_root.path()] {
            assert!(!filesystem.can_write_local_path_with_cwd(path, &root));
            assert!(filesystem.can_read_local_path_with_cwd(path, &root));
        }
        for name in PROTECTED_METADATA_PATH_NAMES {
            assert!(!filesystem.can_write_local_path_with_cwd(root.join(name).as_path(), &root));
        }
    }

    test.codex.shutdown_and_wait().await?;
    Ok(())
}
