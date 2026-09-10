//! Contracts for layer-owned denial facts, parser scoping, and policy conversion.

use super::ConfigPathContext;
use crate::FilesystemConstraints;
use crate::FilesystemDenyReadPattern;
use crate::RequirementSource;
use crate::RequirementsLayerEntry;
use crate::compose_requirements_for_hostname;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn composition_applies_each_layers_base_and_home_to_policy() {
    let native_home = tempdir().expect("native home");
    let first = RequirementsLayerEntry::from_toml(
        RequirementSource::Unknown,
        "[permissions.filesystem]\ndeny_read = ['private', '~/secret', 'private', 'C:\\', '\\\\server\\share\\', 'private/*.key']",
    )
    .with_path_context(ConfigPathContext::new(
        PathConvention::Windows,
        Some(PathUri::parse("file:///C:/first").expect("first base")),
        Some(PathUri::parse("file:///C:/Users/first").expect("first home")),
    ));
    let second = RequirementsLayerEntry::from_toml_value(
        RequirementSource::Unknown,
        toml::toml! { [permissions.filesystem] deny_read = ["private", "~/secret"] }.into(),
    )
    .with_path_context(ConfigPathContext::new(
        PathConvention::Windows,
        Some(PathUri::parse("file:///D:/second").expect("second base")),
        Some(PathUri::parse("file:///D:/Users/second").expect("second home")),
    ));
    let composed = AbsolutePathBufGuard::with_home_directory(native_home.path(), || {
        compose_requirements_for_hostname([first, second], /*hostname*/ None)
            .expect("compose layers")
            .expect("requirements present")
    });
    let constraints = FilesystemConstraints::from(composed.permissions.expect("permissions").value);
    let mut expected = FileSystemSandboxPolicy {
        kind: FileSystemSandboxKind::Restricted,
        glob_scan_max_depth: Some(7),
        entries: vec![FileSystemSandboxEntry::new(
            PathUri::parse("file:///C:/allowed")
                .expect("existing path")
                .into(),
            FileSystemAccessMode::Write,
        )],
    };
    let mut policy = expected.clone();
    expected.entries.extend(
        [
            "file:///D:/second/private",
            "file:///D:/Users/second/secret",
            "file:///C:/first/private",
            "file:///C:/Users/first/secret",
            "file:///C:/",
            "file://server/share",
        ]
        .map(|path| {
            FileSystemSandboxEntry::new(
                PathUri::parse(path).expect("expected denial").into(),
                FileSystemAccessMode::Deny,
            )
        }),
    );
    expected.entries.push(FileSystemSandboxEntry::new(
        FileSystemPath::GlobPattern {
            pattern: r"C:\first\private/*.key".to_string(),
        },
        FileSystemAccessMode::Deny,
    ));
    constraints
        .apply_to_policy(&mut policy, PathConvention::Windows)
        .expect("apply denials");
    constraints
        .apply_to_policy(&mut policy, PathConvention::Windows)
        .expect("deduplicate existing denials");
    assert_eq!(policy, expected);
    assert!(
        constraints
            .apply_to_policy(&mut policy, PathConvention::Posix)
            .is_err(),
        "a foreign denial cannot be reinterpreted using the host convention",
    );
    assert_eq!(policy, expected);
}

#[test]
fn common_parser_uses_supplied_windows_drive_and_glob_separators() {
    let context = ConfigPathContext::new(
        PathConvention::Windows,
        Some(PathUri::parse("file:///C:/base/cwd").expect("base")),
        /*user_home_dir*/ None,
    );
    let _guard = context.enter();
    for (input, expected) in [
        (r"C:private\*.key", r"C:\base\cwd\private/*.key"),
        (r"D:\*.key", r"D:\/*.key"),
        (r"\private\*.key", r"C:\private/*.key"),
    ] {
        assert_eq!(
            FilesystemDenyReadPattern::from_input(input)
                .expect("resolve pattern")
                .as_str(),
            expected,
        );
    }
}

#[test]
fn windows_denial_globs_reject_ambiguous_roots_and_streams() {
    let context = ConfigPathContext::new(
        PathConvention::Windows,
        Some(PathUri::parse("file://server/share/base").expect("UNC base")),
        /*user_home_dir*/ None,
    );
    let _guard = context.enter();
    for input in [
        r"\\?\C:\private",
        r"\\*\share\private",
        r"D:*.key",
        r"private\*\file:stream",
    ] {
        assert!(
            FilesystemDenyReadPattern::from_input(input).is_err(),
            "{input}"
        );
    }
}

