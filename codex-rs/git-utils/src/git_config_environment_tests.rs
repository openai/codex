use super::*;
use pretty_assertions::assert_eq;

#[test]
fn snapshot_binds_present_empty_and_absent_fixed_and_indexed_values() {
    let values = BTreeMap::from([
        (OsString::from("GIT_CONFIG_GLOBAL"), OsString::from("")),
        (
            OsString::from("GIT_CONFIG_PARAMETERS"),
            OsString::from("'safe.parameter'='present'"),
        ),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
        (OsString::from("HOME"), OsString::from("/safe/home")),
        (
            OsString::from("XDG_CONFIG_HOME"),
            OsString::from("/safe/xdg"),
        ),
        (OsString::from("GIT_CONFIG_COUNT"), OsString::from("2")),
        (
            OsString::from("GIT_CONFIG_KEY_0"),
            OsString::from("safe.one"),
        ),
        (
            OsString::from("GIT_CONFIG_VALUE_0"),
            OsString::from("present"),
        ),
        (
            OsString::from("GIT_CONFIG_KEY_1"),
            OsString::from("safe.two"),
        ),
    ]);
    let snapshot = GitConfigEnvironmentSnapshot::capture_from(|name| values.get(name).cloned())
        .expect("capture environment");

    assert_eq!(snapshot.value("GIT_CONFIG_GLOBAL"), Some(OsStr::new("")));
    assert_eq!(snapshot.value("HOME"), Some(OsStr::new("/safe/home")));
    assert_eq!(snapshot.value("GIT_CONFIG_SYSTEM"), None);
    assert_eq!(snapshot.value("GIT_CONFIG_NOSYSTEM"), Some(OsStr::new("1")));
    assert_eq!(
        snapshot.value("GIT_CONFIG_PARAMETERS"),
        Some(OsStr::new("'safe.parameter'='present'"))
    );
    assert_eq!(snapshot.value("GIT_CONFIG_COUNT"), Some(OsStr::new("2")));
    assert_eq!(
        snapshot.value("GIT_CONFIG_KEY_0"),
        Some(OsStr::new("safe.one"))
    );
    assert_eq!(
        snapshot.value("GIT_CONFIG_VALUE_0"),
        Some(OsStr::new("present"))
    );
    assert_eq!(snapshot.value("GIT_CONFIG_VALUE_1"), None);

    let mut command = Command::new("git");
    command
        .env("GIT_CONFIG_SYSTEM", "/later/system")
        .env("GIT_CONFIG_VALUE_1", "later");
    snapshot.apply_to(&mut command);
    let child_environment = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(OsStr::to_owned)))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        child_environment.get(OsStr::new("GIT_CONFIG_SYSTEM")),
        Some(&None)
    );
    assert_eq!(
        child_environment.get(OsStr::new("GIT_CONFIG_VALUE_1")),
        Some(&None)
    );
    assert_eq!(
        child_environment.get(OsStr::new("HOME")),
        Some(&Some(OsString::from("/safe/home")))
    );
}

#[test]
fn snapshot_rejects_malformed_or_unbounded_command_config_count() {
    for count in [OsString::from("invalid"), OsString::from("1025")] {
        let error = GitConfigEnvironmentSnapshot::capture_from(|name| {
            (name == "GIT_CONFIG_COUNT").then(|| count.clone())
        })
        .expect_err("reject count");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}

#[test]
fn snapshot_treats_empty_command_config_count_as_zero() {
    let values = BTreeMap::from([
        (OsString::from("GIT_CONFIG_COUNT"), OsString::from("")),
        (
            OsString::from("GIT_CONFIG_KEY_0"),
            OsString::from("untrusted.key"),
        ),
        (
            OsString::from("GIT_CONFIG_VALUE_0"),
            OsString::from("untrusted-value"),
        ),
    ]);
    let snapshot = GitConfigEnvironmentSnapshot::capture_from(|name| values.get(name).cloned())
        .expect("capture empty count");

    assert_eq!(snapshot.value("GIT_CONFIG_COUNT"), Some(OsStr::new("")));
    assert_eq!(snapshot.value("GIT_CONFIG_KEY_0"), None);
    assert_eq!(snapshot.value("GIT_CONFIG_VALUE_0"), None);

    let mut command = Command::new("git");
    command.env("GIT_CONFIG_COUNT", "1");
    snapshot.apply_to(&mut command);
    let child_environment = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(OsStr::to_owned)))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        child_environment.get(OsStr::new("GIT_CONFIG_COUNT")),
        Some(&Some(OsString::from("")))
    );
}

#[cfg(unix)]
#[test]
fn snapshot_preserves_non_utf8_config_path_bytes() {
    use std::os::unix::ffi::OsStringExt;

    let raw = OsString::from_vec(vec![b'/', b't', b'm', b'p', b'/', 0xff]);
    let snapshot = GitConfigEnvironmentSnapshot::capture_from(|name| {
        (name == "GIT_CONFIG_GLOBAL").then(|| raw.clone())
    })
    .expect("capture non-UTF-8 path");
    assert_eq!(snapshot.value("GIT_CONFIG_GLOBAL"), Some(raw.as_os_str()));
}

#[cfg(windows)]
#[test]
fn snapshot_matches_mixed_case_windows_environment_names() {
    let environment = BTreeMap::from([
        (OsString::from("git_config_count"), OsString::from("1")),
        (
            OsString::from("Git_Config_Key_0"),
            OsString::from("safe.key"),
        ),
        (
            OsString::from("gIt_CoNfIg_VaLuE_0"),
            OsString::from("safe-value"),
        ),
        (OsString::from("Home"), OsString::from(r"C:\safe-home")),
    ]);
    let snapshot = GitConfigEnvironmentSnapshot::capture_from(|name| {
        captured_environment_value(&environment, name)
    })
    .expect("capture mixed-case Windows environment");

    assert_eq!(snapshot.value("GIT_CONFIG_COUNT"), Some(OsStr::new("1")));
    assert_eq!(
        snapshot.value("GIT_CONFIG_KEY_0"),
        Some(OsStr::new("safe.key"))
    );
    assert_eq!(
        snapshot.value("GIT_CONFIG_VALUE_0"),
        Some(OsStr::new("safe-value"))
    );
    assert_eq!(snapshot.value("HOME"), Some(OsStr::new(r"C:\safe-home")));
}
