use crate::common::run_command;
use crate::{Browser, BrowserOptions, Error, ErrorKind, Result, TargetType};
use log::trace;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::process::{Command, Stdio};

macro_rules! try_browser {
    ( $options: expr, $name:expr, $( $arg:expr ),+ ) => {
        for_matching_path($name, |pb| {
            let mut cmd = Command::new(pb);
            $(
                cmd.arg($arg);
            )+
            run_command(&mut cmd, !is_text_browser(&pb), $options)
        })
    }
}

/// Deal with opening of browsers on Linux and *BSD - currently supports only the default browser
///
/// The mechanism of opening the default browser is as follows:
/// 1. Attempt to use $BROWSER env var if available
/// 2. Attempt to use xdg-open
/// 3. Attempt to use window manager specific commands, like gnome-open, kde-open etc. incl. WSL
/// 4. Fallback to x-www-browser
pub(super) fn open_browser_internal(
    browser: Browser,
    target: &TargetType,
    options: &BrowserOptions,
) -> Result<()> {
    match browser {
        Browser::Default => open_browser_default(target, options),
        _ => Err(Error::new(
            ErrorKind::NotFound,
            "only default browser supported",
        )),
    }
}

/// Open the default browser.
///
/// [BrowserOptions::dry_run] is handled inside [run_command], as all execution paths eventually
/// rely on it to execute.
fn open_browser_default(target: &TargetType, options: &BrowserOptions) -> Result<()> {
    let url: &str = target;

    // we first try with the $BROWSER env
    try_with_browser_env(url, options)
        // allow for haiku's open specifically
        .or_else(|_| try_haiku(options, url))
        // then we try with xdg configuration
        .or_else(|_| try_xdg(options, url))
        // else do desktop specific stuff
        .or_else(|r| match guess_desktop_env() {
            "kde" => try_browser!(options, "kde-open", url)
                .or_else(|_| try_browser!(options, "kde-open5", url))
                .or_else(|_| try_browser!(options, "kfmclient", "newTab", url)),

            "gnome" => try_browser!(options, "gio", "open", url)
                .or_else(|_| try_browser!(options, "gvfs-open", url))
                .or_else(|_| try_browser!(options, "gnome-open", url)),

            "mate" => try_browser!(options, "gio", "open", url)
                .or_else(|_| try_browser!(options, "gvfs-open", url))
                .or_else(|_| try_browser!(options, "mate-open", url)),

            "xfce" => try_browser!(options, "exo-open", url)
                .or_else(|_| try_browser!(options, "gio", "open", url))
                .or_else(|_| try_browser!(options, "gvfs-open", url)),

            "wsl" => try_wsl(options, target),

            "flatpak" => try_flatpak(options, target),

            _ => Err(r),
        })
        // at the end, we'll try x-www-browser and return the result as is
        .or_else(|_| try_browser!(options, "x-www-browser", url))
        // if all above failed, map error to not found
        .map_err(|_| {
            Error::new(
                ErrorKind::NotFound,
                "No valid browsers detected. You can specify one in BROWSER environment variable",
            )
        })
        // and convert a successful result into a ()
        .map(|_| ())
}

fn try_with_browser_env(url: &str, options: &BrowserOptions) -> Result<()> {
    // $BROWSER can contain ':' delimited options, each representing a potential browser command line
    for browser in std::env::var("BROWSER")
        .unwrap_or_else(|_| String::from(""))
        .split(':')
    {
        if !browser.is_empty() {
            // each browser command can have %s to represent URL, while %c needs to be replaced
            // with ':' and %% with '%'
            let cmdline = browser
                .replace("%s", url)
                .replace("%c", ":")
                .replace("%%", "%");
            let cmdarr: Vec<&str> = cmdline.split_ascii_whitespace().collect();
            let browser_cmd = cmdarr[0];
            let env_exit = for_matching_path(browser_cmd, |pb| {
                let mut cmd = Command::new(pb);
                for arg in cmdarr.iter().skip(1) {
                    cmd.arg(arg);
                }
                if !browser.contains("%s") {
                    // append the url as an argument only if it was not already set via %s
                    cmd.arg(url);
                }
                run_command(&mut cmd, !is_text_browser(pb), options)
            });
            if env_exit.is_ok() {
                return Ok(());
            }
        }
    }
    Err(Error::new(
        ErrorKind::NotFound,
        "No valid browser configured in BROWSER environment variable",
    ))
}

