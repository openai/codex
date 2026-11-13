//! Read clipboard images and text on the TUI host. Text reads run on the clipboard worker.

#[cfg(target_os = "macos")]
mod macos;
pub(crate) mod text;

use std::path::Path;
use std::path::PathBuf;
use tempfile::Builder;

#[derive(Debug, Clone)]
pub enum PasteImageError {
    ClipboardUnavailable(String),
    NoImage(String),
    EncodeFailed(String),
    IoError(String),
}

impl std::fmt::Display for PasteImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PasteImageError::ClipboardUnavailable(msg) => write!(f, "clipboard unavailable: {msg}"),
            PasteImageError::NoImage(msg) => write!(f, "no image on clipboard: {msg}"),
            PasteImageError::EncodeFailed(msg) => write!(f, "could not encode image: {msg}"),
            PasteImageError::IoError(msg) => write!(f, "io error: {msg}"),
        }
    }
}
impl std::error::Error for PasteImageError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodedImageFormat {
    Png,
    Jpeg,
    Other,
}

impl EncodedImageFormat {
    pub fn label(self) -> &'static str {
        match self {
            EncodedImageFormat::Png => "PNG",
            EncodedImageFormat::Jpeg => "JPEG",
            EncodedImageFormat::Other => "IMG",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PastedImageInfo {
    pub width: u32,
    pub height: u32,
    pub encoded_format: EncodedImageFormat, // Always PNG for now.
}

/// Read the first clipboard image and encode it as a PNG attachment.
#[cfg(not(target_os = "android"))]
pub fn paste_image_as_png() -> Result<(Vec<u8>, PastedImageInfo), PasteImageError> {
    let _span = tracing::debug_span!("paste_image_as_png").entered();
    tracing::debug!("attempting clipboard image read");
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| PasteImageError::ClipboardUnavailable(e.to_string()))?;
    let image = read_clipboard_image(&mut clipboard)?;

    let span = tracing::debug_span!("encode_image", byte_length = tracing::field::Empty).entered();
    let mut png = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| PasteImageError::EncodeFailed(e.to_string()))?;
    span.record("byte_length", png.get_ref().len());
    let info = PastedImageInfo {
        width: image.width(),
        height: image.height(),
        encoded_format: EncodedImageFormat::Png,
    };
    Ok((png.into_inner(), info))
}

#[cfg(not(target_os = "android"))]
fn read_clipboard_image(
    clipboard: &mut arboard::Clipboard,
) -> Result<image::DynamicImage, PasteImageError> {
    // A copied file takes precedence over its clipboard preview.
    let files = clipboard.get().file_list().unwrap_or_default();
    if let Some(image) = files.into_iter().find_map(|path| {
        let reader = image::ImageReader::open(path).ok()?;
        crate::image_decode::decode_oriented(reader, image::Limits::default()).ok()
    }) {
        tracing::debug!(
            "clipboard image opened from file: {}x{}",
            image.width(),
            image.height()
        );
        return Ok(image);
    }

    #[cfg(target_os = "macos")]
    if let Some(image) = macos::clipboard_image() {
        return Ok(image);
    }

    let _span = tracing::debug_span!("get_image").entered();
    let bitmap = clipboard.get_image().map_err(|e| {
        if cfg!(windows) && !matches!(e, arboard::Error::ContentNotAvailable) {
            PasteImageError::ClipboardUnavailable(e.to_string())
        } else {
            PasteImageError::NoImage(e.to_string())
        }
    })?;
    let width = bitmap.width as u32;
    let height = bitmap.height as u32;
    tracing::debug!("clipboard image opened from image: {width}x{height}");
    let rgba = image::RgbaImage::from_raw(width, height, bitmap.bytes.into_owned())
        .ok_or_else(|| PasteImageError::EncodeFailed("invalid RGBA buffer".into()))?;
    Ok(image::DynamicImage::ImageRgba8(rgba))
}

/// Android/Termux does not support arboard; return a clear error.
#[cfg(target_os = "android")]
pub fn paste_image_as_png() -> Result<(Vec<u8>, PastedImageInfo), PasteImageError> {
    Err(PasteImageError::ClipboardUnavailable(
        "clipboard image paste is unsupported on Android".into(),
    ))
}

