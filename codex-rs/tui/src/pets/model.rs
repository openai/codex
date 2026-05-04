use std::collections::HashMap;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

use super::catalog;

#[derive(Debug, Clone)]
pub struct AnimationFrame {
    pub sprite_index: usize,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct Animation {
    pub frames: Vec<AnimationFrame>,
    pub loop_start: Option<usize>,
    pub fallback: String,
}

impl Animation {
    pub(super) fn total_duration(&self) -> Duration {
        self.frames
            .iter()
            .map(|frame| frame.duration)
            .sum::<Duration>()
    }
}

#[derive(Debug, Clone)]
pub struct Pet {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub spritesheet_path: PathBuf,
    pub frame_width: u32,
    pub frame_height: u32,
    pub columns: u32,
    pub rows: u32,
    pub animations: HashMap<String, Animation>,
}

impl Pet {
    pub(super) fn load_with_codex_home(value: &str, codex_home: Option<&Path>) -> Result<Self> {
        if path_like(value) {
            return load_pet_path(value);
        }

        if let Some(custom_id) = value.strip_prefix(CUSTOM_PET_PREFIX) {
            return load_custom_pet(custom_id, codex_home);
        }

        if let Some(builtin) = catalog::builtin_pet(value) {
            return load_builtin_pet(builtin);
        }

        load_custom_pet(value, codex_home)
    }

    pub fn frame_count(&self) -> usize {
        (self.columns * self.rows) as usize
    }
}

pub(super) const CUSTOM_PET_PREFIX: &str = "custom:";

#[derive(Debug, Deserialize)]
struct PetFile {
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "spritesheetPath")]
    spritesheet_path: Option<String>,
    frame: Option<FrameSpec>,
    #[serde(default)]
    animations: HashMap<String, AnimationSpec>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct FrameSpec {
    width: u32,
    height: u32,
    columns: u32,
    rows: u32,
}

impl Default for FrameSpec {
    fn default() -> Self {
        Self {
            width: catalog::DEFAULT_FRAME_WIDTH,
            height: catalog::DEFAULT_FRAME_HEIGHT,
            columns: catalog::DEFAULT_FRAME_COLUMNS,
            rows: catalog::DEFAULT_FRAME_ROWS,
        }
    }
}

pub(super) fn custom_pet_selector(id: &str) -> String {
    format!("{CUSTOM_PET_PREFIX}{id}")
}

#[derive(Debug, Deserialize)]
struct AnimationSpec {
    #[serde(default)]
    frames: Vec<usize>,
    fps: Option<f64>,
    #[serde(rename = "loop")]
    loop_animation: Option<bool>,
    #[serde(default)]
    fallback: String,
}

fn load_builtin_pet(pet: catalog::BuiltinPet) -> Result<Pet> {
    let spritesheet_path = catalog::builtin_spritesheet_path(pet.spritesheet_file);
    if !spritesheet_path.exists() {
        bail!("missing spritesheet {}", spritesheet_path.display());
    }

    Ok(Pet {
        id: pet.id.to_string(),
        display_name: pet.display_name.to_string(),
        description: pet.description.to_string(),
        spritesheet_path,
        frame_width: catalog::DEFAULT_FRAME_WIDTH,
        frame_height: catalog::DEFAULT_FRAME_HEIGHT,
        columns: catalog::DEFAULT_FRAME_COLUMNS,
        rows: catalog::DEFAULT_FRAME_ROWS,
        animations: default_animations(),
    })
}

fn load_custom_pet(value: &str, codex_home: Option<&Path>) -> Result<Pet> {
    let codex_home = codex_home.context("CODEX_HOME is not available")?;
    let pet_dir = codex_home.join("pets").join(value);
    if pet_dir.join("pet.json").is_file() {
        return load_pet_manifest(&pet_dir, "pet.json", value, &custom_pet_cache_id(value));
    }

    let avatar_dir = codex_home.join("avatars").join(value);
    if avatar_dir.join("avatar.json").is_file() {
        return load_pet_manifest(
            &avatar_dir,
            "avatar.json",
            value,
            &custom_pet_cache_id(value),
        );
    }

    bail!("unknown pet {value}")
}

