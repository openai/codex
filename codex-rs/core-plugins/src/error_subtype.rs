use http::StatusCode;

pub(crate) fn http_status_sub_error_type(status: StatusCode) -> &'static str {
    match status.as_u16() {
        401 => "http_401",
        403 => "http_403",
        404 => "http_404",
        409 => "http_409",
        429 => "http_429",
        _ if status.is_server_error() => "http_5xx",
        _ => "http_other",
    }
}
