use std::path::PathBuf;

pub fn normalize_pasted_path(pasted: &str) -> Option<PathBuf> {
    // file:// URL → filesystem path
    if let Ok(url) = url::Url::parse(pasted)
        && url.scheme() == "file"
    {
        return url.to_file_path().ok();
    }

    // shell-escaped single path → unescaped
    let parts: Vec<String> = shlex::Shlex::new(pasted).collect();
    if parts.len() == 1 {
        return parts.into_iter().next().map(PathBuf::from);
    }

    None
}

pub fn get_img_format_label(path: PathBuf) -> String {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
    {
        Some(ext) if ext == "png" => "PNG",
        Some(ext) if ext == "jpg" || ext == "jpeg" => "JPEG",
        _ => "IMG",
    }
    .into()
}

#[cfg(target_os = "macos")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_file_url() {
        let input = "file:///tmp/example.png";
        let result = normalize_pasted_path(input).expect("should parse file URL");
        assert_eq!(result, PathBuf::from("/tmp/example.png"));
    }

    #[test]
    fn normalize_shell_escaped_single_path() {
        let input = "/home/user/My\\ File.png";
        let result = normalize_pasted_path(input).expect("should unescape shell-escaped path");
        assert_eq!(result, PathBuf::from("/home/user/My File.png"));
    }

    #[test]
    fn normalize_simple_quoted_path_fallback() {
        let input = "\"/home/user/My File.png\"";
        let result = normalize_pasted_path(input).expect("should trim simple quotes");
        assert_eq!(result, PathBuf::from("/home/user/My File.png"));
    }

    #[test]
    fn normalize_single_quoted_unix_path() {
        let input = "'/home/user/My File.png'";
        let result = normalize_pasted_path(input).expect("should trim single quotes via shlex");
        assert_eq!(result, PathBuf::from("/home/user/My File.png"));
    }

    #[test]
    fn normalize_multiple_tokens_returns_none() {
        // Two tokens after shell splitting → not a single path
        let input = "/home/user/a\\ b.png /home/user/c.png";
        let result = normalize_pasted_path(input);
        assert!(result.is_none());
    }

    #[test]
    fn img_format_label_png_jpeg_unknown() {
        assert_eq!(get_img_format_label(PathBuf::from("/a/b/c.PNG")), "PNG");
        assert_eq!(get_img_format_label(PathBuf::from("/a/b/c.jpg")), "JPEG");
        assert_eq!(get_img_format_label(PathBuf::from("/a/b/c.JPEG")), "JPEG");
        assert_eq!(get_img_format_label(PathBuf::from("/a/b/c")), "IMG");
        assert_eq!(get_img_format_label(PathBuf::from("/a/b/c.webp")), "IMG");
    }
}

#[cfg(windows)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_single_quoted_windows_path() {
        let input = r"'C:\Users\Alice\My File.jpeg'";
        let result =
            normalize_pasted_path(input).expect("should trim single quotes on windows path");
        assert_eq!(result, PathBuf::from(r"C:\Users\Alice\My File.jpeg"));
    }

    #[test]
    fn img_format_label_with_windows_style_paths() {
        assert_eq!(get_img_format_label(PathBuf::from(r"C:\a\b\c.PNG")), "PNG");
        assert_eq!(
            get_img_format_label(PathBuf::from(r"C:\a\b\c.jpeg")),
            "JPEG"
        );
        assert_eq!(get_img_format_label(PathBuf::from(r"C:\a\b\noext")), "IMG");
    }
}
