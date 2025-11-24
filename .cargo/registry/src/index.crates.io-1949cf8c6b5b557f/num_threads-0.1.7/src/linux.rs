use std::fs;
use std::num::NonZeroUsize;

pub(crate) fn num_threads() -> Option<NonZeroUsize> {
    fs::read_to_string("/proc/self/stat")
        .ok()
        .as_ref()
        // Skip past the pid and (process name) fields
        .and_then(|stat| stat.rsplit(')').next())
        // 20th field, less the two we skipped
        .and_then(|rstat| rstat.split_whitespace().nth(17))
        .and_then(|num_threads| num_threads.parse::<usize>().ok())
        .and_then(NonZeroUsize::new)
}
