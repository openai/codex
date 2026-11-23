//! Verifies patch compatibility and bounded, independent untracked-diff collections.

use super::diff_against_sha;
use crate::GitSha;
use codex_utils_process::background_command;
use pretty_assertions::assert_eq;
use std::path::Path;

fn git(cwd: &Path, args: &[&str]) {
    let output = background_command("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run Git");
    assert!(output.status.success(), "Git {args:?} failed: {output:?}");
}

#[tokio::test]
async fn remote_diff_preserves_slashes_in_the_default_branch_name() {
    let repo = tempfile::tempdir().expect("create repository");
    let cwd = repo.path();
    git(cwd, &["init", "-b", "feature"]);
    git(cwd, &["config", "user.name", "Codex Tests"]);
    git(cwd, &["config", "user.email", "codex-tests@example.com"]);
    git(cwd, &["config", "commit.gpgsign", "false"]);
    git(
        cwd,
        &["config", "core.hooksPath", super::DISABLED_HOOKS_PATH],
    );
    git(cwd, &["remote", "add", "origin", "."]);
    git(cwd, &["commit", "--allow-empty", "-m", "initial"]);
    git(cwd, &["update-ref", "refs/remotes/origin/stable", "HEAD"]);
    std::fs::write(cwd.join("release.txt"), "released\n").expect("write released file");
    git(cwd, &["add", "release.txt"]);
    git(cwd, &["commit", "-m", "release"]);
    git(
        cwd,
        &["update-ref", "refs/remotes/origin/release/stable", "HEAD"],
    );
    git(cwd, &["remote", "set-head", "origin", "release/stable"]);
    let release = super::get_head_commit_hash(cwd).await.expect("release SHA");
    std::fs::write(cwd.join("feature.txt"), "feature\n").expect("write feature file");
    git(cwd, &["add", "feature.txt"]);
    git(cwd, &["commit", "-m", "unpushed"]);

    let diff = super::git_diff_to_remote(cwd).await.expect("remote diff");
    assert_eq!(diff.sha, release);
    assert!(diff.diff.contains("feature.txt"));
    assert!(!diff.diff.contains("release.txt"));
}

#[tokio::test]
async fn tracked_and_untracked_patches_preserve_order_and_apply() {
    let repo = tempfile::tempdir().expect("create repository");
    git(repo.path(), &["init", "-q"]);
    git(repo.path(), &["config", "core.autocrlf", "false"]);
    git(repo.path(), &["config", "core.fsmonitor", "false"]);
    std::fs::write(repo.path().join("tracked.txt"), "before\n").expect("write tracked file");
    git(repo.path(), &["add", "tracked.txt"]);
    git(
        repo.path(),
        &[
            "-c",
            "user.name=Codex Tests",
            "-c",
            "user.email=codex-tests@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            if cfg!(windows) {
                "core.hooksPath=NUL"
            } else {
                "core.hooksPath=/dev/null"
            },
            "commit",
            "-qm",
            "initial",
        ],
    );
    let files: [(&str, &[u8]); 4] = [
        ("tracked.txt", b"after\n"),
        ("-new.txt", b"leading dash\n"),
        ("a space.txt", b"space in filename\n"),
        ("z.bin", b"\x00\x01\xffbinary\x00\xfe"),
    ];
    for (name, contents) in files {
        std::fs::write(repo.path().join(name), contents).expect("write changed file");
    }
    let cwd = repo.path().to_path_buf();
    // The production caller spawns collection; this also checks its Send bound.
    let patch = tokio::spawn(async move {
        diff_against_sha(Path::new("git"), &cwd, &GitSha::new("HEAD")).await
    })
    .await
    .expect("join collection")
    .expect("collect patch");
    assert_eq!(
        patch
            .lines()
            .filter(|line| line.starts_with("diff --git "))
            .collect::<Vec<_>>(),
        vec![
            "diff --git a/tracked.txt b/tracked.txt",
            "diff --git a/-new.txt b/-new.txt",
            "diff --git a/a space.txt b/a space.txt",
            "diff --git a/z.bin b/z.bin",
        ]
    );
    assert!(patch.contains("GIT binary patch"));
    git(repo.path(), &["reset", "--hard", "-q", "HEAD"]);
    for (name, _) in &files[1..] {
        std::fs::remove_file(repo.path().join(name)).expect("remove untracked file");
    }
    let patch_file = tempfile::NamedTempFile::new().expect("create patch file");
    std::fs::write(patch_file.path(), patch).expect("save patch");
    git(
        repo.path(),
        &[
            "apply",
            "--binary",
            patch_file.path().to_str().expect("patch path"),
        ],
    );
    assert_eq!(
        files.map(|(name, _)| std::fs::read(repo.path().join(name)).expect("read applied file")),
        files.map(|(_, contents)| contents.to_vec())
    );
}

