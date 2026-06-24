use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::ProcessId;
use crate::protocol::BashEnvSnapshotParams;
use crate::protocol::ExecEnvPolicy;

struct Fixture {
    _temp: tempfile::TempDir,
    workspace: PathBuf,
    bash_env: PathBuf,
    counter: PathBuf,
}

fn fixture() -> Fixture {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("create workspace");
    Fixture {
        bash_env: workspace.join("bash_env.sh"),
        counter: temp.path().join("counter"),
        workspace,
        _temp: temp,
    }
}

fn startup(fixture: &Fixture, sleep: bool) -> String {
    format!(
        "printf 'x\\n' >> '{counter}'\n\
         TOKEN=ready; SNAPSHOT_ARRAY=(zero one); INTEGER_ARRAY=(0012); declare -airx INTEGER_ARRAY; if (( BASH_VERSINFO[0] >= 4 )); then declare -A ASSOCIATIVE_ARRAY=([key]=assoc); fi; READONLY_TOKEN=locked; readonly READONLY_TOKEN\n\
         export SNAPSHOT_SENTINEL=ready OVERRIDE=from-startup\n\
         export PATH='{workspace}/bin':\"$PATH\"\n\
         snapshot_func() {{ printf %s \"$TOKEN\"; }}; export -f snapshot_func; readonly -f snapshot_func\n{sleep}\n",
        counter = fixture.counter.display(),
        workspace = fixture.workspace.display(),
        sleep = if sleep { "sleep 0.1" } else { ":" },
    )
}

fn capture_count(fixture: &Fixture) -> usize {
    fs::read_to_string(&fixture.counter)
        .expect("counter")
        .lines()
        .count()
}

fn params(fixture: &Fixture, script: &str) -> ExecParams {
    let workspace = PathUri::from_host_native_path(&fixture.workspace).expect("workspace URI");
    ExecParams {
        process_id: ProcessId::from("snapshot-test"),
        argv: vec![
            "/bin/bash".to_string(),
            "-c".to_string(),
            script.to_string(),
        ],
        cwd: workspace.clone(),
        env_policy: None,
        env: HashMap::from([
            (
                "BASH_ENV".to_string(),
                fixture.bash_env.to_string_lossy().into_owned(),
            ),
            (
                "PATH".to_string(),
                std::env::var("PATH").unwrap_or_default() + ":/usr/bin:/bin",
            ),
            ("BASH_FUNC_return%%".to_string(), "() { :; }".to_string()),
            ("OVERRIDE".to_string(), "explicit".to_string()),
        ]),
        tty: false,
        pipe_stdin: false,
        arg0: None,
        sandbox: None,
        enforce_managed_network: false,
        managed_network: None,
        bash_env_snapshot: Some(BashEnvSnapshotParams {
            workspace_root: workspace,
            preserve_env_keys: vec!["OVERRIDE".to_string()],
        }),
    }
}

async fn prepare(
    cache: &BashEnvSnapshotCache,
    params: &ExecParams,
) -> (Vec<String>, HashMap<String, String>) {
    cache
        .prepare_launch(params, &params.env, /*runtime_paths*/ None)
        .await
}

async fn assert_fallback(cache: &BashEnvSnapshotCache, params: ExecParams) {
    let prepared = prepare(cache, &params).await;
    assert_eq!(prepared, (params.argv, params.env));
}

async fn run(argv: &[String], env: &HashMap<String, String>, cwd: &Path) -> std::process::Output {
    tokio::process::Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(cwd)
        .env_clear()
        .envs(env)
        .output()
        .await
        .expect("run command")
}

fn snapshot_path(cache: &BashEnvSnapshotCache) -> PathBuf {
    fs::read_dir(cache.root.as_ref().expect("snapshot root").path())
        .expect("snapshot directory")
        .map(|entry| entry.expect("entry").path())
        .find(|path| path.extension().is_some_and(|ext| ext == "sh"))
        .expect("snapshot")
}

