use anyhow::Result;
use codex_config::CONFIG_TOML_FILE;
use codex_core::plugins::marketplace_install_root;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::thread;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

fn write_marketplace_source(source: &Path, marker: &str) -> Result<()> {
    std::fs::create_dir_all(source.join(".agents/plugins"))?;
    std::fs::create_dir_all(source.join("plugins/sample/.codex-plugin"))?;
    std::fs::write(
        source.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        source.join("plugins/sample/.codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )?;
    std::fs::write(source.join("plugins/sample/marker.txt"), marker)?;
    Ok(())
}

#[tokio::test]
async fn marketplace_add_supports_local_directory_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source(source.path(), "local ref")?;
    let source_parent = source.path().parent().unwrap();
    let source_arg = format!("./{}", source.path().file_name().unwrap().to_string_lossy());

    codex_command(codex_home.path())?
        .current_dir(source_parent)
        .args(["marketplace", "add", source_arg.as_str()])
        .assert()
        .success();

    let installed_root = marketplace_install_root(codex_home.path()).join("debug");
    assert_eq!(
        std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt"))?,
        "local ref"
    );

    let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;
    let config: toml::Value = toml::from_str(&config)?;
    let expected_source = source.path().canonicalize()?.display().to_string();
    assert_eq!(
        config["marketplaces"]["debug"]["source_type"].as_str(),
        Some("path")
    );
    assert_eq!(
        config["marketplaces"]["debug"]["source"].as_str(),
        Some(expected_source.as_str())
    );

    Ok(())
}

fn spawn_manifest_server(body: String) -> Result<(u16, thread::JoinHandle<Result<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok((
        port,
        thread::spawn(move || {
            let (mut stream, _addr) = listener.accept()?;
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request)?;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes())?;
            Ok(())
        }),
    ))
}

#[tokio::test]
async fn marketplace_add_supports_manifest_url_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    std::fs::create_dir_all(source.path().join(".agents/plugins"))?;
    std::fs::write(
        source.path().join(".agents/plugins/marketplace.json"),
        r#"{"name":"debug-url","plugins":[]}"#,
    )?;
    let (port, server) = spawn_manifest_server(r#"{"name":"debug-url","plugins":[]}"#.to_string())?;
    let url = format!("http://127.0.0.1:{port}/.agents/plugins/marketplace.json");

    codex_command(codex_home.path())?
        .args(["marketplace", "add", &url])
        .assert()
        .success();

    let installed_root = marketplace_install_root(codex_home.path()).join("debug-url");
    assert!(
        installed_root
            .join(".agents/plugins/marketplace.json")
            .is_file()
    );

    let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;
    let config: toml::Value = toml::from_str(&config)?;
    assert_eq!(
        config["marketplaces"]["debug-url"]["source_type"].as_str(),
        Some("manifest_url")
    );
    assert_eq!(
        config["marketplaces"]["debug-url"]["source"].as_str(),
        Some(url.as_str())
    );
    server.join().unwrap()?;

    Ok(())
}
