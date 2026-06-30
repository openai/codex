use super::*;
use pretty_assertions::assert_eq;

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

#[cfg(unix)]
#[test]
fn resolver_skips_repository_controlled_git_and_runs_external_candidate() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let repo_bin = repo.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&repo_bin).expect("repo bin");
    std::fs::create_dir_all(&trusted_bin).expect("trusted bin");
    let marker = repo.join("repository-git-ran");
    write_executable(
        &repo_bin.join("git"),
        &format!("#!/bin/sh\nprintf ran > '{}'\n", marker.display()),
    );
    write_executable(&trusted_bin.join("git"), "#!/bin/sh\nprintf 'trusted\\n'\n");

    let path = std::env::join_paths([repo_bin, trusted_bin.clone()]).expect("PATH");
    let roots = vec![std::fs::canonicalize(&repo).expect("canonical repo")];
    let runner = GitRunner::from_search_path(&roots, &path).expect("trusted Git");
    assert_eq!(runner.executable(), trusted_bin.join("git"));
    let output = runner.output(runner.command()).expect("run trusted Git");
    assert_eq!(output.stdout, b"trusted\n");
    assert!(!marker.exists(), "repository-controlled Git must not run");
}

#[cfg(unix)]
#[test]
fn resolver_rejects_repository_candidates_symlinks_and_relative_entries() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let repo_bin = repo.join("bin");
    let outside = fixture.path().join("outside");
    std::fs::create_dir_all(&repo_bin).expect("repo bin");
    std::fs::create_dir_all(&outside).expect("outside");
    write_executable(&repo_bin.join("owned-git"), "#!/bin/sh\nexit 0\n");
    std::os::unix::fs::symlink(repo_bin.join("owned-git"), outside.join("git"))
        .expect("outside symlink into repository");

    let roots = vec![std::fs::canonicalize(&repo).expect("canonical repo")];
    let path =
        std::env::join_paths([PathBuf::new(), PathBuf::from("relative"), outside]).expect("PATH");
    assert_eq!(
        GitRunner::from_search_path(&roots, &path).unwrap_err(),
        GitReadError::NoTrustedGit
    );

    let trusted = fixture.path().join("trusted");
    std::fs::create_dir_all(&trusted).expect("trusted");
    write_executable(&trusted.join("git"), "#!/bin/sh\nexit 0\n");
    std::fs::remove_file(repo_bin.join("git")).ok();
    std::os::unix::fs::symlink(trusted.join("git"), repo_bin.join("git"))
        .expect("repository symlink out");
    let path = std::env::join_paths([repo_bin]).expect("PATH");
    assert_eq!(
        GitRunner::from_search_path(&roots, &path).unwrap_err(),
        GitReadError::NoTrustedGit
    );
}

#[cfg(unix)]
#[test]
fn resolver_skips_non_executable_files() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let bin = fixture.path().join("bin");
    std::fs::create_dir_all(&repo).expect("repo");
    std::fs::create_dir_all(&bin).expect("bin");
    std::fs::write(bin.join("git"), "not executable\n").expect("candidate");
    let roots = vec![std::fs::canonicalize(repo).expect("canonical repo")];
    let path = std::env::join_paths([bin]).expect("PATH");
    assert_eq!(
        GitRunner::from_search_path(&roots, &path).unwrap_err(),
        GitReadError::NoTrustedGit
    );
}

#[test]
fn linked_worktree_marks_main_and_linked_roots_untrusted() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let linked = fixture.path().join("linked");
    let git_dir = main.join(".git/worktrees/linked");
    std::fs::create_dir_all(&git_dir).expect("linked Git directory");
    std::fs::create_dir_all(&linked).expect("linked worktree");
    std::fs::write(
        linked.join(".git"),
        format!("gitdir: {}\n", git_dir.display()),
    )
    .expect("linked .git file");

    let roots = untrusted_roots_for_cwd(&linked).expect("untrusted roots");
    assert_eq!(
        roots,
        vec![
            std::fs::canonicalize(&linked).expect("canonical linked root"),
            std::fs::canonicalize(&main).expect("canonical main root"),
        ]
    );
}

#[cfg(windows)]
#[test]
fn resolver_selects_native_git_exe_only() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let scripts = fixture.path().join("scripts");
    let native = fixture.path().join("native");
    std::fs::create_dir_all(&repo).expect("repo");
    std::fs::create_dir_all(&scripts).expect("scripts");
    std::fs::create_dir_all(&native).expect("native");
    std::fs::write(scripts.join("git.cmd"), "@exit /b 0\r\n").expect("script");
    std::fs::write(native.join("git.exe"), b"MZ").expect("native executable fixture");
    let roots = vec![std::fs::canonicalize(repo).expect("canonical repo")];
    let path = std::env::join_paths([scripts, native.clone()]).expect("PATH");
    let runner = GitRunner::from_search_path(&roots, &path).expect("native Git");
    assert_eq!(runner.executable(), native.join("git.exe"));
}
