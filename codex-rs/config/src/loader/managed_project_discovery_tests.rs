use crate::ConfigLayerSource;
use crate::ConfigLayerStack;
use crate::LoaderOverrides;
use crate::NoopThreadConfigLoader;
use crate::loader::LocalConfigLayers;
use crate::loader::load_config_layers_state;
use crate::loader::local::load_local_config_layers_with_overrides;
use crate::loader::project_trust_key;
use crate::loader::tests::TestFileSystem;
use crate::merge_toml_values;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use tempfile::TempDir;
use toml::Value as TomlValue;

struct Fixture {
    _temp: TempDir,
    home: PathBuf,
    repo: AbsolutePathBuf,
    project: AbsolutePathBuf,
    cwd: AbsolutePathBuf,
    managed: PathBuf,
    overrides: LoaderOverrides,
}

impl Fixture {
    fn new() -> anyhow::Result<Self> {
        let temp = tempfile::tempdir()?;
        let home = temp.path().join("home");
        let repo = AbsolutePathBuf::from_absolute_path(temp.path().join("repo"))?;
        let project = repo.join("project");
        let cwd = project.join("child");
        std::fs::create_dir_all(&home)?;
        std::fs::create_dir_all(repo.join(".git"))?;
        std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n")?;
        for (dir, model) in [(&repo, "ancestor"), (&project, "project"), (&cwd, "child")] {
            std::fs::create_dir_all(dir.join(".codex"))?;
            std::fs::write(
                dir.join(".codex/config.toml"),
                format!("model = \"{model}\"\n"),
            )?;
        }
        std::fs::write(project.join(".company-root"), "")?;
        let managed = temp.path().join("managed_config.toml");
        let mut overrides = LoaderOverrides::with_managed_config_path_for_tests(managed.clone());
        overrides.system_config_path = Some(temp.path().join("system.toml"));
        let fixture = Self {
            _temp: temp,
            home,
            repo,
            project,
            cwd,
            managed,
            overrides,
        };
        std::fs::write(fixture.home.join("config.toml"), fixture.trust("trusted"))?;
        Ok(fixture)
    }

    fn trust(&self, level: &str) -> String {
        let key = TomlValue::String(project_trust_key(self.repo.as_path()));
        format!("[projects.{key}]\ntrust_level = \"{level}\"\n")
    }

    async fn load(&self) -> anyhow::Result<(ConfigLayerStack, LocalConfigLayers)> {
        let canonical = load_config_layers_state(
            &TestFileSystem,
            &self.home,
            Some(self.cwd.clone()),
            &[
                (
                    "project_root_markers".into(),
                    toml::Value::try_from([".git"])?,
                ),
                ("model".into(), TomlValue::String("session".into())),
            ],
            self.overrides.clone(),
            &NoopThreadConfigLoader,
        )
        .await?;
        let local = load_local_config_layers_with_overrides(
            &TestFileSystem,
            &self.home,
            &self.cwd,
            &self.overrides,
        )
        .await?;
        Ok((canonical, local))
    }
}