/// Convenience: write to a temp file and return its path + info.
#[cfg(not(target_os = "android"))]
pub fn paste_image_to_temp_png() -> Result<(PathBuf, PastedImageInfo), PasteImageError> {
    // First attempt: read image from system clipboard via arboard (native paths or image data).
    match paste_image_as_png() {
        Ok((png, info)) => {
            // Create a unique temporary file with a .png suffix to avoid collisions.
            let tmp = Builder::new()
                .prefix("codex-clipboard-")
                .suffix(".png")
                .tempfile()
                .map_err(|e| PasteImageError::IoError(e.to_string()))?;
            std::fs::write(tmp.path(), &png)
                .map_err(|e| PasteImageError::IoError(e.to_string()))?;
            // Persist the file (so it remains after the handle is dropped) and return its PathBuf.
            let (_file, path) = tmp
                .keep()
                .map_err(|e| PasteImageError::IoError(e.error.to_string()))?;
            Ok((path, info))
        }
        Err(e) => {
            #[cfg(target_os = "linux")]
            {
                try_wsl_clipboard_fallback(&e).or(Err(e))
            }
            #[cfg(not(target_os = "linux"))]
            {
                Err(e)
            }
        }
    }
}

/// Attempt WSL fallback for clipboard image paste.
///
/// If clipboard is unavailable (common under WSL because arboard cannot access
/// the Windows clipboard), attempt a WSL fallback that calls PowerShell on the
/// Windows side to write the clipboard image to a temporary file, then return
/// the corresponding WSL path.
#[cfg(target_os = "linux")]
fn try_wsl_clipboard_fallback(
    error: &PasteImageError,
) -> Result<(PathBuf, PastedImageInfo), PasteImageError> {
    use PasteImageError::ClipboardUnavailable;
    use PasteImageError::NoImage;

    if !is_probably_wsl() || !matches!(error, ClipboardUnavailable(_) | NoImage(_)) {
        return Err(error.clone());
    }

    tracing::debug!("attempting Windows PowerShell clipboard fallback");
    let Some(win_path) = try_dump_windows_clipboard_image() else {
        return Err(error.clone());
    };

    tracing::debug!("powershell produced path: {}", win_path);
    let Some(mapped_path) = convert_windows_path_to_wsl(&win_path) else {
        return Err(error.clone());
    };

    let Ok((w, h)) = image::image_dimensions(&mapped_path) else {
        return Err(error.clone());
    };

    // Return the mapped path directly without copying.
    // The file will be read and base64-encoded during serialization.
    Ok((
        mapped_path,
        PastedImageInfo {
            width: w,
            height: h,
            encoded_format: EncodedImageFormat::Png,
        },
    ))
}

/// Try to call a Windows PowerShell command (several common names) to save the
/// clipboard image to a temporary PNG and return the Windows path to that file.
/// Returns None if no command succeeded or no image was present.
#[cfg(target_os = "linux")]
fn try_dump_windows_clipboard_image() -> Option<String> {
    // Powershell script: save image from clipboard to a temp png and print the path.
    // Force UTF-8 output to avoid encoding issues between powershell.exe (UTF-16LE default)
    // and pwsh (UTF-8 default).
    let script = r#"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $img = Get-Clipboard -Format Image; if ($img -ne $null) { $p=[System.IO.Path]::GetTempFileName(); $p = [System.IO.Path]::ChangeExtension($p,'png'); $img.Save($p,[System.Drawing.Imaging.ImageFormat]::Png); Write-Output $p } else { exit 1 }"#;

    for cmd in ["powershell.exe", "pwsh", "powershell"] {
        match std::process::Command::new(cmd)
            .args(["-NoProfile", "-Command", script])
            .output()
        {
            // Executing PowerShell command
            Ok(output) => {
                if output.status.success() {
                    // Decode as UTF-8 (forced by the script above).
                    let win_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !win_path.is_empty() {
                        tracing::debug!("{} saved clipboard image to {}", cmd, win_path);
                        return Some(win_path);
                    }
                } else {
                    tracing::debug!("{} returned non-zero status", cmd);
                }
            }
            Err(err) => {
                tracing::debug!("{} not executable: {}", cmd, err);
            }
        }
    }
    None
}

#[cfg(target_os = "android")]
pub fn paste_image_to_temp_png() -> Result<(PathBuf, PastedImageInfo), PasteImageError> {
    // Keep error consistent with paste_image_as_png.
    Err(PasteImageError::ClipboardUnavailable(
        "clipboard image paste is unsupported on Android".into(),
    ))
}

/// Normalize pasted text for a single-line search query.
pub(crate) fn normalize_pasted_search_query(pasted: &str) -> Option<String> {
    let normalized = pasted.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty()).then_some(normalized)
}

