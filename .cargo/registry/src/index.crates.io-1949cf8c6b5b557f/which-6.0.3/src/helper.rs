use std::path::Path;

/// Check if given path has extension which in the given vector.
pub fn has_executable_extension<T: AsRef<Path>, S: AsRef<str>>(path: T, pathext: &[S]) -> bool {
    let ext = path.as_ref().extension().and_then(|e| e.to_str());
    match ext {
        Some(ext) => pathext
            .iter()
            .any(|e| ext.eq_ignore_ascii_case(&e.as_ref()[1..])),
        _ => false,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extension_in_extension_vector() {
        // Case insensitive
        assert!(has_executable_extension(
            PathBuf::from("foo.exe"),
            &[".COM", ".EXE", ".CMD"]
        ));

        assert!(has_executable_extension(
            PathBuf::from("foo.CMD"),
            &[".COM", ".EXE", ".CMD"]
        ));
    }

    #[test]
    fn test_extension_not_in_extension_vector() {
        assert!(!has_executable_extension(
            PathBuf::from("foo.bar"),
            &[".COM", ".EXE", ".CMD"]
        ));
    }
}
