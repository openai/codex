use super::MAX_FRAGMENT_BYTES;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

const FRAGMENT_PLACEHOLDER: &str = "<!--__INLINE_VISUALIZATION_FRAGMENT__-->";
const VIEWER_DIRECTORY_NAME: &str = ".codex-viewers";

// Keep these assets in sync with the bundled visualize skill's browser renderer.
const VIEWER_STYLESHEET: &str = include_str!("assets/visualize.css");
const VIEWER_RUNTIME: &str = include_str!("assets/visualize.html");

const FRAME_CSP: &str = "default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' blob: data: https://cdnjs.cloudflare.com https://cdn.jsdelivr.net https://esm.sh https://fonts.bunny.net https://fonts.googleapis.com https://fonts.gstatic.com https://unpkg.com; style-src 'unsafe-inline' blob: data: https://cdnjs.cloudflare.com https://cdn.jsdelivr.net https://esm.sh https://fonts.bunny.net https://fonts.googleapis.com https://fonts.gstatic.com https://unpkg.com; img-src blob: data: https://cdnjs.cloudflare.com https://cdn.jsdelivr.net https://esm.sh https://fonts.bunny.net https://fonts.googleapis.com https://fonts.gstatic.com https://unpkg.com; font-src blob: data: https://cdnjs.cloudflare.com https://cdn.jsdelivr.net https://esm.sh https://fonts.bunny.net https://fonts.googleapis.com https://fonts.gstatic.com https://unpkg.com; media-src blob: data:; worker-src blob:; connect-src blob: data:; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
const SHELL_STYLE: &str = ":root{color-scheme:light dark;background:light-dark(rgb(255 255 255), rgb(24 24 24))}html,body{margin:0}body{box-sizing:border-box;padding:1rem;background:inherit}iframe{display:block;width:100%;max-width:736px;height:calc(100vh - 2rem);margin:0 auto;border:0}";

pub(super) fn materialize_document(path: &Path, thread_dir: &Path) -> std::io::Result<PathBuf> {
    let metadata = path.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_FRAGMENT_BYTES {
        return Err(std::io::Error::other("invalid visualization fragment"));
    }

    let fragment = fs::read_to_string(path)?;
    let title = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Visualization")
        .replace('-', " ");
    let document = render_fragment(&fragment, &title);

    let thread_dir = fs::canonicalize(thread_dir)?;
    let viewer_dir = thread_dir.join(VIEWER_DIRECTORY_NAME);
    fs::create_dir_all(&viewer_dir)?;
    let viewer_dir = fs::canonicalize(viewer_dir)?;
    if !viewer_dir.starts_with(&thread_dir) {
        return Err(std::io::Error::other(
            "visualization viewer directory escapes thread directory",
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("visualization fragment has no file name"))?;
    let viewer_path = viewer_dir.join(file_name);
    if fs::read_to_string(&viewer_path).is_ok_and(|existing| existing == document) {
        return Ok(viewer_path);
    }

    let mut temporary = tempfile::NamedTempFile::new_in(&viewer_dir)?;
    temporary.write_all(document.as_bytes())?;
    temporary.flush()?;
    temporary
        .persist(&viewer_path)
        .map_err(|error| error.error)?;
    Ok(viewer_path)
}

fn render_fragment(fragment: &str, title: &str) -> String {
    let runtime = VIEWER_RUNTIME.replacen(FRAGMENT_PLACEHOLDER, fragment, /*count*/ 1);
    let escaped_title = escape_html(title);
    let frame = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"referrer\" content=\"no-referrer\"><meta http-equiv=\"Content-Security-Policy\" content=\"{FRAME_CSP}\"><title>{escaped_title}</title><style>{VIEWER_STYLESHEET}\nhtml>body{{padding:0}}</style></head><body>{runtime}</body></html>"
    );

    // A srcdoc frame inherits its parent's CSP, so the shell must grant every
    // resource type that the stricter frame CSP may use. The frame itself stays
    // sandboxed without allow-same-origin.
    let shell_csp = FRAME_CSP.replace("frame-src 'none'", "frame-src 'self'");
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"referrer\" content=\"no-referrer\"><meta http-equiv=\"Content-Security-Policy\" content=\"{shell_csp}\"><title>{escaped_title}</title><style>{SHELL_STYLE}</style></head><body><iframe sandbox=\"allow-scripts\" referrerpolicy=\"no-referrer\" title=\"{escaped_title}\" srcdoc=\"{}\"></iframe></body></html>",
        escape_html(&frame)
    )
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
