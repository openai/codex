use std::path::Path;
use std::path::PathBuf;

use crate::registry::ServerRegistry;
use crate::registry::ServerSpec;

#[derive(Debug, Clone)]
pub struct DetectedServer<'a> {
    pub spec: &'a ServerSpec,
    pub root: PathBuf,
}

pub fn detect_servers_for_file<'a>(
    registry: &'a ServerRegistry,
    file_path: &Path,
) -> Vec<DetectedServer<'a>> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);

    let Some(extension) = extension else {
        return Vec::new();
    };

    registry
        .specs()
        .iter()
        .filter(|spec| {
            spec.extensions
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case(&extension))
        })
        .filter_map(|spec| {
            find_root_with_markers(file_path, &spec.markers)
                .map(|root| DetectedServer { spec, root })
        })
        .collect()
}

pub fn find_root_with_markers(start: &Path, markers: &[&str]) -> Option<PathBuf> {
    let start_dir = if start.is_dir() {
        start
    } else {
        start.parent()?
    };

    for ancestor in start_dir.ancestors() {
        for marker in markers {
            if ancestor.join(marker).exists() {
                return Some(ancestor.to_path_buf());
            }
        }
    }

    None
}
