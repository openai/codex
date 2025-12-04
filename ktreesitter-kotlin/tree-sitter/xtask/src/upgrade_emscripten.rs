use std::{fs, path::Path};

use anyhow::{anyhow, Result};
use git2::Repository;
use serde_json::Value;

use crate::create_commit;

pub fn run() -> Result<()> {
    let response = ureq::get("https://api.github.com/repos/emscripten-core/emsdk/tags")
        .call()?
        .body_mut()
        .read_to_string()?;

    let json = serde_json::from_str::<Value>(&response)?;
    let version = json
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|tag| tag["name"].as_str())
        .ok_or(anyhow!("No tags found"))?;

    let version_file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cli")
        .join("loader")
        .join("emscripten-version");

    fs::write(version_file, version)?;

    println!("Upgraded emscripten version to {version}");

    let repo = Repository::open(".")?;
    create_commit(
        &repo,
        &format!("build(deps): bump emscripten to {version}"),
        &["cli/loader/emscripten-version"],
    )?;

    Ok(())
}
