//! Exercises staging ownership across processes and age-based recovery of legacy checkouts.

use super::*;
use anyhow::Result;
use pretty_assertions::assert_eq;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::process::Stdio;

#[test]
fn cleanup_preserves_live_process_and_reclaims_its_abandoned_checkout() -> Result<()> {
    const CHILD_HOME: &str = "CODEX_MARKETPLACE_STAGING_CHILD_HOME";
    if let Some(home) = std::env::var_os(CHILD_HOME) {
        let staging = MarketplaceStagingDir::new(Path::new(&home))?;
        fs::create_dir_all(staging.path())?;
        fs::write(staging.path().join("payload"), "in-flight clone")?;
        println!("staging-ready");
        std::io::stdout().flush()?;
        std::io::stdin().read_exact(&mut [0])?;
        // Model process exit while a background upgrade still owns a checkout.
        std::process::exit(0);
    }

    let home = tempfile::tempdir()?;
    let mut child = codex_utils_process::background_command(std::env::current_exe()?)
        .args([
            "--exact",
            "marketplace_staging::tests::cleanup_preserves_live_process_and_reclaims_its_abandoned_checkout",
            "--nocapture",
        ])
        .env(CHILD_HOME, home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut ready = false;
    for line in stdout.lines() {
        if line?.contains("staging-ready") {
            ready = true;
            break;
        }
    }
    assert!(ready, "child should publish its checkout before cleanup");
    let staging_root = home.path().join(".staging");
    let active = fs::read_dir(&staging_root)?.next().unwrap()?.path();
    let payload = active.join("checkout/payload");

    // Age must never override the lifetime lock, even across processes.
    remove_stale_staging_dirs(home.path(), SystemTime::now() + LEGACY_ADD_MAX_AGE)?;
    assert_eq!(fs::read_to_string(&payload)?, "in-flight clone");
    let concurrent = MarketplaceStagingDir::new(home.path())?;
    fs::create_dir_all(concurrent.path())?;
    fs::write(concurrent.path().join("payload"), "second clone")?;

    child.stdin.take().unwrap().write_all(b"x")?;
    assert!(child.wait()?.success());
    assert!(payload.exists(), "process exit must bypass TempDir cleanup");
    cleanup_marketplace_staging(home.path());
    assert!(!active.exists());
    assert_eq!(
        fs::read_to_string(concurrent.path().join("payload"))?,
        "second clone"
    );
    drop(concurrent);
    assert_eq!(fs::read_dir(staging_root)?.count(), 0);
    Ok(())
}

#[test]
fn cleanup_reclaims_unpublished_and_aged_legacy_clones() -> Result<()> {
    let home = tempfile::tempdir()?;
    let staging_root = home.path().join(".staging");
    let legacy_add = staging_root.join("marketplace-add-old");
    let legacy_upgrade = staging_root.join("marketplace-upgrade-old");
    let unrelated = staging_root.join("unrelated");
    let foreign = staging_root.join("marketplace-other-os-active");
    let unpublished = staging_root.join(format!(
        "{}unpublished",
        MarketplaceStagingDir::directory_prefix()
    ));
    for path in [
        &legacy_add,
        &legacy_upgrade,
        &unrelated,
        &foreign,
        &unpublished,
    ] {
        fs::create_dir_all(path)?;
    }
    fs::write(legacy_add.join("payload"), "old add")?;
    fs::write(legacy_upgrade.join("payload"), "old upgrade")?;
    let unrelated_file =
        staging_root.join(format!("{}file", MarketplaceStagingDir::directory_prefix()));
    fs::write(&unrelated_file, "keep")?;

    // Base synthetic ages on the filesystem clock, which can differ from the wall clock.
    let now = fs::metadata(&legacy_add)?
        .modified()?
        .max(fs::metadata(&legacy_upgrade)?.modified()?);
    for (age, expected_legacy_dirs) in [
        (Duration::ZERO, [true, true]),
        (
            LEGACY_UPGRADE_MAX_AGE + Duration::from_secs(1),
            [true, false],
        ),
        (LEGACY_ADD_MAX_AGE + Duration::from_secs(1), [false, false]),
    ] {
        // A concurrent fork can retain the previous pass's locks until exec.
        // Wait for both locks and explicitly release them before cleanup.
        for name in [CLEANUP_LOCK, COORDINATION_LOCK] {
            let lock = open_lock(&home.path().join(name))?;
            lock.lock()?;
            lock.unlock()?;
        }
        remove_stale_staging_dirs(home.path(), now + age)?;
        assert!(!unpublished.exists());
        assert_eq!(
            [legacy_add.exists(), legacy_upgrade.exists()],
            expected_legacy_dirs,
            "legacy directories remaining after {age:?}"
        );
    }
    assert!(unrelated.exists());
    assert!(foreign.exists());
    assert_eq!(fs::read_to_string(unrelated_file)?, "keep");
    Ok(())
}

#[cfg(unix)]
#[test]
fn cleanup_does_not_follow_symlinks() -> Result<()> {
    let home = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let staging_root = home.path().join(".staging");
    fs::create_dir_all(&staging_root)?;
    fs::write(outside.path().join("payload"), "keep")?;
    let alias = staging_root.join(format!(
        "{}directory-link",
        MarketplaceStagingDir::directory_prefix()
    ));
    std::os::unix::fs::symlink(outside.path(), &alias)?;
    let legacy_alias = staging_root.join("marketplace-upgrade-link");
    std::os::unix::fs::symlink(outside.path(), &legacy_alias)?;
    let invalid_owner =
        staging_root.join(format!("{}link", MarketplaceStagingDir::directory_prefix()));
    fs::create_dir_all(&invalid_owner)?;
    std::os::unix::fs::symlink(
        outside.path().join("payload"),
        invalid_owner.join(OWNER_LOCK),
    )?;

    remove_stale_staging_dirs(home.path(), SystemTime::now() + LEGACY_ADD_MAX_AGE)?;
    assert_eq!(fs::read_to_string(outside.path().join("payload"))?, "keep");
    assert!(alias.is_symlink());
    assert!(legacy_alias.is_symlink());
    assert!(invalid_owner.exists());
    Ok(())
}

#[test]
fn cleanup_limits_each_pass_and_skips_contended_locks() -> Result<()> {
    let home = tempfile::tempdir()?;
    let staging_root = home.path().join(".staging");
    let count = MAX_CLEANUP_DIRECTORIES + 1;
    for index in 0..count {
        fs::create_dir_all(staging_root.join(format!(
            "{}{index}",
            MarketplaceStagingDir::directory_prefix()
        )))?;
    }

    for name in [CLEANUP_LOCK, COORDINATION_LOCK] {
        let lock = open_lock(&home.path().join(name))?;
        lock.lock()?;
        cleanup_marketplace_staging(home.path());
        assert_eq!(fs::read_dir(&staging_root)?.count(), count);
        if name == CLEANUP_LOCK {
            // A cleanup worker must not prevent another operation from creating a checkout.
            drop(MarketplaceStagingDir::new(home.path())?);
        }
    }

    for remaining in [1, 0] {
        // Other tests can fork while cleanup holds a lock. The child retains
        // that lock until exec, even after this process closes its file.
        // Wait for both locks, then release them before testing the batch size.
        for name in [CLEANUP_LOCK, COORDINATION_LOCK] {
            let lock = open_lock(&home.path().join(name))?;
            lock.lock()?;
            lock.unlock()?;
        }
        cleanup_marketplace_staging(home.path());
        assert_eq!(fs::read_dir(&staging_root)?.count(), remaining);
    }
    Ok(())
}
