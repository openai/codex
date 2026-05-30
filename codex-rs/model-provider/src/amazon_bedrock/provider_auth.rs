use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonBedrockAuth {
    pub bearer_token: String,
    pub region: String,
}

pub(super) type StoredAmazonBedrockAuth = std::result::Result<Option<AmazonBedrockAuth>, String>;

pub fn save_amazon_bedrock_auth(
    codex_home: &Path,
    bearer_token: &str,
    region: &str,
) -> io::Result<()> {
    let auth = AmazonBedrockAuth {
        bearer_token: bearer_token.to_string(),
        region: region.to_string(),
    };
    let auth_path = amazon_bedrock_auth_file(codex_home);
    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_vec_pretty(&auth)?;
    fs::write(&auth_path, contents)?;
    restrict_file_permissions(&auth_path)
}

pub fn load_amazon_bedrock_auth(codex_home: &Path) -> io::Result<Option<AmazonBedrockAuth>> {
    let auth_path = amazon_bedrock_auth_file(codex_home);
    let contents = match fs::read_to_string(&auth_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let auth = serde_json::from_str(&contents)?;
    Ok(Some(auth))
}

pub fn delete_amazon_bedrock_auth(codex_home: &Path) -> io::Result<bool> {
    let auth_path = amazon_bedrock_auth_file(codex_home);
    match fs::remove_file(auth_path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

fn amazon_bedrock_auth_file(codex_home: &Path) -> PathBuf {
    codex_home
        .join("model-providers")
        .join("amazon-bedrock")
        .join("auth.json")
}

#[cfg(unix)]
fn restrict_file_permissions(auth_path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(auth_path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_file_permissions(_auth_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use super::*;

    #[test]
    fn save_load_and_delete_amazon_bedrock_auth() {
        let codex_home = std::env::temp_dir().join(format!(
            "codex-bedrock-auth-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));

        save_amazon_bedrock_auth(&codex_home, "bedrock-key", "us-east-1").expect("save auth");

        assert_eq!(
            load_amazon_bedrock_auth(&codex_home).expect("load auth"),
            Some(AmazonBedrockAuth {
                bearer_token: "bedrock-key".to_string(),
                region: "us-east-1".to_string(),
            })
        );
        assert!(delete_amazon_bedrock_auth(&codex_home).expect("delete auth"));
        assert_eq!(
            load_amazon_bedrock_auth(&codex_home).expect("load missing auth"),
            None
        );
        assert!(!delete_amazon_bedrock_auth(&codex_home).expect("delete missing auth"));
        let _ = fs::remove_dir_all(&codex_home);
    }
}
