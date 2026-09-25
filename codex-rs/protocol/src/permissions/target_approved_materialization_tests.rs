//! Approved commands widen filesystem access without weakening explicit read denials.

use super::*;
use crate::permissions::FileSystemAccessMode::Deny;
use crate::permissions::FileSystemAccessMode::Read;
use crate::permissions::FileSystemAccessMode::Write;
use crate::permissions::PROTECTED_METADATA_PATH_NAMES;
use crate::permissions::RawFileSystemSandboxPolicy;
use crate::permissions::project_roots_glob_pattern;
use pretty_assertions::assert_eq;
use std::path::Path;

fn uri(input: &str) -> PathUri {
    PathUri::parse(input).unwrap()
}

fn entry(path: impl Into<FileSystemPath>, access: FileSystemAccessMode) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry::new(path.into(), access)
}

fn workspace(subpath: &str) -> FileSystemPath {
    FileSystemPath::Special {
        value: FileSystemSpecialPath::project_roots(Some(subpath.into())),
    }
}

fn glob(pattern: String) -> FileSystemPath {
    FileSystemPath::GlobPattern { pattern }
}

fn context<'a>(cwd: &'a PathUri, roots: &'a [PathUri]) -> FileSystemSandboxPolicyContext<'a> {
    FileSystemSandboxPolicyContext {
        cwd,
        workspace_roots: roots,
        user_home_dir: None,
        temporary_directories: Some(&[]),
    }
}

#[test]
fn approved_command_grants_root_metadata_unless_explicitly_denied() {
    for path in [
        "file:///work/repo",
        "file:///C:/work/repo",
        "file://server/share/repo",
    ] {
        let cwd = uri(path);
        let context = context(&cwd, std::slice::from_ref(&cwd));
        let root = file_system_root(&context).unwrap();
        let private = cwd.join("private").unwrap();
        let pattern = project_roots_glob_pattern(Path::new("**/*.env"));
        let mut policy = FileSystemSandboxPolicy::read_only();
        policy.entries.extend([
            entry(cwd.clone(), Write),
            entry(workspace("private"), Deny),
            entry(private.join("public").unwrap(), Read),
            entry(private.join("writable").unwrap(), Write),
            entry(glob(pattern), Deny),
        ]);
        let approved = policy.for_approved_command(&context);
        let mut expected = FileSystemSandboxPolicy::restricted(vec![
            entry(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                Write,
            ),
            entry(private, Deny),
            entry(
                glob(cwd.join("**/*.env").unwrap().inferred_native_path_string()),
                Deny,
            ),
        ]);
        expected.entries.extend(
            PROTECTED_METADATA_PATH_NAMES
                .iter()
                .map(|name| entry(root.join_descendant(name).unwrap(), Write)),
        );
        assert_eq!(approved, expected);
        let raw = RawFileSystemSandboxPolicy::try_from(approved.clone()).unwrap();
        let wire: RawFileSystemSandboxPolicy =
            serde_json::from_value(serde_json::to_value(raw).unwrap()).unwrap();
        assert_eq!(FileSystemSandboxPolicy::try_from(wire).unwrap(), approved);
        for name in PROTECTED_METADATA_PATH_NAMES {
            assert!(approved.can_write_path(&root.join_descendant(name).unwrap(), &context));
        }
        assert!(approved.can_write_path(&cwd.join(".git/FETCH_HEAD").unwrap(), &context));
        let matcher = ReadDenyMatcher::try_new_with_context(&approved, &context)
            .unwrap()
            .unwrap();
        for denied in [
            "private/public/file",
            "private/writable/file",
            "nested/secret.env",
        ] {
            assert!(
                matcher.is_read_denied_uri(&cwd.join(denied).unwrap(), &context),
                "{path}: {denied}"
            );
        }

        let root_path = |name| root.join_descendant(name).unwrap();
        let ordinary = FileSystemSandboxPolicy::restricted(vec![
            entry(root.clone(), Write),
            entry(root_path(".git"), Deny),
            entry(
                glob(root_path(".agent?").inferred_native_path_string()),
                Deny,
            ),
            entry(root_path(".codex/private"), Deny),
            entry(
                glob(root_path(".codex/*.env").inferred_native_path_string()),
                Deny,
            ),
        ]);
        let approved = ordinary.for_approved_command(&context);
        let metadata_paths = [".git", ".agents", ".codex"].map(root_path);
        assert_eq!(
            metadata_paths
                .each_ref()
                .map(|path| ordinary.can_write_path(path, &context)),
            [false, false, false],
        );
        assert_eq!(
            metadata_paths
                .each_ref()
                .map(|path| approved.can_write_path(path, &context)),
            [false, false, true],
        );
        assert!(approved.can_write_path(&root_path(".codex/public"), &context));
        assert!(!approved.can_write_path(&root_path(".codex/private/file"), &context));
        let matcher = ReadDenyMatcher::try_new_with_context(&approved, &context)
            .unwrap()
            .unwrap();
        assert_eq!(
            [
                ".git",
                ".agents",
                ".codex/private/file",
                ".codex/secret.env",
                ".codex/public"
            ]
            .map(|path| matcher.is_read_denied_uri(&root_path(path), &context)),
            [true, true, true, true, false],
        );
    }
}

