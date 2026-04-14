use bytes::Bytes;
use http::Method;
use reqwest::header::HeaderMap;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RequestCompression {
    #[default]
    None,
    Zstd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestBody {
    Json(Value),
    Raw(Bytes),
}

impl RequestBody {
    pub fn json(&self) -> Option<&Value> {
        match self {
            Self::Json(value) => Some(value),
            Self::Raw(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: HeaderMap,
    pub body: Option<RequestBody>,
    pub compression: RequestCompression,
    pub timeout: Option<Duration>,
}

impl Request {
    pub fn new(method: Method, url: String) -> Self {
        Self {
            method,
            url,
            headers: HeaderMap::new(),
            body: None,
            compression: RequestCompression::None,
            timeout: None,
        }
    }

    pub fn with_json<T: Serialize>(mut self, body: &T) -> Self {
        self.body = serde_json::to_value(body).ok().map(RequestBody::Json);
        self
    }

    pub fn with_raw_body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Some(RequestBody::Raw(body.into()));
        self
    }

    pub fn with_compression(mut self, compression: RequestCompression) -> Self {
        self.compression = compression;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: http::StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}
