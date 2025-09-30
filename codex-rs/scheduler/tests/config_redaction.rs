use codex_scheduler::config::ArangoConfig;
use codex_scheduler::config::SchedulerConfig;

#[test]
fn arango_password_is_redacted_in_debug() {
    let a = ArangoConfig {
        url: "https://localhost:8529".into(),
        database: "codex".into(),
        username: "root".into(),
        password: "super-secret".into(),
        runs_collection: "runs".into(),
        events_collection: "events".into(),
        notifications_collection: "notifications".into(),
        state_collection: "state".into(),
        allow_insecure: false,
    };
    let s = format!("{:?}", a);
    assert!(s.contains("<redacted>"));
    assert!(!s.contains("super-secret"));
}

#[test]
fn https_required_by_default() {
    let src = r#"
[scheduler]
enabled = true

[database.arango]
url = "http://localhost:8529"
database = "codex"
username = "root"
password = "pw"
runs_collection = "runs"
events_collection = "events"
notifications_collection = "notifications"
state_collection = "state"
"#;
    let res = SchedulerConfig::from_toml(src);
    assert!(
        res.is_err(),
        "http:// should be rejected unless allow_insecure=true"
    );
}

#[test]
fn http_allowed_when_allow_insecure_true() {
    let src = r#"
[scheduler]
enabled = true

[database.arango]
url = "http://localhost:8529"
database = "codex"
username = "root"
password = "pw"
runs_collection = "runs"
events_collection = "events"
notifications_collection = "notifications"
state_collection = "state"
allow_insecure = true
"#;
    let res = SchedulerConfig::from_toml(src);
    if let Err(e) = &res {
        eprintln!("parse error: {}", e);
    }
    assert!(
        res.is_ok(),
        "allow_insecure=true should permit http:// for local dev"
    );
}
