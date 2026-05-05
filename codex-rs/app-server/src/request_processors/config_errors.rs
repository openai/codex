use super::*;

fn cloud_requirements_load_error(err: &std::io::Error) -> Option<&CloudRequirementsLoadError> {
    let mut current: Option<&(dyn std::error::Error + 'static)> = err
        .get_ref()
        .map(|source| source as &(dyn std::error::Error + 'static));
    while let Some(source) = current {
        if let Some(cloud_error) = source.downcast_ref::<CloudRequirementsLoadError>() {
            return Some(cloud_error);
        }
        current = source.source();
    }
    None
}

pub(super) fn config_load_error(err: &std::io::Error) -> JSONRPCErrorError {
    let data = cloud_requirements_load_error(err).map(|cloud_error| {
        let mut data = serde_json::json!({
            "reason": "cloudRequirements",
            "errorCode": format!("{:?}", cloud_error.code()),
            "detail": cloud_error.to_string(),
        });
        if let Some(status_code) = cloud_error.status_code() {
            data["statusCode"] = serde_json::json!(status_code);
        }
        if cloud_error.code() == CloudRequirementsLoadErrorCode::Auth {
            data["action"] = serde_json::json!("relogin");
        }
        data
    });

    JSONRPCErrorError {
        code: INVALID_REQUEST_ERROR_CODE,
        message: format!("failed to load configuration: {err}"),
        data,
    }
}
