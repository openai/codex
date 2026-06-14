use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;

pub(crate) fn resolve_bare_program_on_path(
    argv: &[String],
    cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Vec<String> {
    let Some(program) = argv.first() else {
        return Vec::new();
    };

    if !is_bare_program_name(program) {
        return argv.to_vec();
    }

    let search_path = env_map
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("PATH"))
        .map(|(_, value)| OsString::from(value));

    let Ok(resolved) = which::which_in(program, search_path.as_ref(), cwd) else {
        return argv.to_vec();
    };

    let mut resolved_argv = argv.to_vec();
    resolved_argv[0] = resolved.to_string_lossy().into_owned();
    resolved_argv
}

fn is_bare_program_name(program: &str) -> bool {
    !program.is_empty() && !program.contains(['\\', '/', ':'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolves_bare_programs_from_path() {
        let temp = TempDir::new().expect("tempdir");
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        let exe_path = bin_dir.join("go.exe");
        fs::write(&exe_path, b"").expect("write executable stub");

        let env_map = HashMap::from([(
            "PATH".to_string(),
            bin_dir.to_string_lossy().into_owned(),
        )]);
        let argv = vec!["go".to_string(), "version".to_string()];

        let resolved = resolve_bare_program_on_path(&argv, temp.path(), &env_map);

        assert_eq!(resolved[0], exe_path.to_string_lossy());
        assert_eq!(resolved[1], "version");
    }

    #[test]
    fn leaves_explicit_paths_unchanged() {
        let env_map = HashMap::from([("PATH".to_string(), "C:\\tools".to_string())]);
        let argv = vec![r"C:\tools\go.exe".to_string(), "version".to_string()];

        let resolved = resolve_bare_program_on_path(&argv, Path::new("."), &env_map);

        assert_eq!(resolved, argv);
    }

    #[test]
    fn leaves_missing_programs_unchanged() {
        let env_map = HashMap::from([("PATH".to_string(), "C:\\tools".to_string())]);
        let argv = vec!["missing-tool".to_string(), "--help".to_string()];

        let resolved = resolve_bare_program_on_path(&argv, Path::new("."), &env_map);

        assert_eq!(resolved, argv);
    }
}