#[cfg(target_os = "linux")]
struct GatedGit {
    directory: tempfile::TempDir,
    gates: Vec<std::fs::File>,
    ready: std::fs::File,
    events: futures::channel::mpsc::UnboundedReceiver<String>,
    reader: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
impl GatedGit {
    fn new(file_counts: &[usize]) -> Self {
        use std::io::BufRead;

        let directory = tempfile::tempdir().expect("create fixture directory");
        let ready_path = directory.path().join("ready");
        let mut paths = vec![ready_path];
        for (batch, count) in file_counts.iter().enumerate() {
            let cwd = directory.path().join(format!("batch-{batch}"));
            std::fs::create_dir(&cwd).expect("create batch directory");
            paths.extend((0..*count).map(|index| cwd.join(format!("file-{index:02}"))));
        }
        let output = background_command("mkfifo")
            .args(&paths)
            .output()
            .expect("create FIFO gates");
        assert!(output.status.success(), "mkfifo failed: {output:?}");
        let mut gates: Vec<_> = paths
            .iter()
            .map(|path| {
                // Linux permits O_RDWR on a FIFO without another opener, so cleanup
                // can always release gates even when their child has not started.
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path)
                    .expect("open FIFO")
            })
            .collect();
        let ready = gates.remove(0);
        let input = ready.try_clone().expect("clone readiness FIFO");
        let (sender, events) = futures::channel::mpsc::unbounded();
        codex_utils_cargo_bin::write_executable(
            &directory.path().join("git"),
            r#"#!/bin/sh
while [ "$1" = '-c' ]; do shift 2; done
case "$1" in
config) exit 1 ;;
ls-files) for file in file-*; do printf '%s\n' "$file"; done ;;
diff)
  case " $* " in
  *' --no-index '*)
    for file do :; done
    printf '%s/%s\n' "$PWD" "$file" > ../ready
    read -r release < "$file"
    if [ "$file" = 'file-04' ]; then printf 'failed-output\n'; exit 2; fi
    printf '%s\n' "$file"
    exit 1 ;;
  *) printf 'tracked\n' ;;
  esac ;;
esac
"#,
        )
        .expect("write fake Git");
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(input).lines() {
                let Ok(line) = line else { break };
                if line == "stop" || sender.unbounded_send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            directory,
            gates,
            ready,
            events,
            reader: Some(reader),
        }
    }

    async fn ready(&mut self, batches: usize) {
        use futures::StreamExt;

        for _ in 0..8 * batches {
            tokio::time::timeout(std::time::Duration::from_secs(3), self.events.next())
                .await
                .expect("each collection must fill its own eight slots")
                .expect("read readiness event");
        }
    }

    fn children(&self) -> usize {
        use std::os::unix::ffi::OsStrExt;

        let wrapper = self.directory.path().join("git");
        let mut children = std::collections::BTreeSet::new();
        for task in std::fs::read_dir("/proc/self/task").expect("list test threads") {
            let task = task.expect("read thread entry");
            if let Ok(pids) = std::fs::read_to_string(task.path().join("children")) {
                children.extend(pids.split_whitespace().map(str::to_string));
            }
        }
        children
            .into_iter()
            .filter(|pid| {
                std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|args| {
                    args.split(|byte| *byte == 0)
                        .any(|arg| arg == wrapper.as_os_str().as_bytes())
                })
            })
            .count()
    }

    fn release(&mut self) {
        use std::io::Write;
        for gate in &mut self.gates {
            let _ = gate.write_all(b"go\n");
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for GatedGit {
    fn drop(&mut self) {
        use std::io::Write;
        self.release();
        let _ = self.ready.write_all(b"stop\n");
        if let Some(reader) = self.reader.take() {
            reader.join().expect("join readiness reader");
        }
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn untracked_collections_bound_children_independently_and_skip_failed_files() {
    // At most 16 Git children even if the bound regresses. Keeping both phases
    // in one test also prevents the fixtures from overlapping under nextest.
    for counts in [&[12][..], &[8, 8][..]] {
        let mut fixture = GatedGit::new(counts);
        let collections: Vec<_> = (0..counts.len())
            .map(|batch| {
                let git = fixture.directory.path().join("git");
                let cwd = fixture.directory.path().join(format!("batch-{batch}"));
                tokio::spawn(
                    async move { diff_against_sha(&git, &cwd, &GitSha::new("HEAD")).await },
                )
            })
            .collect();
        fixture.ready(counts.len()).await;
        let children = fixture.children();
        fixture.release();
        for (collection, count) in collections.into_iter().zip(counts) {
            let expected = "tracked\n".to_string()
                + &(0..*count)
                    .filter(|index| *index != 4)
                    .map(|index| format!("file-{index:02}\n"))
                    .collect::<String>();
            assert_eq!(collection.await.expect("join collection"), Some(expected));
        }
        assert_eq!(children, 8 * counts.len());
    }
}
