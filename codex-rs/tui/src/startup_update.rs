use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

const WRAPPER_REPORTED_UPDATE_STATUS_ENV: &str = "CODEX_WRAPPER_REPORTED_UPDATE_STATUS";
const AUTO_UPDATE_ENV: &str = "CODEX_AUTO_UPDATE";
const UPDATE_TOOL_ENV: &str = "CODEX_UPDATE_TOOL";
const UPDATE_BASE_URL_ENV: &str = "CODEX_UPDATE_BASE_URL";
const UPDATE_FILENAME_PREFIX_ENV: &str = "CODEX_UPDATE_FILENAME_PREFIX";
const UPDATE_TIMEOUT_SECS_ENV: &str = "CODEX_UPDATE_TIMEOUT_SECS";

const DEFAULT_UPDATE_TOOL: &str = "bbb";
const DEFAULT_UPDATE_BASE_URL: &str = "az://oaiphx8/oaikhai/codex/";
const DEFAULT_UPDATE_FILENAME_PREFIX: &str = "codex-tui-";
const DEFAULT_UPDATE_TIMEOUT_SECS: i64 = 15;

#[derive(Clone, Debug)]
struct UpdaterConfig {
    enabled: bool,
    tool: String,
    base_url: String,
    filename_prefix: String,
    timeout: Duration,
}

impl UpdaterConfig {
    fn from_env() -> Self {
        let enabled = match std::env::var(AUTO_UPDATE_ENV) {
            Ok(v) => v != "0",
            Err(_) => true,
        };
        let tool =
            std::env::var(UPDATE_TOOL_ENV).unwrap_or_else(|_| DEFAULT_UPDATE_TOOL.to_owned());
        let base_url = std::env::var(UPDATE_BASE_URL_ENV)
            .unwrap_or_else(|_| DEFAULT_UPDATE_BASE_URL.to_owned());
        let filename_prefix = std::env::var(UPDATE_FILENAME_PREFIX_ENV)
            .unwrap_or_else(|_| DEFAULT_UPDATE_FILENAME_PREFIX.to_owned());
        let timeout_secs = std::env::var(UPDATE_TIMEOUT_SECS_ENV)
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(DEFAULT_UPDATE_TIMEOUT_SECS)
            .max(1);
        let timeout = Duration::from_secs(timeout_secs as u64);

        Self {
            enabled,
            tool,
            base_url,
            filename_prefix,
            timeout,
        }
    }
}

#[derive(Clone, Debug)]
struct RemoteCandidate {
    remote_path: String,
    name: String,
    version: String,
    is_sha: bool,
}

#[allow(clippy::print_stderr)]
pub(crate) fn maybe_emit_update_status() {
    if std::env::var_os(WRAPPER_REPORTED_UPDATE_STATUS_ENV).is_some() {
        return;
    }

    let config = UpdaterConfig::from_env();
    let Some(target_triple) = target_triple() else {
        eprintln!("[codex] update skipped (unsupported platform)");
        return;
    };

    let exe = std::env::current_exe().ok();
    let version = env!("CARGO_PKG_VERSION");

    let local_binary_name = if cfg!(windows) {
        "codex-tui.exe"
    } else {
        "codex-tui"
    };

    let cache_dir = match cache_dir(target_triple) {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("[codex] update skipped ({err})");
            return;
        }
    };

    let local_binary_path = cache_dir.join(local_binary_name);
    let local_version_path = cache_dir.join(format!("{local_binary_name}.version"));
    let local_sha_path = cache_dir.join(format!("{local_binary_name}.sha256"));
    let local_version = read_trimmed(&local_version_path);
    let local_sha = read_trimmed(&local_sha_path);

    let mut banner = vec![
        format!("codex-tui v{version}"),
        format!("target={target_triple}"),
    ];
    if let Some(installed) = local_version.as_deref() {
        banner.push(format!("installed={installed}"));
    }
    if let Some(exe) = exe.as_ref() {
        banner.push(format!("exe={}", exe.display()));
    }
    eprintln!("[codex] {}", banner.join(" "));

    if !config.enabled {
        eprintln!("[codex] update disabled ({AUTO_UPDATE_ENV}=0)");
        return;
    }

    eprintln!(
        "[codex] checking for update ({})...",
        redact_query(&config.base_url)
    );

    let remote_paths = match list_remote_paths(&config) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("[codex] update skipped ({err})");
            return;
        }
    };

    let Some(latest) = select_latest_candidate(&remote_paths, config.filename_prefix.as_str())
    else {
        eprintln!(
            "[codex] update skipped (no remote matches for {}YYYY-MM-DD)",
            config.filename_prefix
        );
        return;
    };
    let latest_version = latest.version.as_str();

    let remote_sha = match find_remote_sha(&config, &remote_paths, &latest) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("[codex] update warning (failed to read remote sha256: {err})");
            None
        }
    };

    if let Some(local_version_value) = local_version.as_deref()
        && local_version_value >= latest_version
        && local_binary_path.exists()
        && (remote_sha.is_none() || local_sha.as_deref() == remote_sha.as_deref())
    {
        let suffix = local_sha
            .as_deref()
            .map(|sha| format!(" {}", &sha[..sha.len().min(12)]))
            .unwrap_or_default();
        eprintln!("[codex] update up-to-date ({local_version_value}{suffix})");
        return;
    }

    if let Err(err) = std::fs::create_dir_all(&cache_dir) {
        eprintln!("[codex] update failed (create cache dir: {err})");
        return;
    }

    eprintln!("[codex] update downloading {latest_version}...");

    let tmp_path = cache_dir.join(format!("{local_binary_name}.tmp"));
    let tmp_path_str = tmp_path.to_string_lossy().to_string();
    let download_args = ["cp", latest.remote_path.as_str(), tmp_path_str.as_str()];
    match run_tool(&config.tool, &download_args) {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            eprintln!(
                "[codex] update failed (download {latest_version}: {})",
                first_line(&tool_output_text(&output))
            );
            return;
        }
        Err(err) => {
            eprintln!("[codex] update failed (download {latest_version}: {err})");
            return;
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if let Ok(metadata) = std::fs::metadata(&tmp_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&tmp_path, perms);
        }
    }

    if let Err(err) = atomic_replace(&tmp_path, &local_binary_path) {
        eprintln!("[codex] update failed (install: {err})");
        return;
    }

    let previous = local_version.as_deref().unwrap_or("none");
    if let Err(err) = write_text_file(&local_version_path, format!("{latest_version}\n")) {
        eprintln!("[codex] update warning (failed to persist version: {err})");
    }
    if let Some(sha) = remote_sha.as_deref()
        && let Err(err) = write_text_file(&local_sha_path, format!("{sha}\n"))
    {
        eprintln!("[codex] update warning (failed to persist sha256: {err})");
    }

    if previous == "none" {
        eprintln!("[codex] update installed {latest_version}");
    } else if previous == latest_version {
        eprintln!("[codex] update reinstalled {latest_version}");
    } else {
        eprintln!("[codex] update {previous} -> {latest_version}");
    }
}

