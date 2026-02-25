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

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: HeaderMap,
    pub body: Option<Value>,
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
        self.body = serde_json::to_value(body).ok();
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
