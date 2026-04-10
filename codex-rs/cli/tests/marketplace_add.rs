use anyhow::Context;
use anyhow::Result;
use codex_core::plugins::marketplace_install_root;
use codex_core::plugins::validate_marketplace_root;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

fn write_marketplace_source(source: &Path, marker: &str) -> Result<()> {
    write_marketplace_source_with_name(source, "debug", marker)
}

fn write_marketplace_source_with_name(
    source: &Path,
    marketplace_name: &str,
    marker: &str,
) -> Result<()> {
    std::fs::create_dir_all(source.join(".agents/plugins"))?;
    std::fs::create_dir_all(source.join("plugins/sample/.codex-plugin"))?;
    std::fs::write(
        source.join(".agents/plugins/marketplace.json"),
        format!(
            r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{
      "name": "sample",
      "source": {{
        "source": "local",
        "path": "./plugins/sample"
      }}
    }}
  ]
}}"#
        ),
    )?;
    std::fs::write(
        source.join("plugins/sample/.codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )?;
    std::fs::write(source.join("plugins/sample/marker.txt"), marker)?;
    Ok(())
}

struct RemoteGitMarketplace {
    _server: HttpFileServer,
    repo_path: PathBuf,
    url: String,
}

struct HttpFileServer {
    _root: TempDir,
    address: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

fn write_git_marketplace_source(source: &Path, marker: &str) -> Result<RemoteGitMarketplace> {
    write_marketplace_source(source, marker)?;
    init_git_repo(source)?;
    RemoteGitMarketplace::from_source(source)
}

fn write_git_marketplace_source_with_name(
    source: &Path,
    marketplace_name: &str,
    marker: &str,
) -> Result<RemoteGitMarketplace> {
    write_marketplace_source_with_name(source, marketplace_name, marker)?;
    init_git_repo(source)?;
    RemoteGitMarketplace::from_source(source)
}

fn init_git_repo(source: &Path) -> Result<()> {
    run_git(source, ["init"])?;
    run_git(source, ["config", "user.email", "codex@example.com"])?;
    run_git(source, ["config", "user.name", "Codex Test"])?;
    commit_git_repo(source, "initial marketplace")?;
    Ok(())
}

fn commit_git_repo(source: &Path, message: &str) -> Result<()> {
    run_git(source, ["add", "."])?;
    run_git(source, ["commit", "-m", message])?;
    Ok(())
}

fn run_git<I, S>(cwd: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect::<Vec<OsString>>();
    let output = Command::new("git").current_dir(cwd).args(&args).output()?;
    let args_display = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        output.status.success(),
        "git {} failed\nstdout:\n{}\nstderr:\n{}",
        args_display,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

impl RemoteGitMarketplace {
    fn from_source(source: &Path) -> Result<Self> {
        let root = TempDir::new()?;
        let repo_name = "marketplace.git";
        let repo_path = root.path().join(repo_name);
        clone_bare_repo(source, root.path(), repo_name)?;
        let server = HttpFileServer::start(root)?;
        let url = format!("http://{}/{repo_name}", server.address);
        Ok(Self {
            _server: server,
            repo_path,
            url,
        })
    }

    fn refresh_from_source(&self, source: &Path) -> Result<()> {
        std::fs::remove_dir_all(&self.repo_path)?;
        let parent = self
            .repo_path
            .parent()
            .context("served git repository should have a parent")?;
        let repo_name = self
            .repo_path
            .file_name()
            .context("served git repository should have a file name")?
            .to_string_lossy()
            .to_string();
        clone_bare_repo(source, parent, &repo_name)
    }
}

fn clone_bare_repo(source: &Path, target_parent: &Path, repo_name: &str) -> Result<()> {
    run_git(
        target_parent,
        [
            OsString::from("clone"),
            OsString::from("--bare"),
            source.as_os_str().to_os_string(),
            OsString::from(repo_name),
        ],
    )?;
    run_git(
        target_parent.join(repo_name).as_path(),
        ["update-server-info"],
    )?;
    Ok(())
}

impl HttpFileServer {
    fn start(root: TempDir) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let root_path = root.path().to_path_buf();
        let handle = thread::spawn(move || {
            while !thread_shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = serve_http_file(&root_path, &mut stream);
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            _root: root,
            address,
            shutdown,
            handle: Some(handle),
        })
    }
}

impl Drop for HttpFileServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn serve_http_file(root: &Path, stream: &mut TcpStream) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..bytes_read]);
        if request.len() > 16 * 1024 {
            write_http_response(stream, "413 Payload Too Large", &[])?;
            return Ok(());
        }
    }

    let request = String::from_utf8_lossy(&request);
    let Some(request_line) = request.lines().next() else {
        write_http_response(stream, "400 Bad Request", &[])?;
        return Ok(());
    };
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    if method != "GET" && method != "HEAD" {
        write_http_response(stream, "405 Method Not Allowed", &[])?;
        return Ok(());
    }

    let Some(relative_path) = safe_http_path(target) else {
        write_http_response(stream, "400 Bad Request", &[])?;
        return Ok(());
    };
    let path = root.join(relative_path);
    if !path.is_file() {
        write_http_response(stream, "404 Not Found", &[])?;
        return Ok(());
    }

    let body = std::fs::read(path)?;
    if method == "HEAD" {
        write_http_response(stream, "200 OK", &[])?;
    } else {
        write_http_response(stream, "200 OK", &body)?;
    }
    Ok(())
}

