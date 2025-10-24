use std::path::Path;
use std::path::PathBuf;

use shlex::try_join;

pub(crate) fn escape_command(command: &[String]) -> String {
    try_join(command.iter().map(|s| s.as_str())).unwrap_or_else(|_| command.join(" "))
}

pub(crate) fn strip_bash_lc_and_escape(command: &[String]) -> String {
    match command {
        [first, second, third] if first == "bash" && second == "-lc" => third.clone(),
        _ => escape_command(command),
    }
}

/// If `path` is absolute and inside $HOME, return the part *after* the home
/// directory; otherwise, return the path as-is. Note if `path` is the homedir,
/// this will return and empty path.
pub(crate) fn relativize_to_home<P>(path: P) -> Option<PathBuf>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if !path.is_absolute() {
        // If the path is not absolute, we can’t do anything with it.
        return None;
    }

    if let Some(home_dir) = std::env::var_os("HOME").map(PathBuf::from)
        && let Ok(rel) = path.strip_prefix(&home_dir)
    {
        return Some(rel.to_path_buf());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_command() {
        let args = vec!["foo".into(), "bar baz".into(), "weird&stuff".into()];
        let cmdline = escape_command(&args);
        assert_eq!(cmdline, "foo 'bar baz' 'weird&stuff'");
    }

    #[test]
    fn test_strip_bash_lc_and_escape() {
        let args = vec!["bash".into(), "-lc".into(), "echo hello".into()];
        let cmdline = strip_bash_lc_and_escape(&args);
        assert_eq!(cmdline, "echo hello");
    }
}
