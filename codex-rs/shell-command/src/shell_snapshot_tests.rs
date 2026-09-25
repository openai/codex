use super::CapturedSnapshot;
use super::PreparedSnapshot;
use super::SnapshotCaptureOptions;
use super::SnapshotCredentialEnvironment;
use super::SnapshotStartup;
use super::prepare_snapshot_credentials;
use super::snapshot_capture_script;
use crate::shell_detect::ShellType;
use anyhow::Context;
use anyhow::Result;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::process::Command;
use tempfile::tempdir;

fn empty_snapshot_credentials<'a>(
    empty: &'a HashMap<String, String>,
    is_allowed_unset: &'a dyn Fn(&str) -> bool,
) -> SnapshotCredentialEnvironment<'a> {
    SnapshotCredentialEnvironment {
        original: empty,
        restored: empty,
        configured: empty,
        discovered: empty,
        allowed: empty,
        is_allowed_unset,
        brokered_keys: &[],
        brokered_alias_keys: &[],
        allowed_brokered_keys: &[],
    }
}

const CAPTURE_ALL: SnapshotCaptureOptions = SnapshotCaptureOptions {
    startup: SnapshotStartup::Interactive,
    declarations: true,
    environment: true,
};

const CAPTURE_NON_INTERACTIVE: SnapshotCaptureOptions = SnapshotCaptureOptions {
    startup: SnapshotStartup::NonInteractive,
    ..CAPTURE_ALL
};

fn snapshot_script(shell_type: ShellType) -> Option<String> {
    snapshot_capture_script(shell_type, CAPTURE_ALL)
}

#[test]
fn snapshot_capture_requires_marker_and_complete_records() {
    for captured in [
        "missing header\0\0\0",
        "# Snapshot file\n\0",
        "# Snapshot file\n\0\0NAME\0export NAME=value\n",
    ] {
        assert!(CapturedSnapshot::parse(ShellType::Bash, captured.as_bytes()).is_none());
    }
}

#[test]
fn snapshot_capture_selects_declarations_and_environment() -> Result<()> {
    let dir = tempdir()?;
    for (shell, shell_type) in [
        ("/bin/bash", ShellType::Bash),
        ("/bin/sh", ShellType::Sh),
        ("/bin/zsh", ShellType::Zsh),
    ] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        for (declarations, environment) in
            [(true, false), (false, true), (true, true), (false, false)]
        {
            let script = snapshot_capture_script(
                shell_type,
                SnapshotCaptureOptions {
                    startup: SnapshotStartup::NonInteractive,
                    declarations,
                    environment,
                },
            )
            .unwrap();
            let output = Command::new(shell)
                .args(["-c", &script])
                .env_clear()
                .env("HOME", dir.path())
                .env("PATH", "/usr/bin:/bin")
                .env("APP_SETTING", "value")
                .output()?;
            assert!(output.status.success(), "{shell}: {output:?}");
            let captured = CapturedSnapshot::parse(shell_type, &output.stdout).unwrap();
            assert_eq!(
                (
                    captured
                        .exports
                        .iter()
                        .any(|export| export.key == "APP_SETTING"),
                    captured
                        .environment
                        .split(|byte| *byte == 0)
                        .any(|entry| entry == b"APP_SETTING=value"),
                ),
                (declarations, environment),
                "{shell}"
            );
        }
    }
    Ok(())
}

#[test]
fn bash_snapshot_filters_invalid_exports() -> Result<()> {
    let output = Command::new("/bin/bash")
        .arg("-c")
        .arg(snapshot_script(ShellType::Bash).expect("bash supports snapshots"))
        .env("BASH_ENV", "/dev/null")
        .env("VALID_NAME", "ok")
        .env("PWD", "/tmp/stale")
        .env("NEXTEST_BIN_EXE_codex-write-config-schema", "/path/to/bin")
        .env("BAD-NAME", "broken")
        .output()?;

    assert!(output.status.success());

    let stdout = captured_script(ShellType::Bash, &String::from_utf8(output.stdout)?)?;
    assert!(stdout.contains("VALID_NAME"));
    assert!(!stdout.contains("PWD=/tmp/stale"));
    assert!(!stdout.contains("NEXTEST_BIN_EXE_codex-write-config-schema"));
    assert!(!stdout.contains("BAD-NAME"));

    Ok(())
}

#[test]
fn bash_snapshot_preserves_multiline_exports() -> Result<()> {
    let multiline_cert = "-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----";
    let output = Command::new("/bin/bash")
        .arg("-c")
        .arg(snapshot_script(ShellType::Bash).expect("bash supports snapshots"))
        .env("BASH_ENV", "/dev/null")
        .env("MULTILINE_CERT", multiline_cert)
        .output()?;

    assert!(output.status.success());

    let stdout = captured_script(ShellType::Bash, &String::from_utf8(output.stdout)?)?;
    assert!(
        stdout.contains("MULTILINE_CERT=") || stdout.contains("MULTILINE_CERT"),
        "snapshot should include the multiline export name"
    );

    let dir = tempdir()?;
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, stdout.as_bytes())?;

    let validate = Command::new("/bin/bash")
        .arg("-c")
        .arg("set -e; . \"$1\"")
        .arg("bash")
        .arg(&snapshot_path)
        .env("BASH_ENV", "/dev/null")
        .output()?;

    assert!(
        validate.status.success(),
        "snapshot validation failed: {}",
        String::from_utf8_lossy(&validate.stderr)
    );

    Ok(())
}

#[test]
fn snapshot_environment_preserves_native_path_exports_and_shell_level() -> Result<()> {
    let dir = tempdir()?;
    for (shell, shell_type, setup) in [
        ("/bin/bash", ShellType::Bash, "export -n PATH"),
        ("/bin/zsh", ShellType::Zsh, "typeset +x PATH"),
    ] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        let run = |script: &str| {
            // Keep Bash's final-command exec optimization from decrementing SHLVL.
            Command::new(shell)
                .args(["-c", &format!("{setup}\n{script}\n:")])
                .env_clear()
                .env("HOME", dir.path())
                .env("PATH", "/usr/bin:/bin")
                .env("SHLVL", "3")
                .output()
        };
        let native = run("/usr/bin/env -0")?;
        let script = snapshot_capture_script(
            shell_type,
            SnapshotCaptureOptions {
                startup: SnapshotStartup::NonInteractive,
                ..CAPTURE_ALL
            },
        )
        .unwrap();
        let output = run(&script)?;
        assert!(native.status.success() && output.status.success());
        let captured = CapturedSnapshot::parse(shell_type, &output.stdout).unwrap();
        let metadata = |environment: &[u8]| {
            environment
                .split(|byte| *byte == 0)
                .filter(|entry| entry.starts_with(b"PATH=") || entry.starts_with(b"SHLVL="))
                .map(<[u8]>::to_vec)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            metadata(captured.environment),
            metadata(&native.stdout),
            "{shell}"
        );
    }
    Ok(())
}

#[test]
fn sh_declarations_preserve_path_export_state() -> Result<()> {
    let dir = tempdir()?;
    let snapshot_path = dir.path().join("snapshot.sh");
    let script = snapshot_capture_script(
        ShellType::Sh,
        SnapshotCaptureOptions {
            startup: SnapshotStartup::NonInteractive,
            ..CAPTURE_ALL
        },
    )
    .expect("sh supports snapshots");
    for shell in [
        "/bin/sh",
        #[cfg(target_os = "macos")]
        "/bin/dash",
    ] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        for (setup, expected) in [
            ("PATH=/enterprise/bin; export PATH", "x:/enterprise/bin"),
            ("PATH=; export PATH", "x:"),
            ("unset PATH; export PATH", ":"),
            ("unset PATH; PATH=/enterprise/bin", ":"),
        ] {
            let captured = Command::new(shell)
                .args(["-uc", &format!("{setup}\n{script}")])
                .env_clear()
                .output()?;
            assert!(captured.status.success(), "capture failed: {captured:?}");
            std::fs::write(
                &snapshot_path,
                captured_script(ShellType::Sh, &String::from_utf8(captured.stdout)?)?,
            )?;
            let restored = Command::new(shell)
                .args([
                    "-c",
                    "unset PATH; . \"$1\"; printf '%s:%s' \"${PATH+x}\" \"${PATH-}\"",
                    "snapshot",
                ])
                .arg(&snapshot_path)
                .env_clear()
                .output()?;
            assert!(restored.status.success(), "replay failed: {restored:?}");
            assert_eq!(restored.stdout, expected.as_bytes(), "{setup}");
        }
    }
    Ok(())
}

#[test]
fn posix_startup_path_expansion_preserves_supported_forms() -> Result<()> {
    let script = format!(
        "{}\n__codex_snapshot_expand_env \"$ENV\"",
        super::posix_env_path_expansion_function()
    );
    let output = Command::new("/bin/sh")
        .arg("-uc")
        .arg(&script)
        .env("ENV", "$MISSING")
        .output()?;

    assert!(output.status.success(), "snapshot failed: {output:?}");
    assert_eq!(output.stdout, b"$MISSING");

    let dir = tempdir()?;
    let startup = dir.path().join("startup.sh");
    for (env_value, expansion_env) in [
        (
            startup.display().to_string(),
            Vec::<(String, String)>::new(),
        ),
        (
            "$STARTUP_FILE".to_string(),
            vec![("STARTUP_FILE".to_string(), startup.display().to_string())],
        ),
        (
            "${STARTUP_FILE}".to_string(),
            vec![("STARTUP_FILE".to_string(), startup.display().to_string())],
        ),
        (
            "${STARTUP_DIR}/startup.sh".to_string(),
            vec![("STARTUP_DIR".to_string(), dir.path().display().to_string())],
        ),
        (
            "${PATH%%:*}/startup.sh".to_string(),
            vec![(
                "PATH".to_string(),
                format!("{}:/usr/bin:/bin", dir.path().display()),
            )],
        ),
        (
            "$PATH/startup.sh".to_string(),
            vec![("PATH".to_string(), dir.path().display().to_string())],
        ),
        (
            "${PATH}/startup.sh".to_string(),
            vec![("PATH".to_string(), dir.path().display().to_string())],
        ),
        (
            format!("$PATH{}", startup.display()),
            vec![("PATH".to_string(), String::new())],
        ),
        (
            format!("${{PATH}}{}", startup.display()),
            vec![("PATH".to_string(), String::new())],
        ),
        (
            "~/startup.sh".to_string(),
            vec![("HOME".to_string(), dir.path().display().to_string())],
        ),
    ] {
        let output = Command::new("/bin/sh")
            .arg("-uc")
            .arg(&script)
            .env("ENV", &env_value)
            .envs(expansion_env)
            .output()?;

        assert!(output.status.success(), "snapshot failed: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout)?,
            startup.display().to_string(),
            "ENV={env_value}"
        );
    }

    let default_startup_dir = dir.path().join(".config");
    for xdg_config_home in [
        None,
        Some(std::ffi::OsStr::new("")),
        Some(default_startup_dir.as_os_str()),
    ] {
        let mut command = Command::new("/bin/sh");
        command
            .arg("-uc")
            .arg(&script)
            .env("HOME", dir.path())
            .env("ENV", "${XDG_CONFIG_HOME:-$HOME/.config}/startup.sh");
        if let Some(xdg_config_home) = xdg_config_home {
            command.env("XDG_CONFIG_HOME", xdg_config_home);
        } else {
            command.env_remove("XDG_CONFIG_HOME");
        }
        let output = command.output()?;

        assert!(output.status.success(), "snapshot failed: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout)?,
            default_startup_dir.join("startup.sh").display().to_string()
        );
    }

    let injected_marker = dir.path().join("env-injected");
    let injected_env = format!(
        "\"; touch '{}'; __codex_env_file=\"",
        injected_marker.display()
    );
    let output = Command::new("/bin/sh")
        .arg("-uc")
        .arg(&script)
        .env("ENV", injected_env)
        .output()?;
    assert!(output.status.success(), "snapshot failed: {output:?}");
    assert!(
        !injected_marker.exists(),
        "snapshot interpreted ENV as shell source"
    );

    Ok(())
}