fn safe_http_path(target: &str) -> Option<PathBuf> {
    let path = target.split('?').next()?.trim_start_matches('/');
    let path = percent_decode(path)?;
    let relative_path = PathBuf::from(path);
    if relative_path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(relative_path)
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            output.push(hex_value(high)? * 16 + hex_value(low)?);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn write_http_response(stream: &mut TcpStream, status: &str, body: &[u8]) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

#[tokio::test]
async fn marketplace_add_git_source_installs_valid_marketplace_root() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    let source = write_git_marketplace_source(source.path(), "first install")?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args(["marketplace", "add", &source.url])
        .assert()
        .success()
        .stdout(contains("Added marketplace `debug`"));

    let installed_root = marketplace_install_root(codex_home.path()).join("debug");
    assert_eq!(validate_marketplace_root(&installed_root)?, "debug");
    assert_git_marketplace_config(codex_home.path(), "debug", &source.url)?;
    assert!(
        installed_root
            .join("plugins/sample/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(!installed_root.join(".codex-marketplace-source").exists());
    assert!(
        !codex_home
            .path()
            .join(".tmp/known_marketplaces.json")
            .exists()
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_add_rejects_invalid_marketplace_name() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    let source = write_git_marketplace_source_with_name(
        source.path(),
        "debug.market",
        "invalid marketplace",
    )?;

    codex_command(codex_home.path())?
        .args(["marketplace", "add", &source.url])
        .assert()
        .failure()
        .stderr(contains(
            "invalid marketplace name: only ASCII letters, digits, `_`, and `-` are allowed",
        ));

    assert!(
        !marketplace_install_root(codex_home.path())
            .join("debug.market")
            .exists()
    );
    assert!(
        !codex_home
            .path()
            .join(".tmp/known_marketplaces.json")
            .exists()
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_add_same_source_is_idempotent() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    let served_source = write_git_marketplace_source(source.path(), "first install")?;

    codex_command(codex_home.path())?
        .args(["marketplace", "add", &served_source.url])
        .assert()
        .success()
        .stdout(contains("Added marketplace `debug`"));

    std::fs::write(
        source.path().join("plugins/sample/marker.txt"),
        "source changed after add",
    )?;
    commit_git_repo(source.path(), "update marketplace")?;
    served_source.refresh_from_source(source.path())?;

    codex_command(codex_home.path())?
        .args(["marketplace", "add", &served_source.url])
        .assert()
        .success()
        .stdout(contains("Marketplace `debug` is already added"));

    let installed_root = marketplace_install_root(codex_home.path()).join("debug");
    assert_eq!(
        std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt"))?,
        "first install"
    );
    assert_git_marketplace_config(codex_home.path(), "debug", &served_source.url)?;
    assert!(!installed_root.join(".codex-marketplace-source").exists());
    assert!(
        !codex_home
            .path()
            .join(".tmp/known_marketplaces.json")
            .exists()
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_add_rejects_same_name_from_different_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let first_source = TempDir::new()?;
    let second_source = TempDir::new()?;
    let first_source = write_git_marketplace_source(first_source.path(), "first install")?;
    let second_source = write_git_marketplace_source(second_source.path(), "replacement install")?;

    codex_command(codex_home.path())?
        .args(["marketplace", "add", &first_source.url])
        .assert()
        .success()
        .stdout(contains("Added marketplace `debug`"));

    codex_command(codex_home.path())?
        .args(["marketplace", "add", &second_source.url])
        .assert()
        .failure()
        .stderr(contains(
            "marketplace `debug` is already added from a different source",
        ));

    let installed_root = marketplace_install_root(codex_home.path()).join("debug");
    assert_eq!(
        std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt"))?,
        "first install"
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_add_rolls_back_install_when_config_write_fails() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    let source = write_git_marketplace_source(source.path(), "rollback")?;

    let config_path = codex_home.path().join("config.toml");
    std::fs::write(&config_path, "")?;
    let original_permissions = std::fs::metadata(&config_path)?.permissions();
    let mut read_only_permissions = original_permissions.clone();
    read_only_permissions.set_readonly(true);
    std::fs::set_permissions(&config_path, read_only_permissions)?;

    let output = codex_command(codex_home.path())?
        .args(["marketplace", "add", &source.url])
        .output()?;

    std::fs::set_permissions(&config_path, original_permissions)?;

    assert!(
        !output.status.success(),
        "command unexpectedly succeeded: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("failed to add marketplace `debug` to user config.toml"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !marketplace_install_root(codex_home.path())
            .join("debug")
            .exists(),
        "installed marketplace should be rolled back when config.toml cannot be persisted"
    );

    Ok(())
}

fn assert_git_marketplace_config(
    codex_home: &Path,
    marketplace_name: &str,
    source_url: &str,
) -> Result<()> {
    let config = std::fs::read_to_string(codex_home.join("config.toml"))?;
    let config: toml::Value = toml::from_str(&config)?;
    let marketplace = config
        .get("marketplaces")
        .and_then(|marketplaces| marketplaces.get(marketplace_name))
        .context("marketplace config should be written")?;

    assert!(
        marketplace
            .get("last_updated")
            .and_then(toml::Value::as_str)
            .is_some_and(|last_updated| {
                last_updated.len() == "2026-04-10T12:34:56Z".len() && last_updated.ends_with('Z')
            }),
        "last_updated should be an RFC3339-like UTC timestamp"
    );
    assert_eq!(
        marketplace.get("source_type").and_then(toml::Value::as_str),
        Some("git")
    );
    assert_eq!(
        marketplace.get("source").and_then(toml::Value::as_str),
        Some(source_url)
    );
    assert_eq!(marketplace.get("ref").and_then(toml::Value::as_str), None);
    assert!(marketplace.get("sparse_paths").is_none());
    assert!(marketplace.get("source_id").is_none());
    assert!(marketplace.get("install_root").is_none());
    assert!(marketplace.get("install_location").is_none());

    Ok(())
}

#[tokio::test]
async fn marketplace_add_rejects_local_directory_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source(source.path(), "local ref")?;

    codex_command(codex_home.path())?
        .args(["marketplace", "add", source.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains(
            "local marketplace sources are not supported yet; use an HTTP(S) Git URL, SSH Git URL, or GitHub owner/repo",
        ));

    assert!(
        !marketplace_install_root(codex_home.path())
            .join("debug")
            .exists()
    );

    Ok(())
}
