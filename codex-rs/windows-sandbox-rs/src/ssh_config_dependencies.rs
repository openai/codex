use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

const SSH_PROFILE_PATH_DIRECTIVES: &[&str] = &[
    "certificatefile",
    "controlpath",
    "globalknownhostsfile",
    "identityagent",
    "identityfile",
    "revokedhostkeys",
    "userknownhostsfile",
];

pub(crate) fn ssh_config_dependency_paths(user_profile: &Path) -> Vec<PathBuf> {
    let ssh_dir = user_profile.join(".ssh");
    let mut paths = vec![ssh_dir.join("config")];
    visit_config(
        &ssh_dir.join("config"),
        user_profile,
        &ssh_dir,
        &mut HashSet::new(),
        &mut paths,
        /*depth*/ 0,
    );
    paths
}

fn visit_config(
    path: &Path,
    user_profile: &Path,
    ssh_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    paths: &mut Vec<PathBuf>,
    depth: usize,
) {
    if depth == 32 {
        return;
    }
    let key = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(key) {
        return;
    }

    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    for (key, args) in contents.lines().filter_map(directive) {
        match key.to_ascii_lowercase().as_str() {
            "include" => {
                for arg in args {
                    for include in include_paths(&arg, user_profile, ssh_dir) {
                        paths.push(include.clone());
                        visit_config(&include, user_profile, ssh_dir, visited, paths, depth + 1);
                    }
                }
            }
            key if SSH_PROFILE_PATH_DIRECTIVES.contains(&key) => {
                for arg in args {
                    if let Some(path) =
                        profile_path_arg(&arg, user_profile, /*relative_base*/ None)
                    {
                        paths.push(path);
                    }
                }
            }
            _ => {}
        }
    }
}

fn include_paths(arg: &str, user_profile: &Path, ssh_dir: &Path) -> Vec<PathBuf> {
    let Some(pattern_path) = profile_path_arg(arg, user_profile, Some(ssh_dir)) else {
        return Vec::new();
    };
    let pattern = pattern_path.to_string_lossy().replace('\\', "/");
    let Ok(paths) = glob::glob(&pattern) else {
        return Vec::new();
    };
    paths.filter_map(Result::ok).collect()
}

fn directive(line: &str) -> Option<(String, Vec<String>)> {
    let mut words = words(line);
    let first = words.first()?.to_string();
    if let Some((key, value)) = first.split_once('=')
        && !key.is_empty()
    {
        let mut args = Vec::new();
        if !value.is_empty() {
            args.push(value.to_string());
        }
        args.extend(words.drain(1..));
        Some((key.to_string(), args))
    } else {
        let key = words.remove(0);
        if let Some(arg) = words.first_mut()
            && let Some(value) = arg.strip_prefix('=')
        {
            *arg = value.to_string();
        }
        words.retain(|arg| !arg.is_empty());
        Some((key, words))
    }
}

fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '#' if quote.is_none() => break,
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            '\\' => {
                if let Some(&next) = chars.peek() {
                    if matches!(next, '\'' | '"' | '\\') || (quote.is_none() && next == ' ') {
                        if let Some(escaped) = chars.next() {
                            word.push(escaped);
                        }
                    } else {
                        word.push(ch);
                    }
                } else {
                    word.push(ch);
                }
            }
            ch if ch.is_whitespace() && quote.is_none() => {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
            }
            ch => word.push(ch),
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

fn profile_path_arg(
    arg: &str,
    user_profile: &Path,
    relative_base: Option<&Path>,
) -> Option<PathBuf> {
    if arg.eq_ignore_ascii_case("none") {
        return None;
    }
    if arg == "~" || arg == "%d" || arg == "${HOME}" {
        return Some(user_profile.to_path_buf());
    }
    if let Some(rest) = arg
        .strip_prefix("~/")
        .or_else(|| arg.strip_prefix(r"~\"))
        .or_else(|| arg.strip_prefix("%d/"))
        .or_else(|| arg.strip_prefix(r"%d\"))
        .or_else(|| arg.strip_prefix("${HOME}/"))
        .or_else(|| arg.strip_prefix(r"${HOME}\"))
    {
        return Some(user_profile.join(rest));
    }

    let path = PathBuf::from(arg);
    if path.is_absolute() {
        Some(path)
    } else {
        relative_base.map(|base| base.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::ssh_config_dependency_paths;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn collects_path_directive_profile_entries() {
        let tmp = TempDir::new().expect("tempdir");
        let home = tmp.path();
        fs::create_dir_all(home.join(".ssh")).expect("create .ssh");
        fs::write(
            home.join(".ssh/config"),
            r#"
Host devbox
  IdentityFile ~/.keys/id_ed25519
  IdentityFile '~/.keys/quoted key'
  CertificateFile = %d/.certs/devbox-cert.pub
  UserKnownHostsFile ${HOME}/.known_hosts_custom
  GlobalKnownHostsFile ~/.global_known_hosts
  ControlPath ~/.control/%h-%p-%r
  IdentityAgent=%d/.agent/socket
  RevokedHostKeys ~/.revoked/keys
"#,
        )
        .expect("write config");

        assert_eq!(
            vec![
                home.join(".ssh/config"),
                home.join(".keys/id_ed25519"),
                home.join(".keys/quoted key"),
                home.join(".certs/devbox-cert.pub"),
                home.join(".known_hosts_custom"),
                home.join(".global_known_hosts"),
                home.join(".control/%h-%p-%r"),
                home.join(".agent/socket"),
                home.join(".revoked/keys"),
            ],
            slash_paths(ssh_config_dependency_paths(home))
        );
    }

    #[test]
    fn recursively_collects_include_dependencies() {
        let tmp = TempDir::new().expect("tempdir");
        let home = tmp.path();
        let ssh_dir = home.join(".ssh");
        fs::create_dir_all(ssh_dir.join("conf.d")).expect("create conf.d");
        fs::write(ssh_dir.join("config"), "Include conf.d/*.conf\n").expect("write config");
        fs::write(
            ssh_dir.join("conf.d/devbox.conf"),
            "CertificateFile ~/.included/devbox-cert.pub\n",
        )
        .expect("write include");

        assert_eq!(
            vec![
                home.join(".ssh/config"),
                ssh_dir.join("conf.d/devbox.conf"),
                home.join(".included/devbox-cert.pub"),
            ],
            slash_paths(ssh_config_dependency_paths(home))
        );
    }

    fn slash_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
        paths
            .into_iter()
            .map(|path| PathBuf::from(path.to_string_lossy().replace('\\', "/")))
            .collect()
    }
}
