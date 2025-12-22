use hyper::Body;
use hyper::Response;
use hyper::StatusCode;
use serde::Serialize;

pub fn text_response(status: StatusCode, body: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::from(body.to_string())))
}

pub fn json_response<T: Serialize>(value: &T) -> Response<Body> {
    let body = match serde_json::to_string(value) {
        Ok(body) => body,
        Err(_) => "{}".to_string(),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::from("{}")))
}
