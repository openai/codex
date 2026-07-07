use super::*;
use crate::git_command::GitRunner;
use crate::safe_git::isolate_git_command_environment;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;

fn run_git(cwd: &Path, args: &[&str]) {
    let output = git_output(cwd, args);
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output(cwd: &Path, args: &[&str]) -> std::process::Output {
    let mut command = std::process::Command::new("git");
    isolate_git_command_environment(&mut command);
    command.current_dir(cwd).args(args).output().expect("Git")
}

fn config_file_value(_cwd: &Path, config: &Path, key: &str) -> Option<String> {
    let mut command = std::process::Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command
        .current_dir(config.parent().expect("config parent"))
        .args(["config", "--file"])
        .arg(config)
        .args(["--get", key])
        .output()
        .expect("read config file");
    if output.status.code() == Some(1) {
        return None;
    }
    assert!(
        output.status.success(),
        "read {key:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_isolated_overlay_test(test_name: &str, env: &[(&str, &OsStr)]) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    isolate_git_command_environment(&mut command);
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_MERGE_OVERLAY_CHILD", "1")
        .env("RUST_TEST_THREADS", "1");
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("run isolated overlay test");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn unspecified_safe_attributes() -> SafeApplyAttributes {
    SafeApplyAttributes {
        whitespace: ProjectedAttribute::Unspecified,
        conflict_marker_size: ProjectedAttribute::Unspecified,
        text: ProjectedAttribute::Unspecified,
        crlf: ProjectedAttribute::Unspecified,
        eol: ProjectedAttribute::Unspecified,
        ident: ProjectedAttribute::Unspecified,
        working_tree_encoding: ProjectedAttribute::Unspecified,
        _filter: ProjectedAttribute::Unspecified,
    }
}

fn frozen_path(merge: BuiltinMergeDriver) -> FrozenPathApplyAttributes {
    FrozenPathApplyAttributes {
        merge: MergeSelection::Builtin(merge),
        safe: unspecified_safe_attributes(),
    }
}

fn complete_attribute_output(path: &str, overrides: &[(&str, &[u8])]) -> Vec<u8> {
    let names = [
        "merge",
        "whitespace",
        "conflict-marker-size",
        "text",
        "crlf",
        "eol",
        "ident",
        "filter",
        "working-tree-encoding",
    ];
    let mut output = Vec::new();
    for name in names {
        let value = overrides
            .iter()
            .find_map(|(candidate, value)| (*candidate == name).then_some(*value))
            .unwrap_or(b"unspecified");
        output.extend_from_slice(path.as_bytes());
        output.push(0);
        output.extend_from_slice(name.as_bytes());
        output.push(0);
        output.extend_from_slice(value);
        output.push(0);
    }
    output
}

#[test]
fn index_stage_snapshot_parser_enforces_canonical_framing_and_format() {
    let paths = ["file.txt".to_string()];
    let sha1 = "1".repeat(40);
    let sha256 = "a".repeat(64);
    let sha1_record = format!("100644 {sha1} 0\tfile.txt\0");
    let sha256_record = format!("100644 {sha256} 0\tfile.txt\0");

    assert_eq!(
        parse_index_stage_snapshot(&[], &paths, IndexObjectFormat::Sha1).expect("empty snapshot"),
        IndexStageSnapshot::new()
    );
    assert!(
        parse_index_stage_snapshot(sha1_record.as_bytes(), &paths, IndexObjectFormat::Sha1).is_ok()
    );
    assert!(
        parse_index_stage_snapshot(sha256_record.as_bytes(), &paths, IndexObjectFormat::Sha256)
            .is_ok()
    );
    assert!(
        parse_index_stage_snapshot(sha1_record.as_bytes(), &paths, IndexObjectFormat::Sha256)
            .is_err()
    );
    assert!(
        parse_index_stage_snapshot(sha256_record.as_bytes(), &paths, IndexObjectFormat::Sha1)
            .is_err()
    );
    let tab_path = ["tab\tpath.txt".to_string()];
    let tab_record = format!("100644 {sha1} 0\ttab\tpath.txt\0");
    assert!(
        parse_index_stage_snapshot(tab_record.as_bytes(), &tab_path, IndexObjectFormat::Sha1)
            .is_ok()
    );

    let uppercase = format!("100644 {} 0\tfile.txt\0", "A".repeat(40));
    let zero_stage_zero = format!("100644 {} 0\tfile.txt\0", "0".repeat(40));
    let lone_unmerged = format!("100644 {sha1} 2\tfile.txt\0");
    let stage_zero_and_one = format!("100644 {sha1} 0\tfile.txt\0100644 {sha1} 1\tfile.txt\0");
    let malformed = [
        b"\0".to_vec(),
        format!("\0{sha1_record}").into_bytes(),
        format!("{sha1_record}\0").into_bytes(),
        sha1_record.trim_end_matches('\0').as_bytes().to_vec(),
        format!("100644  {sha1} 0\tfile.txt\0").into_bytes(),
        format!("100644 {sha1}  0\tfile.txt\0").into_bytes(),
        format!("10064 {sha1} 0\tfile.txt\0").into_bytes(),
        format!("100600 {sha1} 0\tfile.txt\0").into_bytes(),
        format!("100644 {sha1} 4\tfile.txt\0").into_bytes(),
        format!("100644 {sha1} 0\tfile.txt\textra\0").into_bytes(),
        uppercase.into_bytes(),
        zero_stage_zero.into_bytes(),
        lone_unmerged.into_bytes(),
        stage_zero_and_one.into_bytes(),
    ];
    for value in malformed {
        assert!(
            parse_index_stage_snapshot(&value, &paths, IndexObjectFormat::Sha1).is_err(),
            "accepted malformed stage output {value:?}"
        );
    }
}

#[test]
fn exit_one_completion_witness_requires_new_noncustom_unmerged_state() {
    fn entry(byte: u8) -> IndexStageEntry {
        IndexStageEntry {
            mode: 0o100644,
            object_id: IndexObjectId::Sha1([byte; 20]),
        }
    }

    let custom = BTreeSet::from(["custom.txt".to_string()]);
    let clean_before = BTreeMap::from([(
        "peer.txt".to_string(),
        BTreeMap::from([(0, entry(/*byte*/ 1))]),
    )]);
    let newly_conflicted = BTreeMap::from([(
        "peer.txt".to_string(),
        BTreeMap::from([
            (1, entry(/*byte*/ 1)),
            (2, entry(/*byte*/ 2)),
            (3, entry(/*byte*/ 3)),
        ]),
    )]);
    assert!(newly_unmerged_noncustom_path(
        &clean_before,
        &newly_conflicted,
        &custom
    ));

    let already_unmerged = BTreeMap::from([(
        "peer.txt".to_string(),
        BTreeMap::from([
            (1, entry(/*byte*/ 1)),
            (2, entry(/*byte*/ 2)),
            (3, entry(/*byte*/ 3)),
        ]),
    )]);
    let changed_unmerged = BTreeMap::from([(
        "peer.txt".to_string(),
        BTreeMap::from([
            (1, entry(/*byte*/ 4)),
            (2, entry(/*byte*/ 5)),
            (3, entry(/*byte*/ 6)),
        ]),
    )]);
    assert!(!newly_unmerged_noncustom_path(
        &already_unmerged,
        &changed_unmerged,
        &custom
    ));

    let custom_conflict = BTreeMap::from([(
        "custom.txt".to_string(),
        BTreeMap::from([
            (1, entry(/*byte*/ 1)),
            (2, entry(/*byte*/ 2)),
            (3, entry(/*byte*/ 3)),
        ]),
    )]);
    assert!(!newly_unmerged_noncustom_path(
        &IndexStageSnapshot::new(),
        &custom_conflict,
        &custom
    ));
}

#[test]
fn sealed_merge_override_rejects_another_operation_identity() {
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    run_git(repo.path(), &["config", "merge.unused.driver", "false"]);
    let first_git = GitRunner::for_cwd_io(repo.path()).expect("first runner");
    let second_git = GitRunner::for_cwd_io(repo.path()).expect("second runner");
    let mut first =
        GuardedGitConfig::authorize(&first_git, repo.path(), Vec::new()).expect("first config");
    let mut second =
        GuardedGitConfig::authorize(&second_git, repo.path(), Vec::new()).expect("second config");
    first
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("first apply snapshot");
    second
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("second apply snapshot");

    let snapshot = first.read_merge_config_snapshot().expect("merge snapshot");
    let attributes = MergeAttributeSnapshot::from_effective(
        &first.identity,
        &snapshot,
        BTreeMap::from([(
            "file.txt".to_string(),
            ParsedPathApplyAttributes {
                merge: "unspecified".to_string(),
                merge_sentinel: None,
                safe: unspecified_safe_attributes(),
            },
        )]),
        &BTreeMap::new(),
    )
    .expect("merge attribute snapshot");
    let neutralizer = first
        .build_merge_override(
            &snapshot,
            &attributes,
            vec!["file.txt".to_string()],
            BTreeMap::new(),
            BTreeSet::new(),
            BTreeMap::new(),
        )
        .expect("build isolated merge config");
    let error = second
        .attach_merge_override(neutralizer)
        .expect_err("cross-operation override must refuse");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn merge_config_snapshot_tracks_namespaces_and_sticky_conditional_invalidity() {
    let owner = Arc::new(CapabilityIdentity);
    let snapshot = MergeConfigSnapshot::from_records(
        &owner,
        vec![
            MergeConfigRecord {
                key: "merge.default".to_string(),
                value: GitConfigValue::Explicit("union".to_string()),
            },
            MergeConfigRecord {
                key: "merge.demo.name".to_string(),
                value: GitConfigValue::Implicit,
            },
            MergeConfigRecord {
                key: "merge.demo.name".to_string(),
                value: GitConfigValue::Explicit("later display name".to_string()),
            },
            MergeConfigRecord {
                key: "merge.dotted.name.unknown".to_string(),
                value: GitConfigValue::Implicit,
            },
            MergeConfigRecord {
                key: "merge..unknown".to_string(),
                value: GitConfigValue::Implicit,
            },
            MergeConfigRecord {
                key: "merge.default".to_string(),
                value: GitConfigValue::Implicit,
            },
            MergeConfigRecord {
                key: "merge.default".to_string(),
                value: GitConfigValue::Explicit("binary".to_string()),
            },
        ],
    )
    .expect("merge config snapshot");

    assert_eq!(snapshot.default_driver(), Some("binary"));
    assert_eq!(
        snapshot.namespaces(),
        &BTreeSet::from([String::new(), "demo".to_string(), "dotted.name".to_string(),])
    );
    assert!(
        snapshot.conditional_invalid(),
        "a later explicit value must not cure an earlier implicit known value"
    );

    let unknown_only = MergeConfigSnapshot::from_records(
        &owner,
        vec![MergeConfigRecord {
            key: "merge.unknown.arbitrary".to_string(),
            value: GitConfigValue::Implicit,
        }],
    )
    .expect("implicit unknown snapshot");
    assert_eq!(
        unknown_only.namespaces(),
        &BTreeSet::from(["unknown".to_string()])
    );
    assert!(!unknown_only.conditional_invalid());
}

#[test]
fn conditional_invalid_merge_marker_is_fixed_and_valueless() {
    let directory = tempfile::tempdir().expect("config directory");
    let config = directory.path().join("config");
    std::fs::write(&config, b"[core]\n\tbare = false\n").expect("write base config");

    append_conditional_invalid_merge_config(&config).expect("append fixed marker");

    assert_eq!(
        std::fs::read(&config).expect("read marker config"),
        b"[core]\n\tbare = false\n\n[merge \"codex-conditional-invalid\"]\n\tdriver\n"
    );
}

#[test]
fn projected_merge_attribute_patterns_are_rooted_and_literal() {
    let target = "dir/literal *?[]\"quote\"\\slash\tline\n.txt".to_string();
    #[cfg(not(windows))]
    let wildcard_decoy = target.replacen('*', "X", 1);
    #[cfg(not(windows))]
    let nested_decoy = format!("nested/{target}");
    #[cfg(not(windows))]
    let projected_target = target.clone();
    #[cfg(windows)]
    let projected_target = target;
    let projection = projected_merge_attributes(&BTreeMap::from([(
        projected_target,
        frozen_path(BuiltinMergeDriver::Union),
    )]))
    .expect("literal projection");
    #[cfg(not(windows))]
    let projection_for_git = projection.clone();
    let rendered = String::from_utf8(projection).expect("UTF-8 projection");
    assert!(rendered.starts_with("\"/dir/"), "{rendered:?}");
    assert!(rendered.contains("\\\\*"), "{rendered:?}");
    assert!(rendered.contains("\\\\?"), "{rendered:?}");
    assert!(rendered.contains("\\\\["), "{rendered:?}");
    assert!(rendered.contains("\\\\]"), "{rendered:?}");
    assert!(rendered.contains("\\\"quote\\\""), "{rendered:?}");
    assert!(rendered.contains("\\\\\\\\slash"), "{rendered:?}");
    assert!(rendered.contains("\\tline\\n.txt"), "{rendered:?}");

    #[cfg(not(windows))]
    {
        let repo = tempfile::tempdir().expect("repo");
        run_git(repo.path(), &["init", "-q"]);
        std::fs::write(repo.path().join(".git/info/attributes"), projection_for_git)
            .expect("write projected attributes");
        let output = git_output(
            repo.path(),
            &[
                "check-attr",
                "-z",
                "merge",
                "--",
                &target,
                &wildcard_decoy,
                &nested_decoy,
            ],
        );
        assert!(
            output.status.success(),
            "check projected attributes: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let paths = vec![target.clone(), wildcard_decoy.clone(), nested_decoy.clone()];
        let parsed = crate::merge_driver::parse_merge_attributes(&output.stdout, &paths)
            .expect("parse projected attributes");
        assert_eq!(parsed.get(&target).map(String::as_str), Some("union"));
        assert_eq!(
            parsed.get(&wildcard_decoy).map(String::as_str),
            Some("unspecified")
        );
        assert_eq!(
            parsed.get(&nested_decoy).map(String::as_str),
            Some("unspecified")
        );
    }
}

#[test]
fn projected_whitespace_states_mask_lower_sources() {
    let paths = ["set.txt", "unset.txt", "unspecified.txt", "value.txt"].map(str::to_string);
    let whitespace = [
        ProjectedAttribute::Set,
        ProjectedAttribute::Unset,
        ProjectedAttribute::Unspecified,
        ProjectedAttribute::Value(b"indent-with-non-tab,tabwidth=4".to_vec()),
    ];
    let attributes = paths
        .iter()
        .cloned()
        .zip(whitespace.iter().cloned())
        .map(|(path, whitespace)| {
            let mut attributes = frozen_path(BuiltinMergeDriver::Text);
            attributes.safe.whitespace = whitespace;
            (path, attributes)
        })
        .collect::<BTreeMap<_, _>>();
    let projection =
        projected_merge_attributes(&attributes).expect("project all whitespace states");
    let rendered = String::from_utf8(projection.clone()).expect("UTF-8 projection");
    assert!(rendered.contains("/set.txt\" merge=text !filter whitespace "));
    assert!(rendered.contains("/unset.txt\" merge=text !filter -whitespace "));
    assert!(rendered.contains("/unspecified.txt\" merge=text !filter !whitespace "));
    assert!(
        rendered
            .contains("/value.txt\" merge=text !filter whitespace=indent-with-non-tab,tabwidth=4 ")
    );

    let repo = tempfile::tempdir().expect("repo");
    let root = repo.path();
    run_git(root, &["init", "-q"]);
    let lower = tempfile::NamedTempFile::new().expect("core attributes file");
    std::fs::write(
        lower.path(),
        "set.txt -whitespace\nunset.txt whitespace\nunspecified.txt -whitespace\nvalue.txt whitespace\n",
    )
    .expect("write lower core attributes");
    run_git(
        root,
        &[
            "config",
            "core.attributesFile",
            lower.path().to_str().expect("UTF-8 attributes path"),
        ],
    );
    std::fs::write(
        root.join(".gitattributes"),
        "set.txt -whitespace\nunset.txt whitespace\nunspecified.txt whitespace\nvalue.txt -whitespace\n",
    )
    .expect("write lower worktree attributes");
    std::fs::write(root.join(".git/info/attributes"), projection)
        .expect("write highest-precedence projection");

    let output = git_output(
        root,
        &[
            "check-attr",
            "-z",
            "merge",
            "whitespace",
            "conflict-marker-size",
            "text",
            "crlf",
            "eol",
            "ident",
            "filter",
            "working-tree-encoding",
            "--",
            &paths[0],
            &paths[1],
            &paths[2],
            &paths[3],
        ],
    );
    assert!(
        output.status.success(),
        "check projected attributes: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual =
        parse_three_way_attributes(&output.stdout, &paths).expect("parse projected attributes");
    assert!(actual.values().all(|attributes| attributes.merge == "text"));
    assert_eq!(
        actual
            .values()
            .map(|attributes| attributes.safe.whitespace.clone())
            .collect::<Vec<_>>(),
        vec![
            ProjectedAttribute::Ambiguous(AttributeSentinel::Set),
            ProjectedAttribute::Ambiguous(AttributeSentinel::Unset),
            ProjectedAttribute::Ambiguous(AttributeSentinel::Unspecified),
            whitespace[3].clone(),
        ]
    );
    assert!(actual.values().all(|attributes| {
        attributes.safe._filter == ProjectedAttribute::Ambiguous(AttributeSentinel::Unspecified)
    }));
}

#[test]
fn three_way_attribute_parser_rejects_malformed_or_unrepresentable_values() {
    let paths = vec!["file.txt".to_string()];
    let valid = complete_attribute_output(
        "file.txt",
        &[
            ("whitespace", b"native-unknown,,tabwidth=4junk"),
            ("conflict-marker-size", b"12"),
            ("eol", b"crlf"),
            ("working-tree-encoding", b"UTF-16LE"),
            ("filter", b"selected-but-never-projected"),
        ],
    );
    let attributes = parse_three_way_attributes(&valid, &paths).expect("valid attribute snapshot");
    let safe = &attributes.get("file.txt").expect("file attributes").safe;
    assert_eq!(
        safe.whitespace,
        ProjectedAttribute::Value(b"native-unknown,,tabwidth=4junk".to_vec())
    );
    assert_eq!(
        safe.conflict_marker_size,
        ProjectedAttribute::Value(b"12".to_vec())
    );
    assert_eq!(
        safe._filter,
        ProjectedAttribute::Value(b"selected-but-never-projected".to_vec())
    );
    assert_eq!(
        ProjectedAttribute::from_check_attr(b"").expect("native empty value"),
        ProjectedAttribute::Value(Vec::new())
    );
    assert_eq!(
        ProjectedAttribute::Value(Vec::new()).projected_token("whitespace"),
        b"whitespace="
    );

    let mut duplicate = valid.clone();
    duplicate.extend_from_slice(b"file.txt\0text\0set\0");
    let mut unexpected_name = valid.clone();
    let name_start = unexpected_name
        .windows(b"whitespace".len())
        .position(|window| window == b"whitespace")
        .expect("whitespace name");
    unexpected_name.splice(
        name_start..name_start + b"whitespace".len(),
        b"other".iter().copied(),
    );
    let mut unrepresentable = valid;
    let value_start = unrepresentable
        .windows(b"native-unknown,,tabwidth=4junk".len())
        .position(|window| window == b"native-unknown,,tabwidth=4junk")
        .expect("whitespace value");
    unrepresentable[value_start] = b' ';
    for malformed in [
        duplicate,
        unexpected_name,
        complete_attribute_output("other.txt", &[]),
        complete_attribute_output("file.txt", &[])[..20].to_vec(),
        unrepresentable,
    ] {
        assert!(
            parse_three_way_attributes(&malformed, &paths).is_err(),
            "accepted malformed attribute output {malformed:?}"
        );
    }
}

#[test]
fn projected_safe_attributes_preserve_raw_values_and_neutralize_filter() {
    let mut attributes = frozen_path(BuiltinMergeDriver::Union);
    attributes.safe = SafeApplyAttributes {
        whitespace: ProjectedAttribute::Value(b"unknown,,tabwidth=4junk".to_vec()),
        conflict_marker_size: ProjectedAttribute::Value(b"12".to_vec()),
        text: ProjectedAttribute::Set,
        crlf: ProjectedAttribute::Unset,
        eol: ProjectedAttribute::Value(b"crlf".to_vec()),
        ident: ProjectedAttribute::Set,
        working_tree_encoding: ProjectedAttribute::Value(b"UTF-16LE".to_vec()),
        _filter: ProjectedAttribute::Value(b"executable".to_vec()),
    };
    let projection =
        projected_merge_attributes(&BTreeMap::from([("file.txt".to_string(), attributes)]))
            .expect("project safe attributes");
    let rendered = String::from_utf8(projection).expect("ASCII projection");
    assert!(rendered.contains("merge=union"), "{rendered}");
    assert!(rendered.contains("!filter"), "{rendered}");
    assert!(!rendered.contains("filter=executable"), "{rendered}");
    assert!(
        rendered.contains("whitespace=unknown,,tabwidth=4junk"),
        "{rendered}"
    );
    assert!(rendered.contains("conflict-marker-size=12"), "{rendered}");
    assert!(rendered.contains(" text "), "{rendered}");
    assert!(rendered.contains(" -crlf "), "{rendered}");
    assert!(rendered.contains(" eol=crlf "), "{rendered}");
    assert!(rendered.contains(" ident "), "{rendered}");
    assert!(
        rendered.contains("working-tree-encoding=UTF-16LE"),
        "{rendered}"
    );
}

#[test]
fn projected_attribute_lines_split_below_git_limit_or_fail_closed() {
    let mut split = frozen_path(BuiltinMergeDriver::Text);
    split.safe.whitespace = ProjectedAttribute::Value(vec![b'w'; 1_500]);
    split.safe.conflict_marker_size = ProjectedAttribute::Value(vec![b'1'; 1_500]);
    let projection = projected_merge_attributes(&BTreeMap::from([("file.txt".to_string(), split)]))
        .expect("split long projection");
    let lines = projection
        .strip_suffix(b"\n")
        .expect("terminated projection")
        .split(|byte| *byte == b'\n')
        .collect::<Vec<_>>();
    assert!(lines.len() >= 2, "projection did not split");
    assert!(
        lines
            .iter()
            .all(|line| line.len() < GIT_ATTRIBUTE_LINE_LENGTH_LIMIT),
        "oversized projected line"
    );

    let mut too_long = frozen_path(BuiltinMergeDriver::Text);
    too_long.safe.whitespace = ProjectedAttribute::Value(vec![b'w'; 2_048]);
    assert_eq!(
        projected_merge_attributes(&BTreeMap::from([("file.txt".to_string(), too_long,)]))
            .expect_err("reject unprojectable value")
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn native_check_attr_marks_conflated_sentinels_unresolved_for_every_projected_name() {
    let repo = tempfile::tempdir().expect("repo");
    let root = repo.path();
    run_git(root, &["init", "-q"]);
    let names = [
        "merge",
        "whitespace",
        "conflict-marker-size",
        "text",
        "crlf",
        "eol",
        "ident",
        "filter",
        "working-tree-encoding",
    ];
    let bare_set = names.join(" ");
    let literal_set = names
        .iter()
        .map(|name| format!("{name}=set"))
        .collect::<Vec<_>>()
        .join(" ");
    let bare_unset = names
        .iter()
        .map(|name| format!("-{name}"))
        .collect::<Vec<_>>()
        .join(" ");
    let literal_unset = names
        .iter()
        .map(|name| format!("{name}=unset"))
        .collect::<Vec<_>>()
        .join(" ");
    let bare_unspecified = names
        .iter()
        .map(|name| format!("!{name}"))
        .collect::<Vec<_>>()
        .join(" ");
    let literal_unspecified = names
        .iter()
        .map(|name| format!("{name}=unspecified"))
        .collect::<Vec<_>>()
        .join(" ");
    std::fs::write(
        root.join(".gitattributes"),
        format!(
            "bare-set {bare_set}\nliteral-set {literal_set}\nbare-unset {bare_unset}\nliteral-unset {literal_unset}\nbare-unspecified {bare_unspecified}\nliteral-unspecified {literal_unspecified}\n"
        ),
    )
    .expect("write sentinel fixtures");
    let paths = [
        "bare-set",
        "literal-set",
        "bare-unset",
        "literal-unset",
        "bare-unspecified",
        "literal-unspecified",
    ];
    let mut args = vec!["check-attr", "-z"];
    args.extend(names);
    args.push("--");
    args.extend(paths);
    let output = git_output(root, &args);
    assert!(
        output.status.success(),
        "check sentinel attributes: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let fields = output.stdout.split(|byte| *byte == 0).collect::<Vec<_>>();
    assert_eq!(fields.len(), paths.len() * names.len() * 3 + 1);
    let values = fields[..fields.len() - 1]
        .chunks_exact(3)
        .map(|record| record[2])
        .collect::<Vec<_>>();
    for (path_index, expected) in [
        b"set".as_slice(),
        b"set".as_slice(),
        b"unset".as_slice(),
        b"unset".as_slice(),
        b"unspecified".as_slice(),
        b"unspecified".as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            values[path_index * names.len()..(path_index + 1) * names.len()]
                .iter()
                .all(|actual| *actual == expected),
            "path {} did not render as {:?}",
            paths[path_index],
            String::from_utf8_lossy(expected)
        );
    }

    let expected_paths = paths.map(str::to_string).to_vec();
    let parsed = parse_three_way_attributes(&output.stdout, &expected_paths)
        .expect("parse conventional sentinel rendering");
    assert_eq!(
        parsed["literal-set"].safe.text,
        ProjectedAttribute::Ambiguous(AttributeSentinel::Set)
    );
    assert_eq!(
        parsed["literal-unset"].safe.ident,
        ProjectedAttribute::Ambiguous(AttributeSentinel::Unset)
    );
    assert_eq!(
        parsed["literal-unspecified"].safe._filter,
        ProjectedAttribute::Ambiguous(AttributeSentinel::Unspecified)
    );
    assert_eq!(
        parsed["literal-set"].merge_sentinel,
        Some(AttributeSentinel::Set)
    );
}

#[test]
fn merge_attribute_snapshot_normalizes_only_builtin_semantics() {
    for (attribute, default, expected) in [
        ("set", "union", BuiltinMergeDriver::Text),
        ("text", "binary", BuiltinMergeDriver::Text),
        ("unset", "text", BuiltinMergeDriver::Binary),
        ("binary", "union", BuiltinMergeDriver::Binary),
        ("union", "binary", BuiltinMergeDriver::Union),
        ("missing-custom", "union", BuiltinMergeDriver::Text),
        ("unspecified", "binary", BuiltinMergeDriver::Binary),
        ("unspecified", "union", BuiltinMergeDriver::Union),
        ("unspecified", "other", BuiltinMergeDriver::Text),
    ] {
        assert_eq!(
            BuiltinMergeDriver::from_effective_attribute(attribute, default),
            expected,
            "attribute={attribute:?}, default={default:?}"
        );
    }
}

#[test]
fn isolated_format_snapshot_ignores_non_common_and_included_spoofs() {
    const TEST_NAME: &str = "guarded_config::merge_overlay::tests::isolated_format_snapshot_ignores_non_common_and_included_spoofs";
    if std::env::var_os("CODEX_GIT_UTILS_MERGE_OVERLAY_CHILD").is_none() {
        let environment = tempfile::tempdir().expect("config environment");
        let global = environment.path().join("global.gitconfig");
        let system = environment.path().join("system.gitconfig");
        std::fs::write(
            &global,
            "[core]\n\trepositoryFormatVersion = 1\n\tsharedRepository = world\n[extensions]\n\tobjectFormat = sha256\n",
        )
        .expect("write global spoof");
        std::fs::write(&system, "").expect("write system config");
        run_isolated_overlay_test(
            TEST_NAME,
            &[
                ("GIT_CONFIG_GLOBAL", global.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system.as_os_str()),
                (
                    "CODEX_APPLY_GIT_CFG",
                    OsStr::new("core.repositoryFormatVersion=1,extensions.objectFormat=sha256"),
                ),
            ],
        );
        return;
    }

    let repo = tempfile::tempdir().expect("repo");
    let root = repo.path();
    run_git(root, &["init", "-q"]);
    run_git(root, &["config", "core.sharedRepository", "0660"]);
    run_git(root, &["config", "extensions.worktreeConfig", "true"]);
    run_git(
        root,
        &["config", "--worktree", "core.repositoryFormatVersion", "1"],
    );
    run_git(
        root,
        &["config", "--worktree", "extensions.objectFormat", "sha256"],
    );
    let included = tempfile::NamedTempFile::new().expect("included config");
    std::fs::write(
        included.path(),
        "[core]\n\trepositoryFormatVersion = 1\n[extensions]\n\tobjectFormat = sha256\n",
    )
    .expect("write included spoof");
    run_git(
        root,
        &[
            "config",
            "--add",
            "include.path",
            included.path().to_str().expect("UTF-8 include path"),
        ],
    );

    let git = GitRunner::for_cwd_io(root).expect("runner");
    let mut guarded =
        GuardedGitConfig::authorize(&git, root, crate::apply::configured_git_config_parts())
            .expect("guarded config");
    guarded
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("filter snapshot");
    guarded
        .install_three_way_merge_policy(&["file.txt".to_string()])
        .expect("install merge policy");
    let isolated = guarded
        .merge_common_config_path()
        .expect("isolated common config");

    assert_eq!(
        config_file_value(root, &isolated, "core.repositoryFormatVersion"),
        Some("0".to_string())
    );
    assert_eq!(
        config_file_value(root, &isolated, "extensions.objectFormat"),
        None
    );
    assert_eq!(
        config_file_value(root, &isolated, "core.sharedRepository"),
        Some("0660".to_string())
    );
}

#[test]
fn shared_repository_values_are_normalized_or_rejected() {
    assert_eq!(
        normalize_shared_repository(&GitConfigValue::Implicit)
            .expect("valid implicit shared repository value"),
        "group"
    );
    for (input, expected) in [
        ("", "umask"),
        ("true", "group"),
        ("YES", "group"),
        ("false", "umask"),
        ("off", "umask"),
        ("group", "group"),
        ("world", "all"),
        ("everybody", "all"),
        ("0", "umask"),
        ("00", "umask"),
        ("1", "group"),
        ("01", "group"),
        ("2", "all"),
        ("02", "all"),
        ("600", "0600"),
        ("0600", "0600"),
        ("640", "0640"),
        ("0640", "0640"),
        ("660", "0660"),
        ("0660", "0660"),
        ("664", "0664"),
        ("0777", "0666"),
        ("000660", "0660"),
        ("06600", "0600"),
        ("+0660", "0660"),
        ("-0", "umask"),
        ("-1", "0666"),
        ("-2", "0666"),
        ("-3", "0664"),
        ("8", "group"),
        ("9", "group"),
        ("0x1", "group"),
        ("0x2", "group"),
        ("1k", "group"),
        ("-1k", "group"),
        ("0x0", "umask"),
        ("0k", "umask"),
        (" 0660", "0660"),
        ("2147483647", "group"),
    ] {
        let value = GitConfigValue::Explicit(input.to_string());
        assert_eq!(
            normalize_shared_repository(&value).expect("valid shared repository value"),
            expected,
            "{input:?}"
        );
    }
    for invalid in [
        "UMASK",
        "GROUP",
        "ALL",
        "WORLD",
        "EVERYBODY",
        "3",
        "7",
        "10",
        "20",
        "100",
        "0444",
        "0003",
        "-0660",
        "-0777",
        "invalid",
        "0999",
        "0o660",
        "0660 ",
        " 0660 ",
        " true",
        "true ",
        " ",
        "2147483648",
        "777777777777777777777777777777777777777",
    ] {
        let value = GitConfigValue::Explicit(invalid.to_string());
        assert_eq!(
            normalize_shared_repository(&value)
                .expect_err("reject malformed shared repository value")
                .kind(),
            io::ErrorKind::InvalidData,
            "{invalid:?}"
        );
    }
}

#[test]
fn pinned_direct_common_reader_preserves_compat_object_format_without_repo_setup() {
    let repo = tempfile::tempdir().expect("repo");
    let root = repo.path();
    run_git(root, &["init", "-q", "--object-format=sha256"]);
    let git = GitRunner::for_cwd_io(root).expect("runner before compat extension");
    let guarded = GuardedGitConfig::authorize(&git, root, Vec::new()).expect("guarded config");
    let config_path = root.join(".git/config");
    let mut config = std::fs::read_to_string(&config_path).expect("read common config");
    config.push_str("\n[extensions]\n\tcompatObjectFormat = sha1\n");
    std::fs::write(&config_path, config).expect("append compat format");

    let (output, expected_origin) = git
        .read_active_common_config_without_includes(
            REPOSITORY_FORMAT_CONFIG_PATTERN,
            /*show_scope*/ true,
        )
        .expect("read direct common format without repository setup");
    assert!(
        output.status.success(),
        "direct format read: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let entries = crate::git_config::parse_config_entries(&output.stdout)
        .expect("parse direct common format");
    assert!(entries.iter().all(|entry| {
        entry.origin == crate::git_config::GitConfigOrigin::File(expected_origin.clone())
    }));
    assert!(
        entries
            .iter()
            .any(|entry| { entry.key == "core.repositoryformatversion" && entry.value == "1" })
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.key == "extensions.objectformat" && entry.value == "sha256")
    );
    assert!(
        entries
            .iter()
            .any(|entry| { entry.key == "extensions.compatobjectformat" && entry.value == "sha1" })
    );

    let isolated = git
        .create_isolated_common_dir()
        .expect("isolated common directory");
    let isolated_config = isolated.config_path();
    for entry in &entries {
        guarded
            .write_sanitized_config_value(&isolated_config, &entry.key, &entry.value)
            .expect("project direct format entry");
    }
    assert_eq!(
        config_file_value(root, &isolated_config, "extensions.compatObjectFormat"),
        Some("sha1".to_string())
    );
}