fn load_pet_path(value: &str) -> Result<Pet> {
    let path = expand_path(value)?;
    let metadata = fs::metadata(&path).with_context(|| format!("pet path {}", path.display()))?;
    let dir = if metadata.is_dir() {
        path
    } else {
        path.parent()
            .context("pet json path has no containing directory")?
            .to_path_buf()
    };
    let pet_dir = dir
        .canonicalize()
        .with_context(|| format!("resolve {}", dir.display()))?;
    let manifest_file = if pet_dir.join("pet.json").is_file() {
        "pet.json"
    } else if pet_dir.join("avatar.json").is_file() {
        "avatar.json"
    } else {
        bail!("missing pet.json or avatar.json in {}", pet_dir.display());
    };
    let fallback_id = pet_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("pet");
    load_pet_manifest(&pet_dir, manifest_file, fallback_id, fallback_id)
}

fn load_pet_manifest(
    pet_dir: &Path,
    manifest_file: &str,
    fallback_id: &str,
    cache_id: &str,
) -> Result<Pet> {
    let config_path = pet_dir.join(manifest_file);
    let raw = fs::read_to_string(&config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let file: PetFile =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", config_path.display()))?;

    let manifest_id = file
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let display_name = file
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .or(manifest_id)
        .unwrap_or(fallback_id)
        .to_string();
    let pet_id = if cache_id == fallback_id {
        manifest_id.unwrap_or(fallback_id).to_string()
    } else {
        cache_id.to_string()
    };
    let description = file
        .description
        .map(|description| description.trim().to_string())
        .unwrap_or_default();
    let spritesheet_path = resolve_spritesheet_path(
        pet_dir,
        file.spritesheet_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .unwrap_or("spritesheet.webp"),
    )?;
    if !spritesheet_path.exists() {
        bail!("missing spritesheet {}", spritesheet_path.display());
    }
    validate_app_spritesheet_dimensions(&spritesheet_path)?;

    let frame = file.frame.unwrap_or_default();
    Ok(Pet {
        id: pet_id,
        display_name,
        description,
        spritesheet_path,
        frame_width: frame.width,
        frame_height: frame.height,
        columns: frame.columns,
        rows: frame.rows,
        animations: load_animations(file.animations),
    })
}

fn resolve_spritesheet_path(pet_dir: &Path, spritesheet_path: &str) -> Result<PathBuf> {
    let path = Path::new(spritesheet_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        bail!("spritesheet path must stay inside {}", pet_dir.display());
    }
    Ok(pet_dir.join(path))
}

fn validate_app_spritesheet_dimensions(path: &Path) -> Result<()> {
    let (width, height) =
        image::image_dimensions(path).with_context(|| format!("read {}", path.display()))?;
    if width != catalog::SPRITESHEET_WIDTH || height != catalog::SPRITESHEET_HEIGHT {
        bail!(
            "spritesheet must be {}x{} pixels",
            catalog::SPRITESHEET_WIDTH,
            catalog::SPRITESHEET_HEIGHT
        );
    }
    Ok(())
}

fn custom_pet_cache_id(id: &str) -> String {
    format!("custom-{id}")
}

fn path_like(value: &str) -> bool {
    value == "."
        || value == ".."
        || value.starts_with("~/")
        || value.starts_with("../")
        || value.starts_with("./")
        || Path::new(value).is_absolute()
        || value.contains('/')
        || value.contains('\\')
}

fn expand_path(value: &str) -> Result<PathBuf> {
    if value == "~" || value.starts_with("~/") {
        let home = std::env::var_os("HOME").context("HOME is not set")?;
        if value == "~" {
            return Ok(PathBuf::from(home));
        }
        return Ok(PathBuf::from(home).join(&value[2..]));
    }

    Ok(PathBuf::from(value))
}

fn load_animations(specs: HashMap<String, AnimationSpec>) -> HashMap<String, Animation> {
    let mut animations = default_animations();
    if specs.is_empty() {
        return animations;
    }

    for (name, spec) in specs {
        if spec.frames.is_empty() {
            continue;
        }

        let fps = spec.fps.filter(|fps| *fps > 0.0).unwrap_or(8.0);
        let duration = Duration::from_secs_f64(1.0 / fps);
        let fallback = if spec.fallback.is_empty() {
            "idle".to_string()
        } else {
            spec.fallback
        };
        let loop_start = spec
            .loop_animation
            .unwrap_or(/*default*/ true)
            .then_some(/*loop_start*/ 0);

        animations.insert(
            name.clone(),
            Animation {
                frames: spec
                    .frames
                    .into_iter()
                    .map(|sprite_index| AnimationFrame {
                        sprite_index,
                        duration,
                    })
                    .collect(),
                loop_start,
                fallback,
            },
        );
    }

    animations
        .entry("idle".to_string())
        .or_insert_with(idle_animation);
    animations
}

fn default_animations() -> HashMap<String, Animation> {
    [
        ("idle", idle_animation()),
        (
            "running-right",
            app_state_animation(
                /*row_index*/ 1, /*frame_count*/ 8, /*frame_duration_ms*/ 120,
                /*final_frame_duration_ms*/ 220,
            ),
        ),
        (
            "running-left",
            app_state_animation(
                /*row_index*/ 2, /*frame_count*/ 8, /*frame_duration_ms*/ 120,
                /*final_frame_duration_ms*/ 220,
            ),
        ),
        (
            "waving",
            app_state_animation(
                /*row_index*/ 3, /*frame_count*/ 4, /*frame_duration_ms*/ 140,
                /*final_frame_duration_ms*/ 280,
            ),
        ),
        (
            "jumping",
            app_state_animation(
                /*row_index*/ 4, /*frame_count*/ 5, /*frame_duration_ms*/ 140,
                /*final_frame_duration_ms*/ 280,
            ),
        ),
        (
            "failed",
            app_state_animation(
                /*row_index*/ 5, /*frame_count*/ 8, /*frame_duration_ms*/ 140,
                /*final_frame_duration_ms*/ 240,
            ),
        ),
        (
            "waiting",
            app_state_animation(
                /*row_index*/ 6, /*frame_count*/ 6, /*frame_duration_ms*/ 150,
                /*final_frame_duration_ms*/ 260,
            ),
        ),
        (
            "running",
            app_state_animation(
                /*row_index*/ 7, /*frame_count*/ 6, /*frame_duration_ms*/ 120,
                /*final_frame_duration_ms*/ 220,
            ),
        ),
        (
            "review",
            app_state_animation(
                /*row_index*/ 8, /*frame_count*/ 6, /*frame_duration_ms*/ 150,
                /*final_frame_duration_ms*/ 280,
            ),
        ),
        (
            "move_right",
            app_state_animation(
                /*row_index*/ 1, /*frame_count*/ 8, /*frame_duration_ms*/ 120,
                /*final_frame_duration_ms*/ 220,
            ),
        ),
        (
            "move_left",
            app_state_animation(
                /*row_index*/ 2, /*frame_count*/ 8, /*frame_duration_ms*/ 120,
                /*final_frame_duration_ms*/ 220,
            ),
        ),
        (
            "wave",
            app_state_animation(
                /*row_index*/ 3, /*frame_count*/ 4, /*frame_duration_ms*/ 140,
                /*final_frame_duration_ms*/ 280,
            ),
        ),
        (
            "bounce",
            app_state_animation(
                /*row_index*/ 4, /*frame_count*/ 5, /*frame_duration_ms*/ 140,
                /*final_frame_duration_ms*/ 280,
            ),
        ),
        (
            "sad",
            app_state_animation(
                /*row_index*/ 5, /*frame_count*/ 8, /*frame_duration_ms*/ 140,
                /*final_frame_duration_ms*/ 240,
            ),
        ),
    ]
    .into_iter()
    .map(|(name, animation)| (name.to_string(), animation))
    .collect()
}

fn idle_animation() -> Animation {
    Animation {
        frames: [(0, 1680), (1, 660), (2, 660), (3, 840), (4, 840), (5, 1920)]
            .into_iter()
            .map(|(sprite_index, duration_ms)| AnimationFrame {
                sprite_index,
                duration: Duration::from_millis(duration_ms),
            })
            .collect(),
        loop_start: Some(/*loop_start*/ 0),
        fallback: "idle".to_string(),
    }
}

fn app_state_animation(
    row_index: usize,
    frame_count: usize,
    frame_duration_ms: u64,
    final_frame_duration_ms: u64,
) -> Animation {
    let primary_frames = (0..frame_count)
        .map(|column_index| AnimationFrame {
            sprite_index: row_index * catalog::DEFAULT_FRAME_COLUMNS as usize + column_index,
            duration: Duration::from_millis(if column_index == frame_count - 1 {
                final_frame_duration_ms
            } else {
                frame_duration_ms
            }),
        })
        .collect::<Vec<_>>();
    let primary_frame_count = primary_frames.len() * 3;
    let frames = primary_frames
        .iter()
        .chain(primary_frames.iter())
        .chain(primary_frames.iter())
        .cloned()
        .chain(idle_animation().frames)
        .collect();
    Animation {
        frames,
        loop_start: Some(primary_frame_count),
        fallback: "idle".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_minimal_pet() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("pet.json"),
            r#"{
                "id": "chefito",
                "displayName": "Chefito",
                "description": "A tiny recipe-loving chef",
                "spritesheetPath": "spritesheet.webp"
            }"#,
        )
        .unwrap();
        fs::copy(
            catalog::builtin_spritesheet_path("codex-spritesheet-v3.webp"),
            dir.path().join("spritesheet.webp"),
        )
        .unwrap();
        dir
    }

    #[test]
    fn load_builtin_pet_uses_app_catalog_storage() {
        let codex_home = tempfile::tempdir().unwrap();

        let pet =
            Pet::load_with_codex_home("dewey", /*codex_home*/ Some(codex_home.path())).unwrap();

        assert_eq!(pet.id, "dewey");
        assert_eq!(pet.display_name, "Dewey");
        assert_eq!(pet.description, "A tidy duck for calm workspace days.");
        assert_eq!(
            pet.spritesheet_path,
            catalog::builtin_spritesheet_path("dewey-spritesheet-v3.webp")
        );
        assert_eq!(pet.frame_width, 192);
        assert_eq!(pet.frame_height, 208);
        assert_eq!(pet.columns, 8);
        assert_eq!(pet.rows, 9);
    }

    #[test]
    fn app_idle_animation_uses_calm_loop() {
        let animations = default_animations();
        let idle = &animations["idle"];

        assert_eq!(sprite_indices(idle), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(durations_ms(idle), vec![1680, 660, 660, 840, 840, 1920]);
        assert_eq!(idle.loop_start, Some(/*loop_start*/ 0));
    }

    #[test]
    fn app_running_animation_repeats_then_settles_into_idle() {
        let animations = default_animations();
        let running = &animations["running"];
        let primary = vec![56, 57, 58, 59, 60, 61];

        assert_eq!(sprite_indices(running)[0..6], primary);
        assert_eq!(sprite_indices(running)[6..12], primary);
        assert_eq!(sprite_indices(running)[12..18], primary);
        assert_eq!(
            sprite_indices(running)[18..],
            sprite_indices(&animations["idle"])
        );
        assert_eq!(
            durations_ms(running)[0..6],
            vec![120, 120, 120, 120, 120, 220]
        );
        assert_eq!(running.loop_start, Some(/*loop_start*/ 18));
    }

    #[test]
    fn app_notification_states_use_expected_rows() {
        let animations = default_animations();

        assert_eq!(
            sprite_indices(&animations["waiting"])[0..6],
            vec![48, 49, 50, 51, 52, 53]
        );
        assert_eq!(
            sprite_indices(&animations["review"])[0..6],
            vec![64, 65, 66, 67, 68, 69]
        );
        assert_eq!(
            sprite_indices(&animations["failed"])[0..8],
            vec![40, 41, 42, 43, 44, 45, 46, 47]
        );
    }

    #[test]
    fn custom_animation_specs_keep_manifest_fps_and_loop_shape() {
        let animations = load_animations(HashMap::from([(
            "custom".to_string(),
            AnimationSpec {
                frames: vec![1, 2],
                fps: Some(/*fps*/ 2.0),
                loop_animation: Some(/*loop_animation*/ false),
                fallback: "idle".to_string(),
            },
        )]));
        let custom = &animations["custom"];

        assert_eq!(sprite_indices(custom), vec![1, 2]);
        assert_eq!(durations_ms(custom), vec![500, 500]);
        assert_eq!(custom.loop_start, None);
        assert_eq!(custom.fallback, "idle");
    }

    #[test]
    fn load_pet_directory_uses_app_pet_manifest_defaults() {
        let dir = write_minimal_pet();

        let pet =
            Pet::load_with_codex_home(dir.path().to_str().unwrap(), /*codex_home*/ None).unwrap();

        assert_eq!(pet.id, "chefito");
        assert_eq!(pet.display_name, "Chefito");
        assert_eq!(pet.frame_width, 192);
        assert_eq!(pet.frame_height, 208);
        assert_eq!(pet.columns, 8);
        assert_eq!(pet.rows, 9);
        assert!(!pet.animations["idle"].frames.is_empty());
    }

    #[test]
    fn load_pet_json_path_uses_containing_directory() {
        let dir = write_minimal_pet();

        let pet = Pet::load_with_codex_home(
            dir.path().join("pet.json").to_str().unwrap(),
            /*codex_home*/ None,
        )
        .unwrap();
        let expected = dir.path().join("spritesheet.webp").canonicalize().unwrap();

        assert_eq!(pet.spritesheet_path, expected);
    }

    #[test]
    fn custom_pet_selector_loads_codex_home_pet_manifest() {
        let dir = write_minimal_pet();
        let codex_home = tempfile::tempdir().unwrap();
        let pet_dir = codex_home.path().join("pets").join("chefito");
        fs::create_dir_all(&pet_dir).unwrap();
        fs::copy(dir.path().join("pet.json"), pet_dir.join("pet.json")).unwrap();
        fs::copy(
            dir.path().join("spritesheet.webp"),
            pet_dir.join("spritesheet.webp"),
        )
        .unwrap();

        let pet = Pet::load_with_codex_home(
            &custom_pet_selector("chefito"),
            /*codex_home*/ Some(codex_home.path()),
        )
        .unwrap();

        assert_eq!(pet.id, "custom-chefito");
        assert_eq!(pet.spritesheet_path, pet_dir.join("spritesheet.webp"),);
    }

    #[test]
    fn custom_pet_selector_falls_back_to_legacy_avatar_manifest() {
        let dir = write_minimal_pet();
        let codex_home = tempfile::tempdir().unwrap();
        let avatar_dir = codex_home.path().join("avatars").join("legacy");
        fs::create_dir_all(&avatar_dir).unwrap();
        fs::copy(dir.path().join("pet.json"), avatar_dir.join("avatar.json")).unwrap();
        fs::copy(
            dir.path().join("spritesheet.webp"),
            avatar_dir.join("spritesheet.webp"),
        )
        .unwrap();

        let pet = Pet::load_with_codex_home(
            &custom_pet_selector("legacy"),
            /*codex_home*/ Some(codex_home.path()),
        )
        .unwrap();

        assert_eq!(pet.id, "custom-legacy");
        assert_eq!(pet.display_name, "Chefito");
    }

    #[test]
    fn custom_pet_rejects_spritesheet_path_escape() {
        let codex_home = tempfile::tempdir().unwrap();
        let pet_dir = codex_home.path().join("pets").join("escape");
        fs::create_dir_all(&pet_dir).unwrap();
        fs::write(
            pet_dir.join("pet.json"),
            r#"{
                "displayName": "Escape",
                "spritesheetPath": "../spritesheet.webp"
            }"#,
        )
        .unwrap();

        let err = Pet::load_with_codex_home(
            &custom_pet_selector("escape"),
            /*codex_home*/ Some(codex_home.path()),
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("spritesheet path must stay inside")
        );
    }

    fn sprite_indices(animation: &Animation) -> Vec<usize> {
        animation
            .frames
            .iter()
            .map(|frame| frame.sprite_index)
            .collect()
    }

    fn durations_ms(animation: &Animation) -> Vec<u128> {
        animation
            .frames
            .iter()
            .map(|frame| frame.duration.as_millis())
            .collect()
    }
}