/// Check if we are inside WSL on Windows, and interoperability with Windows tools is
/// enabled.
fn is_wsl() -> bool {
    // we should check in procfs only on linux, as for non-linux it will likely be
    // a disk hit, which we should avoid.
    if cfg!(target_os = "linux") {
        // we check if interop with windows tools is allowed, as if it isn't, we won't
        // be able to invoke windows commands anyways.
        // See: https://learn.microsoft.com/en-us/windows/wsl/filesystems#disable-interoperability
        if let Ok(s) = std::fs::read_to_string("/proc/sys/fs/binfmt_misc/WSLInterop") {
            s.contains("enabled")
        } else {
            false
        }
    } else {
        // we short-circuit and return false on non-linux
        false
    }
}

/// Check if we're running inside Flatpak
#[inline]
fn is_flatpak() -> bool {
    std::env::var("container")
        .map(|x| x.eq_ignore_ascii_case("flatpak"))
        .unwrap_or(false)
}

/// Detect the desktop environment
fn guess_desktop_env() -> &'static str {
    let unknown = "unknown";
    let xcd: String = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_else(|_| unknown.into())
        .to_ascii_lowercase();
    let dsession: String = std::env::var("DESKTOP_SESSION")
        .unwrap_or_else(|_| unknown.into())
        .to_ascii_lowercase();

    if is_flatpak() {
        "flatpak"
    } else if xcd.contains("gnome") || xcd.contains("cinnamon") || dsession.contains("gnome") {
        // GNOME and its derivatives
        "gnome"
    } else if xcd.contains("kde")
        || std::env::var("KDE_FULL_SESSION").is_ok()
        || std::env::var("KDE_SESSION_VERSION").is_ok()
    {
        // KDE: https://userbase.kde.org/KDE_System_Administration/Environment_Variables#Automatically_Set_Variables
        "kde"
    } else if xcd.contains("mate") || dsession.contains("mate") {
        // We'll treat MATE as distinct from GNOME due to mate-open
        "mate"
    } else if xcd.contains("xfce") || dsession.contains("xfce") {
        // XFCE
        "xfce"
    } else if is_wsl() {
        // WSL
        "wsl"
    } else {
        // All others
        unknown
    }
}

/// Open browser in WSL environments
fn try_wsl(options: &BrowserOptions, target: &TargetType) -> Result<()> {
    match target.0.scheme() {
        "http" | "https" => {
            let url: &str = target;
            try_browser!(
                options,
                "cmd.exe",
                "/c",
                "start",
                url.replace('^', "^^").replace('&', "^&")
            )
            .or_else(|_| {
                try_browser!(
                    options,
                    "powershell.exe",
                    "Start",
                    url.replace('&', "\"&\"")
                )
            })
            .or_else(|_| try_browser!(options, "wsl-open", url))
        }
        #[cfg(all(
            target_os = "linux",
            not(feature = "hardened"),
            not(feature = "disable-wsl")
        ))]
        "file" => {
            // we'll need to detect the default browser and then invoke it
            // with wsl translated path
            let wc = wsl::get_wsl_win_config()?;
            let mut cmd = if wc.powershell_path.is_some() {
                wsl::get_wsl_windows_browser_ps(&wc, target)
            } else {
                wsl::get_wsl_windows_browser_cmd(&wc, target)
            }?;
            run_command(&mut cmd, true, options)
        }
        _ => Err(Error::new(ErrorKind::NotFound, "invalid browser")),
    }
}

