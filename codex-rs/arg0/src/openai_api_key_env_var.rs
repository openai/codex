const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";

pub(crate) fn extract_locked_openai_api_key() -> Option<&'static str> {
    match std::env::var(OPENAI_API_KEY_ENV_VAR) {
        Ok(key) => {
            if key.is_empty() {
                return None;
            }

            // Safety: modifying environment variables is only done before new
            // threads are spawned.
            clear_api_key_env_var();

            // into_boxed_str() may reallocate, so only lock the memory after
            // the final allocation is known.
            let leaked: &'static mut str = Box::leak(key.into_boxed_str());
            mlock_str(leaked);
            Some(leaked)
        }
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            // Cannot possibly be a valid API key, but we will clear it anyway.
            clear_api_key_env_var();

            None
        }
    }
}

/// Note this does not guarantee that the memory is wiped, only that the
/// environment variable is removed from this process's environment.
fn clear_api_key_env_var() {
    unsafe {
        std::env::remove_var(OPENAI_API_KEY_ENV_VAR);
    }
}

#[cfg(unix)]
fn mlock_str(value: &str) {
    use libc::_SC_PAGESIZE;
    use libc::c_void;
    use libc::mlock;
    use libc::sysconf;

    if value.is_empty() {
        return;
    }

    // Safety: we only read the pointer and length for mlock bookkeeping.
    let page_size = unsafe { sysconf(_SC_PAGESIZE) };
    if page_size <= 0 {
        return;
    }
    let page_size = page_size as usize;
    if page_size == 0 {
        return;
    }

    let addr = value.as_ptr() as usize;
    let len = value.len();
    let start = addr & !(page_size - 1);
    let addr_end = match addr.checked_add(len) {
        Some(v) => match v.checked_add(page_size - 1) {
            Some(total) => total,
            None => return,
        },
        None => return,
    };
    let end = addr_end & !(page_size - 1);
    let size = end.saturating_sub(start);
    if size == 0 {
        return;
    }

    // Best-effort; ignore failures because mlock may require privileges.
    let _ = unsafe { mlock(start as *const c_void, size) };
}

#[cfg(not(unix))]
fn mlock_str(_value: &str) {}