#[tokio::test]
async fn scopes_and_invalidates_cache_across_workspace_config_and_reconnect() {
    let fixture = fixture();
    fs::write(&fixture.bash_env, startup(&fixture, /*sleep*/ false)).expect("write BASH_ENV");
    let cache = BashEnvSnapshotCache::default();
    let base = params(&fixture, "true");
    prepare(&cache, &base).await;
    prepare(&cache, &base).await;
    assert_eq!(capture_count(&fixture), 1);
    let root = cache.root.as_ref().expect("snapshot root").path();
    assert_eq!(
        fs::metadata(root).expect("root metadata").mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(snapshot_path(&cache))
            .expect("metadata")
            .mode()
            & 0o777,
        0o600
    );
    let mut changed = base.clone();
    changed.env_policy = Some(ExecEnvPolicy {
        inherit: ShellEnvironmentPolicyInherit::All,
        ignore_default_excludes: false,
        exclude: Vec::new(),
        r#set: HashMap::from([
            ("SECOND".to_string(), "2".to_string()),
            ("FIRST".to_string(), "1".to_string()),
        ]),
        include_only: Vec::new(),
    });
    let mut equivalent = changed.clone();
    equivalent.env_policy.as_mut().expect("policy").r#set = HashMap::from([
        ("FIRST".to_string(), "1".to_string()),
        ("SECOND".to_string(), "2".to_string()),
    ]);
    assert_eq!(
        BashEnvSnapshotRequest::new(&changed, &changed.env)
            .expect("scope")
            .key,
        BashEnvSnapshotRequest::new(&equivalent, &equivalent.env)
            .expect("equivalent scope")
            .key,
    );
    prepare(&cache, &changed).await;
    fs::write(
        &fixture.bash_env,
        format!("{}# setup changed\n", startup(&fixture, /*sleep*/ false)),
    )
    .expect("change BASH_ENV");
    prepare(&cache, &base).await;
    let shell_dir = fixture._temp.path().join("shell");
    fs::create_dir(&shell_dir).expect("create shell dir");
    let alternate_shell = shell_dir.join("bash");
    symlink("/bin/bash", &alternate_shell).expect("link bash");
    changed = base.clone();
    changed.argv[0] = alternate_shell.to_string_lossy().into_owned();
    prepare(&cache, &changed).await;
    let second_workspace = fixture._temp.path().join("second");
    fs::create_dir(&second_workspace).expect("create second workspace");
    changed.cwd = PathUri::from_host_native_path(&second_workspace).expect("second URI");
    changed
        .bash_env_snapshot
        .as_mut()
        .expect("snapshot params")
        .workspace_root = changed.cwd.clone();
    prepare(&cache, &changed).await;
    cache.clear();
    prepare(&cache, &changed).await;
    assert_eq!(capture_count(&fixture), 6);
    let mut descendant = base.clone();
    let nested = fixture.workspace.join("nested");
    fs::create_dir(&nested).expect("create nested cwd");
    descendant.cwd = PathUri::from_host_native_path(&nested).expect("nested URI");
    prepare(&cache, &descendant).await;
    assert_eq!(capture_count(&fixture), 7);
    let mut outside = base;
    outside.cwd = PathUri::from_host_native_path(fixture._temp.path()).expect("outside URI");
    assert_fallback(&cache, outside).await;
    assert_eq!(capture_count(&fixture), 7);
}