/// Open browser in Flatpak environments
fn try_flatpak(options: &BrowserOptions, target: &TargetType) -> Result<()> {
    match target.0.scheme() {
        "http" | "https" => {
            let url: &str = target;
            // we assume xdg-open to be present, given that it's a part of standard
            // runtime & SDK of flatpak
            try_browser!(options, "xdg-open", url)
        }
        // we support only http urls under Flatpak to adhere to the defined
        // Consistent Behaviour, as effectively DBUS is used interally, and
        // there doesn't seem to be a way for us to determine actual browser
        _ => Err(Error::new(ErrorKind::NotFound, "only http urls supported")),
    }
}

/// Handle Haiku explicitly, as it uses an "open" command, similar to macos
/// but on other Unixes, open ends up translating to shell open fd
fn try_haiku(options: &BrowserOptions, url: &str) -> Result<()> {
    if cfg!(target_os = "haiku") {
        try_browser!(options, "open", url).map(|_| ())
    } else {
        Err(Error::new(ErrorKind::NotFound, "Not on haiku"))
    }
}

/// Dig into XDG settings (if xdg is available) to force it to open the browser, instead of
/// the default application
fn try_xdg(options: &BrowserOptions, url: &str) -> Result<()> {
    // run: xdg-settings get default-web-browser
    let browser_name_os = for_matching_path("xdg-settings", |pb| {
        Command::new(pb)
            .args(["get", "default-web-browser"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
    })
    .map_err(|_| Error::new(ErrorKind::NotFound, "unable to determine xdg browser"))?
    .stdout;

    // convert browser name to a utf-8 string and trim off the trailing newline
    let browser_name = String::from_utf8(browser_name_os)
        .map_err(|_| Error::new(ErrorKind::NotFound, "invalid default browser name"))?
        .trim()
        .to_owned();
    if browser_name.is_empty() {
        return Err(Error::new(ErrorKind::NotFound, "no default xdg browser"));
    }
    trace!("found xdg browser: {:?}", &browser_name);

    // search for the config file corresponding to this browser name
    let mut config_found = false;
    let app_suffix = "applications";
    for xdg_dir in get_xdg_dirs().iter_mut() {
        let mut config_path = xdg_dir.join(app_suffix).join(&browser_name);
        trace!("checking for xdg config at {config_path:?}");
        let mut metadata = config_path.metadata();
        if metadata.is_err() && browser_name.contains('-') {
            // as per the spec, we need to replace '-' with /
            let child_path = browser_name.replace('-', "/");
            config_path = xdg_dir.join(app_suffix).join(child_path);
            metadata = config_path.metadata();
        }
        if metadata.is_ok() {
            // we've found the config file, so we try running using that
            config_found = true;
            match open_using_xdg_config(&config_path, options, url) {
                Ok(x) => return Ok(x), // return if successful
                Err(err) => {
                    // if we got an error other than NotFound, then we short
                    // circuit, and do not try any more options, else we
                    // continue to try more
                    if err.kind() != ErrorKind::NotFound {
                        return Err(err);
                    }
                }
            }
        }
    }

    if config_found {
        Err(Error::new(ErrorKind::Other, "xdg-open failed"))
    } else {
        Err(Error::new(ErrorKind::NotFound, "no valid xdg config found"))
    }
}

/// Opens `url` using xdg configuration found in `config_path`
///
/// See https://specifications.freedesktop.org/desktop-entry-spec/latest for details
fn open_using_xdg_config(config_path: &PathBuf, options: &BrowserOptions, url: &str) -> Result<()> {
    let file = std::fs::File::open(config_path)?;
    let mut in_desktop_entry = false;
    let mut hidden = false;
    let mut cmdline: Option<String> = None;
    let mut requires_terminal = false;

    // we capture important keys under the [Desktop Entry] section, as defined under:
    // https://specifications.freedesktop.org/desktop-entry-spec/latest/ar01s06.html
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
        } else if line.starts_with('[') {
            in_desktop_entry = false;
        } else if in_desktop_entry && !line.starts_with('#') {
            if let Some(idx) = line.find('=') {
                let key = &line[..idx];
                let value = &line[idx + 1..];
                match key {
                    "Exec" => cmdline = Some(value.to_owned()),
                    "Hidden" => hidden = value == "true",
                    "Terminal" => requires_terminal = value == "true",
                    _ => (), // ignore
                }
            }
        }
    }

    if hidden {
        // we ignore this config if it was marked hidden/deleted
        return Err(Error::new(ErrorKind::NotFound, "xdg config is hidden"));
    }

    if let Some(cmdline) = cmdline {
        // we have a valid configuration
        let cmdarr: Vec<&str> = cmdline.split_ascii_whitespace().collect();
        let browser_cmd = cmdarr[0];
        for_matching_path(browser_cmd, |pb| {
            let mut cmd = Command::new(pb);
            let mut url_added = false;
            for arg in cmdarr.iter().skip(1) {
                match *arg {
                    "%u" | "%U" | "%f" | "%F" => {
                        url_added = true;
                        cmd.arg(url)
                    }
                    _ => cmd.arg(arg),
                };
            }
            if !url_added {
                // append the url as an argument only if it was not already set
                cmd.arg(url);
            }
            run_command(&mut cmd, !requires_terminal, options)
        })
    } else {
        // we don't have a valid config
        Err(Error::new(ErrorKind::NotFound, "not a valid xdg config"))
    }
}

