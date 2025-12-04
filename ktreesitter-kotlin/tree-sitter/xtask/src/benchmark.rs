use anyhow::Result;

use crate::{bail_on_err, Benchmark};

pub fn run(args: &Benchmark) -> Result<()> {
    if let Some(ref example) = args.example_file_name {
        std::env::set_var("TREE_SITTER_BENCHMARK_EXAMPLE_FILTER", example);
    }

    if let Some(ref language) = args.language {
        std::env::set_var("TREE_SITTER_BENCHMARK_LANGUAGE_FILTER", language);
    }

    if args.repetition_count != 5 {
        std::env::set_var(
            "TREE_SITTER_BENCHMARK_REPETITION_COUNT",
            args.repetition_count.to_string(),
        );
    }

    if args.debug {
        let output = std::process::Command::new("cargo")
            .arg("bench")
            .arg("benchmark")
            .arg("-p")
            .arg("tree-sitter-cli")
            .arg("--no-run")
            .arg("--message-format=json")
            .spawn()?
            .wait_with_output()?;

        bail_on_err(&output, "Failed to run `cargo bench`")?;

        let json_output = serde_json::from_slice::<serde_json::Value>(&output.stdout)?;

        let test_binary = json_output
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid JSON output"))?
            .iter()
            .find_map(|message| {
                if message
                    .get("target")
                    .and_then(|target| target.get("name"))
                    .and_then(|name| name.as_str())
                    .is_some_and(|name| name == "benchmark")
                    && message
                        .get("executable")
                        .and_then(|executable| executable.as_str())
                        .is_some()
                {
                    message
                        .get("executable")
                        .and_then(|executable| executable.as_str())
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Failed to find benchmark executable"))?;

        println!("{test_binary}");
    } else {
        let status = std::process::Command::new("cargo")
            .arg("bench")
            .arg("benchmark")
            .arg("-p")
            .arg("tree-sitter-cli")
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to run `cargo bench`");
        }
    }

    Ok(())
}