#[tokio::test]
async fn failures_oversize_and_corruption_fall_back_to_bash_env() {
    let fixture = fixture();
    let cache = BashEnvSnapshotCache::default();
    fs::write(&fixture.bash_env, "exit 17\n").expect("write failure");
    let failure = params(&fixture, "true");
    assert_fallback(&cache, failure.clone()).await;
    let mut unsupported = failure.clone();
    unsupported.argv[0] = "/bin/sh".to_string();
    assert_fallback(&cache, unsupported).await;
    let mut oversized_env = failure.clone();
    oversized_env
        .env
        .insert("LARGE".to_string(), "x".repeat(MAX_ENV_BYTES + 1));
    assert_fallback(&cache, oversized_env).await;
    fs::write(
        &fixture.bash_env,
        format!("export HUGE='{}'\n", "x".repeat(MAX_SNAPSHOT_BYTES + 1024)),
    )
    .expect("write oversized BASH_ENV");
    let oversized = params(&fixture, "true");
    assert_fallback(&cache, oversized).await;
    let next_bash_env = fixture.workspace.join("next_bash_env.sh");
    fs::write(&next_bash_env, "").expect("write nested BASH_ENV");
    fs::write(
        &fixture.bash_env,
        format!(
            "{}set -a\nshopt -s expand_aliases\nalias snapshot_alias='printf alias-ok'\n\
             export BASH_ENV='{}'\n\
             declare() {{ return 91; }}; set() {{ return 91; }}; shopt() {{ return 91; }}; alias() {{ return 91; }}\n\
             unset() {{ return 91; }}; export() {{ return 91; }}; command() {{ return 91; }}\n\
             function [ {{ return 91; }}; read() {{ return 91; }}; true() {{ return 91; }}\n",
            startup(&fixture, /*sleep*/ false),
            next_bash_env.display(),
        ),
    )
    .expect("write valid BASH_ENV");
    let cache = BashEnvSnapshotCache::default();
    let mut no_checksum = params(&fixture, "true");
    no_checksum.env.extend([
        ("PATH".into(), fixture.workspace.display().to_string()),
        ("BASH_FUNC_sha256sum%%".into(), "() { :; }".into()),
    ]);
    assert_fallback(&cache, no_checksum).await;
    assert!(!fixture.counter.exists());
    let tools = fixture._temp.path().join("tools");
    fs::create_dir(&tools).expect("create tools");
    let digest_file = tools.join("digest");
    fs::write(
        tools.join("sha256sum"),
        format!(
            "#!/bin/sh\nIFS=' ' read -r digest remove < '{}'\nprintf '%s  %s\\n' \"$digest\" \"$1\"\ncase \"$remove\" in true) rm -f \"$1\";; esac\n",
            digest_file.display()
        ),
    )
    .expect("write fake sha256sum");
    fs::set_permissions(tools.join("sha256sum"), fs::Permissions::from_mode(0o755))
        .expect("make executable");
    let checksum_path = format!("{}:/usr/bin:/bin", tools.display());
    let write_digest = |path: &Path, remove: bool| {
        let digest = Sha256::digest(fs::read(path).expect("read snapshot"));
        fs::write(&digest_file, format!("{digest:x} {remove}\n")).expect("write digest");
    };
    let mut replay = params(
        &fixture,
        "printf '%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s' \"$SNAPSHOT_SENTINEL\" \"$(snapshot_func)\" \"${SNAPSHOT_ARRAY[1]}\" \"${INTEGER_ARRAY[0]}\" \"$(d=$(builtin declare -p INTEGER_ARRAY); f=${d#declare }; f=${f%% *}; [[ $f == *a* && $f == *i* && $f == *r* && $f == *x* ]] && builtin printf attrs)\" \"$(if (( BASH_VERSINFO[0] >= 4 )); then d=$(builtin declare -p ASSOCIATIVE_ARRAY); [[ ${ASSOCIATIVE_ARRAY[key]} == assoc && $d == declare\\ -A* ]] && builtin printf assoc; else builtin printf assoc; fi)\" \"$(builtin declare -p READONLY_TOKEN)\" \"$OVERRIDE\" \"${PATH%%:*}\" \"$(snapshot_alias)\" \"$(/bin/bash -c 'printf %s \"${TOKEN+x}\"')\" \"$(builtin compgen -e | grep -c '^__CODEX_SNAPSHOT_' || true)\" \"$(/bin/bash -c 'type -t snapshot_func')\" \"$(builtin declare -F | grep ' snapshot_func$')\"",
    );
    replay.env.insert("PATH".into(), checksum_path.clone());
    let (argv, env) = prepare(&cache, &replay).await;
    assert!(
        !fs::read_to_string(snapshot_path(&cache))
            .expect("read snapshot")
            .contains("__CODEX_SNAPSHOT_")
    );
    let path = snapshot_path(&cache);
    write_digest(&path, /*remove*/ false);
    let expected = format!(
        "ready|ready|one|0012|attrs|assoc|declare -r READONLY_TOKEN=\"locked\"|explicit|{}/bin|alias-ok||0|function|declare -frx snapshot_func",
        fixture.workspace.display()
    );
    let output = run(&argv, &env, &fixture.workspace).await;
    assert_eq!(String::from_utf8(output.stdout).expect("utf8"), expected);
    let mut corrupt = fs::read_to_string(&path).expect("read snapshot");
    let marker_end = corrupt.find('\n').expect("opening marker") + 1;
    corrupt.insert_str(marker_end, "builtin exit 0\n");
    fs::write(&path, corrupt).expect("corrupt snapshot while retaining markers");
    fs::write(&digest_file, "bad false\n").expect("write bad digest");
    let output = run(&argv, &env, &fixture.workspace).await;
    assert_eq!(String::from_utf8(output.stdout).expect("utf8"), expected);
    assert_eq!(capture_count(&fixture), 2);
    fs::write(&path, "").expect("truncate snapshot");
    let output = run(&argv, &env, &fixture.workspace).await;
    assert_eq!(String::from_utf8(output.stdout).expect("utf8"), expected);
    assert_eq!(capture_count(&fixture), 3);
    fs::write(&fixture.bash_env, startup(&fixture, /*sleep*/ false)).expect("write race BASH_ENV");
    let cache = BashEnvSnapshotCache::default();
    let mut race_params = params(&fixture, "builtin printf %s \"$SNAPSHOT_SENTINEL\"");
    race_params.env.insert("PATH".into(), checksum_path.clone());
    let (argv, env) = prepare(&cache, &race_params).await;
    let path = snapshot_path(&cache);
    write_digest(&path, /*remove*/ true);
    let output = run(&argv, &env, &fixture.workspace).await;
    assert_eq!(String::from_utf8(output.stdout).expect("utf8"), "ready");
    assert!(!path.exists());
    fs::write(
        &fixture.bash_env,
        format!("{}unset BASH_ENV\n", startup(&fixture, /*sleep*/ false)),
    )
    .expect("write unsetting BASH_ENV");
    let cache = BashEnvSnapshotCache::default();
    let mut unset_params = params(
        &fixture,
        "printf '%s|%s|%s' \"${BASH_ENV+x}\" \"$SNAPSHOT_SENTINEL\" \"$(/bin/bash -c 'printf %s \"${BASH_ENV+x}\"')\"",
    );
    unset_params.env.insert("PATH".into(), checksum_path);
    let (argv, env) = prepare(&cache, &unset_params).await;
    let path = snapshot_path(&cache);
    write_digest(&path, /*remove*/ false);
    assert_eq!(
        String::from_utf8(run(&argv, &env, &fixture.workspace).await.stdout).expect("utf8"),
        "|ready|"
    );
    fs::write(&path, "").expect("truncate snapshot");
    fs::write(&digest_file, "bad false\n").expect("write bad digest");
    assert_eq!(
        String::from_utf8(run(&argv, &env, &fixture.workspace).await.stdout).expect("utf8"),
        "|ready|"
    );
}

