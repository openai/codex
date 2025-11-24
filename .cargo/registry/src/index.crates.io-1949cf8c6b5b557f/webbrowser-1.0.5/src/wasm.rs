use crate::{Browser, BrowserOptions, Error, ErrorKind, Result, TargetType};

/// Deal with opening a URL in wasm. This implementation ignores the browser attribute
/// and always opens URLs in the same browser where wasm vm is running.
pub(super) fn open_browser_internal(
    _: Browser,
    target: &TargetType,
    options: &BrowserOptions,
) -> Result<()> {
    // ensure we're opening only http/https urls, failing otherwise
    let url = target.get_http_url()?;

    // always return true for a dry run
    if options.dry_run {
        if web_sys::window().is_some() {
            return Ok(());
        } else {
            return Err(Error::new(ErrorKind::Other, "no browser window available"));
        }
    }

    let window = web_sys::window();
    match window {
        Some(w) => match w.open_with_url_and_target(url, &options.target_hint) {
            Ok(x) => match x {
                Some(_) => Ok(()),
                None => {
                    wasm_console_log(POPUP_ERR_MSG, options);
                    Err(Error::new(ErrorKind::Other, POPUP_ERR_MSG))
                }
            },
            Err(_) => {
                wasm_console_log("window error while opening url", options);
                Err(Error::new(ErrorKind::Other, "error opening url"))
            }
        },
        None => Err(Error::new(ErrorKind::Other, "no browser window available")),
    }
}

/// Print to browser console
fn wasm_console_log(_msg: &str, _options: &BrowserOptions) {
    #[cfg(all(debug_assertions, feature = "wasm-console"))]
    if !_options.suppress_output {
        web_sys::console::log_1(&format!("[webbrowser] {}", &_msg).into());
    }
}

const POPUP_ERR_MSG: &str = "popup blocked? window detected, but open_url failed";
