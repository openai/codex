//! Verify fallback among installed layouts while ignoring non-directory entries.

use pretty_assertions::assert_eq;

use super::plugin_directory;

#[test]
fn discovers_available_layout_without_selecting_a_file() -> std::io::Result<()> {
    let root = std::env::temp_dir().join(format!(
        "codex-alsa-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    let multiarch = root.join("multiarch");
    let lib64 = root.join("lib64");
    let generic = root.join("generic");
    let candidates = [
        multiarch.to_str().unwrap(),
        lib64.to_str().unwrap(),
        generic.to_str().unwrap(),
    ];

    assert_eq!(plugin_directory(&candidates), None);
    std::fs::create_dir(&generic)?;
    assert_eq!(plugin_directory(&candidates), Some(candidates[2]));
    std::fs::write(&lib64, "not a directory")?;
    assert_eq!(plugin_directory(&candidates), Some(candidates[2]));
    std::fs::remove_file(&lib64)?;
    std::fs::create_dir(&lib64)?;
    assert_eq!(plugin_directory(&candidates), Some(candidates[1]));
    std::fs::create_dir(&multiarch)?;
    assert_eq!(plugin_directory(&candidates), Some(candidates[0]));
    std::fs::remove_dir_all(root)
}
