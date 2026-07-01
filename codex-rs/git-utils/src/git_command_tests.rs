use super::*;
use pretty_assertions::assert_eq;
use std::process::Command;

use crate::safe_git::DISABLED_HOOKS_PATH;

#[cfg(unix)]
fn write_executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, body).expect("write executable");
    let mut permissions = std::fs::metadata(path)
        .expect("executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("set executable permissions");
}

fn run_git(cwd: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    isolate_git_command_environment(&mut command);
    let status = command
        .args([
            "-c",
            &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .current_dir(cwd)
        .status()
        .expect("run real Git");
    assert!(status.success(), "git {args:?} failed");
}

fn commit_all(cwd: &Path, message: &str) {
    run_git(
        cwd,
        &[
            "-c",
            "user.name=Codex Test",
            "-c",
            "user.email=codex@example.com",
            "-c",
            "commit.gpgSign=false",
            "commit",
            "-qam",
            message,
        ],
    );
}

fn write_git_candidate(directory: &Path) {
    std::fs::create_dir_all(directory).expect("create candidate directory");
    let candidate = directory.join(git_executable_name());
    #[cfg(unix)]
    write_executable(&candidate, "#!/bin/sh\nexit 0\n");
    #[cfg(windows)]
    std::fs::write(candidate, b"MZ").expect("write native executable fixture");
    #[cfg(not(any(unix, windows)))]
    std::fs::write(candidate, b"git fixture").expect("write executable fixture");
}

#[cfg(windows)]
fn create_junction(path: &Path, target: &Path) {
    let output = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(path)
        .arg(target)
        .output()
        .expect("create junction");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "mklink failed: stdout={stdout} stderr={stderr}"
    );
}

fn locations_for_root(root: &Path) -> UntrustedGitLocations {
    let mut roots = vec![root.to_path_buf()];
    push_unique(
        &mut roots,
        std::fs::canonicalize(root).expect("canonical root"),
    );
    UntrustedGitLocations {
        roots,
        common_dirs: Vec::new(),
    }
}

fn raw_parent_traversal(root: &Path, sibling: &str) -> PathBuf {
    let separator = std::path::MAIN_SEPARATOR.to_string();
    let mut path = root.as_os_str().to_os_string();
    path.push(&separator);
    path.push("..");
    path.push(&separator);
    path.push(sibling);
    path.into()
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 fixture path")
}

fn selected_git(locations: &UntrustedGitLocations, directories: &[&Path]) -> PathBuf {
    let search_path = std::env::join_paths(directories).expect("PATH");
    GitRunner::from_search_path(locations, &search_path)
        .expect("trusted Git")
        .executable
}

#[cfg(unix)]
#[test]
fn resolver_skips_untrusted_path_entries_and_runs_external_candidate() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let repo_bin = repo.join("bin");
    let outside = fixture.path().join("outside");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&repo_bin).expect("repo bin");
    std::fs::create_dir_all(&outside).expect("outside bin");
    std::fs::create_dir_all(&trusted_bin).expect("trusted bin");
    write_executable(&repo_bin.join("git"), "#!/bin/sh\nexit 1\n");
    std::os::unix::fs::symlink(repo_bin.join("git"), outside.join("git"))
        .expect("outside symlink into repository");
    write_executable(&trusted_bin.join("git"), "#!/bin/sh\nprintf 'trusted\\n'\n");

    let path = std::env::join_paths([
        PathBuf::from("relative"),
        repo_bin,
        outside,
        trusted_bin.clone(),
    ])
    .expect("PATH");
    let locations = locations_for_root(&repo);
    let runner = GitRunner::from_search_path(&locations, &path).expect("trusted Git");
    assert_eq!(runner.executable, trusted_bin.join("git"));
    let output = runner.output(runner.command()).expect("run trusted Git");
    assert_eq!(output.stdout, b"trusted\n");
}

#[test]
fn linked_worktree_rejects_git_from_main_and_linked_worktrees() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let linked = fixture.path().join("linked");
    let git_dir = main.join(".git/worktrees/linked");
    let main_bin = main.join("bin");
    std::fs::create_dir_all(&git_dir).expect("linked Git directory");
    std::fs::create_dir_all(&linked).expect("linked worktree");
    std::fs::write(
        linked.join(".git"),
        format!("gitdir: {}\n", git_dir.display()),
    )
    .expect("linked .git file");
    write_git_candidate(&main_bin);

    let locations = untrusted_git_locations_for_cwd(&linked).expect("untrusted locations");
    assert!(path_is_untrusted(
        &main_bin.join(git_executable_name()),
        &locations
    ));
}

