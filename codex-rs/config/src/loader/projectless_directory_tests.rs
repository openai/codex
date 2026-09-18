//! Projectless classification requires completed discovery without project inputs.

use crate::ConfigLayerStack;
use crate::LoaderOverrides;
use crate::NoopThreadConfigLoader;
use crate::loader::find_project_root;
use crate::loader::load_config_layers_state;
use crate::loader::project_trust_key;
use crate::loader::tests::TestFileSystem;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use toml::Value as TomlValue;

struct Fixture {
    _temp: TempDir,
    home: AbsolutePathBuf,
    cwd: AbsolutePathBuf,
    overrides: LoaderOverrides,
}

impl Fixture {
    fn new() -> anyhow::Result<Self> {
        let temp = tempfile::tempdir()?;
        let root = AbsolutePathBuf::from_absolute_path(temp.path().canonicalize()?)?;
        let home = root.join("home");
        let cwd = root.join("workspace");
        std::fs::create_dir_all(&home)?;
        std::fs::create_dir_all(&cwd)?;
        Ok(Self {
            _temp: temp,
            home,
            cwd,
            overrides: LoaderOverrides::without_managed_config_for_tests(),
        })
    }

    async fn load(&self) -> anyhow::Result<ConfigLayerStack> {
        Ok(load_config_layers_state(
            &TestFileSystem,
            self.home.as_path(),
            Some(self.cwd.clone()),
            &[],
            self.overrides.clone(),
            &NoopThreadConfigLoader,
        )
        .await?)
    }
}

#[tokio::test]
async fn project_root_lookup_preserves_cwd_fallback() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    let markers = vec![".company-root".to_string()];
    for markers in [&[][..], markers.as_slice()] {
        assert_eq!(
            find_project_root(&TestFileSystem, &fixture.cwd, markers).await?,
            fixture.cwd,
        );
    }

    std::fs::create_dir(fixture.cwd.join(".company-root"))?;
    let nested = fixture.cwd.join("nested");
    std::fs::create_dir(&nested)?;
    assert_eq!(
        find_project_root(&TestFileSystem, &nested, &markers).await?,
        fixture.cwd,
    );
    Ok(())
}

#[tokio::test]
async fn unmarked_directory_is_projectless_even_with_saved_trust() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    assert!(fixture.load().await?.is_projectless());
    let key = TomlValue::String(project_trust_key(fixture.cwd.as_path()));
    for level in ["trusted", "untrusted"] {
        std::fs::write(
            fixture.home.join("config.toml"),
            format!("[projects.{key}]\ntrust_level = \"{level}\"\n"),
        )?;
        assert!(fixture.load().await?.is_projectless(), "{level}");
    }
    Ok(())
}

#[tokio::test]
async fn skipped_discovery_does_not_claim_projectless() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    for (cwd, ignore_project_config) in [(None, false), (Some(fixture.cwd.clone()), true)] {
        let layers = load_config_layers_state(
            &TestFileSystem,
            fixture.home.as_path(),
            cwd,
            &[],
            LoaderOverrides {
                ignore_project_config,
                ..fixture.overrides.clone()
            },
            &NoopThreadConfigLoader,
        )
        .await?;
        assert!(!layers.is_projectless());
    }
    Ok(())
}

#[tokio::test]
async fn project_markers_and_local_layers_prevent_projectless_classification() -> anyhow::Result<()>
{
    for (marker, config, child) in [
        (".git", "", ""),
        (".git", "", "nested"),
        (".git", "project_root_markers = []", "nested"),
        (".codex", "", ""),
        (".codex", "project_root_markers = ['.codex']", "nested"),
        (
            ".company-root",
            "project_root_markers = ['.company-root']",
            "",
        ),
        (
            ".company-root",
            "project_root_markers = ['.company-root']",
            "nested",
        ),
    ] {
        let mut fixture = Fixture::new()?;
        std::fs::create_dir(fixture.cwd.join(marker))?;
        if marker == ".git" {
            std::fs::write(fixture.cwd.join(".git/HEAD"), "ref: refs/heads/main\n")?;
        }
        std::fs::write(fixture.home.join("config.toml"), config)?;
        fixture.cwd = fixture.cwd.join(child);
        std::fs::create_dir_all(&fixture.cwd)?;
        assert!(
            !fixture.load().await?.is_projectless(),
            "marker={marker}, config={config}, child={child}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn user_codex_home_is_not_a_project_layer() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.home = fixture.cwd.join(".codex");
    std::fs::create_dir(&fixture.home)?;
    std::fs::write(fixture.home.join("config.toml"), "model = 'user-model'\n")?;
    assert!(fixture.load().await?.is_projectless());
    Ok(())
}

#[tokio::test]
async fn managed_root_markers_control_projectless_classification() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    std::fs::create_dir(fixture.cwd.join(".company-root"))?;
    fixture.cwd = fixture.cwd.join("nested");
    std::fs::create_dir(&fixture.cwd)?;
    std::fs::write(
        fixture.home.join("config.toml"),
        "project_root_markers = []",
    )?;
    let managed = fixture.home.join("managed_config.toml");
    fixture.overrides = LoaderOverrides::with_managed_config_path_for_tests(managed.to_path_buf());
    for (markers, expected) in [("['.company-root']", false), ("[]", true)] {
        std::fs::write(&managed, format!("project_root_markers = {markers}\n"))?;
        assert_eq!(
            fixture.load().await?.is_projectless(),
            expected,
            "{markers}"
        );
    }
    Ok(())
}
