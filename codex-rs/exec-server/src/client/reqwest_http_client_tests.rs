use super::ReqwestHttpRequestRunner;
use super::is_literal_loopback_url;
use crate::protocol::HttpRedirectPolicy;
use crate::protocol::HttpRequestParams;
use axum::Router;
use axum::extract::Path;
use axum::response::IntoResponse;
use axum::response::Redirect;
use axum::response::Response;
use axum::routing::get;
use reqwest::Url;
use std::time::Duration;

#[test]
fn literal_loopback_urls_bypass_proxies() {
    for url in [
        "http://127.0.0.1:3210/mcp",
        "https://127.42.0.9/mcp",
        "http://[::1]:3210/mcp",
    ] {
        let url = Url::parse(url).expect("valid URL");
        assert!(
            is_literal_loopback_url(&url),
            "expected {url} to bypass proxies"
        );
    }
}

#[test]
fn other_urls_preserve_normal_proxy_behavior() {
    for url in [
        "http://localhost:3210/mcp",
        "http://192.0.2.1/mcp",
        "http://[2001:db8::1]/mcp",
        "https://example.com/mcp",
        "ftp://127.0.0.1/mcp",
    ] {
        let url = Url::parse(url).expect("valid URL");
        assert!(
            !is_literal_loopback_url(&url),
            "expected {url} to preserve normal proxy behavior"
        );
    }
}

#[tokio::test]
async fn loopback_direct_client_follows_literal_loopback_redirects() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("test listener address");
    let router = Router::new()
        .route("/", get(|| async { Redirect::temporary("/target") }))
        .route("/target", get(|| async { "redirected" }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve redirect test");
    });

    let (response, stream) = ReqwestHttpRequestRunner::run(HttpRequestParams {
        method: "GET".to_string(),
        url: format!("http://{addr}/"),
        headers: Vec::new(),
        body: None,
        timeout_ms: Some(1_000),
        redirect_policy: HttpRedirectPolicy::Follow,
        request_id: "same-loopback-redirect-test".to_string(),
        stream_response: false,
    })
    .await
    .expect("same-loopback redirect should be followed");

    task.abort();
    let _ = task.await;
    assert_eq!(response.status, 200);
    assert_eq!(response.body.into_inner(), b"redirected");
    assert!(stream.is_none());
}

#[tokio::test]
async fn loopback_direct_client_preserves_ten_redirect_limit() {
    async fn redirect_chain(Path(remaining): Path<u8>) -> Response {
        if remaining == 0 {
            "redirected".into_response()
        } else {
            Redirect::temporary(&format!("/{}", remaining - 1)).into_response()
        }
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("test listener address");
    let router = Router::new().route("/{remaining}", get(redirect_chain));
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve redirect boundary test");
    });

    let (response, stream) = ReqwestHttpRequestRunner::run(HttpRequestParams {
        method: "GET".to_string(),
        url: format!("http://{addr}/10"),
        headers: Vec::new(),
        body: None,
        timeout_ms: Some(5_000),
        redirect_policy: HttpRedirectPolicy::Follow,
        request_id: "ten-redirects-test".to_string(),
        stream_response: false,
    })
    .await
    .expect("ten loopback redirects should be followed");
    assert_eq!(response.status, 200);
    assert_eq!(response.body.into_inner(), b"redirected");
    assert!(stream.is_none());

    let result = ReqwestHttpRequestRunner::run(HttpRequestParams {
        method: "GET".to_string(),
        url: format!("http://{addr}/11"),
        headers: Vec::new(),
        body: None,
        timeout_ms: Some(5_000),
        redirect_policy: HttpRedirectPolicy::Follow,
        request_id: "eleven-redirects-test".to_string(),
        stream_response: false,
    })
    .await;

    task.abort();
    let _ = task.await;
    let Err(error) = result else {
        panic!("eleven redirects should be rejected");
    };
    assert!(
        error
            .message
            .starts_with("http/request failed: error following redirect"),
        "unexpected redirect error: {}",
        error.message
    );
}

#[tokio::test]
async fn loopback_direct_client_rejects_non_literal_loopback_redirects() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("test listener address");
    let redirect_target = format!("http://localhost:{}/target", addr.port());
    let router = Router::new()
        .route(
            "/",
            get(move || {
                let redirect_target = redirect_target.clone();
                async move { Redirect::temporary(&redirect_target) }
            }),
        )
        .route("/target", get(|| async { "unexpected redirect" }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve redirect test");
    });

    let result = ReqwestHttpRequestRunner::run(HttpRequestParams {
        method: "GET".to_string(),
        url: format!("http://{addr}/"),
        headers: Vec::new(),
        body: None,
        timeout_ms: Some(1_000),
        redirect_policy: HttpRedirectPolicy::Follow,
        request_id: "redirect-test".to_string(),
        stream_response: false,
    })
    .await;

    task.abort();
    let _ = task.await;
    assert!(result.is_err(), "redirect should have been rejected");
}

#[tokio::test]
async fn loopback_direct_client_bounds_redirect_cycles() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("test listener address");
    let router = Router::new().route("/loop", get(|| async { Redirect::temporary("/loop") }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve redirect loop test");
    });

    let result = tokio::time::timeout(
        Duration::from_secs(1),
        ReqwestHttpRequestRunner::run(HttpRequestParams {
            method: "GET".to_string(),
            url: format!("http://{addr}/loop"),
            headers: Vec::new(),
            body: None,
            timeout_ms: None,
            redirect_policy: HttpRedirectPolicy::Follow,
            request_id: "redirect-loop-test".to_string(),
            stream_response: false,
        }),
    )
    .await
    .expect("redirect loop should be bounded");

    task.abort();
    let _ = task.await;
    assert!(result.is_err(), "redirect loop should have been rejected");
}
