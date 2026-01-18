use lsp_types::Uri;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

pub(crate) fn uri_to_file_path(uri: &Uri) -> Option<PathBuf> {
    let url = url::Url::parse(uri.as_str()).ok()?;
    url.to_file_path().ok()
}

pub(crate) fn uri_from_file_path(path: &Path) -> Option<Uri> {
    let url = url::Url::from_file_path(path).ok()?;
    Uri::from_str(url.as_str()).ok()
}

pub(crate) fn uri_from_directory_path(path: &Path) -> Option<Uri> {
    let url = url::Url::from_directory_path(path).ok()?;
    Uri::from_str(url.as_str()).ok()
}
