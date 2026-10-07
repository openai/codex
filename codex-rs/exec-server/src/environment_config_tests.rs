use super::*;
use pretty_assertions::assert_eq;

/// Legacy file and MDM values must still win after a CLI preference is projected.
#[test]
fn startup_preference_stays_below_both_managed_layers() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let cwd = AbsolutePathBuf::from_absolute_path(temp.path())?;
    let project = ConfigLayerSource::Project {
        dot_codex_folder: cwd.join(".codex"),
    };
    let file = ConfigLayerSource::LegacyManagedConfigTomlFromFile {
        file: cwd.join("managed_config.toml"),
    };
    for managed in [file, ConfigLayerSource::LegacyManagedConfigTomlFromMdm] {
        let mut layers = LocalConfigLayers {
            config: LocalTomlLayerStack {
                layers: vec![project.clone(), managed.clone()]
                    .into_iter()
                    .map(|source| LocalTomlLayer {
                        source,
                        base_dir: cwd.clone(),
                        toml: toml::Value::Table(toml::toml! { [features] prefer_mxc = false }),
                    })
                    .collect(),
                cloud_insertion_index: 0,
            },
            requirements: LocalTomlLayerStack {
                layers: Vec::new(),
                cloud_insertion_index: 0,
            },
        };
        include_startup_preference(&mut layers, &cwd, Some(true));
        let projected = layers.project(&[vec!["features".into(), "prefer_mxc".into()]], &[]);
        assert_eq!(
            projected
                .config
                .layers
                .iter()
                .map(|l| &l.source)
                .collect::<Vec<_>>(),
            [&project, &ConfigLayerSource::SessionFlags, &managed],
        );
        let mut effective = toml::Value::Table(toml::map::Map::new());
        for layer in projected.config.layers {
            codex_config::merge_toml_values(&mut effective, &layer.toml);
        }
        assert_eq!(effective["features"]["prefer_mxc"].as_bool(), Some(false));
        assert_eq!(projected.config.cloud_insertion_index, 0);
    }
    Ok(())
}