#[test]
fn brokered_snapshots_filter_complete_multiline_exports() -> Result<()> {
    let dir = tempdir()?;
    for (shell, shell_type) in [
        ("/bin/bash", ShellType::Bash),
        ("/bin/sh", ShellType::Sh),
        ("/bin/dash", ShellType::Sh),
        ("/bin/zsh", ShellType::Zsh),
    ] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        for value in [
            "before\nexport NOT_A_REAL_VARIABLE=value\nafter",
            "before'\ndeclare -x NOT_A_REAL_VARIABLE=\"quoted\"\ntypeset OTHER=value\nafter\\",
            "before\t\n# exports 99\nexport NOT_A_REAL_VARIABLE=value\nafter",
            "certificate\nwith trailing newlines\n\n",
            "before\nexport NOT_A_REAL_VARIABLE\nafter",
            "before\nexport name=not_the_actual_value\nafter",
        ] {
            let original = HashMap::from([
                ("APP_SCRIPT".to_string(), value.to_string()),
                ("EXCLUDED_SCRIPT".to_string(), value.to_string()),
                ("ZZZ_AFTER".to_string(), "kept".to_string()),
                ("name".to_string(), "original name".to_string()),
                ("value".to_string(), "original value".to_string()),
                ("escaped".to_string(), "original escaped".to_string()),
            ]);
            let mut allowed = original.clone();
            allowed.remove("EXCLUDED_SCRIPT");
            let function_value = "before\n# exports 1\nexport NOT_A_REAL_VARIABLE=value\nafter";
            let capture_script = snapshot_capture_script(
                shell_type,
                SnapshotCaptureOptions {
                    startup: SnapshotStartup::NonInteractive,
                    ..CAPTURE_ALL
                },
            )
            .expect("shell supports snapshots");
            let capture = if matches!(shell_type, ShellType::Sh) {
                capture_script
            } else {
                format!("emit_source() {{ cat <<'EOF'\n{function_value}\nEOF\n}}\n{capture_script}")
            };
            let capture = if matches!(shell_type, ShellType::Sh | ShellType::Zsh) {
                format!("export APP_SETTING EXCLUDED_SETTING\n{capture}")
            } else {
                capture
            };
            let captured = Command::new(shell)
                .args(["-c", &capture])
                .env_clear()
                .envs(&original)
                .env("PATH", "/usr/bin:/bin")
                .output()?;
            assert!(captured.status.success(), "{shell}: {captured:?}");
            let mut snapshot = String::from_utf8(captured.stdout)?;
            let records = CapturedSnapshot::parse(shell_type, snapshot.as_bytes()).unwrap();
            assert_eq!(
                records
                    .exports
                    .iter()
                    .filter(|export| export.key == "name")
                    .count(),
                1,
                "{shell}: duplicate export record"
            );
            if matches!(shell_type, ShellType::Sh) {
                let raw_path = dir.path().join("raw-snapshot.sh");
                std::fs::write(&raw_path, captured_script(shell_type, &snapshot)?)?;
                let replayed = Command::new(shell)
                    .args(["-c", ". \"$1\"; printf '%s\\n' \"${APP_SETTING-unset}\"; APP_SETTING=production; /usr/bin/printenv APP_SETTING; NOT_A_REAL_VARIABLE=unexpected; /usr/bin/printenv NOT_A_REAL_VARIABLE", "snapshot"])
                    .arg(&raw_path)
                    .env_clear()
                    .output()?;
                assert_eq!(
                    replayed.stdout, b"unset\nproduction\n",
                    "{shell}: {snapshot}"
                );
                assert_eq!(replayed.status.code(), Some(1));
            }
            rewrite_snapshot_credentials(
                shell_type,
                &mut snapshot,
                SnapshotCredentialEnvironment {
                    original: &original,
                    restored: &original,
                    configured: &HashMap::new(),
                    discovered: &allowed,
                    allowed: &allowed,
                    is_allowed_unset: &|key| key == "APP_SETTING",
                    brokered_keys: &[],
                    brokered_alias_keys: &[],
                    allowed_brokered_keys: &[],
                },
                |_| true,
            );
            let path = dir.path().join("snapshot.sh");
            std::fs::write(&path, snapshot)?;
            let replayed = Command::new(shell)
                .args(["-c", ". \"$1\"; printf '%s\\0%s\\0%s\\0%s\\0%s\\0%s' \"$APP_SCRIPT\" \"${EXCLUDED_SCRIPT-unset}\" \"$ZZZ_AFTER\" \"$name\" \"$value\" \"$escaped\"; if command -v emit_source >/dev/null; then emit_source; fi; APP_SETTING=production; EXCLUDED_SETTING=denied; /usr/bin/printenv APP_SETTING; /usr/bin/printenv EXCLUDED_SETTING || :", "snapshot"])
                .arg(&path)
                .env_clear()
                .output()?;
            assert!(replayed.status.success(), "{shell}: {replayed:?}");
            let function_value = if matches!(shell_type, ShellType::Sh) {
                String::new()
            } else {
                format!("{function_value}\n")
            };
            let exported_setting = if matches!(shell_type, ShellType::Sh | ShellType::Zsh) {
                "production\n"
            } else {
                ""
            };
            assert_eq!(
                replayed.stdout,
                format!("{value}\0unset\0kept\0original name\0original value\0original escaped{function_value}{exported_setting}").as_bytes(),
                "{shell}"
            );
        }
    }
    Ok(())
}