fn cache_dir(target_triple: &str) -> anyhow::Result<PathBuf> {
    if cfg!(windows) {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(dirs::home_dir)
            .ok_or_else(|| anyhow::anyhow!("no home dir"))?;
        return Ok(base.join("codex").join("bin").join(target_triple));
    }

    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".cache")))
        .ok_or_else(|| anyhow::anyhow!("no home dir"))?;
    Ok(base.join("codex").join("bin").join(target_triple))
}

fn target_triple() -> Option<&'static str> {
    if cfg!(target_os = "linux") || cfg!(target_os = "android") {
        if cfg!(target_arch = "x86_64") {
            Some("x86_64-unknown-linux-musl")
        } else if cfg!(target_arch = "aarch64") {
            Some("aarch64-unknown-linux-musl")
        } else {
            None
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "x86_64") {
            Some("x86_64-apple-darwin")
        } else if cfg!(target_arch = "aarch64") {
            Some("aarch64-apple-darwin")
        } else {
            None
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "x86_64") {
            Some("x86_64-pc-windows-msvc")
        } else if cfg!(target_arch = "aarch64") {
            Some("aarch64-pc-windows-msvc")
        } else {
            None
        }
    } else {
        None
    }
}

fn list_remote_paths(config: &UpdaterConfig) -> anyhow::Result<Vec<String>> {
    let base = ensure_trailing_slash(&config.base_url);
    let output = run_tool_with_timeout(
        &config.tool,
        &["ll", "--machine", base.as_str()],
        config.timeout,
    )?;
    if !output.status.success() {
        let output_text = tool_output_text(&output);
        let summary = first_line(&output_text);
        if should_suggest_azure_login(config, output_text.as_str()) {
            anyhow::bail!("list failed: {summary} (try `az login`)");
        }
        anyhow::bail!("list failed: {summary}");
    }
    Ok(parse_machine_ls(tool_output_text(&output).as_str()))
}

fn should_suggest_azure_login(config: &UpdaterConfig, output_text: &str) -> bool {
    if !config.base_url.starts_with("az://") && !config.base_url.contains("blob.core.windows.net") {
        return false;
    }

    let output_lower = output_text.to_ascii_lowercase();
    output_lower.contains("requesting an access token")
        || output_lower.contains("azureclicredential")
        || output_lower.contains("az login")
}

fn select_latest_candidate(remote_paths: &[String], prefix: &str) -> Option<RemoteCandidate> {
    remote_paths
        .iter()
        .filter_map(|path| parse_candidate(path.as_str(), prefix))
        .filter(|c| !c.is_sha)
        .filter(|c| is_runnable_candidate(c.name.as_str()))
        .max_by(|a, b| {
            a.version.cmp(&b.version).then_with(|| {
                candidate_score(a.name.as_str()).cmp(&candidate_score(b.name.as_str()))
            })
        })
}

