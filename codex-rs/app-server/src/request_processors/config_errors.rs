use super::*;

fn cloud_config_bundle_load_error(err: &std::io::Error) -> Option<&CloudConfigBundleLoadError> {
    let mut current: Option<&(dyn std::error::Error + 'static)> = err
        .get_ref()
        .map(|source| source as &(dyn std::error::Error + 'static));
    while let Some(source) = current {
        if let Some(cloud_error) = source.downcast_ref::<CloudConfigBundleLoadError>() {
            return Some(cloud_error);
        }
        current = source.source();
    }
    None
}

pub(super) fn config_load_error(err: &std::io::Error) -> JSONRPCErrorError {
    let data = cloud_config_bundle_load_error(err).map(|cloud_error| {
        let mut data = serde_json::json!({
            "reason": "cloudConfigBundle",
            "errorCode": format!("{:?}", cloud_error.code()),
            "detail": cloud_error.to_string(),
        });
        if let Some(status_code) = cloud_error.status_code() {
            data["statusCode"] = serde_json::json!(status_code);
        }
        if cloud_error.code() == CloudConfigBundleLoadErrorCode::Auth {
            data["action"] = serde_json::json!("relogin");
        }
        data
    });

    let mut error = invalid_request(format!("failed to load configuration: {err}"));
    error.data = data;
    error
}