#[cfg(unix)]
#[test]
fn linux_root_metadata_writes_inherit_the_root_without_reopening_other_paths() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().canonicalize().unwrap();
    let root = uri("file:///");
    let mut policy = FileSystemSandboxPolicy::restricted(vec![
        entry(root.clone(), Write),
        entry(
            PathUri::from_host_native_path(cwd.join("private")).unwrap(),
            Deny,
        ),
    ]);
    let roots = policy.get_writable_roots_with_cwd_inheriting_root_metadata(&cwd);
    assert_eq!(
        roots[0].protected_metadata_names,
        PROTECTED_METADATA_PATH_NAMES
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>(),
    );

    let metadata = PROTECTED_METADATA_PATH_NAMES
        .iter()
        .map(|name| entry(root.join_descendant(name).unwrap(), Write))
        .collect::<Vec<_>>();
    policy.entries.extend(metadata.clone());
    let roots = policy.get_writable_roots_with_cwd_inheriting_root_metadata(&cwd);
    assert_eq!(
        roots
            .iter()
            .map(|root| root.root.as_path())
            .collect::<Vec<_>>(),
        [Path::new("/")],
    );
    assert!(roots[0].protected_metadata_names.is_empty());

    let standalone = FileSystemSandboxPolicy::restricted(metadata);
    assert_eq!(
        standalone.get_writable_roots_with_cwd_inheriting_root_metadata(&cwd),
        standalone.get_writable_roots_with_cwd(&cwd),
    );
}

#[test]
fn approved_command_discards_other_volume_grants_before_materialization() {
    let cwd = uri("file:///C:/run");
    let workspace_root = uri("file:///C:/work");
    let context = context(&cwd, std::slice::from_ref(&workspace_root));
    let mut policy = FileSystemSandboxPolicy::read_only();
    policy.entries.extend([
        entry(workspace(r"D:\vault"), Read),
        entry(uri("file:///D:/scoped"), Write),
        entry(workspace("private"), Deny),
    ]);
    let approved = policy.for_approved_command(&context);
    assert_eq!(
        [
            "file:///C:/work/public",
            "file:///C:/work/private/file",
            "file:///D:/scoped"
        ]
        .map(|path| approved.resolve_access(&uri(path), &context)),
        [Write, Deny, Deny],
    );
}

#[test]
fn approved_command_declines_root_or_unresolvable_denials() {
    for path in ["file:///work/repo", "file:///C:/work/repo"] {
        let cwd = uri(path);
        let known = context(&cwd, std::slice::from_ref(&cwd));
        let root = file_system_root(&known).unwrap();
        for (context, denied) in [
            (
                known,
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
            ),
            (known, root.clone().into()),
            (known, glob(root.inferred_native_path_string())),
            (known, glob("secrets/[z-a]".into())),
            (known, workspace("~/private")),
            (context(&cwd, &[]), workspace("private")),
            (
                context(&cwd, &[]),
                glob(project_roots_glob_pattern(Path::new("**/*.env"))),
            ),
        ] {
            let mut policy = FileSystemSandboxPolicy::read_only();
            policy
                .entries
                .extend([entry(denied, Deny), entry(cwd.clone(), Write)]);
            assert_eq!(policy.for_approved_command(&context), policy);
        }
    }
}
