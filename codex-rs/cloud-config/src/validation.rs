use codex_config::AbsolutePathBuf;
use codex_config::CloudConfigBundle;
use codex_config::CloudConfigBundleLayers;
use codex_config::CloudConfigBundleLoadError;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::compose_requirements;

pub(crate) fn validate_bundle(
    bundle: &CloudConfigBundle,
    base_dir: &AbsolutePathBuf,
) -> Result<(), CloudConfigBundleLoadError> {
    let bundle_layers =
        CloudConfigBundleLayers::from_bundle(bundle.clone(), base_dir).map_err(|err| {
            CloudConfigBundleLoadError::new(
                CloudConfigBundleLoadErrorCode::InvalidBundle,
                /*status_code*/ None,
                format!("invalid cloud config bundle: {err}"),
            )
        })?;
    let CloudConfigBundleLayers {
        enterprise_managed_config: _,
        enterprise_managed_requirements,
    } = bundle_layers;

    compose_requirements(enterprise_managed_requirements).map_err(|err| {
        CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::InvalidBundle,
            /*status_code*/ None,
            format!("invalid cloud config bundle: {err}"),
        )
    })?;

    Ok(())
}