#[test]
fn brokered_zsh_snapshot_preserves_tied_credential_aliases() -> Result<()> {
    if !std::path::Path::new("/bin/zsh").exists() {
        return Ok(());
    }
    let dir = tempdir()?;
    let real = "password$abcdefghijklmno";
    let dummy = "dummy-0123456789abcdef";
    let last = "ordinary'quote\nnext";
    let brokered_keys = vec!["VENDOR_PASSWORD".to_string()];
    for separator in [":", "|", "'", "\t", "$", "_", "x"] {
        let appended = format!("ordinary{separator}other");
        for (rcquotes, captured_credential) in [
            (false, real),
            (true, real),
            (false, "captured$0123456789abcdef"),
            (true, "pin_ab$cd"),
        ] {
            let script = snapshot_capture_script(ShellType::Zsh, CAPTURE_NON_INTERACTIVE).unwrap();
            let setup = format!(
                "{}\nexport -UT SERVICE_URLS service_urls=( \"https://user:$VENDOR_PASSWORD@example.invalid/path\" '' {} ) {}\nservice_urls+=({})\n{script}",
                if rcquotes {
                    "setopt rcquotes rcexpandparam"
                } else {
                    ":"
                },
                shlex::try_quote(last)?,
                shlex::try_quote(separator)?,
                shlex::try_quote(&appended)?,
            );
            let original = HashMap::from([
                (
                    "VENDOR_PASSWORD".to_string(),
                    captured_credential.to_string(),
                ),
                (
                    "SERVICE_URLS".to_string(),
                    [
                        format!("https://user:{captured_credential}@example.invalid/path"),
                        String::new(),
                        last.to_string(),
                        appended.clone(),
                    ]
                    .join(separator),
                ),
            ]);
            let captured = Command::new("/bin/zsh")
                .args(["-fc", &setup])
                .env_clear()
                .env("VENDOR_PASSWORD", captured_credential)
                .env("PATH", "/usr/bin:/bin")
                .output()?;
            assert!(captured.status.success(), "{captured:?}");
            let mut snapshot = String::from_utf8(captured.stdout)?;
            let allowed = original
                .iter()
                .map(|(key, value)| (key.clone(), value.replace(captured_credential, dummy)))
                .collect::<HashMap<_, _>>();
            let restored = original
                .iter()
                .map(|(key, value)| (key.clone(), value.replace(captured_credential, real)))
                .collect::<HashMap<_, _>>();
            let rewritten = rewrite_snapshot_credentials(
                ShellType::Zsh,
                &mut snapshot,
                SnapshotCredentialEnvironment {
                    original: &original,
                    restored: &restored,
                    configured: &HashMap::new(),
                    discovered: &allowed,
                    allowed: &allowed,
                    is_allowed_unset: &|_| true,
                    brokered_keys: &brokered_keys,
                    brokered_alias_keys: &[],
                    allowed_brokered_keys: &brokered_keys,
                },
                |text| {
                    *text = text
                        .replace(real, dummy)
                        .replace(captured_credential, dummy);
                    true
                },
            )
            .expect("tied export can be brokered");
            assert_eq!(
                rewritten.aliases,
                HashMap::from([("SERVICE_URLS".to_string(), allowed["SERVICE_URLS"].clone())])
            );
            assert!(!snapshot.contains(real));
            let path = dir.path().join("snapshot.sh");
            std::fs::write(&path, &snapshot)?;
            for live in [dummy, real] {
                let replayed = Command::new("/bin/zsh")
                    .args(["-fc", "set -e; . \"$1\"; printf '%s\\0' \"$SERVICE_URLS\" \"${service_urls[@]}\" \"$options[rcquotes]\"", "snapshot"])
                    .arg(&path)
                    .env_clear()
                    .env("VENDOR_PASSWORD", live)
                    .output()?;
                assert!(replayed.status.success(), "{replayed:?}");
                let scalar = original["SERVICE_URLS"].replace(captured_credential, live);
                let mut array = vec![
                    format!("https://user:{live}@example.invalid/path"),
                    String::new(),
                    last.to_string(),
                ];
                array.push(appended.clone());
                assert_eq!(
                    replayed.stdout,
                    format!(
                        "{scalar}\0{}\0{}\0",
                        array.join("\0"),
                        if rcquotes { "on" } else { "off" }
                    )
                    .into_bytes()
                );
            }
        }
    }
    for unique in [false, true] {
        for separator in [":", "|", "'", "\t"] {
            let flags = if unique { "-UT" } else { "-T" };
            let separator = shlex::try_quote(separator)?;
            let binding = format!("export {flags} SERVICE_URLS service_urls {separator}");
            let script = snapshot_capture_script(ShellType::Zsh, CAPTURE_NON_INTERACTIVE).unwrap();
            let captured = Command::new("/bin/zsh")
                .args([
                    "-fc",
                    &format!(
                        "export {flags} SERVICE_URLS service_urls=( '' \"$VENDOR_PASSWORD\" '' plain plain '' ) {separator}\nunset VENDOR_PASSWORD\n{script}"
                    ),
                ])
                .env_clear()
                .env("VENDOR_PASSWORD", real)
                .env("PATH", "/usr/bin:/bin")
                .output()?;
            assert!(captured.status.success(), "{captured:?}");
            let mut snapshot = String::from_utf8(captured.stdout)?;
            let original = CapturedSnapshot::parse(ShellType::Zsh, snapshot.as_bytes())
                .unwrap()
                .environment
                .split(|byte| *byte == 0)
                .filter_map(|entry| std::str::from_utf8(entry).ok()?.split_once('='))
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<HashMap<_, _>>();
            assert!(!original.contains_key("VENDOR_PASSWORD"));
            let allowed = original
                .iter()
                .map(|(key, value)| (key.clone(), value.replace(real, dummy)))
                .collect::<HashMap<_, _>>();
            let alias_keys = vec!["SERVICE_URLS".to_string()];
            let rewritten = rewrite_snapshot_credentials(
                ShellType::Zsh,
                &mut snapshot,
                SnapshotCredentialEnvironment {
                    original: &original,
                    restored: &original,
                    configured: &HashMap::new(),
                    discovered: &allowed,
                    allowed: &allowed,
                    is_allowed_unset: &|_| true,
                    brokered_keys: &[],
                    brokered_alias_keys: &alias_keys,
                    allowed_brokered_keys: &[],
                },
                |text| {
                    *text = text.replace(real, dummy);
                    true
                },
            )
            .expect("source-less tied alias can be brokered");
            assert_eq!(rewritten.aliases, HashMap::new());
            assert!(!snapshot.contains(real));
            assert!(!snapshot.contains(dummy));
            let path = dir.path().join("source-less-snapshot.sh");
            std::fs::write(&path, &snapshot)?;
            let inspect = "printf '%s\\0' \"${SERVICE_URLS-unset}\" \"${service_urls[@]}\" \"${(t)service_urls}\" \"${VENDOR_PASSWORD-unset}\"; if [ -n \"${SERVICE_URLS+x}\" ]; then SERVICE_URLS=$UPDATE; printf '%s\\0' \"$SERVICE_URLS\" \"${service_urls[@]}\"; unset SERVICE_URLS; fi; printf '%s:%s' \"${+SERVICE_URLS}\" \"${+service_urls}\"";
            for live in [Some(allowed["SERVICE_URLS"].as_str()), Some(""), None] {
                for source in [None, Some("different-live-source")] {
                    let mut outputs = Vec::new();
                    for script in [
                        format!("set -e; . \"$1\"; {inspect}"),
                        format!(
                            "set -e; if [ -n \"${{SERVICE_URLS+x}}\" ]; then {binding}; fi; {inspect}"
                        ),
                    ] {
                        let mut command = Command::new("/bin/zsh");
                        command
                            .args(["-fc", &script, "snapshot"])
                            .arg(&path)
                            .env_clear()
                            .env(
                                "UPDATE",
                                allowed["SERVICE_URLS"].replace(dummy, "new-live-dummy"),
                            );
                        if let Some(live) = live {
                            command.env("SERVICE_URLS", live);
                        }
                        if let Some(source) = source {
                            command.env("VENDOR_PASSWORD", source);
                        }
                        let output = command.output()?;
                        assert!(output.status.success(), "{script}: {output:?}");
                        outputs.push(String::from_utf8(output.stdout)?);
                    }
                    assert_eq!(outputs[0], outputs[1], "{binding}, {live:?}, {source:?}");
                }
            }
        }
    }
    Ok(())
}

#[test]
fn brokered_snapshot_rejects_credentials_crossing_array_elements() {
    let real = "password$abcdefghijklmno";
    let dummy = "dummy-0123456789abcdef";
    let keys = vec!["VENDOR_PASSWORD".to_string()];
    let restored = HashMap::from([(keys[0].clone(), real.to_string())]);
    for (captured_value, allowed_value) in [(real, dummy), (real, real), (dummy, dummy)] {
        let original = HashMap::from([
            (keys[0].clone(), captured_value.to_string()),
            ("AUTH_HEADER".to_string(), captured_value.to_string()),
        ]);
        let allowed = HashMap::from([
            (keys[0].clone(), allowed_value.to_string()),
            ("AUTH_HEADER".to_string(), allowed_value.to_string()),
        ]);
        let (first, last) = captured_value.split_at(/*mid*/ 10);
        let data = format!(
            "# Snapshot file\n\0\0AUTH_HEADER\0typeset -xT AUTH_HEADER header=( '{first}' '{last}' ) ''\n\0\0"
        );
        let capture = CapturedSnapshot::parse(ShellType::Zsh, data.as_bytes()).unwrap();
        let prepared = prepare_snapshot_credentials(
            &capture,
            SnapshotCredentialEnvironment {
                original: &original,
                restored: &restored,
                configured: &HashMap::new(),
                discovered: &allowed,
                allowed: &allowed,
                is_allowed_unset: &|_| true,
                brokered_keys: &keys,
                brokered_alias_keys: &[],
                allowed_brokered_keys: &keys,
            },
            |text| {
                *text = text.replace(real, allowed_value);
                true
            },
        );
        assert!(prepared.is_none(), "{captured_value}, {allowed_value}");
    }
}

#[test]
fn zsh_literal_bytes_match_native_modifiers() -> Result<()> {
    if !std::path::Path::new("/bin/zsh").exists() {
        return Ok(());
    }
    for word in [
        r"$'\M-C\M-)'",
        r"$'\xC3'$'\M-)'",
        r"$'\M-\C-?'",
        r"$'\C-\M-?'",
        r"$'\C-\xC3'",
        r"$'\M\x41'",
        r"$'\M-\u0041X'",
        r"$'\M-\q'",
    ] {
        let script = format!("printf '%s' {word}");
        let native = Command::new("/bin/zsh")
            .args(["-fc", &script])
            .env_clear()
            .output()?;
        assert!(native.status.success(), "{word}: {native:?}");
        let tree = crate::bash::try_parse_shell(&script).unwrap();
        let node = tree.root_node().named_child(0).unwrap();
        let mut cursor = node.walk();
        let argument = node
            .children_by_field_name("argument", &mut cursor)
            .last()
            .unwrap();
        let decoded = super::literals::literal_word_bytes(
            argument,
            &script,
            ShellType::Zsh,
            super::literals::Quoting::Posix,
        )
        .unwrap();
        assert_eq!(decoded, native.stdout, "{word}");
    }
    Ok(())
}