/// Normalize pasted text that may represent a filesystem path.
///
/// Supports:
/// - `file://` URLs (converted to local paths)
/// - Windows/UNC paths
/// - shell-escaped single paths (via `shlex`)
pub fn normalize_pasted_path(pasted: &str) -> Option<PathBuf> {
    let pasted = pasted.trim();
    let unquoted = pasted
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| pasted.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(pasted);

    // file:// URL → filesystem path
    if let Ok(url) = url::Url::parse(unquoted)
        && url.scheme() == "file"
    {
        return url.to_file_path().ok();
    }

    // TODO: We'll improve the implementation/unit tests over time, as appropriate.
    // Possibly use typed-path: https://github.com/openai/codex/pull/2567/commits/3cc92b78e0a1f94e857cf4674d3a9db918ed352e
    //
    // Bypass POSIX shlex for Windows paths, where backslashes are separators.
    let path = if is_windows_drive_path(unquoted) || unquoted.starts_with("\\\\") {
        unquoted.to_owned()
    } else {
        let mut parts = shlex::Shlex::new(pasted);
        let part = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        part
    };

    #[cfg(target_os = "linux")]
    if is_windows_drive_path(&path) && is_probably_wsl() {
        return convert_windows_path_to_wsl(&path);
    }

    Some(PathBuf::from(path))
}

#[cfg(target_os = "linux")]
pub(crate) fn is_probably_wsl() -> bool {
    // Primary: Check /proc/version for "microsoft" or "WSL" (most reliable for standard WSL).
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
        let version_lower = version.to_lowercase();
        if version_lower.contains("microsoft") || version_lower.contains("wsl") {
            return true;
        }
    }

    // Fallback: Check WSL environment variables. This handles edge cases like
    // custom Linux kernels installed in WSL where /proc/version may not contain
    // "microsoft" or "WSL".
    std::env::var_os("WSL_DISTRO_NAME").is_some() || std::env::var_os("WSL_INTEROP").is_some()
}

#[cfg(target_os = "linux")]
fn convert_windows_path_to_wsl(input: &str) -> Option<PathBuf> {
    convert_windows_path(input, Path::new("/bin/wslpath"), || {
        std::fs::read("/proc/self/mountinfo")
    })
}

#[cfg(target_os = "linux")]
fn convert_windows_path(
    input: &str,
    wslpath: &Path,
    read_mountinfo: impl FnOnce() -> std::io::Result<Vec<u8>>,
) -> Option<PathBuf> {
    use std::ffi::OsStr;
    use std::io::ErrorKind;
    use std::os::unix::ffi::OsStrExt;
    use std::process::Command;
    use std::process::Stdio;
    use std::time::Duration;
    use std::time::Instant;

    if !is_windows_drive_path(input) {
        return None;
    }

    match std::fs::metadata(wslpath) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return windows_path_on_default_mount(input, &read_mountinfo().ok()?);
        }
        Err(_) => return None,
    }

    // Use the WSL-installed helper without looking in the working directory or PATH.
    let mut command = Command::new(wslpath);
    command
        .args(["-u", input])
        // wslpath reads its working directory even for absolute paths.
        .current_dir("/")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    let output = crate::command_output::command_stdout_before_deadline(
        &mut command,
        Instant::now() + Duration::from_secs(/*secs*/ 1),
    )?;
    let path = Path::new(OsStr::from_bytes(output.strip_suffix(b"\n")?));
    path.is_absolute().then(|| path.to_path_buf())
}

#[cfg(target_os = "linux")]
fn windows_path_on_default_mount(input: &str, mountinfo: &[u8]) -> Option<PathBuf> {
    let drive = char::from(input.as_bytes()[0].to_ascii_lowercase());
    let mount_point = format!("/mnt/{drive}");
    // Field five is the mount point in this process's mount namespace.
    // An ordinary directory at the same path is insufficient.
    if !mountinfo
        .split(|byte| *byte == b'\n')
        .any(|line| line.split(|byte| *byte == b' ').nth(4) == Some(mount_point.as_bytes()))
    {
        return None;
    }

    let mut path = PathBuf::from(&mount_point);
    for component in input[2..].split(['\\', '/']) {
        match component {
            "" | "." => {}
            ".." => {
                if path == Path::new(&mount_point) {
                    return None;
                }
                path.pop();
            }
            component => path.push(component),
        }
    }
    Some(path)
}

