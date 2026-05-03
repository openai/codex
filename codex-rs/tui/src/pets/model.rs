use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Animation {
    pub frames: Vec<usize>,
    pub fps: f64,
    pub loop_animation: bool,
    pub fallback: String,
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
    pub(super) fn load(value: &str) -> Result<Self> {
        Self::load_with_codex_home(
            value,
            crate::legacy_core::config::find_codex_home()
                .ok()
                .as_deref(),
        )
    }

    pub(super) fn load_with_codex_home(value: &str, codex_home: Option<&Path>) -> Result<Self> {
        let pet_dir = resolve_pet_dir(value, codex_home)?;
        let config_path = pet_dir.join("pet.json");
        let raw = fs::read_to_string(&config_path)
            .with_context(|| format!("read {}", config_path.display()))?;
        let mut file: PetFile = serde_json::from_str(&raw)
            .with_context(|| format!("parse {}", config_path.display()))?;

        if file.id.is_empty() {
            file.id = pet_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("pet")
                .to_string();
        }
        if file.display_name.is_empty() {
            file.display_name.clone_from(&file.id);
        }
        if file.spritesheet_path.is_empty() {
            file.spritesheet_path = "spritesheet.webp".to_string();
        }

        let frame = file.frame.unwrap_or_default();
        let spritesheet_path = if Path::new(&file.spritesheet_path).is_absolute() {
            PathBuf::from(&file.spritesheet_path)
        } else {
            pet_dir.join(&file.spritesheet_path)
        };
        if !spritesheet_path.exists() {
            bail!("missing spritesheet {}", spritesheet_path.display());
        }

        Ok(Self {
            id: file.id,
            display_name: file.display_name,
            description: file.description,
            spritesheet_path,
            frame_width: frame.width,
            frame_height: frame.height,
            columns: frame.columns,
            rows: frame.rows,
            animations: load_animations(file.animations),
        })
    }

    pub fn frame_count(&self) -> usize {
        (self.columns * self.rows) as usize
    }
}

#[derive(Debug, Deserialize)]
struct PetFile {
    #[serde(default)]
    id: String,
    #[serde(default, rename = "displayName")]
    display_name: String,
    #[serde(default)]
    description: String,
    #[serde(default, rename = "spritesheetPath")]
    spritesheet_path: String,
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
            width: 192,
            height: 208,
            columns: 8,
            rows: 9,
        }
    }
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

fn resolve_pet_dir(value: &str, codex_home: Option<&Path>) -> Result<PathBuf> {
    if path_like(value) {
        let path = expand_path(value)?;
        let metadata =
            fs::metadata(&path).with_context(|| format!("pet path {}", path.display()))?;
        let dir = if metadata.is_dir() {
            path
        } else {
            path.parent()
                .context("pet json path has no containing directory")?
                .to_path_buf()
        };
        return dir
            .canonicalize()
            .with_context(|| format!("resolve {}", dir.display()));
    }

    Ok(resolve_named_pet_dir(value, codex_home))
}

fn resolve_named_pet_dir(value: &str, codex_home: Option<&Path>) -> PathBuf {
    if let Some(codex_home) = codex_home {
        let installed_pet = codex_home.join("pets").join(value);
        if installed_pet.join("pet.json").is_file() {
            return installed_pet;
        }
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("pets")
        .join(value)
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
        let fallback = if spec.fallback.is_empty() {
            "idle".to_string()
        } else {
            spec.fallback
        };

        animations.insert(
            name.clone(),
            Animation {
                frames: spec.frames,
                fps,
                loop_animation: spec.loop_animation.unwrap_or(true),
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
    let idle = idle_animation();
    [
        ("idle", idle.frames, idle.fps, idle.loop_animation, "idle"),
        (
            "move_left",
            vec![8, 9, 10, 11, 12, 13, 14, 15],
            10.0,
            true,
            "idle",
        ),
        (
            "move_right",
            vec![16, 17, 18, 19, 20, 21, 22, 23],
            10.0,
            true,
            "idle",
        ),
        ("wave", vec![24, 25, 26, 27], 7.0, false, "idle"),
        ("sit", vec![32, 33, 34, 35, 36], 6.0, true, "idle"),
        ("sad", vec![40, 41, 42, 43, 44, 45, 46], 6.0, true, "idle"),
        ("sleep", vec![43, 44, 47], 3.0, true, "idle"),
        ("sip", vec![48, 49, 50, 51, 52, 53], 8.0, false, "idle"),
        ("bounce", vec![56, 57, 58, 59, 60, 61], 9.0, false, "idle"),
        ("grumpy", vec![64, 65, 66, 67, 68, 69], 6.0, false, "idle"),
    ]
    .into_iter()
    .map(|(name, frames, fps, loop_animation, fallback)| {
        (
            name.to_string(),
            Animation {
                frames,
                fps,
                loop_animation,
                fallback: fallback.to_string(),
            },
        )
    })
    .collect()
}

fn idle_animation() -> Animation {
    Animation {
        frames: vec![0, 1, 2, 3, 4, 5],
        fps: 5.0,
        loop_animation: true,
        fallback: "idle".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

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
        fs::File::create(dir.path().join("spritesheet.webp"))
            .unwrap()
            .write_all(b"not-used-by-loader")
            .unwrap();
        dir
    }

    #[test]
    fn load_pet_directory_uses_installed_pet_defaults() {
        let dir = write_minimal_pet();

        let pet = Pet::load(dir.path().to_str().unwrap()).unwrap();

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

        let pet = Pet::load(dir.path().join("pet.json").to_str().unwrap()).unwrap();
        let expected = dir.path().join("spritesheet.webp").canonicalize().unwrap();

        assert_eq!(pet.spritesheet_path, expected);
    }

    #[test]
    fn named_pet_prefers_codex_home_installation() {
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

        let pet = Pet::load_with_codex_home("chefito", Some(codex_home.path())).unwrap();

        assert_eq!(pet.id, "chefito");
        assert_eq!(pet.spritesheet_path, pet_dir.join("spritesheet.webp"),);
    }
}
