use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::{io, iter::repeat_with};

use crate::error::IoResultExt;

fn tmpname(rng: &mut fastrand::Rng, prefix: &OsStr, suffix: &OsStr, rand_len: usize) -> OsString {
    let capacity = prefix
        .len()
        .saturating_add(suffix.len())
        .saturating_add(rand_len);
    let mut buf = OsString::with_capacity(capacity);
    buf.push(prefix);
    let mut char_buf = [0u8; 4];
    for c in repeat_with(|| rng.alphanumeric()).take(rand_len) {
        buf.push(c.encode_utf8(&mut char_buf));
    }
    buf.push(suffix);
    buf
}

pub fn create_helper<R>(
    base: &Path,
    prefix: &OsStr,
    suffix: &OsStr,
    random_len: usize,
    mut f: impl FnMut(PathBuf) -> io::Result<R>,
) -> io::Result<R> {
    // Make the path absolute. Otherwise, changing the current directory can invalidate a stored
    // path (causing issues when cleaning up temporary files.
    let mut base = base; // re-borrow to shrink lifetime
    let base_path_storage; // slot to store the absolute path, if necessary.
    if !base.is_absolute() {
        let cur_dir = std::env::current_dir()?;
        base_path_storage = cur_dir.join(base);
        base = &base_path_storage;
    }

    let num_retries = if random_len != 0 {
        crate::NUM_RETRIES
    } else {
        1
    };

    // We fork the fastrand rng.
    let mut rng = fastrand::Rng::new();
    for i in 0..num_retries {
        // If we fail to create the file the first three times, re-seed from system randomness in
        // case an attacker is predicting our randomness (fastrand is predictable). If re-seeding
        // doesn't help, either:
        //
        // 1. We have lots of temporary files, possibly created by an attacker but not necessarily.
        //    Re-seeding the randomness won't help here.
        // 2. We're failing to create random files for some other reason. This shouldn't be the case
        //    given that we're checking error kinds, but it could happen.
        #[cfg(all(
            feature = "getrandom",
            any(windows, unix, target_os = "redox", target_os = "wasi")
        ))]
        if i == 3 {
            if let Ok(seed) = getrandom::u64() {
                rng.seed(seed);
            }
        }
        let _ = i; // avoid unused variable warning for the above.

        let path = base.join(tmpname(&mut rng, prefix, suffix, random_len));
        return match f(path) {
            Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists && num_retries > 1 => continue,
            // AddrInUse can happen if we're creating a UNIX domain socket and
            // the path already exists.
            Err(ref e) if e.kind() == io::ErrorKind::AddrInUse && num_retries > 1 => continue,
            res => res,
        };
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many temporary files exist",
    ))
    .with_err_path(|| base)
}
