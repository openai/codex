use std::collections::BTreeSet;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

const FILE_DIRECTIVES: &[&str] = &[
    "certificatefile",
    "controlpath",
    "globalknownhostsfile",
    "identityagent",
    "identityfile",
    "pkcs11provider",
    "revokedhostkeys",
    "securitykeyprovider",
    "userknownhostsfile",
    "xauthlocation",
];
const COMMAND_DIRECTIVES: &[&str] = &["knownhostscommand", "localcommand", "proxycommand"];

pub(crate) fn ssh_config_dependency_profile_entry_names(user_profile: &Path) -> BTreeSet<String> {
    let ssh_dir = user_profile.join(".ssh");
    let mut entries = BTreeSet::from([".ssh".to_string()]);
    visit_config(
        &ssh_dir.join("config"),
        user_profile,
        &ssh_dir,
        &mut HashSet::new(),
        &mut entries,
        0,
    );
    entries
}

fn visit_config(
    path: &Path,
    user_profile: &Path,
    ssh_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    entries: &mut BTreeSet<String>,
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
                        record_profile_entry(user_profile, &include, entries);
                        visit_config(&include, user_profile, ssh_dir, visited, entries, depth + 1);
                    }
                }
            }
            key if FILE_DIRECTIVES.contains(&key) => {
                for arg in args {
                    if let Some(path) = profile_path_arg(&arg, user_profile, None) {
                        record_profile_entry(user_profile, &path, entries);
                    }
                }
            }
            key if COMMAND_DIRECTIVES.contains(&key) => {
                for arg in args {
                    for word in words(&arg) {
                        if let Some(path) = profile_path_arg(&word, user_profile, None) {
                            record_profile_entry(user_profile, &path, entries);
                        }
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
    let pattern = pattern_path.to_string_lossy();
    let Ok(paths) = glob::glob(&pattern) else {
        return vec![glob_parent(pattern_path)];
    };
    let paths: Vec<PathBuf> = paths.filter_map(Result::ok).collect();
    if paths.is_empty() {
        vec![glob_parent(pattern_path)]
    } else {
        paths
    }
}

fn directive(line: &str) -> Option<(String, Vec<String>)> {
    let mut words = words(line);
    let first = words.first()?.clone();
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
        Some((words.remove(0), words))
    }
}

fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quote = false;
    let mut chars = line.chars();

    while let Some(ch) = chars.next() {
        match ch {
            '#' if !quote => break,
            '"' => quote = !quote,
            '\\' if quote => word.extend(chars.next()),
            ch if ch.is_whitespace() && !quote => {
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

fn record_profile_entry(user_profile: &Path, path: &Path, entries: &mut BTreeSet<String>) {
    let profile = user_profile.to_string_lossy().replace('\\', "/");
    let path = path.to_string_lossy().replace('\\', "/");
    let profile = profile.trim_end_matches('/');
    let relative = if path.eq_ignore_ascii_case(profile) {
        ""
    } else {
        let prefix = format!("{profile}/");
        path.strip_prefix(&prefix).unwrap_or_default()
    };
    if let Some(entry) = relative.split('/').find(|part| !part.is_empty()) {
        entries.insert(entry.to_string());
    }
}

fn glob_parent(path: PathBuf) -> PathBuf {
    let path = path.to_string_lossy();
    PathBuf::from(
        path.split(['*', '?', '['])
            .next()
            .unwrap_or_default()
            .trim_end_matches(['/', '\\']),
    )
}

#[cfg(test)]
mod tests {
    use super::ssh_config_dependency_profile_entry_names;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeSet;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn collects_file_directive_profile_entries() {
        let tmp = TempDir::new().expect("tempdir");
        let home = tmp.path();
        fs::create_dir_all(home.join(".ssh")).expect("create .ssh");
        fs::write(
            home.join(".ssh/config"),
            r#"
Host devbox
  IdentityFile ~/.keys/id_ed25519
  CertificateFile %d/.certs/devbox-cert.pub
  UserKnownHostsFile ${HOME}/.known_hosts_custom
  ControlPath ~/.ssh/control-%h-%p-%r
"#,
        )
        .expect("write config");

        assert_eq!(
            BTreeSet::from([
                ".certs".to_string(),
                ".keys".to_string(),
                ".known_hosts_custom".to_string(),
                ".ssh".to_string(),
            ]),
            ssh_config_dependency_profile_entry_names(home)
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
            BTreeSet::from([".included".to_string(), ".ssh".to_string()]),
            ssh_config_dependency_profile_entry_names(home)
        );
    }

    #[test]
    fn command_directives_only_record_explicit_profile_paths() {
        let tmp = TempDir::new().expect("tempdir");
        let home = tmp.path();
        fs::create_dir_all(home.join(".ssh")).expect("create .ssh");
        fs::write(
            home.join(".ssh/config"),
            r#"
Host devbox
  ProxyCommand ~/.helpers/proxy --state ${HOME}/.proxy-state %h %p
  KnownHostsCommand "%d/.known-hosts/bin" %H
"#,
        )
        .expect("write config");

        assert_eq!(
            BTreeSet::from([
                ".helpers".to_string(),
                ".known-hosts".to_string(),
                ".proxy-state".to_string(),
                ".ssh".to_string(),
            ]),
            ssh_config_dependency_profile_entry_names(home)
        );
    }
}
