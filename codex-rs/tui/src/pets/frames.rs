use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use image::GenericImageView;

use super::model::Pet;

pub(super) fn prepare_png_frames(pet: &Pet, frame_dir: &Path) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(frame_dir).with_context(|| format!("create {}", frame_dir.display()))?;

    let expected: Vec<PathBuf> = (0..pet.frame_count())
        .map(|index| frame_dir.join(format!("frame_{index:03}.png")))
        .collect();

    let complete = expected.iter().all(|path| path.exists());
    if !complete {
        for stale in glob_frame_files(frame_dir)? {
            let _ = fs::remove_file(stale);
        }

        let spritesheet = image::open(&pet.spritesheet_path)
            .with_context(|| format!("read {}", pet.spritesheet_path.display()))?;
        for row in 0..pet.rows {
            for column in 0..pet.columns {
                let index = row
                    .checked_mul(pet.columns)
                    .and_then(|row_offset| row_offset.checked_add(column))
                    .context("pet frame index overflow")?;
                let index = usize::try_from(index).context("pet frame index does not fit usize")?;
                let path = expected
                    .get(index)
                    .context("pet frame index exceeds expected frame count")?;
                let x = column
                    .checked_mul(pet.frame_width)
                    .context("pet frame x offset overflow")?;
                let y = row
                    .checked_mul(pet.frame_height)
                    .context("pet frame y offset overflow")?;
                let frame = spritesheet.try_view(x, y, pet.frame_width, pet.frame_height)?;
                frame
                    .to_image()
                    .save_with_format(path, image::ImageFormat::Png)
                    .with_context(|| format!("write {}", path.display()))?;
            }
        }
    }

    Ok(expected)
}

fn glob_frame_files(frame_dir: &Path) -> Result<Vec<PathBuf>> {
    if !frame_dir.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in fs::read_dir(frame_dir).with_context(|| format!("read {}", frame_dir.display()))? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("frame_") && name.ends_with(".png"))
        {
            paths.push(path);
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use image::ImageBuffer;
    use image::Rgba;

    use super::*;

    #[test]
    fn prepare_png_frames_slices_spritesheet_without_external_command() {
        let dir = tempfile::tempdir().unwrap();
        let spritesheet_path = dir.path().join("spritesheet.png");
        let spritesheet: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 255, 0, 255])
            }
        });
        spritesheet.save(&spritesheet_path).unwrap();

        let frames = prepare_png_frames(
            &Pet {
                id: "tiny".to_string(),
                display_name: "Tiny".to_string(),
                description: String::new(),
                spritesheet_path,
                frame_width: 1,
                frame_height: 1,
                columns: 2,
                rows: 1,
                frame_count: 2,
                animations: HashMap::new(),
            },
            &dir.path().join("frames"),
        )
        .unwrap();

        assert_eq!(frames.len(), 2);
        assert!(frames[0].exists());
        assert!(frames[1].exists());
    }
}