/// Get the list of directories in which the desktop file needs to be searched
fn get_xdg_dirs() -> Vec<PathBuf> {
    let mut xdg_dirs: Vec<PathBuf> = Vec::new();

    #[allow(deprecated)]
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::home_dir().map(|path| path.join(".local/share")));
    if let Some(data_home) = data_home {
        xdg_dirs.push(data_home);
    }

    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for d in data_dirs.split(':') {
            xdg_dirs.push(PathBuf::from(d));
        }
    } else {
        xdg_dirs.push(PathBuf::from("/usr/local/share"));
        xdg_dirs.push(PathBuf::from("/usr/share"));
    }

    xdg_dirs
}

/// Returns true if specified command refers to a known list of text browsers
fn is_text_browser(pb: &Path) -> bool {
    for browser in TEXT_BROWSERS.iter() {
        if pb.ends_with(browser) {
            return true;
        }
    }
    false
}

fn for_matching_path<F, T>(name: &str, op: F) -> Result<T>
where
    F: FnOnce(&PathBuf) -> Result<T>,
{
    let err = Err(Error::new(ErrorKind::NotFound, "command not found"));

    // if the name already includes path separator, we should not try to do a PATH search on it
    // as it's likely an absolutely or relative name, so we treat it as such.
    if name.contains(MAIN_SEPARATOR) {
        let pb = std::path::PathBuf::from(name);
        if let Ok(metadata) = pb.metadata() {
            if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                return op(&pb);
            }
        } else {
            return err;
        }
    } else {
        // search for this name inside PATH
        if let Ok(path) = std::env::var("PATH") {
            for entry in path.split(':') {
                let mut pb = std::path::PathBuf::from(entry);
                pb.push(name);
                if let Ok(metadata) = pb.metadata() {
                    if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                        return op(&pb);
                    }
                }
            }
        }
    }
    // return the not found err, if we didn't find anything above
    err
}

static TEXT_BROWSERS: [&str; 9] = [
    "lynx", "links", "links2", "elinks", "w3m", "eww", "netrik", "retawq", "curl",
];

#[cfg(test)]
mod tests_xdg {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn get_temp_path(name: &str, suffix: &str) -> String {
        let pid = std::process::id();
        std::env::temp_dir()
            .join(format!("{name}.{pid}.{suffix}"))
            .into_os_string()
            .into_string()
            .expect("failed to convert into string")
    }