#[test]
fn failed_nested_composition_restores_outer_context_and_then_native_resolution() {
    let base = tempdir().expect("native base");
    let _native_guard = AbsolutePathBufGuard::new(base.path());
    let native = FilesystemDenyReadPattern::from_input("private");
    {
        let outer = ConfigPathContext::new(
            PathConvention::Posix,
            Some(PathUri::parse("file:///outer").expect("outer base")),
            /*user_home_dir*/ None,
        );
        let _outer_guard = outer.enter();
        let inner = RequirementsLayerEntry::from_toml(
            RequirementSource::Unknown,
            "[permissions.filesystem]\ndeny_read = ['relative']",
        )
        .with_path_context(ConfigPathContext::new(
            PathConvention::Windows,
            /*base_dir*/ None,
            /*user_home_dir*/ None,
        ));
        assert!(compose_requirements_for_hostname([inner], /*hostname*/ None).is_err());
        assert_eq!(
            FilesystemDenyReadPattern::from_input("private")
                .expect("outer context restored")
                .as_str(),
            "/outer/private",
        );
    }
    assert_eq!(FilesystemDenyReadPattern::from_input("private"), native);
}

#[test]
fn native_guard_and_supplied_native_facts_use_the_same_parser() {
    let base = tempdir().expect("base");
    let home = tempdir().expect("home");
    let _base_guard = AbsolutePathBufGuard::new(base.path());
    AbsolutePathBufGuard::with_home_directory(home.path(), || {
        let inputs = ["./private", "~/secret/*.txt"];
        let native = inputs.map(FilesystemDenyReadPattern::from_input);
        let context = ConfigPathContext::new(
            PathConvention::native(),
            Some(PathUri::from_host_native_path(base.path()).expect("native base URI")),
            Some(PathUri::from_host_native_path(home.path()).expect("native home URI")),
        );
        let _context_guard = context.enter();
        assert_eq!(inputs.map(FilesystemDenyReadPattern::from_input), native);
    });
}

#[test]
fn missing_supplied_home_does_not_fall_back_to_native_home() {
    let home = tempdir().expect("native home");
    let layer = RequirementsLayerEntry::from_toml(
        RequirementSource::Unknown,
        "[permissions.filesystem]\ndeny_read = ['~/secret']",
    )
    .with_path_context(ConfigPathContext::new(
        PathConvention::Windows,
        Some(PathUri::parse("file:///C:/base").expect("base")),
        /*user_home_dir*/ None,
    ));
    AbsolutePathBufGuard::with_home_directory(home.path(), || {
        assert!(compose_requirements_for_hostname([layer], /*hostname*/ None).is_err());
    });
}

#[test]
fn denial_resolution_rejects_glob_syntax_in_supplied_facts() {
    for directory in ["file:///home/sam[1]", "file:///C:/Users/sam[1]"] {
        let directory = PathUri::parse(directory).unwrap();
        let context = ConfigPathContext::new(
            directory.infer_path_convention().unwrap(),
            Some(directory.clone()),
            Some(directory),
        );
        // Literal denials also reject metacharacters introduced by path facts.
        for input in [
            "private/*.key",
            "~/private/*.key",
            "private/key",
            "~/private/key",
        ] {
            let layer = RequirementsLayerEntry::from_toml(
                RequirementSource::Unknown,
                format!("[permissions.filesystem]\ndeny_read = ['{input}']"),
            )
            .with_path_context(context.clone());
            assert!(compose_requirements_for_hostname([layer], /*hostname*/ None).is_err());
        }
    }
}

#[test]
fn native_and_supplied_denials_reject_the_same_unsafe_facts() {
    let parent = tempdir().unwrap();
    let directory = parent.path().join("sam[1]");
    let _base_guard = AbsolutePathBufGuard::new(&directory);
    AbsolutePathBufGuard::with_home_directory(&directory, || {
        let inputs = [
            "private/*.key",
            "~/private/*.key",
            "private/key",
            "~/private/key",
        ];
        let native = inputs.map(FilesystemDenyReadPattern::from_input);
        assert!(native.iter().all(Result::is_err));
        let directory = PathUri::from_host_native_path(&directory).unwrap();
        let context = ConfigPathContext::new(
            PathConvention::native(),
            Some(directory.clone()),
            Some(directory),
        );
        let _context_guard = context.enter();
        assert_eq!(inputs.map(FilesystemDenyReadPattern::from_input), native);
    });
}