#[test]
fn brokered_snapshots_check_complete_alias_literals() -> Result<()> {
    let real = "token_ab'cdefghijklmnopqrstuvwx";
    let original = HashMap::from([("VENDOR_PASSWORD".to_string(), real.to_string())]);
    let allowed = HashMap::from([(
        "VENDOR_PASSWORD".to_string(),
        "token_dummy_abcdefghijklmnopqr".to_string(),
    )]);
    let brokered_keys = vec!["VENDOR_PASSWORD".to_string()];
    let controls = (1u8..=31)
        .map(char::from)
        .chain(['\u{7f}', '\u{e9}'])
        .collect::<String>();
    for (shell, aliases) in [("/bin/bash", "alias -p"), ("/bin/zsh", "alias -L")] {
        let shell_type = crate::shell_detect::detect_shell_type(shell).unwrap();
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        for (value, accepted) in [
            (format!("printf '%s' \"{real}\""), false),
            (format!("printf '%s' \"{real}\"\n:"), false),
            (
                "printf %s token_ab\\'cdefghijklmnopqrstuvwx\n:".to_string(),
                false,
            ),
            (
                "printf %s 'token_ab'\\''cdefghijklmnopqrstuvwx'\n:".to_string(),
                false,
            ),
            (
                "printf '%s' \"ordinary'quoted\"\n# tab\tbell\u{7}escape\u{1b}backslash\\\n:"
                    .to_string(),
                true,
            ),
            (format!("printf '%s' \"{controls}\""), true),
        ] {
            let output = Command::new(shell)
                .args(["-c", &format!("alias helper=\"$ALIAS_VALUE\"; {aliases}")])
                .env_clear()
                .env("ALIAS_VALUE", &value)
                .output()?;
            assert!(output.status.success(), "{shell}: {output:?}");
            let mut snapshot = format!(
                "# aliases\n{}\n# exports 0\n",
                String::from_utf8(output.stdout)?
            );
            let before = snapshot.clone();
            let rewritten = rewrite_snapshot_credentials(
                shell_type,
                &mut snapshot,
                SnapshotCredentialEnvironment {
                    original: &original,
                    restored: &original,
                    configured: &HashMap::new(),
                    discovered: &allowed,
                    allowed: &allowed,
                    is_allowed_unset: &|_| false,
                    brokered_keys: &brokered_keys,
                    brokered_alias_keys: &[],
                    allowed_brokered_keys: &brokered_keys,
                },
                |text| {
                    *text = text.replace(real, &allowed["VENDOR_PASSWORD"]);
                    true
                },
            );
            assert_eq!(rewritten.is_some(), accepted, "{shell}: {before}");
            if accepted {
                assert_eq!(snapshot, before);
                let expected = if shell_type == ShellType::Zsh {
                    let replay = Command::new(shell)
                        .args(["-fc", &format!("{snapshot}\nprint -rn -- $aliases[helper]")])
                        .env_clear()
                        .output()?;
                    assert!(replay.status.success(), "{replay:?}");
                    String::from_utf8(replay.stdout)?
                } else {
                    value.clone()
                };
                assert!(
                    super::literals::literal_words(&snapshot, shell_type)
                        .unwrap()
                        .contains(&format!("helper={expected}")),
                    "{shell}: {value:?}; {snapshot}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn brokered_snapshots_check_prefixed_alias_declarations() -> Result<()> {
    let empty = HashMap::new();
    let real = "token_ab'cd\"efghijklmnopqrstuvwx";
    let body = format!("printf '%s' {}", shlex::try_quote(real)?);
    for (shell, shell_type, setup) in [
        ("/bin/bash", ShellType::Bash, "shopt -s expand_aliases"),
        ("/bin/zsh", ShellType::Zsh, "setopt posixbuiltins"),
    ] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        for (prefix, creates_alias, name) in [
            ("alias", true),
            ("builtin alias", true),
            ("builtin builtin alias", true),
            ("time builtin alias", true),
            ("command alias", true),
            ("command -p -- alias", true),
            ("command -pp alias", true),
            ("command builtin alias", true),
            ("builtin command -p alias", true),
            ("command -v alias", false),
            ("command -pV alias", false),
        ]
        .into_iter()
        .chain((shell_type == ShellType::Bash).then_some(("builtin -- alias", true)))
        .chain((shell_type == ShellType::Bash).then_some(("time -p builtin alias", true)))
        .chain(
            (shell_type == ShellType::Zsh)
                .then_some([
                    ("noglob builtin alias", true),
                    ("builtin - alias", true),
                    ("- builtin alias", true),
                ])
                .into_iter()
                .flatten(),
        )
        .flat_map(|(prefix, creates_alias)| {
            ["captured", "\"$name\""].map(|name| (prefix, creates_alias, name))
        }) {
            let capture = format!(
                "helper() {{ local name=captured; {prefix} {name}={}; }}\ntypeset -f helper",
                shlex::try_quote(&body)?
            );
            let output = Command::new(shell)
                .args(["-fc", &capture])
                .env_clear()
                .output()?;
            assert!(output.status.success(), "{shell}: {output:?}");
            let original = String::from_utf8(output.stdout)?;
            assert!(!original.contains(real), "{shell}: {original}");
            for contains_credential in [false, true] {
                let mut snapshot = original.clone();
                let result = rewrite_snapshot_credentials(
                    shell_type,
                    &mut snapshot,
                    empty_snapshot_credentials(&empty, &|_| false),
                    |text| {
                        if contains_credential {
                            *text = text.replace(real, "dummy");
                        }
                        true
                    },
                );
                assert_eq!(
                    result.is_some(),
                    !contains_credential || !creates_alias,
                    "{shell}, {prefix}: {snapshot}"
                );
                assert_eq!(snapshot, original);
                if !contains_credential && creates_alias {
                    let replay = Command::new(shell)
                        .args([
                            "-fc",
                            &format!("{setup}\n{snapshot}\nhelper\neval captured"),
                        ])
                        .env_clear()
                        .output()?;
                    assert!(replay.status.success(), "{shell}, {prefix}: {replay:?}");
                    assert_eq!(String::from_utf8(replay.stdout)?, real);
                }
            }
        }
    }
    Ok(())
}

#[test]
fn brokered_snapshots_check_array_initializers_and_binary_literals() {
    let empty = HashMap::new();
    let real = "password$abcdefghijklmno";
    for (shell_type, original, has_credential) in [
        (
            ShellType::Bash,
            r#"declare -ax API_HEADERS='([0]="Bearer password\$abcdefghijklmno")'"#,
        ),
        (
            ShellType::Bash,
            r#"builtin declare -ax API_HEADERS='([0]="Bearer password\$abcdefghijklmno")'"#,
        ),
        (
            ShellType::Bash,
            r#"command -p typeset -ax API_HEADERS='([0]="Bearer password\$abcdefghijklmno")'"#,
        ),
        (
            ShellType::Bash,
            r#"declare -Ax API_HEADERS=([auth]="Bearer password\$abcdefghijklmno")"#,
        ),
        (
            ShellType::Bash,
            r#"helper() { local -a headers; local headers='([0]="Bearer password\$abcdefghijklmno")'; }"#,
        ),
        (
            ShellType::Bash,
            r#"declare -a headers; builtin declare headers='([0]="Bearer password\$abcdefghijklmno")'"#,
        ),
        (
            ShellType::Bash,
            r#"declare -a headers; command -p typeset headers='([0]="Bearer password\$abcdefghijklmno")'"#,
        ),
        (
            ShellType::Bash,
            r#"declare SCALAR_TEMPLATE='([0]="Bearer password\$abcdefghijklmno")'"#,
        ),
        (
            ShellType::Bash,
            r#"helper() { local name=headers; local -a headers; local "${name}"='([0]="Bearer password\$abcdefghijklmno")'; }"#,
        ),
        (
            ShellType::Bash,
            r#"helper() { local -a headers; local headers='([0]="Bearer password\$abcdefghijklmno" [1]="'"$OTHER"'")'; }"#,
        ),
        (
            ShellType::Bash,
            r#"helper() { local -a headers; local headers=$PREFIX'([0]="Bearer password\$abcdefghijklmno")'$SUFFIX; }"#,
        ),
        (
            ShellType::Zsh,
            r#"helper() { printf '%s' $'\x89password\x24abcdefghijklmno\xff'; }"#,
        ),
    ]
    .into_iter()
    .map(|(shell_type, original)| (shell_type, original, true))
    .chain(
        [
            r#"local TEMPLATE="(\$'\\xZZ')""#,
            r#"local TEMPLATE="(\$'\\uD800')""#,
            r#"local TEMPLATE="(\$'\\UFFFFFFFF')""#,
            r#"local TEMPLATE="(\$'\\c')""#,
            r#"local TEMPLATE="(\$'\\')""#,
            "local TEMPLATE=\"(\\$'\\\\c\u{e9}')\"",
            r#"local TEMPLATE="(\$'incomplete)""#,
            r#"local -a headers; local headers='([0]="Bearer '$CREDENTIAL'")'"#,
        ]
        .map(|original| (ShellType::Bash, original, false)),
    ) {
        for contains_credential in [false, true] {
            let mut snapshot = original.to_string();
            let result = rewrite_snapshot_credentials(
                shell_type,
                &mut snapshot,
                empty_snapshot_credentials(&empty, &|_| true),
                |text| {
                    if contains_credential {
                        *text = text.replace(real, "dummy");
                    }
                    true
                },
            );
            assert_eq!(
                result.is_some(),
                !contains_credential || !has_credential,
                "{original}"
            );
            assert_eq!(snapshot, original);
        }
    }
}

#[test]
fn brokered_sh_snapshots_inspect_the_captured_shell_dialect() -> Result<()> {
    let empty = HashMap::new();
    let real = "password$abcdefghijklmno";
    let script = snapshot_capture_script(ShellType::Sh, CAPTURE_NON_INTERACTIVE).unwrap();
    for (shell, bash_backed) in [("/bin/bash", true), ("/bin/dash", false)] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        for (setup, contains_credential) in [
            (
                r#"helper() { local -a headers; local headers='([0]="Bearer password\$abcdefghijklmno")'; }"#,
                true,
            ),
            (
                r#"helper() { time -p builtin alias captured='printf "%s" "password\$abcdefghijklmno"'; }"#,
                true,
            ),
            (
                r#"helper() { local name=captured; alias "$name"='printf "%s" "password\$abcdefghijklmno"'; }"#,
                true,
            ),
            (
                r#"export TEMPLATE='([0]="Bearer password\$abcdefghijklmno")'"#,
                true,
            ),
            (r#"export TEMPLATE="(\$'incomplete)""#, false),
            (
                r#"helper() { local -a headers; local headers='([0]="ordinary\$value")'; }; export TEMPLATE=ordinary"#,
                false,
            ),
        ] {
            let capture = Command::new(shell)
                .arg0("sh")
                .args(["-c", &format!("{setup}\n{script}")])
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .output()?;
            assert!(capture.status.success(), "{shell}: {capture:?}");
            let original = String::from_utf8(capture.stdout)?;
            assert_eq!(
                original.starts_with(super::BASH_SH_SNAPSHOT_HEADER),
                bash_backed
            );
            let mut snapshot = original.clone();
            let result = rewrite_snapshot_credentials(
                ShellType::Sh,
                &mut snapshot,
                empty_snapshot_credentials(&empty, &|_| true),
                |text| {
                    *text = text.replace(real, "dummy");
                    true
                },
            );
            assert_eq!(
                result.is_some(),
                !bash_backed || !contains_credential,
                "{shell}: {setup}"
            );
            assert_eq!(
                snapshot,
                if result.is_some() {
                    captured_script(ShellType::Sh, &original)?
                } else {
                    original
                }
            );
            if result.is_some() {
                let replay = Command::new(shell)
                    .arg0("sh")
                    .args(["-c", &format!("{snapshot}\nprintf '%s' \"${{TEMPLATE-}}\"")])
                    .env_clear()
                    .output()?;
                let control = Command::new(shell)
                    .arg0("sh")
                    .args(["-c", &format!("{setup}\nprintf '%s' \"${{TEMPLATE-}}\"")])
                    .env_clear()
                    .output()?;
                assert!(replay.status.success(), "{shell}: {replay:?}");
                assert!(control.status.success(), "{shell}: {control:?}");
                assert_eq!(replay.stdout, control.stdout);
            }
        }
    }
    Ok(())
}

#[test]
fn brokered_snapshots_check_native_quoting() -> Result<()> {
    let empty = HashMap::new();
    for (shell, shell_type, aliases) in [
        ("/bin/bash", ShellType::Bash, "alias -p"),
        ("/bin/zsh", ShellType::Zsh, "alias -L"),
    ] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        let mut expressions = vec![
            r#""`printf '%s' 'password\$abcdefghijklmno'`""#,
            r#"`printf '%s' 'password\$abcdefghijklmno'`"#,
            r#""`printf '%s' \`printf '%s' 'password\\\$abcdefghijklmno'\``""#,
            r#""${EMPTY-`printf '%s' 'password\"abcdefghijklmno'`}""#,
            r#"$"token_ab'cdefghijklmnopqrstuvwx""#,
            r#"prefix$"token_ab'cdefghijklmnopqrstuvwx""#,
            r#"$"token_ab'cdefghijklmnopqrstuvwx""${EMPTY-}""#,
            r#"$'token_\c\\abcdefghijklmnopqrstuvwx'"#,
            r#""$(printf '%s' $'token_\c\\abcdefghijklmnopqrstuvwx')""#,
            r#"$'token_\C-Aabcdefghijklmnopqrstuvwx'"#,
            r#"$'token_\C-\x41abcdefghijklmnopqrstuvwx'"#,
            r#"$'token_\C-\101abcdefghijklmnopqrstuvwx'"#,
            r#"$'token_\C-\u0041abcdefghijklmnopqrstuvwx'"#,
            r#"$'token_\C\101abcdefghijklmnopqrstuvwx'"#,
        ];
        let mut array_expressions = Vec::new();
        if shell_type == ShellType::Bash {
            expressions.extend([
                r#""$(array_value() { local -a headers; local headers='([0]="password\$abcdefghijklmno")'; printf '%s' "${headers[0]}"; }; array_value)""#,
                r#""$(declare -a headers; builtin declare headers='([0]="password\$abcdefghijklmno")'; printf '%s' "${headers[0]}")""#,
                r#""$(declare -a headers; command -p typeset headers='([0]="password\$abcdefghijklmno")'; printf '%s' "${headers[0]}")""#,
                r#""$(array_value() { local name=headers; local -a headers; local "${name}"='([0]="password\$abcdefghijklmno")'; printf '%s' "${headers[0]}"; }; array_value)""#,
                r#""$(array_value() { local -a headers; local headers='([0]="password\$abcdefghijklmno" [1]="'"$OTHER"'")'; printf '%s' "${headers[0]}"; }; array_value)""#,
                r#""$(array_value() { local -a headers; local headers=$PREFIX'([0]="password\$abcdefghijklmno")'$SUFFIX; printf '%s' "${headers[0]}"; }; array_value)""#,
            ]);
            for escape in [r"\xZZ", r"\uD800", r"\UFFFFFFFF", r"\c"] {
                let initializer = format!(r#"([0]=$'{escape}' [1]="password\$abcdefghijklmno")"#);
                let initializer = shlex::try_quote(&initializer)?;
                array_expressions.push(format!(
                    "\"$(declare -a headers; declare headers={initializer}; printf '%s' \"${{headers[1]}}\")\""
                ));
            }
        } else if shell_type == ShellType::Zsh {
            expressions.extend([
                "password_a$'\\xc3'$'\\xa9'bcdefghijklmno\"${EMPTY-}\"",
                "password_a$'\\M-C'$'\\M-)'bcdefghijklmno\"${EMPTY-}\"",
                "password_a$'\\xc3'\\\n$'\\xa9'bcdefghijklmno\"${EMPTY-}\"",
                "password\\\n\\$abcdefghijklmno",
                "pass\\\nword\\\n\\\n\\$abcdefghijklmno",
                "'password'\\\n'$abcdefghijklmno'",
                "\"password\"\\\n'$abcdefghijklmno'",
                "$'pass'\"word\"\\\n'$abcdefghijklmno'",
            ]);
        }
        for expression in expressions
            .into_iter()
            .map(str::to_string)
            .chain(array_expressions)
        {
            let body = format!("printf '%s' {expression}");
            let native = Command::new(shell)
                .args(["-fc", &body])
                .env_clear()
                .output()?;
            assert!(native.status.success(), "{shell}, {expression}: {native:?}");
            let real = String::from_utf8(native.stdout)?;
            assert!(real.len() >= 16, "{shell}, {expression}: {real:?}");
            let mut captures = vec![
                format!("helper() {{ {body}; }}\ntypeset -f helper"),
                format!("alias helper=\"$ALIAS_VALUE\"\n{aliases}"),
            ];
            if shell_type == ShellType::Zsh {
                captures.push(format!("alias -g helper=\"$ALIAS_VALUE\"\n{aliases}"));
                let script = snapshot_capture_script(shell_type, CAPTURE_NON_INTERACTIVE)
                    .expect("zsh supports snapshots");
                captures.push(format!(
                    "quoted() {{ cat <<'EOF'\n\" ${{(@f)EMPTY:-password\\$abcdefghijklmno}}\nEOF\n}}\nhelper() {{ {body}; }}\n{script}"
                ));
            }
            for capture in captures {
                let output = Command::new(shell)
                    .args(["-fc", &capture])
                    .env_clear()
                    .env("ALIAS_VALUE", &body)
                    .output()?;
                assert!(output.status.success(), "{shell}: {output:?}");
                let captured = String::from_utf8(output.stdout)?;
                let original = if captured.contains('\0') {
                    let capture = CapturedSnapshot::parse(shell_type, captured.as_bytes()).unwrap();
                    format!("{}{}", capture.state, capture.aliases)
                } else {
                    captured
                };
                for contains_credential in [false, true] {
                    let mut snapshot = original.clone();
                    let result = rewrite_snapshot_credentials(
                        shell_type,
                        &mut snapshot,
                        empty_snapshot_credentials(&empty, &|_| false),
                        |text| {
                            if contains_credential {
                                *text = text.replace(&real, "dummy");
                            }
                            true
                        },
                    );
                    assert_eq!(
                        result.is_some(),
                        !contains_credential,
                        "{shell}, {expression}: {snapshot}"
                    );
                    assert_eq!(snapshot, original);
                    if !contains_credential {
                        let setup = if shell_type == ShellType::Bash {
                            "shopt -s expand_aliases\n"
                        } else {
                            ""
                        };
                        let replay = Command::new(shell)
                            .args(["-fc", &format!("{setup}{snapshot}\neval helper")])
                            .env_clear()
                            .output()?;
                        assert!(
                            replay.status.success(),
                            "{shell}, {expression}: {snapshot}\n{replay:?}"
                        );
                        assert_eq!(String::from_utf8(replay.stdout)?, real);
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn zsh_snapshot_checks_functions_before_restoring_quote_options() -> Result<()> {
    if !std::path::Path::new("/bin/zsh").exists() {
        return Ok(());
    }
    let dir = tempdir()?;
    let real = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    let original = HashMap::from([("GH_TOKEN".to_string(), real.to_string())]);
    let allowed = HashMap::from([("GH_TOKEN".to_string(), "dummy-github-token".to_string())]);
    let keys = vec!["GH_TOKEN".to_string()];
    let script = snapshot_capture_script(ShellType::Zsh, CAPTURE_NON_INTERACTIVE)
        .expect("zsh supports snapshots");
    for option in ["setopt rcquotes", "unsetopt rcquotes"] {
        for (value, accepted) in [(real, false), ("ordinaryvalue", true)] {
            let (first, second) = value.split_at(value.len() / 2);
            let startup = dir.path().join("startup.sh");
            std::fs::write(
                &startup,
                format!("helper() {{ printf '%s' '{first}''{second}'; }}\n{option}\n"),
            )?;
            let output = Command::new("/bin/zsh")
                .args(["-fc", &format!(". \"$1\"\n{script}"), "snapshot"])
                .arg(startup)
                .env_clear()
                .envs(&original)
                .env("HOME", dir.path())
                .output()?;
            assert!(output.status.success(), "{output:?}");
            let mut snapshot = String::from_utf8(output.stdout)?;
            let rewritten = rewrite_snapshot_credentials(
                ShellType::Zsh,
                &mut snapshot,
                SnapshotCredentialEnvironment {
                    original: &original,
                    restored: &original,
                    configured: &HashMap::new(),
                    discovered: &allowed,
                    allowed: &allowed,
                    is_allowed_unset: &|_| false,
                    brokered_keys: &keys,
                    brokered_alias_keys: &[],
                    allowed_brokered_keys: &keys,
                },
                |text| {
                    *text = text.replace(real, &allowed["GH_TOKEN"]);
                    true
                },
            );
            assert_eq!(rewritten.is_some(), accepted, "{option}: {snapshot}");
            if accepted {
                let path = dir.path().join("snapshot.sh");
                std::fs::write(&path, &snapshot)?;
                let replay = Command::new("/bin/zsh")
                    .args(["-fc", ". \"$1\"; helper", "snapshot"])
                    .arg(path)
                    .env_clear()
                    .envs(&allowed)
                    .output()?;
                assert!(replay.status.success(), "{replay:?}");
                assert_eq!(String::from_utf8(replay.stdout)?, value);
            }
        }
    }
    Ok(())
}

#[test]
fn zsh_snapshot_checks_deferred_quote_modes() -> Result<()> {
    if !std::path::Path::new("/bin/zsh").exists() {
        return Ok(());
    }
    let dir = tempdir()?;
    let script = snapshot_capture_script(ShellType::Zsh, CAPTURE_NON_INTERACTIVE)
        .expect("zsh supports snapshots");
    for (real, startup) in [
        (
            "token_ab'cdefghijklmnopqrstuvwx",
            "helper() { printf '%s' \"$(printf '%s' 'token_ab''cdefghijklmnopqrstuvwx')\"; }\nsetopt rcquotes\n",
        ),
        (
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "alias helper=\"setopt rcquotes; printf '%s' 'ghp_abcdefghijklmno''pqrstuvwxyz0123456789'\"\n",
        ),
        (
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "alias helper=\"printf '%s' 'ghp_abcdefghijkl'\\\\\n'mnopqrstuvwxyz0123456789'\"\nsetopt rcquotes\n",
        ),
    ] {
        let startup_path = dir.path().join("startup.sh");
        std::fs::write(&startup_path, startup)?;
        let output = Command::new("/bin/zsh")
            .args(["-fc", &format!(". \"$1\"\n{script}"), "snapshot"])
            .arg(startup_path)
            .env_clear()
            .env("HOME", dir.path())
            .output()?;
        assert!(output.status.success(), "{output:?}");
        let snapshot = captured_script(ShellType::Zsh, &String::from_utf8(output.stdout)?)?;
        let path = dir.path().join("snapshot.sh");
        std::fs::write(&path, &snapshot)?;
        let replay = Command::new("/bin/zsh")
            .args(["-fc", ". \"$1\"; eval helper", "snapshot"])
            .arg(path)
            .env_clear()
            .output()?;
        assert!(replay.status.success(), "{replay:?}");
        assert_eq!(String::from_utf8(replay.stdout)?, real);
        assert!(
            super::literals::literal_words(&snapshot, ShellType::Zsh)
                .expect("valid snapshot")
                .contains(&real.to_string()),
            "runtime literal was not checked: {snapshot}"
        );
    }
    Ok(())
}

#[test]
fn zsh_snapshot_checks_rcquotes_without_changing_alias_behavior() -> Result<()> {
    if !std::path::Path::new("/bin/zsh").exists() {
        return Ok(());
    }
    let dir = tempdir()?;
    let real = "token_ab'cdefghijklmnopqrstuvwx";
    let original = HashMap::from([("VENDOR_PASSWORD".to_string(), real.to_string())]);
    let allowed = HashMap::from([(
        "VENDOR_PASSWORD".to_string(),
        "token_dummy_abcdefghijklmnopqr".to_string(),
    )]);
    let brokered_keys = vec!["VENDOR_PASSWORD".to_string()];
    let script = snapshot_capture_script(ShellType::Zsh, CAPTURE_NON_INTERACTIVE)
        .expect("zsh supports snapshots");
    for (option, option_value) in [("setopt rcquotes", "on"), ("unsetopt rcquotes", "off")] {
        for (value, accepted) in [(real, false), ("ordinary'quoted", true)] {
            for alias in [
                r#"alias helper="printf '%s' \"$ALIAS_VALUE\"""#,
                r#"alias helper="printf '%s' ${(qq)ALIAS_VALUE}""#,
                r#"alias helper="printf '%s' ${(qq)ALIAS_VALUE}\"\${EMPTY-}\"""#,
            ] {
                let output = Command::new("/bin/zsh")
                    .args([
                        "-fc",
                        &format!("{option}\n{alias}\nunset ALIAS_VALUE\n{script}"),
                    ])
                    .env_clear()
                    .envs(&original)
                    .env("HOME", dir.path())
                    .env("ALIAS_VALUE", value)
                    .output()?;
                assert!(output.status.success(), "{output:?}");
                let mut snapshot = String::from_utf8(output.stdout)?;
                let rewritten = rewrite_snapshot_credentials(
                    ShellType::Zsh,
                    &mut snapshot,
                    SnapshotCredentialEnvironment {
                        original: &original,
                        restored: &original,
                        configured: &HashMap::new(),
                        discovered: &allowed,
                        allowed: &allowed,
                        is_allowed_unset: &|_| false,
                        brokered_keys: &brokered_keys,
                        brokered_alias_keys: &[],
                        allowed_brokered_keys: &brokered_keys,
                    },
                    |text| {
                        *text = text.replace(real, &allowed["VENDOR_PASSWORD"]);
                        true
                    },
                );
                assert_eq!(
                    rewritten.is_some(),
                    accepted,
                    "{option}, {alias}: {snapshot}"
                );
                if accepted {
                    let path = dir.path().join("snapshot.sh");
                    std::fs::write(&path, &snapshot)?;
                    let replay = Command::new("/bin/zsh")
                        .args([
                            "-fc",
                            ". \"$1\"; eval helper; printf '\\n%s\\n' $options[rcquotes]",
                            "snapshot",
                        ])
                        .arg(path)
                        .env_clear()
                        .envs(&allowed)
                        .output()?;
                    assert!(replay.status.success(), "{replay:?}");
                    assert_eq!(
                        String::from_utf8(replay.stdout)?,
                        format!("{value}\n{option_value}\n")
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn brokered_snapshots_check_static_fragments_next_to_expansions() -> Result<()> {
    let real = "password$abcdefghijklmno";
    let dir = tempdir()?;
    let invocation = dir.path().join("invoke.sh");
    std::fs::write(&invocation, "helper\n")?;
    let inner_invocation = dir.path().join("inner-invoke.sh");
    std::fs::write(&inner_invocation, "inner\n")?;
    for (shell, aliases) in [("/bin/bash", "alias -p"), ("/bin/zsh", "alias -L")] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        for (body, accepted) in [
            (r#"printf '%s' "${USER}:password\$abcdefghijklmno""#, false),
            (r#"printf '%s' "${USER}:pass"'word$abcdefghijklmno'"#, false),
            (r#"printf '%s' "password\$abcdefghijklmno:${USER}""#, false),
            (
                r#"printf '%s' "${USER}:"$'password\x24abcdefghijklmno'"#,
                false,
            ),
            (r#"printf '%s' "${USER}:$VENDOR_PASSWORD""#, true),
            (r#"printf '%s' "${USER}:ordinary\$value""#, true),
            (
                "cat <<EOF\nAuthorization: password\\$abcdefghijklmno\nEOF\n",
                false,
            ),
            (
                "cat <<EOF\nAuthorization: pass\\\nword\\$abcdefghijklmno\nEOF\n",
                false,
            ),
            (
                "cat <<EOF\n${USER}:password\\$abcdefghijklmno\nEOF\n",
                false,
            ),
            ("cat <<-EOF\n\tpassword\\$abcdefghijklmno\n\tEOF\n", false),
            ("cat <<'EOF'\npassword\\$abcdefghijklmno\nEOF\n", true),
            ("cat <<EOF\n${USER}:$VENDOR_PASSWORD\nEOF\n", true),
            ("cat <<EOF\nordinary\\$value\nEOF\n", true),
        ]
        .into_iter()
        .chain(
            (shell == "/bin/zsh")
                .then_some([
                    (
                        "export GH_COPY=password\\\n\\$abcdefghijklmno; printf '%s' \"$GH_COPY\"",
                        false,
                    ),
                    (
                        "GH_COPY=password\\\n\\$abcdefghijklmno; printf '%s' \"$GH_COPY\"",
                        false,
                    ),
                    (
                        "password=''\\\n'$abcdefghijklmno'; printf '%s' \"$password\"",
                        true,
                    ),
                    (
                        "ali\\\nas inner='printf \"%s\" \"password\\$abcdefghijklmno\"'",
                        false,
                    ),
                    (
                        "builtin ali\\\nas inner='printf \"%s\" \"password\\$abcdefghijklmno\"'",
                        false,
                    ),
                    (
                        "alias inner='printf \"%s\" \"password'\\\n'\\$abcdefghijklmno\"'",
                        false,
                    ),
                    (
                        "alias inner='printf \"%s\" \"password'\\\n'\\$abcdefghijklmno\"'\"${EMPTY}\"",
                        false,
                    ),
                    (
                        "alias inner=\"${EMPTY}\"'printf \"%s\" \"password'\\\n'\\$abcdefghijklmno\"'",
                        false,
                    ),
                    (
                        "alias inner='printf \"%s\" \"password'\"${EMPTY}\"\\\n'\\$abcdefghijklmno\"'",
                        true,
                    ),
                    (
                        "alias inner='printf \"%s\" \"ordinary'\\\n'value\"'\"${EMPTY}\"",
                        true,
                    ),
                    (
                        "printf '%s' '${USER}:pass'\\\n'word$abcdefghijklmno'",
                        false,
                    ),
                    (
                        "printf '%s' \"${USER}:pass\"\\\n'word$abcdefghijklmno'",
                        false,
                    ),
                    (
                        "printf '%s' 'password'\\\n\"${USER}\\$abcdefghijklmno\"",
                        true,
                    ),
                    ("printf '%s' 'password\n$abcdefghijklmno'", true),
                    ("printf '%s' 'password\\\n$abcdefghijklmno'", true),
                    ("printf '%s' \"password\n\\$abcdefghijklmno\"", true),
                    ("printf '%s' password\\\\\\\n\\$abcdefghijklmno", true),
                    ("printf '%s ' password \\\n\\$abcdefghijklmno", true),
                    ("printf '%s ' password\\\n \\$abcdefghijklmno", true),
                ])
                .into_iter()
                .flatten(),
        ) {
            for script in [
                format!("helper() {{\n{body}\n}}; typeset -f helper"),
                format!("alias helper=\"$ALIAS_VALUE\"; {aliases}"),
            ] {
                let output = Command::new(shell)
                    .args(["-c", &script])
                    .env_clear()
                    .env("ALIAS_VALUE", body)
                    .output()?;
                assert!(output.status.success(), "{shell}: {output:?}");
                let mut snapshot = String::from_utf8(output.stdout)?;
                let original = snapshot.clone();
                let result = rewrite_snapshot_credentials(
                    crate::shell_detect::detect_shell_type(shell).unwrap(),
                    &mut snapshot,
                    empty_snapshot_credentials(&HashMap::new(), &|_| false),
                    |text| {
                        *text = text.replace(real, "dummy");
                        true
                    },
                );
                assert_eq!(result.is_some(), accepted, "{shell}: {original}");
                assert_eq!(snapshot, original);
                let invokes_inner = body.contains("inner=");
                if shell == "/bin/zsh" && (accepted || invokes_inner) {
                    let path = dir.path().join("snapshot.sh");
                    std::fs::write(&path, &snapshot)?;
                    // Source each invocation after its alias has been defined, without eval.
                    let replay_command = if invokes_inner {
                        ". \"$1\"; . \"$2\"; . \"$3\""
                    } else {
                        ". \"$1\"; . \"$2\""
                    };
                    let replay = Command::new(shell)
                        .args(["-c", replay_command, "snapshot"])
                        .args([&path, &invocation, &inner_invocation])
                        .env_clear()
                        .output()?;
                    let native_command = if invokes_inner {
                        format!("{body}\n. \"$1\"")
                    } else {
                        body.to_string()
                    };
                    let native = Command::new(shell)
                        .args(["-c", &native_command, "native"])
                        .arg(&inner_invocation)
                        .env_clear()
                        .output()?;
                    assert!(replay.status.success(), "{shell}: {replay:?}");
                    assert!(native.status.success(), "{shell}: {native:?}");
                    assert_eq!(replay.stdout, native.stdout, "{shell}: {body}");
                    if !accepted && invokes_inner {
                        assert_eq!(native.stdout, real.as_bytes(), "{body}");
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn brokered_bash_snapshot_does_not_rewrite_function_syntax() -> Result<()> {
    let dir = tempdir()?;
    let startup = dir.path().join("startup.sh");
    let real = "ghp_abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGH";
    let dummy = "ghp_brokered_dummy_abcdefghijklmnopqrstuvwxyz0123";
    let credential_shaped_function = "ghp_0123456789abcdefghijklmnopqrstuvwxyzABCD";
    std::fs::write(
        &startup,
        format!(
            "helper() {{\n  cat <<'EOF'\nexport LEGACY_SETTING=production\nAuthorization: {real}\nEOF\n}}\n{credential_shaped_function}() {{ printf UNEXPECTED_EXECUTION; }}\nexport -f helper\nexport GH_TOKEN='{real}'\n"
        ),
    )?;
    let output = Command::new("/bin/bash")
        .arg("-c")
        .arg(
            snapshot_capture_script(ShellType::Bash, CAPTURE_ALL).expect("bash supports snapshots"),
        )
        .env_clear()
        .env("HOME", dir.path())
        .env("PATH", "/usr/bin:/bin")
        .env("BASH_ENV", &startup)
        .output()?;
    assert!(output.status.success());

    let mut snapshot = String::from_utf8(output.stdout)?;
    let original = HashMap::from([("GH_TOKEN".to_string(), real.to_string())]);
    let discovered = HashMap::from([("GH_TOKEN".to_string(), dummy.to_string())]);
    let brokered_keys = vec!["GH_TOKEN".to_string()];
    let snapshot_is_safe = rewrite_snapshot_credentials(
        ShellType::Bash,
        &mut snapshot,
        SnapshotCredentialEnvironment {
            original: &original,
            restored: &original,
            configured: &HashMap::new(),
            discovered: &discovered,
            allowed: &discovered,
            is_allowed_unset: &|_| false,
            brokered_keys: &brokered_keys,
            brokered_alias_keys: &[],
            allowed_brokered_keys: &brokered_keys,
        },
        |text| {
            if text.contains(credential_shaped_function) {
                *text = text.replace(credential_shaped_function, "");
                return false;
            }
            *text = text.replace(real, dummy);
            true
        },
    );
    assert!(snapshot_is_safe.is_none());
    assert!(snapshot.contains(real));
    assert!(snapshot.contains(credential_shaped_function));

    Ok(())
}

#[test]
fn brokered_zsh_tied_alias_respects_policy_override() -> Result<()> {
    if !std::path::Path::new("/bin/zsh").exists() {
        return Ok(());
    }
    let real = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    let dummy = "ghp_dummy_abcdefghijklmnopqrstuvwxyz0123";
    let keys = vec!["GH_TOKEN".to_string()];
    let script = snapshot_capture_script(ShellType::Zsh, CAPTURE_NON_INTERACTIVE).unwrap();
    for (replacement, source_present, source_allowed) in ["", "replacement|plain|replacement"]
        .into_iter()
        .flat_map(|replacement| {
            [
                (replacement, true, true),
                (replacement, true, false),
                (replacement, false, false),
            ]
        })
    {
        let unset_source = if source_present { "" } else { "unset GH_TOKEN" };
        let output = Command::new("/bin/zsh")
            .args([
                "-fc",
                &format!(
                    "export -UT AUTH_HEADER auth_header=(\"Bearer $GH_TOKEN\" plain) '|'\n{unset_source}\n{script}"
                ),
            ])
            .env_clear()
            .env("GH_TOKEN", real)
            .output()?;
        assert!(output.status.success(), "{output:?}");
        let mut snapshot = String::from_utf8(output.stdout)?;
        let mut original = HashMap::from([
            ("GH_TOKEN".to_string(), real.to_string()),
            ("AUTH_HEADER".to_string(), format!("Bearer {real}|plain")),
        ]);
        let brokered_keys = if source_present {
            keys.as_slice()
        } else {
            original.remove("GH_TOKEN");
            &[]
        };
        let discovered = original
            .iter()
            .map(|(key, value)| (key.clone(), value.replace(real, dummy)))
            .collect::<HashMap<_, _>>();
        let configured = HashMap::from([("AUTH_HEADER".to_string(), replacement.to_string())]);
        let mut allowed = discovered.clone();
        allowed.extend(configured.clone());
        let allowed_keys = if source_allowed {
            keys.as_slice()
        } else {
            allowed.remove("GH_TOKEN");
            &[]
        };
        let prepared = rewrite_snapshot_credentials(
            ShellType::Zsh,
            &mut snapshot,
            SnapshotCredentialEnvironment {
                original: &original,
                restored: &original,
                configured: &configured,
                discovered: &discovered,
                allowed: &allowed,
                is_allowed_unset: &|_| true,
                brokered_keys,
                brokered_alias_keys: &[],
                allowed_brokered_keys: allowed_keys,
            },
            |value| {
                if !source_allowed && (value.contains(real) || value.contains(dummy)) {
                    return false;
                }
                *value = value.replace(real, dummy);
                true
            },
        )
        .expect("policy override keeps the tied alias usable");
        assert_eq!(
            prepared.aliases,
            if source_present {
                configured
            } else {
                HashMap::new()
            }
        );
        assert!(!snapshot.contains(real));
        let inspect = "printf '%s\\0' \"$AUTH_HEADER\" \"${auth_header[@]}\"; AUTH_HEADER='next|next|last'; printf '%s\\0' \"$AUTH_HEADER\" \"${auth_header[@]}\"";
        let mut outputs = Vec::new();
        for setup in [snapshot.as_str(), "export -UT AUTH_HEADER auth_header '|'"] {
            let output = Command::new("/bin/zsh")
                .args(["-fc", &format!("set -e; {setup}\n{inspect}")])
                .env_clear()
                .env("AUTH_HEADER", replacement)
                .output()?;
            assert!(output.status.success(), "{output:?}");
            outputs.push(output.stdout);
        }
        assert_eq!(outputs[0], outputs[1]);
    }
    Ok(())
}

#[test]
fn brokered_snapshot_alias_values_follow_allowed_credential_overrides() {
    let real = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    let discovered_dummy = "ghp_discovered_dummy_abcdefghijklmnopqr";
    let configured_dummy = "ghp_configured_dummy_abcdefghijklmnopqr";
    let original = HashMap::from([
        ("GH_TOKEN".to_string(), real.to_string()),
        ("AUTH_HEADER".to_string(), format!("Bearer {real}")),
    ]);
    let discovered = HashMap::from([("GH_TOKEN".to_string(), discovered_dummy.to_string())]);
    let brokered_keys = vec!["GH_TOKEN".to_string()];
    for (source_allowed, alias, expected) in [
        (
            true,
            format!("Bearer {discovered_dummy}"),
            Some((
                format!("Bearer {configured_dummy}"),
                r#"export AUTH_HEADER='Bearer '"${GH_TOKEN-}""#,
            )),
        ),
        (
            true,
            String::new(),
            Some((String::new(), "export AUTH_HEADER=''")),
        ),
        (false, format!("Bearer {discovered_dummy}"), None),
    ] {
        let mut allowed = HashMap::from([
            ("GH_TOKEN".to_string(), configured_dummy.to_string()),
            ("AUTH_HEADER".to_string(), alias),
        ]);
        let allowed_brokered_keys = if source_allowed {
            brokered_keys.as_slice()
        } else {
            allowed.remove("GH_TOKEN");
            &[]
        };
        let mut snapshot =
            format!("# Snapshot file\n\0\0AUTH_HEADER\0export AUTH_HEADER='Bearer {real}'\n\0\0");
        let prepared = rewrite_snapshot_credentials(
            ShellType::Bash,
            &mut snapshot,
            SnapshotCredentialEnvironment {
                original: &original,
                restored: &original,
                configured: &HashMap::new(),
                discovered: &discovered,
                allowed: &allowed,
                is_allowed_unset: &|_| false,
                brokered_keys: &brokered_keys,
                brokered_alias_keys: &[],
                allowed_brokered_keys,
            },
            |text| {
                *text = text.replace(real, discovered_dummy);
                true
            },
        );
        let expected = match expected {
            Some((value, assignment)) => (
                format!("# Snapshot file\n# exports (native declarations)\n{assignment}\n"),
                HashMap::from([("AUTH_HEADER".to_string(), value)]),
                Vec::new(),
            ),
            None => (
                "# Snapshot file\n".to_string(),
                HashMap::new(),
                vec!["AUTH_HEADER".to_string()],
            ),
        };
        assert_eq!(
            prepared.map(|rewrite| (rewrite.script, rewrite.aliases, rewrite.rejected_alias_keys)),
            Some(expected),
        );
    }
}

#[test]
fn brokered_short_alias_respects_policy_override() -> Result<()> {
    let real = "pin_abcdefgh";
    let dummy = "pin_qrstuvwx";
    let keys = vec!["VENDOR_PASSWORD".to_string()];
    for captured in [real, "pin_ijklmnop"] {
        let original = HashMap::from([
            ("VENDOR_PASSWORD".to_string(), captured.to_string()),
            ("AUTH_HEADER".to_string(), format!("Bearer {captured}")),
        ]);
        let restored = HashMap::from([
            ("VENDOR_PASSWORD".to_string(), real.to_string()),
            ("AUTH_HEADER".to_string(), format!("Bearer {real}")),
        ]);
        let discovered = HashMap::from([
            ("VENDOR_PASSWORD".to_string(), dummy.to_string()),
            ("AUTH_HEADER".to_string(), format!("Bearer {dummy}")),
        ]);
        for replacement in ["", "replacement"] {
            let configured = HashMap::from([("AUTH_HEADER".to_string(), replacement.to_string())]);
            let mut allowed = discovered.clone();
            allowed.extend(configured.clone());
            let mut snapshot = format!(
                "# Snapshot file\n\0\0AUTH_HEADER\0export AUTH_HEADER='Bearer {captured}'\n\0\0"
            );
            let prepared = rewrite_snapshot_credentials(
                ShellType::Bash,
                &mut snapshot,
                SnapshotCredentialEnvironment {
                    original: &original,
                    restored: &restored,
                    configured: &configured,
                    discovered: &discovered,
                    allowed: &allowed,
                    is_allowed_unset: &|_| false,
                    brokered_keys: &keys,
                    brokered_alias_keys: &[],
                    allowed_brokered_keys: &keys,
                },
                |value| {
                    *value = value.replace(real, dummy).replace(captured, dummy);
                    true
                },
            )
            .expect("policy override keeps short aliases usable");
            assert_eq!(prepared.aliases, configured);
            assert!(!snapshot.contains(real));
            assert!(!snapshot.contains(captured));
            let output = Command::new("/bin/bash")
                .args(["-c", &format!("{snapshot}\nprintf '%s' \"$AUTH_HEADER\"")])
                .env_clear()
                .output()?;
            assert!(output.status.success(), "{output:?}");
            assert_eq!(output.stdout, replacement.as_bytes());
        }
    }
    Ok(())
}

#[test]
fn brokered_snapshot_aliases_follow_captured_dummies() -> Result<()> {
    for (real, captured, dummy) in [
        (
            "ghp_real_abcdefghijklmnopqrstuvwxyz012345",
            "ghp_old_abcdefghijklmnopqrstuvwxyz0123456",
            "ghp_new_abcdefghijklmnopqrstuvwxyz0123456",
        ),
        ("pin_abcdefgh", "pin_ijklmnop", "pin_ijklmnop"),
        ("pin_abcdefgh", "pin_ijklmnop", "pin_qrstuvwx"),
    ] {
        let original = HashMap::from([
            ("VENDOR_PASSWORD".to_string(), captured.to_string()),
            ("AUTH_HEADER".to_string(), format!("Bearer {captured}")),
            ("READONLY_HEADER".to_string(), format!("Bearer {captured}")),
        ]);
        let mut restored = original.clone();
        restored.insert("VENDOR_PASSWORD".to_string(), real.to_string());
        let allowed = HashMap::from([
            ("VENDOR_PASSWORD".to_string(), dummy.to_string()),
            ("AUTH_HEADER".to_string(), format!("Bearer {dummy}")),
            ("READONLY_HEADER".to_string(), format!("Bearer {dummy}")),
        ]);
        let keys = vec!["VENDOR_PASSWORD".to_string()];
        let mut snapshot = format!(
            "# Snapshot file\n\0\0AUTH_HEADER\0export AUTH_HEADER='Bearer {captured}'\n\0READONLY_HEADER\0declare -rx READONLY_HEADER='Bearer {captured}'\n\0\0"
        );
        let rewrite = rewrite_snapshot_credentials(
            ShellType::Bash,
            &mut snapshot,
            SnapshotCredentialEnvironment {
                original: &original,
                restored: &restored,
                configured: &HashMap::new(),
                discovered: &allowed,
                allowed: &allowed,
                is_allowed_unset: &|_| false,
                brokered_keys: &keys,
                brokered_alias_keys: &[],
                allowed_brokered_keys: &keys,
            },
            |text| {
                *text = text
                    .replace(&format!("Bearer {real}"), &format!("Bearer {dummy}"))
                    .replace(&format!("Bearer {captured}"), &format!("Bearer {dummy}"));
                true
            },
        )
        .expect("captured dummy aliases should be rewritten");
        assert_eq!(
            rewrite.aliases,
            HashMap::from([
                ("AUTH_HEADER".to_string(), format!("Bearer {dummy}")),
                ("READONLY_HEADER".to_string(), format!("Bearer {dummy}")),
            ])
        );
        let replay = Command::new("/bin/bash")
            .args([
                "-c",
                &format!("{snapshot}\nprintf '%s\\n%s' \"$AUTH_HEADER\" \"$READONLY_HEADER\""),
            ])
            .env_clear()
            .env("VENDOR_PASSWORD", real)
            .output()?;
        assert!(replay.status.success(), "{replay:?}");
        assert_eq!(
            replay.stdout,
            format!("Bearer {real}\nBearer {real}").as_bytes()
        );
        if std::path::Path::new("/bin/zsh").exists() {
            let script = snapshot_capture_script(ShellType::Zsh, CAPTURE_NON_INTERACTIVE).unwrap();
            let capture_output = Command::new("/bin/zsh")
                .args(["-fc", &format!("readonly READONLY_HEADER\n{script}")])
                .env_clear()
                .envs(&original)
                .env("PATH", "/usr/bin:/bin")
                .output()?;
            assert!(capture_output.status.success(), "{capture_output:?}");
            let mut snapshot = String::from_utf8(capture_output.stdout)?;
            let rewritten = rewrite_snapshot_credentials(
                ShellType::Zsh,
                &mut snapshot,
                SnapshotCredentialEnvironment {
                    original: &original,
                    restored: &restored,
                    configured: &HashMap::new(),
                    discovered: &allowed,
                    allowed: &allowed,
                    is_allowed_unset: &|_| false,
                    brokered_keys: &keys,
                    brokered_alias_keys: &[],
                    allowed_brokered_keys: &keys,
                },
                |text| {
                    *text = text.replace(real, dummy).replace(captured, dummy);
                    true
                },
            )
            .expect("omitted readonly exports retain their alias metadata");
            assert_eq!(rewritten.aliases, rewrite.aliases);
            let replay = Command::new("/bin/zsh")
                .args(["-fc", &format!("readonly READONLY_HEADER\n{snapshot}\n[[ $parameters[READONLY_HEADER] == *readonly* ]] || exit 1\nprintf '%s' \"$READONLY_HEADER\"")])
                .env_clear()
                .env("VENDOR_PASSWORD", real)
                .env("READONLY_HEADER", format!("Bearer {real}"))
                .output()?;
            assert!(replay.status.success(), "{replay:?}");
            assert_eq!(replay.stdout, format!("Bearer {real}").as_bytes());
        }
    }
    Ok(())
}

#[test]
fn brokered_snapshot_rejects_shell_escaped_credentials() {
    let real = "token_ab'cdefgh";
    let dummy = "token_xy'zabcde";
    let mut snapshot = format!(
        "# Snapshot file\n# aliases 1\nalias helper='tool {escaped}'\n# exports 0\n",
        escaped = real.replace('\'', r#"'\''"#),
    );
    let original = snapshot.clone();

    let snapshot_is_safe = rewrite_snapshot_credentials(
        ShellType::Bash,
        &mut snapshot,
        empty_snapshot_credentials(&HashMap::new(), &|_| false),
        |text| {
            *text = text.replace(real, dummy);
            true
        },
    );

    assert!(snapshot_is_safe.is_none());
    assert_eq!(snapshot, original);
}

#[cfg(target_os = "macos")]
#[test]
fn zsh_snapshot_restores_tied_path() -> Result<()> {
    let dir = tempdir()?;
    let path_with_spaces = dir.path().join("path with spaces").join("bin");
    let plain_path = dir.path().join("plain-path").join("bin");
    let expected_path = format!(
        "{}:{}:/usr/bin:/bin",
        path_with_spaces.display(),
        plain_path.display()
    );
    let zshrc = format!(
        "export -UT PATH path=('{}' '{}' '{}' /usr/bin /bin)\n",
        path_with_spaces.display(),
        plain_path.display(),
        plain_path.display()
    );
    std::fs::write(dir.path().join(".zshrc"), zshrc)?;

    let snapshot = Command::new("/bin/zsh")
        .arg("-f")
        .arg("-c")
        .arg(snapshot_script(ShellType::Zsh).expect("zsh supports snapshots"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("ZDOTDIR", dir.path())
        .output()?;
    assert!(snapshot.status.success());

    let snapshot_path = dir.path().join("snapshot.sh");
    let snapshot = captured_script(ShellType::Zsh, &String::from_utf8(snapshot.stdout)?)?;
    std::fs::write(&snapshot_path, &snapshot)?;

    let restored = Command::new("/bin/zsh")
        .arg("-f")
        .arg("-c")
        .arg("set -e; . \"$1\"; print -r -- \"$PATH\"")
        .arg("zsh")
        .arg(&snapshot_path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()?;
    assert!(restored.status.success());
    assert_eq!(
        String::from_utf8(restored.stdout)?.trim_end(),
        expected_path
    );

    assert!(
        snapshot
            .lines()
            .any(|line| line.starts_with("export -UT PATH path=")),
        "snapshot should capture the tied PATH export"
    );

    std::fs::write(dir.path().join(".zshrc"), "readonly PATH\n")?;
    let readonly_snapshot = Command::new("/bin/zsh")
        .arg("-f")
        .arg("-c")
        .arg(snapshot_script(ShellType::Zsh).expect("zsh supports snapshots"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("ZDOTDIR", dir.path())
        .output()?;
    assert!(readonly_snapshot.status.success());
    let readonly_snapshot = captured_script(
        ShellType::Zsh,
        &String::from_utf8(readonly_snapshot.stdout)?,
    )?;
    std::fs::write(&snapshot_path, &readonly_snapshot)?;

    let readonly_restored = Command::new("/bin/zsh")
        .arg("-f")
        .arg("-c")
        .arg("set -e; . \"$1\"; export PATH='/codex-path':\"$PATH\"; print -r -- \"$PATH\"")
        .arg("zsh")
        .arg(&snapshot_path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()?;
    assert!(readonly_restored.status.success());
    assert_eq!(
        String::from_utf8(readonly_restored.stdout)?.trim_end(),
        "/codex-path:/usr/bin:/bin"
    );

    assert!(
        !readonly_snapshot
            .lines()
            .any(|line| line.starts_with("export -rT PATH path=")),
        "snapshot should not capture the readonly tied PATH export"
    );

    Ok(())
}

// Literal-only fixtures are shell source rather than capture output. Give those fixtures
// empty alias/export sections; real shell fixtures exercise the full framed capture.
fn rewrite_snapshot_credentials(
    shell_type: ShellType,
    snapshot: &mut String,
    environment: SnapshotCredentialEnvironment<'_>,
    virtualize_text: impl FnMut(&mut String) -> bool,
) -> Option<PreparedSnapshot> {
    let source_only = !snapshot.contains('\0');
    let framed = if source_only {
        format!("# Snapshot file\n{snapshot}\0\0\0")
    } else {
        snapshot.clone()
    };
    let capture = CapturedSnapshot::parse(shell_type, framed.as_bytes())?;
    let prepared = prepare_snapshot_credentials(&capture, environment, virtualize_text)?;
    *snapshot = if source_only {
        prepared
            .script
            .strip_prefix("# Snapshot file\n")?
            .to_string()
    } else {
        prepared.script.clone()
    };
    Some(prepared)
}

fn captured_script(shell_type: ShellType, source: &str) -> Result<String> {
    let captured =
        CapturedSnapshot::parse(shell_type, source.as_bytes()).context("invalid native capture")?;
    Ok(captured.render_script(&codex_protocol::config_types::ShellEnvironmentPolicy::default()))
}

#[test]
fn streamed_snapshot_preserves_alias_and_option_parse_order() -> Result<()> {
    for (shell, shell_type, options, expected) in [
        (
            "/bin/bash",
            ShellType::Bash,
            "shopt -s expand_aliases",
            "onetwohelper",
        ),
        (
            "/bin/zsh",
            ShellType::Zsh,
            "module_path=()\nsetopt RC_QUOTES",
            "one'twohelper",
        ),
        (
            "/bin/zsh",
            ShellType::Zsh,
            "module_path=()\nsetopt NO_RC_QUOTES",
            "onetwohelper",
        ),
    ] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        let dir = tempdir()?;
        let setup = format!(
            "{options}\nhelper() {{ printf helper; }}\nfunction : () {{ exit 41; }}\nalias snapshot_alias=\"printf 'one''two'\"\n"
        );
        let mut results = Vec::new();
        // Cover V1's native source, V2's source, and V2's environment fallback.
        for (source_capture, source_replay) in [(false, true), (true, true), (true, false)] {
            let capture = if source_capture {
                super::snapshot_source_capture_script(shell_type, CAPTURE_NON_INTERACTIVE)
            } else {
                snapshot_capture_script(shell_type, CAPTURE_NON_INTERACTIVE)
            }
            .unwrap();
            let shadow = if source_capture && shell_type == ShellType::Zsh {
                "alias alias=false\n"
            } else {
                ""
            };
            let output = Command::new(shell)
                .args(["-c", &format!("{setup}{shadow}{capture}")])
                .output()?;
            assert!(output.status.success(), "{shell}: {output:?}");
            let captured = CapturedSnapshot::parse(shell_type, &output.stdout).unwrap();
            let path = dir.path().join("state");
            std::fs::write(&path, captured.render_state())?;
            let restore = if source_replay {
                ". \"$1\""
            } else {
                "eval \"$(cat \"$1\")\""
            };
            let command =
                format!("{restore}\n\\alias snapshot_alias\neval snapshot_alias\nhelper\n");
            let output = Command::new(shell)
                .args(["-c", &command, "snapshot-test"])
                .arg(&path)
                .output()?;
            assert!(output.status.success(), "{shell}: {output:?}");
            assert!(String::from_utf8_lossy(&output.stdout).contains("snapshot_alias="));
            assert!(String::from_utf8_lossy(&output.stdout).ends_with(expected));
            results.push((output.stdout, output.stderr));
        }
        assert_eq!(results[0], results[1], "{shell}");
        assert_eq!(results[0], results[2], "{shell}");
    }
    Ok(())
}