    #[test]
    fn test_xdg_open_local_file() {
        let _ = env_logger::try_init();

        // ensure flag file is not existing
        let flag_path = get_temp_path("test_xdg", "flag");
        let _ = std::fs::remove_file(&flag_path);

        // create browser script
        let txt_path = get_temp_path("test_xdf", "txt");
        let browser_path = get_temp_path("test_xdg", "browser");
        {
            let mut browser_file =
                File::create(&browser_path).expect("failed to create browser file");
            let _ = browser_file.write_fmt(format_args!(
                r#"#!/bin/bash
                if [ "$1" != "p1" ]; then
                    echo "1st parameter should've been p1" >&2
                    exit 1
                elif [ "$2" != "{}" ]; then
                    echo "2nd parameter should've been {}" >&2
                    exit 1
                elif [ "$3" != "p3" ]; then
                    echo "3rd parameter should've been p3" >&2
                    exit 1
                fi

                echo "$2" > "{}"
            "#,
                &txt_path, &txt_path, &flag_path
            ));
            let mut perms = browser_file
                .metadata()
                .expect("failed to get permissions")
                .permissions();
            perms.set_mode(0o755);
            let _ = browser_file.set_permissions(perms);
        }

        // create xdg desktop config
        let config_path = get_temp_path("test_xdg", "desktop");
        {
            let mut xdg_file =
                std::fs::File::create(&config_path).expect("failed to create xdg desktop file");
            let _ = xdg_file.write_fmt(format_args!(
                r#"# this line should be ignored
[Desktop Entry]
Exec={} p1 %u p3
[Another Entry]
Exec=/bin/ls
# the above Exec line should be getting ignored
            "#,
                &browser_path
            ));
        }

        // now try opening browser using above desktop config
        let result = open_using_xdg_config(
            &PathBuf::from(&config_path),
            &BrowserOptions::default(),
            &txt_path,
        );

        // we need to wait until the flag file shows up due to the async
        // nature of browser invocation
        for _ in 0..10 {
            if std::fs::read_to_string(&flag_path).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        std::thread::sleep(std::time::Duration::from_millis(500));

        // validate that the flag file contains the url we passed
        assert_eq!(
            std::fs::read_to_string(&flag_path)
                .expect("flag file not found")
                .trim(),
            &txt_path,
        );
        assert!(result.is_ok());

        // delete all temp files
        let _ = std::fs::remove_file(&txt_path);
        let _ = std::fs::remove_file(&flag_path);
        let _ = std::fs::remove_file(&browser_path);
        let _ = std::fs::remove_file(&config_path);

        assert!(result.is_ok());
    }
}

/// WSL related browser functionality.
///
/// We treat it as a separate submod, to allow for easy logical grouping
/// and to enable/disable based on some feature easily in future.
#[cfg(all(
    target_os = "linux",
    not(feature = "hardened"),
    not(feature = "disable-wsl")
))]
mod wsl {
    use crate::common::for_each_token;
    use crate::{Result, TargetType};
    use std::io::{Error, ErrorKind};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    pub(super) struct WindowsConfig {
        root: PathBuf,
        cmd_path: PathBuf,
        pub(super) powershell_path: Option<PathBuf>,
    }

    /// Returns a [WindowsConfig] by iterating over PATH entries. This seems to be
    /// the fastest way to determine this.
    pub(super) fn get_wsl_win_config() -> Result<WindowsConfig> {
        let err_fn = || Error::new(ErrorKind::NotFound, "invalid windows config");
        if let Some(path_env) = std::env::var_os("PATH") {
            let mut root: Option<PathBuf> = None;
            let mut cmd_path: Option<PathBuf> = None;
            let mut powershell_path: Option<PathBuf> = None;
            for path in std::env::split_paths(&path_env) {
                let path_s = path.to_string_lossy().to_ascii_lowercase();
                let path_s = path_s.trim_end_matches('/');
                if path_s.ends_with("/windows/system32") {
                    root = Some(std::fs::canonicalize(path.join("../.."))?);
                    cmd_path = Some(path.join("cmd.exe"));
                    break;
                }
            }
            if let Some(ref root) = root {
                for path in std::env::split_paths(&path_env) {
                    if path.starts_with(root) {
                        let pb = path.join("powershell.exe");
                        if pb.is_file() {
                            powershell_path = Some(pb);
                        }
                    }
                }
            }
            if let Some(root) = root {
                let cmd_path = cmd_path.unwrap_or_else(|| (root).join("windows/system32/cmd.exe"));
                Ok(WindowsConfig {
                    root,
                    cmd_path,
                    powershell_path,
                })
            } else {
                Err(err_fn())
            }
        } else {
            Err(err_fn())
        }
    }

