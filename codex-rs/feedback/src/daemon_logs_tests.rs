use super::*;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;

#[test]
fn collects_bounded_tails_and_skips_missing_empty_and_nonregular_files() -> io::Result<()> {
    let home = std::env::temp_dir().join(format!("codex-feedback-daemon-{}", ThreadId::new()));
    let directory = home.join("app-server-daemon");
    assert!(daemon_log_attachments(&home).is_empty());
    fs::create_dir_all(&directory)?;
    let bytes = vec![b'x'; MAX_LOG_BYTES as usize + 100];
    fs::write(directory.join("daemon.stderr.log"), b"already included")?;
    fs::write(directory.join("daemon-updater.stderr.log"), &bytes)?;
    fs::write(
        directory.join("daemon.stderr.log.previous"),
        b"failed restart",
    )?;
    fs::write(
        directory.join("app-server-updater.stderr.log"),
        b"legacy updater",
    )?;
    fs::write(
        directory.join("app-server-updater.stderr.log.previous"),
        b"previous updater",
    )?;
    fs::create_dir(directory.join("app-server.stderr.log.previous"))?;
    let attachments = daemon_log_attachments(&home);
    let actual: Vec<_> = attachments
        .into_iter()
        .map(|attachment| {
            (
                attachment.filename,
                attachment.content_type,
                attachment.buffer,
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            (
                "daemon-updater.stderr.log".to_string(),
                Some("text/plain".to_string()),
                bytes[100..].to_vec()
            ),
            (
                "app-server-updater.stderr.log.previous".to_string(),
                Some("text/plain".to_string()),
                b"previous updater".to_vec()
            ),
        ]
    );
    fs::remove_dir_all(home)?;
    Ok(())
}