fn find_remote_sha(
    config: &UpdaterConfig,
    remote_paths: &[String],
    latest: &RemoteCandidate,
) -> anyhow::Result<Option<String>> {
    let sha_path = remote_paths
        .iter()
        .filter_map(|path| parse_candidate(path.as_str(), config.filename_prefix.as_str()))
        .find_map(|c| {
            if c.is_sha && c.version == latest.version {
                Some(c.remote_path)
            } else {
                None
            }
        });

    let Some(sha_path) = sha_path else {
        return Ok(None);
    };

    let sha_out = run_tool_with_timeout(&config.tool, &["cat", sha_path.as_str()], config.timeout)?;
    if !sha_out.status.success() {
        anyhow::bail!("cat failed: {}", first_line(&tool_output_text(&sha_out)));
    }
    Ok(first_token(tool_output_text(&sha_out).as_str()).map(str::to_owned))
}

fn run_tool(tool: &str, args: &[&str]) -> std::io::Result<Output> {
    Command::new(tool).args(args).output()
}

fn run_tool_with_timeout(tool: &str, args: &[&str], timeout: Duration) -> std::io::Result<Output> {
    let mut child = Command::new(tool)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout_thread = child
        .stdout
        .take()
        .map(spawn_reader_thread)
        .unwrap_or_else(|| std::thread::spawn(|| Ok(Vec::new())));
    let stderr_thread = child
        .stderr
        .take()
        .map(spawn_reader_thread)
        .unwrap_or_else(|| std::thread::spawn(|| Ok(Vec::new())));

    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            let stdout = stdout_thread.join().unwrap_or_else(|_| Ok(Vec::new()))?;
            let stderr = stderr_thread.join().unwrap_or_else(|_| Ok(Vec::new()))?;
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            std::thread::spawn(move || {
                let _ = child.wait();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
            });
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("{tool} timed out after {}s", timeout.as_secs()),
            ));
        }

        std::thread::sleep(Duration::from_millis(25));
    }
}

fn spawn_reader_thread<R>(mut reader: R) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>>
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut buf)?;
        Ok(buf)
    })
}

fn tool_output_text(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = stdout.trim();
    let stderr = stderr.trim();
    if stderr.is_empty() {
        stdout.to_owned()
    } else if stdout.is_empty() {
        stderr.to_owned()
    } else {
        format!("{stderr}\n{stdout}")
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().trim().to_owned()
}

fn first_token(s: &str) -> Option<&str> {
    s.split_whitespace().next()
}

fn ensure_trailing_slash(url: &str) -> String {
    if url.ends_with('/') {
        url.to_owned()
    } else {
        format!("{url}/")
    }
}

fn redact_query(url: &str) -> String {
    url.split_once('?')
        .map_or_else(|| url.to_owned(), |(b, _)| b.to_owned())
}

fn parse_machine_ls(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            // Example:
            //            3  2025-12-28T16:48:04  az://oaiphx8/oaikhai/codex/codex-tui-2025-12-28
            let mut parts = line.split_whitespace();
            let _bytes = parts.next()?;
            let _mtime = parts.next()?;
            parts.next().map(ToOwned::to_owned)
        })
        .collect()
}

fn parse_candidate(remote_path: &str, prefix: &str) -> Option<RemoteCandidate> {
    let name = remote_path.rsplit('/').next()?.to_owned();
    if !name.starts_with(prefix) {
        return None;
    }

    let after_prefix = &name[prefix.len()..];
    let version = extract_iso_date(after_prefix)?;
    let is_sha = after_prefix[version.len()..].starts_with(".sha256");
    Some(RemoteCandidate {
        remote_path: remote_path.to_owned(),
        name,
        version,
        is_sha,
    })
}

fn extract_iso_date(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    Some(s[..10].to_owned())
}

fn is_runnable_candidate(name: &str) -> bool {
    if cfg!(windows) {
        name.ends_with(".exe")
    } else {
        !name.ends_with(".tar.gz")
            && !name.ends_with(".zip")
            && !name.ends_with(".zst")
            && !name.ends_with(".gz")
            && !name.ends_with(".tar")
            && !name.ends_with(".sha256")
    }
}

fn candidate_score(name: &str) -> i64 {
    if cfg!(windows) && name.ends_with(".exe") {
        return 3;
    }
    if name.ends_with(".tar.gz") || name.ends_with(".zip") || name.ends_with(".zst") {
        return 0;
    }
    2
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn write_text_file(path: &Path, content: String) -> std::io::Result<()> {
    std::fs::write(path, content)
}

fn atomic_replace(src: &Path, dst: &Path) -> std::io::Result<()> {
    if dst.exists() {
        let _ = std::fs::remove_file(dst);
    }
    std::fs::rename(src, dst)
}