    /// Try to get default browser command from powershell.exe
    pub(super) fn get_wsl_windows_browser_ps(
        wc: &WindowsConfig,
        url: &TargetType,
    ) -> Result<Command> {
        let err_fn = || Error::new(ErrorKind::NotFound, "powershell.exe error");
        let ps_exe = wc.powershell_path.as_ref().ok_or_else(err_fn)?;
        let mut cmd = Command::new(ps_exe);
        cmd.arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        log::debug!("running command: ${:?}", &cmd);
        let mut child = cmd.spawn()?;

        let mut stdin = child.stdin.take().ok_or_else(err_fn)?;
        std::io::Write::write_all(&mut stdin, WSL_PS_SCRIPT.as_bytes())?;
        drop(stdin); // flush to stdin, and close
        let output_u8 = child.wait_with_output()?;
        let output = String::from_utf8_lossy(&output_u8.stdout);
        let output = output.trim();
        if output.is_empty() {
            Err(err_fn())
        } else {
            parse_wsl_cmdline(wc, output, url)
        }
    }

    /// Try to get default browser command from cmd.exe
    pub(super) fn get_wsl_windows_browser_cmd(
        wc: &WindowsConfig,
        url: &TargetType,
    ) -> Result<Command> {
        let err_fn = || Error::new(ErrorKind::NotFound, "cmd.exe error");
        let mut cmd = Command::new(&wc.cmd_path);
        cmd.arg("/Q")
            .arg("/C")
            .arg("ftype http")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        log::debug!("running command: ${:?}", &cmd);
        let output_u8 = cmd.output()?;

        let output = String::from_utf8_lossy(&output_u8.stdout);
        let output = output.trim();
        if output.is_empty() {
            Err(err_fn())
        } else {
            parse_wsl_cmdline(wc, output, url)
        }
    }

    /// Given the configured command line `cmdline` in registry, and the given `url`,
    /// return the appropriate `Command` to invoke
    fn parse_wsl_cmdline(wc: &WindowsConfig, cmdline: &str, url: &TargetType) -> Result<Command> {
        let mut tokens: Vec<String> = Vec::new();
        let filepath = wsl_get_filepath_from_url(wc, url)?;
        let fp = &filepath;
        for_each_token(cmdline, |token: &str| {
            if matches!(token, "%0" | "%1") {
                tokens.push(fp.to_owned());
            } else {
                tokens.push(token.to_string());
            }
        });
        if tokens.is_empty() {
            Err(Error::new(ErrorKind::NotFound, "invalid command"))
        } else {
            let progpath = wsl_path_win2lin(wc, &tokens[0])?;
            let mut cmd = Command::new(progpath);
            if tokens.len() > 1 {
                cmd.args(&tokens[1..]);
            }
            Ok(cmd)
        }
    }

    fn wsl_get_filepath_from_url(wc: &WindowsConfig, target: &TargetType) -> Result<String> {
        let url = &target.0;
        if url.scheme() == "file" {
            if url.host().is_none() {
                let path = url
                    .to_file_path()
                    .map_err(|_| Error::new(ErrorKind::NotFound, "invalid path"))?;
                wsl_path_lin2win(wc, path)
            } else {
                Ok(format!("\\\\wsl${}", url.path().replace('/', "\\")))
            }
        } else {
            Ok(url.as_str().to_string())
        }
    }

    /// Converts a windows path to linux `PathBuf`
    fn wsl_path_win2lin(wc: &WindowsConfig, path: &str) -> Result<PathBuf> {
        let err_fn = || Error::new(ErrorKind::NotFound, "invalid windows path");
        if path.len() > 3 {
            let pfx = &path[..3];
            if matches!(pfx, "C:\\" | "c:\\") {
                let win_path = path[3..].replace('\\', "/");
                Ok(wc.root.join(win_path))
            } else {
                Err(err_fn())
            }
        } else {
            Err(err_fn())
        }
    }

