use std::{path::Path, process::Command};

use anyhow::Result;

use crate::{bail_on_err, EMSCRIPTEN_VERSION};

pub fn run_fixtures() -> Result<()> {
    let grammars_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test")
        .join("fixtures")
        .join("grammars");

    [
        ("bash", "master"),
        ("c", "master"),
        ("cpp", "master"),
        ("embedded-template", "master"),
        ("go", "master"),
        ("html", "master"),
        ("java", "master"),
        ("javascript", "master"),
        ("jsdoc", "master"),
        ("json", "master"),
        ("php", "master"),
        ("python", "master"),
        ("ruby", "master"),
        ("rust", "master"),
        ("typescript", "master"),
    ]
    .iter()
    .try_for_each(|(grammar, r#ref)| {
        let grammar_dir = grammars_dir.join(grammar);
        let grammar_url = format!("https://github.com/tree-sitter/tree-sitter-{grammar}");

        println!("Updating the {grammar} grammar...");

        if !grammar_dir.exists() {
            let mut command = Command::new("git");
            command.args([
                "clone",
                "--depth",
                "1",
                &grammar_url,
                &grammar_dir.to_string_lossy(),
            ]);
            bail_on_err(
                &command.spawn()?.wait_with_output()?,
                &format!("Failed to clone the {grammar} grammar"),
            )?;
        }

        std::env::set_current_dir(&grammar_dir)?;

        let mut command = Command::new("git");
        command.args(["fetch", "origin", r#ref, "--depth", "1"]);
        bail_on_err(
            &command.spawn()?.wait_with_output()?,
            &format!("Failed to fetch the {grammar} grammar"),
        )?;

        let mut command = Command::new("git");
        command.args(["reset", "--hard", "FETCH_HEAD"]);
        bail_on_err(
            &command.spawn()?.wait_with_output()?,
            &format!("Failed to reset the {grammar} grammar"),
        )?;

        Ok(())
    })
}

pub fn run_emscripten() -> Result<()> {
    let emscripten_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("target")
        .join("emsdk");

    if emscripten_dir.exists() {
        println!("Emscripten SDK already exists");
        return Ok(());
    }
    println!("Cloning the Emscripten SDK...");

    let mut command = Command::new("git");
    command.args([
        "clone",
        "https://github.com/emscripten-core/emsdk.git",
        &emscripten_dir.to_string_lossy(),
    ]);
    bail_on_err(
        &command.spawn()?.wait_with_output()?,
        "Failed to clone the Emscripten SDK",
    )?;

    std::env::set_current_dir(&emscripten_dir)?;

    let emsdk = if cfg!(windows) {
        "emsdk.bat"
    } else {
        "./emsdk"
    };

    let mut command = Command::new(emsdk);
    command.args(["install", EMSCRIPTEN_VERSION]);
    bail_on_err(
        &command.spawn()?.wait_with_output()?,
        "Failed to install Emscripten",
    )?;

    let mut command = Command::new(emsdk);
    command.args(["activate", EMSCRIPTEN_VERSION]);
    bail_on_err(
        &command.spawn()?.wait_with_output()?,
        "Failed to activate Emscripten",
    )
}
