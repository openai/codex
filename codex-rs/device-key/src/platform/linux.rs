use crate::DeviceKeyAlgorithm;
use crate::DeviceKeyBinding;
use crate::DeviceKeyError;
use crate::DeviceKeyInfo;
use crate::DeviceKeyProtectionClass;
use crate::DeviceKeyProvider;
use crate::ProviderCreateRequest;
use crate::ProviderSignature;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[derive(Debug)]
pub(crate) struct LinuxDeviceKeyProvider;

impl DeviceKeyProvider for LinuxDeviceKeyProvider {
    fn create(&self, request: ProviderCreateRequest<'_>) -> Result<DeviceKeyInfo, DeviceKeyError> {
        if !request
            .protection_policy
            .allows(DeviceKeyProtectionClass::HardwareTpm)
        {
            return Err(DeviceKeyError::DegradedProtectionNotAllowed {
                available: DeviceKeyProtectionClass::HardwareTpm,
            });
        }

        let key_id = request.key_id_for(DeviceKeyProtectionClass::HardwareTpm);
        let key_dir = key_dir(&key_id)?;
        if key_material_exists(&key_dir) {
            let info = key_info(&key_id, &key_dir)?;
            store_binding(&key_dir, request.binding)?;
            return Ok(info);
        }

        fs::create_dir_all(&key_dir).map_err(fs_error)?;
        let tmp = TempDir::new(&key_id)?;
        let primary_context = tmp.path.join("primary.ctx");
        let public_blob = tmp.path.join("public.tpm");
        let private_blob = tmp.path.join("private.tpm");

        run_tpm2(
            Command::new("tpm2_createprimary")
                .arg("-C")
                .arg("o")
                .arg("-G")
                .arg("ecc256")
                .arg("-c")
                .arg(&primary_context),
        )?;
        run_tpm2(
            Command::new("tpm2_create")
                .arg("-C")
                .arg(&primary_context)
                .arg("-G")
                .arg("ecc256")
                .arg("-a")
                .arg("fixedtpm|fixedparent|sensitivedataorigin|userwithauth|sign")
                .arg("-u")
                .arg(&public_blob)
                .arg("-r")
                .arg(&private_blob),
        )?;

        replace_file(&public_blob, &key_dir.join("public.tpm"))?;
        replace_file(&private_blob, &key_dir.join("private.tpm"))?;
        store_binding(&key_dir, request.binding)?;
        key_info(&key_id, &key_dir)
    }

    fn get_public(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
    ) -> Result<DeviceKeyInfo, DeviceKeyError> {
        require_hardware_tpm(protection_class)?;
        let key_dir = key_dir(key_id)?;
        if !key_material_exists(&key_dir) {
            return Err(DeviceKeyError::KeyNotFound);
        }
        key_info(key_id, &key_dir)
    }

    fn binding(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
    ) -> Result<DeviceKeyBinding, DeviceKeyError> {
        require_hardware_tpm(protection_class)?;
        let key_dir = key_dir(key_id)?;
        if !key_material_exists(&key_dir) {
            return Err(DeviceKeyError::KeyNotFound);
        }
        load_binding(&key_dir)
    }

    fn sign(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
        payload: &[u8],
    ) -> Result<ProviderSignature, DeviceKeyError> {
        require_hardware_tpm(protection_class)?;
        let key_dir = key_dir(key_id)?;
        if !key_material_exists(&key_dir) {
            return Err(DeviceKeyError::KeyNotFound);
        }

        let tmp = TempDir::new(key_id)?;
        let key_context = load_key_context(&key_dir, &tmp.path)?;
        let digest = tmp.path.join("digest.bin");
        let signature = tmp.path.join("signature.der");
        fs::write(&digest, Sha256::digest(payload)).map_err(fs_error)?;
        run_tpm2(
            Command::new("tpm2_sign")
                .arg("-c")
                .arg(&key_context)
                .arg("-g")
                .arg("sha256")
                .arg("-f")
                .arg("der")
                .arg("-o")
                .arg(&signature)
                .arg(&digest),
        )?;

        Ok(ProviderSignature {
            signature_der: fs::read(signature).map_err(fs_error)?,
            algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
        })
    }
}

fn require_hardware_tpm(protection_class: DeviceKeyProtectionClass) -> Result<(), DeviceKeyError> {
    if protection_class != DeviceKeyProtectionClass::HardwareTpm {
        return Err(DeviceKeyError::KeyNotFound);
    }
    Ok(())
}

