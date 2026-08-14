use codex_utils_absolute_path::AbsolutePathBuf;
use tempfile::TempDir;

use super::should_delay_startup_composer_for_first_login;

#[test]
fn startup_delays_composer_only_for_pristine_default_homes() -> std::io::Result<()> {
    let temporary_directory = TempDir::new()?;
    let codex_home = temporary_directory.path().join("codex-home");
    let system_config_path =
        AbsolutePathBuf::from_absolute_path(temporary_directory.path().join("system.toml"))?;

    assert!(should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || Ok(false),
        |_| None,
    ));
    assert!(should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || Ok(false),
        |name| (name == codex_login::CODEX_ACCESS_TOKEN_ENV_VAR).then(|| "  ".to_string()),
    ));

    std::fs::create_dir_all(codex_home.join("tmp").join("arg0"))?;
    assert!(should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || Ok(false),
        |_| None,
    ));
    let helper_directory = codex_home
        .join("tmp")
        .join("arg0")
        .join("codex-arg0-session");
    std::fs::create_dir(&helper_directory)?;
    std::fs::write(helper_directory.join("apply_patch"), "")?;
    assert!(should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || Ok(false),
        |_| None,
    ));

    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || Ok(false),
        |name| (name == "CODEX_HOME").then(|| "/custom/home".to_string()),
    ));
    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || Ok(false),
        |name| {
            (name == codex_login::CODEX_ACCESS_TOKEN_ENV_VAR).then(|| "access-token".to_string())
        },
    ));
    for disabled_credential in [
        codex_login::OPENAI_API_KEY_ENV_VAR,
        codex_login::CODEX_API_KEY_ENV_VAR,
    ] {
        assert!(should_delay_startup_composer_for_first_login(
            &codex_home,
            Ok(system_config_path.clone()),
            || Ok(false),
            |name| (name == disabled_credential).then(|| "disabled-key".to_string()),
        ));
    }

    for existing_state in ["auth.json", "config.toml", "history.jsonl", "sessions"] {
        let state_path = codex_home.join(existing_state);
        std::fs::write(&state_path, "")?;
        assert!(!should_delay_startup_composer_for_first_login(
            &codex_home,
            Ok(system_config_path.clone()),
            || Ok(false),
            |_| None,
        ));
        std::fs::remove_file(state_path)?;
    }

    let daemon_directory = codex_home.join("app-server-control");
    std::fs::create_dir(&daemon_directory)?;
    std::fs::write(daemon_directory.join("app-server-control.sock"), "")?;
    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || panic!("existing homes should not probe managed configuration"),
        |_| None,
    ));
    std::fs::remove_file(daemon_directory.join("app-server-control.sock"))?;
    std::fs::remove_dir(daemon_directory)?;

    let additional_temporary_state = codex_home.join("tmp").join("other");
    std::fs::write(&additional_temporary_state, "")?;
    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || Ok(false),
        |_| None,
    ));

    let invalid_home = temporary_directory.path().join("invalid-home");
    std::fs::write(&invalid_home, "not a directory")?;
    assert!(!should_delay_startup_composer_for_first_login(
        &invalid_home,
        Ok(system_config_path),
        || Ok(false),
        |_| None,
    ));
    Ok(())
}

#[test]
fn startup_keeps_composer_when_home_state_cannot_be_confirmed() -> std::io::Result<()> {
    let temporary_directory = TempDir::new()?;
    let codex_home = temporary_directory.path().join("codex-home");
    let system_config_path =
        AbsolutePathBuf::from_absolute_path(temporary_directory.path().join("system.toml"))?;

    std::fs::create_dir(&codex_home)?;
    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || Ok(false),
        |_| None,
    ));

    std::fs::write(codex_home.join("tmp"), "not a directory")?;
    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path),
        || Ok(false),
        |_| None,
    ));
    Ok(())
}

#[test]
fn startup_keeps_composer_when_system_configuration_is_possible() -> std::io::Result<()> {
    let temporary_directory = TempDir::new()?;
    let codex_home = temporary_directory.path().join("codex-home");
    let system_config_path =
        AbsolutePathBuf::from_absolute_path(temporary_directory.path().join("system.toml"))?;

    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Err(std::io::Error::other(
            "system configuration path is unavailable"
        )),
        || Ok(false),
        |_| None,
    ));

    #[cfg(unix)]
    {
        let blocking_parent = temporary_directory.path().join("blocking-parent");
        std::fs::write(&blocking_parent, "not a directory")?;
        assert!(!should_delay_startup_composer_for_first_login(
            &codex_home,
            AbsolutePathBuf::from_absolute_path(blocking_parent.join("system.toml")),
            || Ok(false),
            |_| None,
        ));
    }

    std::fs::write(system_config_path.as_path(), "model_provider = 'local'")?;
    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path),
        || Ok(false),
        |_| None,
    ));
    Ok(())
}

#[test]
fn startup_keeps_composer_when_managed_configuration_is_possible() -> std::io::Result<()> {
    let temporary_directory = TempDir::new()?;
    let codex_home = temporary_directory.path().join("codex-home");
    let system_config_path =
        AbsolutePathBuf::from_absolute_path(temporary_directory.path().join("system.toml"))?;

    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path.clone()),
        || Ok(true),
        |_| None,
    ));
    assert!(!should_delay_startup_composer_for_first_login(
        &codex_home,
        Ok(system_config_path),
        || Err(std::io::Error::other(
            "managed configuration is inaccessible"
        )),
        |_| None,
    ));
    Ok(())
}
