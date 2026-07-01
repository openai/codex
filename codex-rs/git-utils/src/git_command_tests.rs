use super::*;
use pretty_assertions::assert_eq;
use std::process::Command;

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
        .args(args)
        .current_dir(cwd)
        .status()
        .expect("run real Git");
    assert!(status.success(), "git {args:?} failed");
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

fn locations_for_root(root: &Path) -> UntrustedGitLocations {
    UntrustedGitLocations {
        roots: vec![std::fs::canonicalize(root).expect("canonical root")],
        common_dir: None,
    }
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