#[tokio::test]
async fn command_dependent_bash_env_falls_back_with_original_execution_string() {
    let fixture = fixture();
    fs::write(
        &fixture.bash_env,
        format!(
            "printf 'x\\n' >> '{}'; SEEN=$BASH_EXECUTION_STRING\n",
            fixture.counter.display()
        ),
    )
    .expect("write command-dependent BASH_ENV");
    let cache = BashEnvSnapshotCache::default();
    let command_params = params(&fixture, "printf '%s' \"$SEEN\"");
    for _ in 0..2 {
        let (argv, env) = prepare(&cache, &command_params).await;
        assert_eq!((&argv, &env), (&command_params.argv, &command_params.env));
        let output = run(&argv, &env, &fixture.workspace).await;
        assert_eq!(
            String::from_utf8(output.stdout).expect("utf8"),
            command_params.argv[2]
        );
    }
    assert_eq!(capture_count(&fixture), 2);
    fs::write(
        &fixture.bash_env,
        format!(
            "trap 'printf \"x\\n\" >> \"{}\"; builtin printf trapped' EXIT\n",
            fixture.counter.display()
        ),
    )
    .expect("write trap BASH_ENV");
    let trap_params = params(&fixture, "builtin printf command");
    let (argv, env) = prepare(&cache, &trap_params).await;
    assert_eq!((&argv, &env), (&trap_params.argv, &trap_params.env));
    let output = run(&argv, &env, &fixture.workspace).await;
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf8"),
        "commandtrapped"
    );
    assert_eq!(capture_count(&fixture), 3);
    for option in ["x", "v"] {
        fs::write(&fixture.bash_env, format!("set -{option}\n")).expect("write traced BASH_ENV");
        let mut traced = params(&fixture, "true");
        traced
            .env
            .insert("OVERRIDE".into(), "explicit-secret".into());
        let (argv, env) = prepare(&cache, &traced).await;
        assert_eq!((&argv, &env), (&traced.argv, &traced.env));
        let stderr = String::from_utf8(run(&argv, &env, &fixture.workspace).await.stderr)
            .expect("utf8 stderr");
        assert!(!stderr.contains("__CODEX") && !stderr.contains("explicit-secret"));
    }
    fs::write(&fixture.bash_env, "function builtin { :; }\n").expect("write shadow BASH_ENV");
    let direct = params(&fixture, "true");
    assert_eq!(prepare(&cache, &direct).await, (direct.argv, direct.env));
    fs::write(&fixture.bash_env, ":\n").expect("write plain BASH_ENV");
    for key in ["BASH_FUNC_builtin%%", "builtin"] {
        let mut inherited = params(&fixture, "true");
        inherited
            .env
            .insert(key.to_string(), "() { :; }".to_string());
        assert_eq!(
            prepare(&cache, &inherited).await,
            (inherited.argv, inherited.env)
        );
    }
}
#[tokio::test]
async fn replays_observable_startup_state() {
    let fixture = fixture();
    let nested = fixture.workspace.join("nested");
    fs::create_dir(&nested).expect("create nested cwd");
    fs::write(
        &fixture.bash_env,
        format!(
            "printf 'x\\n' >> '{counter}'; printf startup-out; printf startup-err >&2\n\
             if [[ ${{CWD_GUARD+x}} ]]; then export CWD_DIRTY=yes; else CWD_GUARD=fresh; readonly CWD_GUARD; fi\n\
             umask 077; cd -L -- '{nested}' 2>/dev/null; set +B\n",
            counter = fixture.counter.display(),
            nested = nested.display(),
        ),
    )
    .expect("write BASH_ENV");
    let script = "builtin printf '|%s|%s|<%s>|%s|%s|%s' \"$(builtin umask)\" \"$PWD\" {a,b} \"$CWD_GUARD\" \"${CWD_DIRTY-}\" \"$BASH_EXECUTION_STRING\"";
    let params = params(&fixture, script);
    let cache = BashEnvSnapshotCache::default();
    let expected = |cwd: &Path| {
        format!(
            "startup-out|0077|{}|<{{a,b}}>|fresh||{script}",
            cwd.display()
        )
    };
    for _ in 0..2 {
        let (argv, env) = prepare(&cache, &params).await;
        let output = run(&argv, &env, &fixture.workspace).await;
        assert_eq!(
            String::from_utf8(output.stdout).expect("utf8"),
            expected(&nested)
        );
        assert_eq!(output.stderr, b"startup-err");
    }
    assert_eq!(capture_count(&fixture), 1);
    fs::remove_dir(&nested).expect("remove nested cwd");
    let (argv, env) = prepare(&cache, &params).await;
    let output = run(&argv, &env, &fixture.workspace).await;
    let fallback_cwd = fs::canonicalize(&fixture.workspace).expect("canonical cwd");
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf8"),
        expected(&fallback_cwd)
    );
    assert_eq!(output.stderr, b"startup-err");
    assert_eq!(capture_count(&fixture), 2);
}

#[tokio::test]
async fn concurrent_requests_capture_once() {
    let fixture = fixture();
    fs::write(&fixture.bash_env, startup(&fixture, /*sleep*/ true)).expect("write BASH_ENV");
    let cache = Arc::new(BashEnvSnapshotCache::default());
    let params = params(&fixture, "true");
    let tasks = (0..8).map(|_| {
        let (cache, params) = (Arc::clone(&cache), params.clone());
        tokio::spawn(async move { prepare(&cache, &params).await })
    });
    for result in futures::future::join_all(tasks).await {
        result.expect("capture task");
    }
    assert_eq!(capture_count(&fixture), 1);
    prepare(&BashEnvSnapshotCache::default(), &params).await;
    assert_eq!(capture_count(&fixture), 2);
}