fn key_info(key_id: &str, key_dir: &Path) -> Result<DeviceKeyInfo, DeviceKeyError> {
    let tmp = TempDir::new(key_id)?;
    let key_context = load_key_context(key_dir, &tmp.path)?;
    let public_pem = tmp.path.join("public.pem");
    run_tpm2(
        Command::new("tpm2_readpublic")
            .arg("-c")
            .arg(&key_context)
            .arg("-f")
            .arg("pem")
            .arg("-o")
            .arg(&public_pem),
    )?;

    let pem = fs::read_to_string(public_pem).map_err(fs_error)?;
    Ok(DeviceKeyInfo {
        key_id: key_id.to_string(),
        public_key_spki_der: pem_to_der(&pem)?,
        algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
        protection_class: DeviceKeyProtectionClass::HardwareTpm,
    })
}

fn load_key_context(key_dir: &Path, tmp_dir: &Path) -> Result<PathBuf, DeviceKeyError> {
    let primary_context = tmp_dir.join("primary.ctx");
    let key_context = tmp_dir.join("key.ctx");
    run_tpm2(
        Command::new("tpm2_createprimary")
            .arg("-C")
            .arg("o")
            .arg("-G")
            .arg("ecc256")
            .arg("-c")
            .arg(&primary_context),
    )?;
    run_tpm2(
        Command::new("tpm2_load")
            .arg("-C")
            .arg(&primary_context)
            .arg("-u")
            .arg(key_dir.join("public.tpm"))
            .arg("-r")
            .arg(key_dir.join("private.tpm"))
            .arg("-c")
            .arg(&key_context),
    )?;
    Ok(key_context)
}

fn key_material_exists(key_dir: &Path) -> bool {
    key_dir.join("public.tpm").is_file() && key_dir.join("private.tpm").is_file()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredBinding {
    account_user_id: String,
    client_id: String,
}

fn store_binding(key_dir: &Path, binding: &DeviceKeyBinding) -> Result<(), DeviceKeyError> {
    let stored = StoredBinding {
        account_user_id: binding.account_user_id.clone(),
        client_id: binding.client_id.clone(),
    };
    let bytes =
        serde_json::to_vec(&stored).map_err(|err| DeviceKeyError::Platform(err.to_string()))?;
    fs::write(key_dir.join("binding.json"), bytes).map_err(fs_error)
}

fn load_binding(key_dir: &Path) -> Result<DeviceKeyBinding, DeviceKeyError> {
    let bytes = fs::read(key_dir.join("binding.json")).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            DeviceKeyError::KeyNotFound
        } else {
            fs_error(err)
        }
    })?;
    let stored: StoredBinding =
        serde_json::from_slice(&bytes).map_err(|err| DeviceKeyError::Platform(err.to_string()))?;
    Ok(DeviceKeyBinding {
        account_user_id: stored.account_user_id,
        client_id: stored.client_id,
    })
}

fn key_dir(key_id: &str) -> Result<PathBuf, DeviceKeyError> {
    let mut root = storage_root()?;
    let digest = Sha256::digest(key_id.as_bytes());
    root.push("device-keys");
    root.push("tpm2");
    root.push(URL_SAFE_NO_PAD.encode(digest));
    Ok(root)
}

fn storage_root() -> Result<PathBuf, DeviceKeyError> {
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(data_home).join("codex"));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| {
        DeviceKeyError::Platform("HOME is not set; cannot locate device key storage".to_string())
    })?;
    Ok(PathBuf::from(home).join(".local/share/codex"))
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), DeviceKeyError> {
    let tmp_destination = destination.with_extension("tmp");
    fs::copy(source, &tmp_destination).map_err(fs_error)?;
    fs::rename(tmp_destination, destination).map_err(fs_error)
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>, DeviceKeyError> {
    let base64 = pem
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("-----"))
        .collect::<String>();
    STANDARD.decode(base64).map_err(|err| {
        DeviceKeyError::Platform(format!("failed to decode TPM public key PEM: {err}"))
    })
}

fn run_tpm2(command: &mut Command) -> Result<(), DeviceKeyError> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command.output().map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            DeviceKeyError::HardwareBackedKeysUnavailable
        } else {
            DeviceKeyError::Platform(format!("failed to run {program}: {err}"))
        }
    })?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("Could not load tcti")
        || stderr.contains("No such file or directory")
        || stderr.contains("/dev/tpm")
    {
        return Err(DeviceKeyError::HardwareBackedKeysUnavailable);
    }
    Err(DeviceKeyError::Platform(format!(
        "{program} failed with status {}: {}",
        output.status,
        stderr.trim()
    )))
}

fn fs_error(err: io::Error) -> DeviceKeyError {
    DeviceKeyError::Platform(err.to_string())
}

#[derive(Debug)]
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(key_id: &str) -> Result<Self, DeviceKeyError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| DeviceKeyError::Platform(err.to_string()))?;
        let mut path = std::env::temp_dir();
        path.push(format!(
            "codex-device-key-{}-{}-{}",
            safe_path_component(key_id),
            std::process::id(),
            now.as_nanos()
        ));
        fs::create_dir(&path).map_err(fs_error)?;
        Ok(Self { path })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn safe_path_component(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}