fn is_windows_drive_path(input: &str) -> bool {
    matches!(input.as_bytes(), [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic())
}

/// Infer an image format for the provided path based on its extension.
pub fn pasted_image_format(path: &Path) -> EncodedImageFormat {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => EncodedImageFormat::Png,
        Some("jpg") | Some("jpeg") => EncodedImageFormat::Jpeg,
        _ => EncodedImageFormat::Other,
    }
}

#[cfg(test)]
mod pasted_search_query_tests {
    use super::*;

    #[test]
    fn collapses_whitespace() {
        assert_eq!(
            normalize_pasted_search_query("  alpha\n\tbeta\r\n gamma  "),
            Some(String::from("alpha beta gamma"))
        );
    }
}

#[cfg(test)]
mod pasted_paths_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[cfg(not(windows))]
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
    fn pasted_image_format_png_jpeg_unknown() {
        assert_eq!(
            pasted_image_format(Path::new("/a/b/c.PNG")),
            EncodedImageFormat::Png
        );
        assert_eq!(
            pasted_image_format(Path::new("/a/b/c.jpg")),
            EncodedImageFormat::Jpeg
        );
        assert_eq!(
            pasted_image_format(Path::new("/a/b/c.JPEG")),
            EncodedImageFormat::Jpeg
        );
        assert_eq!(
            pasted_image_format(Path::new("/a/b/c")),
            EncodedImageFormat::Other
        );
        assert_eq!(
            pasted_image_format(Path::new("/a/b/c.webp")),
            EncodedImageFormat::Other
        );
    }

    #[test]
    fn normalize_windows_paths_with_spaces() {
        let unquoted = r"C:\Users\Alice\My File.jpeg";
        let expected = Some(PathBuf::from(unquoted));
        #[cfg(target_os = "linux")]
        let expected = if is_probably_wsl() {
            use std::os::unix::ffi::OsStringExt;

            // A container on a WSL kernel may lack the distro's wslpath symlink.
            let Ok(mut output) = std::process::Command::new("/bin/wslpath")
                .args(["-u", unquoted])
                .current_dir("/")
                .output()
            else {
                return;
            };
            output.status.success().then(|| {
                assert_eq!(output.stdout.pop(), Some(b'\n'));
                PathBuf::from(std::ffi::OsString::from_vec(output.stdout))
            })
        } else {
            expected
        };

        for input in [
            unquoted,
            r"'C:\Users\Alice\My File.jpeg'",
            r#""C:\Users\Alice\My File.jpeg""#,
        ] {
            assert_eq!(normalize_pasted_path(input), expected, "{input}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn windows_path_fallback_requires_missing_wslpath_and_default_mount() {
        let directory = tempfile::tempdir().unwrap();
        let missing_wslpath = directory.path().join("wslpath");
        let mountinfo = b"24 23 0:21 / /mnt/c rw - 9p none rw\n\
                          25 23 0:22 / /mnt/d/Users rw - 9p none rw\n";

        assert_eq!(
            convert_windows_path(r"C:\Users\Alice/My File.jpeg", &missing_wslpath, || {
                Ok(mountinfo.to_vec())
            }),
            Some(PathBuf::from("/mnt/c/Users/Alice/My File.jpeg"))
        );
        assert_eq!(
            convert_windows_path(r"D:\Users\Alice\My File.jpeg", &missing_wslpath, || {
                Ok(mountinfo.to_vec())
            }),
            None
        );
        assert_eq!(
            convert_windows_path(r"C:\..\etc\image.png", &missing_wslpath, || {
                Ok(mountinfo.to_vec())
            }),
            None
        );
    }

    #[test]
    fn normalize_unc_windows_path() {
        let input = r"\\\\server\\share\\folder\\file.jpg";
        let result = normalize_pasted_path(input).expect("should accept UNC windows path");
        assert_eq!(
            result,
            PathBuf::from(r"\\\\server\\share\\folder\\file.jpg")
        );
    }

    #[test]
    fn pasted_image_format_with_windows_style_paths() {
        assert_eq!(
            pasted_image_format(Path::new(r"C:\\a\\b\\c.PNG")),
            EncodedImageFormat::Png
        );
        assert_eq!(
            pasted_image_format(Path::new(r"C:\\a\\b\\c.jpeg")),
            EncodedImageFormat::Jpeg
        );
        assert_eq!(
            pasted_image_format(Path::new(r"C:\\a\\b\\noext")),
            EncodedImageFormat::Other
        );
    }
}