#[test]
fn nested_repository_rejects_git_from_enclosing_repository() {
    let fixture = tempfile::tempdir().expect("fixture");
    let outer = fixture.path().join("outer");
    let nested = outer.join("nested");
    let outer_bin = outer.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&nested).expect("create nested repository");
    run_git(&outer, &["init", "-q"]);
    run_git(&nested, &["init", "-q"]);
    write_git_candidate(&outer_bin);
    write_git_candidate(&trusted_bin);

    let locations = untrusted_git_locations_for_cwd(&nested).expect("untrusted locations");
    assert!(
        path_is_untrusted(&outer_bin.join(git_executable_name()), &locations),
        "Git from an enclosing repository must remain repository-controlled"
    );
    assert_eq!(
        selected_git(&locations, &[&outer_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[cfg(unix)]
#[test]
fn symlinked_nested_repository_rejects_git_from_lexical_enclosing_repository() {
    let fixture = tempfile::tempdir().expect("fixture");
    let outer = fixture.path().join("outer");
    let physical_nested = fixture.path().join("physical-nested");
    let lexical_nested = outer.join("nested");
    let outer_bin = outer.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&outer).expect("create outer repository");
    std::fs::create_dir_all(&physical_nested).expect("create physical nested repository");
    run_git(&outer, &["init", "-q"]);
    run_git(&physical_nested, &["init", "-q"]);
    std::os::unix::fs::symlink(&physical_nested, &lexical_nested)
        .expect("symlink nested repository");
    write_git_candidate(&outer_bin);
    write_git_candidate(&trusted_bin);

    let locations = untrusted_git_locations_for_cwd(&lexical_nested).expect("untrusted locations");
    assert!(
        path_is_untrusted(&outer_bin.join(git_executable_name()), &locations),
        "Git from the lexical enclosing repository must remain repository-controlled"
    );
    assert_eq!(
        selected_git(&locations, &[&outer_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[test]
fn nested_repository_rejects_git_from_enclosing_repository_main_worktree() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let linked = fixture.path().join("linked");
    let nested = linked.join("nested");
    let main_bin = main.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&main).expect("create main worktree");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    std::fs::create_dir_all(&nested).expect("create nested repository");
    run_git(&nested, &["init", "-q"]);
    write_git_candidate(&main_bin);
    write_git_candidate(&trusted_bin);

    let locations = untrusted_git_locations_for_cwd(&nested).expect("untrusted locations");
    assert!(
        path_is_untrusted(&main_bin.join(git_executable_name()), &locations),
        "all worktrees of an enclosing repository must remain repository-controlled"
    );
    assert_eq!(
        selected_git(&locations, &[&main_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[test]
fn submodule_rejects_git_from_enclosing_superproject() {
    let fixture = tempfile::tempdir().expect("fixture");
    let source = fixture.path().join("source");
    let outer = fixture.path().join("outer");
    let submodule = outer.join("nested");
    let outer_bin = outer.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&source).expect("create source repository");
    std::fs::create_dir_all(&outer).expect("create superproject");
    run_git(&source, &["init", "-q"]);
    std::fs::write(source.join("source.txt"), "source\n").expect("write source file");
    run_git(&source, &["add", "source.txt"]);
    commit_all(&source, "source");
    run_git(&outer, &["init", "-q"]);
    std::fs::write(outer.join("outer.txt"), "outer\n").expect("write outer file");
    run_git(&outer, &["add", "outer.txt"]);
    commit_all(&outer, "outer");
    run_git(
        &outer,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            path_text(&source),
            "nested",
        ],
    );
    write_git_candidate(&outer_bin);
    write_git_candidate(&trusted_bin);

    let locations = untrusted_git_locations_for_cwd(&submodule).expect("untrusted locations");
    assert!(
        path_is_untrusted(&outer_bin.join(git_executable_name()), &locations),
        "Git from a superproject must remain repository-controlled"
    );
    assert_eq!(
        selected_git(&locations, &[&outer_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[test]
fn bare_backed_linked_worktree_allows_external_git_in_sibling_directory() {
    let fixture = tempfile::tempdir().expect("fixture");
    let bare = fixture.path().join("repository.git");
    let linked = fixture.path().join("linked");
    let trusted_bin = fixture.path().join("trusted-bin");
    run_git(fixture.path(), &["init", "--bare", path_text(&bare)]);
    run_git(
        fixture.path(),
        &[
            "--git-dir",
            path_text(&bare),
            "worktree",
            "add",
            "--orphan",
            path_text(&linked),
        ],
    );
    write_git_candidate(&trusted_bin);

    let locations = untrusted_git_locations_for_cwd(&linked).expect("untrusted locations");
    assert_eq!(
        selected_git(&locations, &[&trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[test]
fn separate_dot_git_dir_rejects_main_candidate_and_allows_unrelated_repo_candidate() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let common_dir = fixture.path().join("git-storage/.git");
    let linked = fixture.path().join("linked");
    let main_bin = main.join("bin");
    let unrelated = fixture.path().join("unrelated");
    let unrelated_bin = unrelated.join("bin");
    let malformed = fixture.path().join("malformed");
    let malformed_bin = malformed.join("bin");
    std::fs::create_dir_all(&main).expect("create main worktree");
    std::fs::create_dir_all(common_dir.parent().expect("common-dir parent"))
        .expect("create common-dir parent");
    run_git(
        fixture.path(),
        &[
            "init",
            "--separate-git-dir",
            path_text(&common_dir),
            path_text(&main),
        ],
    );
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    run_git(fixture.path(), &["init", path_text(&unrelated)]);
    write_git_candidate(&main_bin);
    write_git_candidate(&unrelated_bin);
    write_git_candidate(&malformed_bin);
    std::fs::write(malformed.join(".git"), "not a gitdir").expect("malformed marker");

    let locations = untrusted_git_locations_for_cwd(&linked).expect("untrusted locations");
    assert_eq!(
        selected_git(&locations, &[&main_bin, &malformed_bin, &unrelated_bin]),
        unrelated_bin.join(git_executable_name())
    );
}

#[test]
fn resolver_rejects_parent_traversal_spelled_through_repository() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&repo).expect("create repository");
    write_git_candidate(&trusted_bin);

    let locations = locations_for_root(&repo);
    for root in &locations.roots {
        // Construct the PATH entry as raw text so its `..` component survives
        // long enough for the resolver to inspect the original spelling.
        let traversing_path = raw_parent_traversal(root, "trusted-bin");
        let search_path = std::env::join_paths([&traversing_path]).expect("PATH");
        let split_paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
        assert_eq!(split_paths, vec![traversing_path.clone()]);
        assert!(
            search_directory_is_untrusted(&split_paths[0], &locations),
            "raw PATH traversal was not rejected from {root:?}"
        );

        assert!(
            matches!(
                GitRunner::from_search_path(&locations, &search_path),
                Err(GitReadError::NoTrustedGit)
            ),
            "resolver accepted parent traversal from {root:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn resolver_rejects_parent_traversal_across_windows_namespaces() {
    let traversing = [
        r"C:\Repo\..\outside",
        r"\\?\C:\Repo\..\outside",
        r"\\Server\Share\Repo\..\outside",
        r"\\?\UNC\Server\Share\Repo\..\outside",
        r"\\?\unc\Server\Share\Repo\..\outside",
        r"\\.\C:\Repo\..\outside",
        r"\\.\UNC\Server\Share\Repo\..\outside",
        r"\\?\C:\RÉPO\..\outside",
    ];
    for path in traversing {
        assert!(
            windows_path_requires_fail_closed(Path::new(path)),
            "parent traversal was accepted: {path:?}"
        );
    }

    let normalized_external = [
        r"C:\outside",
        r"\\?\C:\outside",
        r"\\Server\Share\outside",
        r"\\?\UNC\Server\Share\outside",
        r"\\?\unc\Server\Share\outside",
        r"\\.\C:\outside",
        r"\\.\UNC\Server\Share\outside",
    ];
    for path in normalized_external {
        assert!(
            !windows_path_requires_fail_closed(Path::new(path)),
            "normalized filesystem path was rejected: {path:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn resolver_rejects_unicode_case_alias_through_repository_junction() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("Répo");
    let outside = fixture.path().join("outside");
    let junction = repo.join("git-bin");
    std::fs::create_dir_all(&repo).expect("create repository");
    write_git_candidate(&outside);
    create_junction(&junction, &outside);

    let case_alias = fixture.path().join("RÉPO").join("git-bin");
    let verbatim_case_alias = PathBuf::from(format!(r"\\?\{}", case_alias.display()));
    assert_eq!(
        std::fs::canonicalize(&verbatim_case_alias).expect("canonical alias"),
        std::fs::canonicalize(&outside).expect("canonical outside")
    );

    let locations = locations_for_root(&repo);
    assert!(
        !path_is_untrusted(&verbatim_case_alias, &locations),
        "fixture must exercise the Unicode alias before canonical ancestry"
    );
    assert!(search_directory_is_untrusted(
        &verbatim_case_alias,
        &locations
    ));
    let search_path = std::env::join_paths([verbatim_case_alias]).expect("PATH");
    assert!(matches!(
        GitRunner::from_search_path(&locations, &search_path),
        Err(GitReadError::NoTrustedGit)
    ));
}

#[cfg(windows)]
#[test]
fn resolver_fails_closed_for_unsupported_windows_device_namespaces() {
    let unsupported = [
        r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1\git.exe",
        r"\\?\Volume{11111111-1111-1111-1111-111111111111}\git.exe",
        r"\\.\PhysicalDrive0",
        r"\\.\pipe\codex-git",
    ];

    for path in unsupported {
        assert!(
            windows_path_requires_fail_closed(Path::new(path)),
            "unsupported namespace was trusted: {path:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn resolver_selects_native_git_exe_only() {
    assert!(paths_equal(
        Path::new(r"C:\Repo\.git"),
        Path::new(r"c:\repo\.GIT")
    ));
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let scripts = fixture.path().join("scripts");
    let native = fixture.path().join("native");
    std::fs::create_dir_all(&repo).expect("repo");
    std::fs::create_dir_all(&scripts).expect("scripts");
    std::fs::create_dir_all(&native).expect("native");
    std::fs::write(scripts.join("git.cmd"), "@exit /b 0\r\n").expect("script");
    std::fs::write(native.join("git.exe"), b"MZ").expect("native executable fixture");
    let locations = locations_for_root(&repo);
    let path = std::env::join_paths([scripts, native.clone()]).expect("PATH");
    let runner = GitRunner::from_search_path(&locations, &path).expect("native Git");
    assert_eq!(runner.executable, native.join("git.exe"));
}
