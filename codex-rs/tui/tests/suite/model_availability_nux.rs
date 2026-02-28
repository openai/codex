use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use serde_json::Value as JsonValue;
use tempfile::tempdir;
use tokio::select;
use tokio::time::sleep;
use tokio::time::timeout;

#[tokio::test]
async fn resume_startup_does_not_consume_model_availability_nux_count() -> Result<()> {
    // run_codex_cli() does not work on Windows due to PTY limitations.
    if cfg!(windows) {
        return Ok(());
    }

    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let codex_home = tempdir()?;

    let source_catalog_path = codex_utils_cargo_bin::find_resource!("../core/models.json")?;
    let source_catalog = std::fs::read_to_string(&source_catalog_path)?;
    let mut source_catalog: JsonValue = serde_json::from_str(&source_catalog)?;
    let models = source_catalog
        .get_mut("models")
        .and_then(JsonValue::as_array_mut)
        .context("models array missing")?;
    for model in models.iter_mut() {
        if let Some(object) = model.as_object_mut() {
            object.remove("availability_nux");
        }
    }
    let first_model = models.first_mut().context("models array is empty")?;
    let first_model_object = first_model
        .as_object_mut()
        .context("first model was not a JSON object")?;
    let model_slug = first_model_object
        .get("slug")
        .and_then(JsonValue::as_str)
        .context("first model missing slug")?
        .to_string();
    first_model_object.insert(
        "availability_nux".to_string(),
        serde_json::json!({
            "message": "Model now available",
        }),
    );

    let custom_catalog_path = codex_home.path().join("catalog.json");
    std::fs::write(
        &custom_catalog_path,
        serde_json::to_string(&source_catalog)?,
    )?;

    let repo_root_display = repo_root.display();
    let catalog_display = custom_catalog_path.display();
    let config_contents = format!(
        r#"model = "{model_slug}"
model_provider = "openai"
model_catalog_json = "{catalog_display}"

[projects."{repo_root_display}"]
trust_level = "trusted"

[tui.model_availability_nux]
"{model_slug}" = 1
"#
    );
    std::fs::write(codex_home.path().join("config.toml"), config_contents)?;

    let fixture_path =
        codex_utils_cargo_bin::find_resource!("../core/tests/cli_responses_fixture.sse")?;
    let codex = if let Ok(path) = codex_utils_cargo_bin::cargo_bin("codex") {
        path
    } else {
        let fallback = repo_root.join("codex-rs/target/debug/codex");
        if fallback.is_file() {
            fallback
        } else {
            eprintln!("skipping integration test because codex binary is unavailable");
            return Ok(());
        }
    };

    let exec_output = std::process::Command::new(&codex)
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg("seed session for resume")
        .env("CODEX_HOME", codex_home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("CODEX_RS_SSE_FIXTURE", fixture_path)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .output()
        .context("failed to execute codex exec")?;
    anyhow::ensure!(
        exec_output.status.success(),
        "codex exec failed: {}",
        String::from_utf8_lossy(&exec_output.stderr)
    );

    let mut env = HashMap::new();
    env.insert(
        "CODEX_HOME".to_string(),
        codex_home.path().display().to_string(),
    );
    env.insert("OPENAI_API_KEY".to_string(), "dummy".to_string());

    let args = vec![
        "resume".to_string(),
        "--last".to_string(),
        "--no-alt-screen".to_string(),
        "-C".to_string(),
        repo_root.display().to_string(),
        "-c".to_string(),
        "analytics.enabled=false".to_string(),
    ];

    let spawned = codex_utils_pty::spawn_pty_process(
        codex.to_string_lossy().as_ref(),
        &args,
        &repo_root,
        &env,
        &None,
    )
    .await?;

    let mut output = Vec::new();
    let mut output_rx = spawned.output_rx;
    let mut exit_rx = spawned.exit_rx;
    let writer_tx = spawned.session.writer_sender();
    let interrupt_writer = writer_tx.clone();
    let interrupt_task = tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;
        for _ in 0..4 {
            let _ = interrupt_writer.send(vec![3]).await;
            sleep(Duration::from_millis(500)).await;
        }
    });

    let exit_code_result = timeout(Duration::from_secs(15), async {
        loop {
            select! {
                result = output_rx.recv() => match result {
                    Ok(chunk) => {
                        if chunk.windows(4).any(|window| window == b"\x1b[6n") {
                            let _ = writer_tx.send(b"\x1b[1;1R".to_vec()).await;
                        }
                        output.extend_from_slice(&chunk);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break exit_rx.await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                },
                result = &mut exit_rx => break result,
            }
        }
    })
    .await;

    interrupt_task.abort();

    let exit_code = match exit_code_result {
        Ok(Ok(code)) => code,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => {
            spawned.session.terminate();
            anyhow::bail!("timed out waiting for codex resume to exit");
        }
    };
    anyhow::ensure!(
        exit_code == 0 || exit_code == 130,
        "unexpected exit code from codex resume: {exit_code}; output: {}",
        String::from_utf8_lossy(&output)
    );

    let config_contents = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let config: toml::Value = toml::from_str(&config_contents)?;
    let shown_count = config
        .get("tui")
        .and_then(|tui| tui.get("model_availability_nux"))
        .and_then(|nux| nux.get(&model_slug))
        .and_then(toml::Value::as_integer)
        .context("missing tui.model_availability_nux count")?;

    assert_eq!(shown_count, 1);

    Ok(())
}
