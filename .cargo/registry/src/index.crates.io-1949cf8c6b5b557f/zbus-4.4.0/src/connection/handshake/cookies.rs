use std::{fmt, path::PathBuf};

use futures_util::StreamExt;
use tracing::trace;
use xdg_home::home_dir;
use zvariant::Str;

use crate::{file::FileLines, Error, Result};

#[derive(Debug)]
pub(super) struct Cookie {
    id: usize,
    cookie: String,
}

impl Cookie {
    #[cfg(feature = "p2p")]
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn cookie(&self) -> &str {
        &self.cookie
    }

    fn keyring_path() -> Result<PathBuf> {
        let mut path = home_dir()
            .ok_or_else(|| Error::Handshake("Failed to determine home directory".into()))?;
        path.push(".dbus-keyrings");
        Ok(path)
    }

    async fn read_keyring(context: &CookieContext<'_>) -> Result<Vec<Cookie>> {
        let mut path = Cookie::keyring_path()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let perms = crate::file::metadata(&path).await?.permissions().mode();
            if perms & 0o066 != 0 {
                return Err(Error::Handshake(
                    "DBus keyring has invalid permissions".into(),
                ));
            }
        }
        #[cfg(not(unix))]
        {
            // FIXME: add code to check directory permissions
        }
        path.push(&*context.0);
        trace!("Reading keyring {:?}", path);
        let mut lines = FileLines::open(&path).await?.enumerate();
        let mut cookies = vec![];
        while let Some((n, line)) = lines.next().await {
            let line = line?;
            let mut split = line.split_whitespace();
            let id = split
                .next()
                .ok_or_else(|| {
                    Error::Handshake(format!(
                        "DBus cookie `{}` missing ID at line {n}",
                        path.display(),
                    ))
                })?
                .parse()
                .map_err(|e| {
                    Error::Handshake(format!(
                        "Failed to parse cookie ID in file `{}` at line {n}: {e}",
                        path.display(),
                    ))
                })?;
            let _ = split.next().ok_or_else(|| {
                Error::Handshake(format!(
                    "DBus cookie `{}` missing creation time at line {n}",
                    path.display(),
                ))
            })?;
            let cookie = split
                .next()
                .ok_or_else(|| {
                    Error::Handshake(format!(
                        "DBus cookie `{}` missing cookie data at line {}",
                        path.to_str().unwrap(),
                        n
                    ))
                })?
                .to_string();
            cookies.push(Cookie { id, cookie })
        }
        trace!("Loaded keyring {:?}", cookies);
        Ok(cookies)
    }

    pub async fn lookup(context: &CookieContext<'_>, id: usize) -> Result<Cookie> {
        let keyring = Self::read_keyring(context).await?;
        keyring
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| Error::Handshake(format!("DBus cookie ID {id} not found")))
    }

    #[cfg(feature = "p2p")]
    pub async fn first(context: &CookieContext<'_>) -> Result<Cookie> {
        let keyring = Self::read_keyring(context).await?;
        keyring
            .into_iter()
            .next()
            .ok_or_else(|| Error::Handshake("No cookies available".into()))
    }
}

#[derive(Debug)]
pub struct CookieContext<'c>(Str<'c>);

impl<'c> TryFrom<Str<'c>> for CookieContext<'c> {
    type Error = Error;

    fn try_from(value: Str<'c>) -> Result<Self> {
        if value.is_empty() {
            return Err(Error::Handshake("Empty cookie context".into()));
        } else if !value.is_ascii() || value.contains(['/', '\\', ' ', '\n', '\r', '\t', '.']) {
            return Err(Error::Handshake(
                "Invalid characters in cookie context".into(),
            ));
        }

        Ok(Self(value))
    }
}

impl fmt::Display for CookieContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for CookieContext<'_> {
    fn default() -> Self {
        Self(Str::from_static("org_freedesktop_general"))
    }
}

impl From<hex::FromHexError> for Error {
    fn from(e: hex::FromHexError) -> Self {
        Error::Handshake(format!("Invalid hexcode: {e}"))
    }
}