fn assert_discovery(
    canonical: &ConfigLayerStack,
    local: &LocalConfigLayers,
    expected_dirs: &[&AbsolutePathBuf],
    markers: &[&str],
) -> anyhow::Result<()> {
    let project_dirs = |sources: Vec<&ConfigLayerSource>| {
        sources
            .into_iter()
            .filter_map(|source| match source {
                ConfigLayerSource::Project { dot_codex_folder } => Some(dot_codex_folder.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let expected_dirs = expected_dirs
        .iter()
        .map(|dir| dir.join(".codex"))
        .collect::<Vec<_>>();
    assert_eq!(
        (
            project_dirs(
                canonical
                    .layers_low_to_high()
                    .map(|layer| &layer.name)
                    .collect()
            ),
            project_dirs(
                local
                    .config
                    .layers
                    .iter()
                    .map(|layer| &layer.source)
                    .collect()
            ),
        ),
        (expected_dirs.clone(), expected_dirs)
    );
    let effective = canonical.effective_config();
    let mut local_effective = TomlValue::Table(toml::map::Map::new());
    for layer in &local.config.layers {
        merge_toml_values(&mut local_effective, &layer.toml);
    }
    let expected_markers = TomlValue::try_from(markers)?;
    assert_eq!(
        (
            effective.get("project_root_markers"),
            local_effective.get("project_root_markers"),
        ),
        (Some(&expected_markers), Some(&expected_markers))
    );
    Ok(())
}

#[tokio::test]
async fn managed_project_discovery_uses_file_markers_without_changing_precedence()
-> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    for (markers, dirs) in [
        (
            vec![".git"],
            vec![&fixture.repo, &fixture.project, &fixture.cwd],
        ),
        (vec![], vec![&fixture.cwd]),
        (vec![".company-root"], vec![&fixture.project, &fixture.cwd]),
    ] {
        let markers_toml = TomlValue::try_from(&markers)?;
        std::fs::write(
            &fixture.managed,
            format!("project_root_markers = {markers_toml}\nmodel = \"managed\"\n"),
        )?;
        let (canonical, local) = fixture.load().await?;
        assert_discovery(&canonical, &local, &dirs, &markers)?;
        let managed_source = ConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: AbsolutePathBuf::from_absolute_path(&fixture.managed)?,
        };
        assert_eq!(
            (
                canonical.effective_config().get("model").cloned(),
                &canonical.origins()["model"].name,
                &local.config.layers.last().expect("managed layer").source,
            ),
            (
                Some(TomlValue::String("managed".into())),
                &managed_source,
                &managed_source,
            )
        );
    }
    Ok(())
}

#[tokio::test]
async fn managed_project_discovery_ignores_incomplete_nested_git_directory() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    std::fs::create_dir(fixture.cwd.join(".git"))?;
    std::fs::write(&fixture.managed, "project_root_markers = [\".git\"]\n")?;

    let (canonical, local) = fixture.load().await?;
    assert_discovery(
        &canonical,
        &local,
        &[&fixture.repo, &fixture.project, &fixture.cwd],
        &[".git"],
    )?;

    std::fs::write(fixture.home.join("config.toml"), fixture.trust("untrusted"))?;
    let (canonical, local) = fixture.load().await?;
    assert_discovery(&canonical, &local, &[], &[".git"])?;
    let trust_key = project_trust_key(fixture.repo.as_path());
    assert!(
        canonical
            .all_layers_low_to_high()
            .filter(|layer| matches!(layer.name, ConfigLayerSource::Project { .. }))
            .all(|layer| layer.disabled_reason.as_deref().is_some_and(
                |reason| reason.starts_with(&format!("{trust_key} is marked as untrusted"))
            ))
    );
    Ok(())
}

#[tokio::test]
async fn managed_project_discovery_uses_managed_project_trust() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    let trust_key = project_trust_key(fixture.repo.as_path());
    let user_config = fixture.home.join("config.toml");
    let untrusted_reason = format!(
        "{trust_key} is marked as untrusted in the effective configuration. To load project-local config, hooks, and exec policies, update its trust setting. If that setting is managed by your organization, contact your administrator."
    );
    let unknown_reason = format!(
        "To load project-local config, hooks, and exec policies, add {trust_key} as a trusted project in {}.",
        user_config.display()
    );
    for (user, managed, dirs, expected_reason) in [
        (
            fixture.trust("trusted"),
            fixture.trust("untrusted"),
            vec![],
            Some(untrusted_reason.as_str()),
        ),
        (
            fixture.trust("untrusted"),
            fixture.trust("trusted"),
            vec![&fixture.cwd],
            None,
        ),
        (
            String::new(),
            String::new(),
            vec![],
            Some(unknown_reason.as_str()),
        ),
    ] {
        std::fs::write(&user_config, user)?;
        std::fs::write(
            &fixture.managed,
            format!("project_root_markers = []\n{managed}"),
        )?;
        let (canonical, local) = fixture.load().await?;
        assert_discovery(&canonical, &local, &dirs, &[])?;
        assert_eq!(
            canonical
                .all_layers_low_to_high()
                .filter(|layer| matches!(layer.name, ConfigLayerSource::Project { .. }))
                .map(|layer| layer.disabled_reason.as_deref())
                .collect::<Vec<_>>(),
            vec![expected_reason]
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn managed_project_discovery_mdm_overrides_file_markers() -> anyhow::Result<()> {
    use base64::Engine;
    use base64::prelude::BASE64_STANDARD;

    let mut fixture = Fixture::new()?;
    std::fs::write(&fixture.managed, "project_root_markers = [\".git\"]\n")?;
    for (markers, dirs) in [
        (vec![], vec![&fixture.cwd]),
        (vec![".company-root"], vec![&fixture.project, &fixture.cwd]),
    ] {
        let markers_toml = TomlValue::try_from(&markers)?;
        fixture.overrides.managed_preferences_base64 =
            Some(BASE64_STANDARD.encode(format!("project_root_markers = {markers_toml}\n")));
        let (canonical, local) = fixture.load().await?;
        assert_discovery(&canonical, &local, &dirs, &markers)?;
        assert_eq!(
            (
                canonical
                    .layers_high_to_low()
                    .next()
                    .map(|layer| &layer.name),
                local.config.layers.last().map(|layer| &layer.source),
            ),
            (
                Some(&ConfigLayerSource::LegacyManagedConfigTomlFromMdm),
                Some(&ConfigLayerSource::LegacyManagedConfigTomlFromMdm),
            )
        );
    }
    Ok(())
}
