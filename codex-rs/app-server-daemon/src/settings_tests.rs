use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::DaemonSettings;
use super::MAX_SHUTDOWN_GRACE_SECONDS;

#[tokio::test]
async fn remote_control_save_preserves_updater_settings() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("settings.json");
    tokio::fs::write(&path, r#"{"remoteControlEnabled":true}"#)
        .await
        .expect("write legacy settings");

    let settings = DaemonSettings::load(&path).await.expect("load settings");
    assert_eq!(
        settings,
        DaemonSettings {
            remote_control_enabled: true,
            ..DaemonSettings::default()
        }
    );

    tokio::fs::write(
        &path,
        r#"{"remoteControlEnabled":true,"shutdownGraceSeconds":25,"updater":{"autoUpdateEnabled":false,"updateIntervalMinutes":17},"futureSetting":42}"#,
    )
    .await
    .expect("write settings");
    let updated = DaemonSettings {
        remote_control_enabled: false,
        ..DaemonSettings::load(&path)
            .await
            .expect("load updater settings")
    };
    updated.save(&path).await.expect("save remote control");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &tokio::fs::read(&path).await.expect("read settings")
        )
        .expect("parse settings"),
        serde_json::json!({
            "remoteControlEnabled": false,
            "shutdownGraceSeconds": 25,
            "updater": {"autoUpdateEnabled": false, "updateIntervalMinutes": 17},
            "futureSetting": 42,
        })
    );
}

#[tokio::test]
async fn shutdown_grace_accepts_zero_through_five_minutes() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("settings.json");
    assert_eq!(
        DaemonSettings::load(&path).await.expect("missing settings"),
        DaemonSettings::default()
    );
    assert_eq!(
        DaemonSettings::load_for_stop(&path)
            .await
            .shutdown_grace_seconds,
        60
    );

    for seconds in [
        0,
        MAX_SHUTDOWN_GRACE_SECONDS,
        MAX_SHUTDOWN_GRACE_SECONDS + 1,
    ] {
        tokio::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"shutdownGraceSeconds": seconds}))
                .expect("serialize settings"),
        )
        .await
        .expect("write settings");
        let expected = (seconds <= MAX_SHUTDOWN_GRACE_SECONDS).then_some(seconds);
        assert_eq!(
            DaemonSettings::load(&path)
                .await
                .ok()
                .map(|settings| settings.shutdown_grace_seconds),
            expected
        );
        assert_eq!(
            DaemonSettings::load_for_stop(&path)
                .await
                .shutdown_grace_seconds,
            expected.unwrap_or(60)
        );
    }

    tokio::fs::write(&path, r#"{"shutdownGraceSeconds":"unlimited"}"#)
        .await
        .expect("write invalid settings");
    assert!(DaemonSettings::load(&path).await.is_err());
    assert_eq!(
        DaemonSettings::load_for_stop(&path)
            .await
            .shutdown_grace_seconds,
        60
    );
}

#[tokio::test]
async fn update_interval_accepts_long_values_and_rejects_zero() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("settings.json");
    for minutes in [u32::MAX, 0] {
        tokio::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "updater": {"updateIntervalMinutes": minutes},
            }))
            .expect("serialize invalid settings"),
        )
        .await
        .expect("write invalid settings");
        let loaded = DaemonSettings::load(&path).await;
        if minutes == 0 {
            assert!(loaded.is_err());
        } else {
            assert_eq!(
                loaded.expect("load long interval").update_interval_minutes,
                minutes
            );
        }
    }
}

#[tokio::test]
async fn telemetry_distinguishes_presence_from_default_values() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let dir = home.path().join("app-server-daemon");
    tokio::fs::create_dir(&dir).await?;
    let path = dir.join("settings.json");
    for (contents, presence) in [
        ("{}", "default"),
        (
            r#"{"updater":{"autoUpdateEnabled":true,"updateIntervalMinutes":60},"shutdownGraceSeconds":60}"#,
            "configured",
        ),
    ] {
        tokio::fs::write(&path, contents).await?;
        assert_eq!(
            crate::telemetry::settings_tags(home.path())
                .await
                .map(|(_, value)| value),
            ["enabled", presence, presence, presence]
        );
    }
    Ok(())
}