    /// Converts a linux path to windows. We using `String` instead of `OsString` as
    /// return type because the `OsString` will be different b/w Windows & Linux.
    fn wsl_path_lin2win(wc: &WindowsConfig, path: impl AsRef<Path>) -> Result<String> {
        let path = path.as_ref();
        if let Ok(path) = path.strip_prefix(&wc.root) {
            // windows can access this path directly
            Ok(format!("C:\\{}", path.as_os_str().to_string_lossy()).replace('/', "\\"))
        } else {
            // windows needs to access it via network
            let wsl_hostname = get_wsl_distro_name(wc)?;
            Ok(format!(
                "\\\\wsl$\\{}{}",
                &wsl_hostname,
                path.as_os_str().to_string_lossy()
            )
            .replace('/', "\\"))
        }
    }

    /// Gets the WSL distro name
    fn get_wsl_distro_name(wc: &WindowsConfig) -> Result<String> {
        let err_fn = || Error::new(ErrorKind::Other, "unable to determine wsl distro name");

        // mostly we should be able to get it from the WSL_DISTRO_NAME env var
        if let Ok(wsl_hostname) = std::env::var("WSL_DISTRO_NAME") {
            Ok(wsl_hostname)
        } else {
            // but if not (e.g. if we were running as root), we can invoke
            // powershell.exe to determine pwd and from there infer the distro name
            let psexe = wc.powershell_path.as_ref().ok_or_else(err_fn)?;
            let mut cmd = Command::new(psexe);
            cmd.arg("-NoLogo")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg("$loc = Get-Location\nWrite-Output $loc.Path")
                .current_dir("/")
                .stdin(Stdio::null())
                .stderr(Stdio::null());
            log::debug!("running command: ${:?}", &cmd);
            let output_u8 = cmd.output()?.stdout;
            let output = String::from_utf8_lossy(&output_u8);
            let output = output.trim_end_matches('\\');
            let idx = output.find("::\\\\").ok_or_else(err_fn)?;
            Ok((output[idx + 9..]).trim().to_string())
        }
    }

    /// Powershell script to get the default browser command.
    ///
    /// Adapted from https://stackoverflow.com/a/60972216
    const WSL_PS_SCRIPT: &str = r#"
$Signature = @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class Win32Api
{

    [DllImport("Shlwapi.dll", SetLastError = true, CharSet = CharSet.Auto)]
    static extern uint AssocQueryString(AssocF flags, AssocStr str, string pszAssoc, string pszExtra,[Out] System.Text.StringBuilder pszOut, ref uint pcchOut);

    public static string GetDefaultBrowser()
    {
        AssocF assocF = AssocF.IsProtocol;
        AssocStr association = AssocStr.Command;
        string assocString = "http";

        uint length = 1024; // we assume 1k is sufficient memory to hold the command
        var sb = new System.Text.StringBuilder((int) length);
        uint ret = ret = AssocQueryString(assocF, association, assocString, null, sb, ref length);

        return (ret != 0) ? null : sb.ToString();
    }

    [Flags]
    internal enum AssocF : uint
    {
        IsProtocol = 0x1000,
    }

    internal enum AssocStr
    {
        Command = 1,
        Executable,
    }
}
"@

Add-Type -TypeDefinition $Signature

Write-Output $([Win32Api]::GetDefaultBrowser())
"#;

    /*#[cfg(test)]
    mod tests {
        use crate::open;

        #[test]
        fn test_url() {
            let _ = env_logger::try_init();
            assert!(open("https://github.com").is_ok());
        }

        #[test]
        fn test_linux_file() {
            let _ = env_logger::try_init();
            assert!(open("abc.html").is_ok());
        }

        #[test]
        fn test_windows_file() {
            let _ = env_logger::try_init();
            assert!(open("/mnt/c/T/abc.html").is_ok());
        }
    }*/
}
