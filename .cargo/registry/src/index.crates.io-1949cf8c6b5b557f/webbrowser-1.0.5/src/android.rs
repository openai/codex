use crate::{Browser, BrowserOptions, Error, ErrorKind, Result, TargetType};
use jni::objects::{JObject, JValue};
use std::process::{Command, Stdio};

/// Deal with opening of browsers on Android. Only [Browser::Default] is supported, and
/// in options, only [BrowserOptions::dry_run] is honoured.
pub(super) fn open_browser_internal(
    browser: Browser,
    target: &TargetType,
    options: &BrowserOptions,
) -> Result<()> {
    // ensure we're opening only http/https urls, failing otherwise
    let url = target.get_http_url()?;

    match browser {
        Browser::Default => open_browser_default(url, options),
        _ => Err(Error::new(
            ErrorKind::NotFound,
            "only default browser supported",
        )),
    }
}

/// Open the default browser
fn open_browser_default(url: &str, options: &BrowserOptions) -> Result<()> {
    // always return true for a dry run
    if options.dry_run {
        return Ok(());
    }

    // first we try to see if we're in a termux env, because if we are, then
    // the android context may not have been initialized, and it'll panic
    if try_for_termux(url, options).is_ok() {
        return Ok(());
    }

    // Create a VM for executing Java calls
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) }.map_err(|_| {
        Error::new(
            ErrorKind::NotFound,
            "Expected to find JVM via ndk_context crate",
        )
    })?;

    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context() as _) };
    let mut env = vm
        .attach_current_thread()
        .map_err(|_| Error::new(ErrorKind::Other, "Failed to attach current thread"))?;

    // Create ACTION_VIEW object
    let intent_class = env
        .find_class("android/content/Intent")
        .map_err(|_| Error::new(ErrorKind::NotFound, "Failed to find Intent class"))?;
    let action_view = env
        .get_static_field(&intent_class, "ACTION_VIEW", "Ljava/lang/String;")
        .map_err(|_| Error::new(ErrorKind::NotFound, "Failed to get intent.ACTION_VIEW"))?;

    // Create Uri object
    let uri_class = env
        .find_class("android/net/Uri")
        .map_err(|_| Error::new(ErrorKind::NotFound, "Failed to find Uri class"))?;
    let url = env
        .new_string(url)
        .map_err(|_| Error::new(ErrorKind::Other, "Failed to create JNI string"))?;
    let uri = env
        .call_static_method(
            &uri_class,
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&JObject::from(url))],
        )
        .map_err(|_| Error::new(ErrorKind::Other, "Failed to parse JNI Uri"))?;

    // Create new ACTION_VIEW intent with the uri
    let intent = env
        .alloc_object(&intent_class)
        .map_err(|_| Error::new(ErrorKind::Other, "Failed to allocate intent"))?;
    env.call_method(
        &intent,
        "<init>",
        "(Ljava/lang/String;Landroid/net/Uri;)V",
        &[action_view.borrow(), uri.borrow()],
    )
    .map_err(|_| Error::new(ErrorKind::Other, "Failed to initialize intent"))?;

    // Start the intent activity.
    env.call_method(
        &activity,
        "startActivity",
        "(Landroid/content/Intent;)V",
        &[JValue::Object(&intent)],
    )
    .map_err(|_| Error::new(ErrorKind::Other, "Failed to start activity"))?;

    Ok(())
}

/// Attemps to open a browser assuming a termux environment
///
/// See [issue #53](https://github.com/amodm/webbrowser-rs/issues/53)
fn try_for_termux(url: &str, options: &BrowserOptions) -> Result<()> {
    use std::env;
    if env::var("TERMUX_VERSION").is_ok() {
        // return true on dry-run given that termux-open command is guaranteed to be present
        if options.dry_run {
            return Ok(());
        }
        let mut cmd = Command::new("termux-open");
        cmd.arg(url);
        if options.suppress_output {
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
        }
        cmd.status().and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(Error::new(
                    ErrorKind::Other,
                    "command present but exited unsuccessfully",
                ))
            }
        })
    } else {
        Err(Error::new(ErrorKind::Other, "Not a termux environment"))
    }
}
