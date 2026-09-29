//! Persistent, non-secret generations for staged enterprise logins and logout.
//! All reads and writes require the credential lock; missing state never admits an old attempt.

use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::Write;

use anyhow::Result;
use anyhow::ensure;
use codex_utils_home_dir::find_codex_home;
use oauth2::CsrfToken;
use sha2::Digest;
use sha2::Sha256;

use super::RefreshCredentialLock;

#[derive(PartialEq, Eq)]
pub(crate) struct EnterpriseOAuthGeneration([u8; 32]);

pub(crate) struct EnterpriseOAuthGenerationFile {
    file: File,
}

impl EnterpriseOAuthGenerationFile {
    pub(crate) fn open(
        credential_name: &str,
        issuer: &str,
        _lock: &RefreshCredentialLock,
    ) -> Result<Self> {
        let key = super::compute_store_key(credential_name, issuer)?;
        let name = format!("{:x}.enterprise-generation", Sha256::digest(key.as_bytes()));
        let path = find_codex_home()?.join("mcp-oauth-locks").join(name);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
            options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        let file = options.open(path)?;
        ensure!(
            file.metadata()?.is_file(),
            "invalid enterprise generation file"
        );
        Ok(Self { file })
    }

    pub(crate) fn current(&self) -> Result<Option<EnterpriseOAuthGeneration>> {
        let mut file = &self.file;
        file.rewind()?;
        match file.metadata()?.len() {
            0 => Ok(None),
            32 => {
                let mut generation = [0; 32];
                file.read_exact(&mut generation)?;
                Ok(Some(EnterpriseOAuthGeneration(generation)))
            }
            _ => anyhow::bail!("invalid enterprise generation file"),
        }
    }

    pub(crate) fn replace(&self) -> Result<EnterpriseOAuthGeneration> {
        // Reuse OAuth's randomness without storing any credential. Random initialization
        // also prevents an old attempt becoming valid if metadata is removed or truncated.
        let random = CsrfToken::new_random_len(/*num_bytes*/ 32);
        let generation =
            EnterpriseOAuthGeneration(Sha256::digest(random.secret().as_bytes()).into());
        let mut file = &self.file;
        file.set_len(/*size*/ 0)?;
        file.rewind()?;
        file.write_all(&generation.0)?;
        file.sync_all()?;
        Ok(generation)
    }
}
