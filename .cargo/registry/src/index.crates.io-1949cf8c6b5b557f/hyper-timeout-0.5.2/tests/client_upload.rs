use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::{client::legacy::Client, rt::TokioIo};
use std::{net::SocketAddr, time::Duration};
use tokio::io;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task;

use hyper_timeout::TimeoutConnector;

async fn spawn_test_server(listener: TcpListener, shutdown_rx: oneshot::Receiver<()>) {
    let http = http1::Builder::new();
    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    let mut signal = std::pin::pin!(shutdown_rx);

    loop {
        tokio::select! {
            Ok((stream, _addr)) = listener.accept() => {
                let io = TokioIo::new(stream);
                let conn = http.serve_connection(io, service_fn(handle_request));
                // watch this connection
                let fut = graceful.watch(conn);
                tokio::spawn(async move {
                    if let Err(e) = fut.await {
                        eprintln!("Error serving connection: {:?}", e);
                    }
                });
            },

            _ = &mut signal => {
                eprintln!("graceful shutdown signal received");
                break;
            }
        }
    }

    tokio::select! {
        _ = graceful.shutdown() => {
            eprintln!("all connections gracefully closed");
        },
        _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
            eprintln!("timed out wait for all connections to close");
        }
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let body = req.collect().await.expect("Failed to read body").to_bytes();
    assert!(!body.is_empty(), "empty body");

    Ok(Response::new(full("finished")))
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

#[tokio::test]
async fn test_upload_timeout() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to bind listener");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server_addr = listener.local_addr().unwrap();

    let server_handle = task::spawn(spawn_test_server(listener, shutdown_rx));

    let h = hyper_util::client::legacy::connect::HttpConnector::new();
    let mut connector = TimeoutConnector::new(h);
    connector.set_read_timeout(Some(Duration::from_millis(5)));

    // comment this out and the test will fail
    connector.set_reset_reader_on_write(true);

    let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(connector);

    let body = vec![0; 10 * 1024 * 1024]; // 10MB
    let req = Request::post(format!("http://{}/", server_addr))
        .body(full(body))
        .expect("request builder");

    let mut res = client.request(req).await.expect("request failed");

    let mut resp_body = Vec::new();
    while let Some(frame) = res.body_mut().frame().await {
        let bytes = frame
            .expect("frame error")
            .into_data()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Error when consuming frame"))
            .expect("data error");
        resp_body.extend_from_slice(&bytes);
    }

    assert_eq!(res.status(), 200);
    assert_eq!(resp_body, b"finished");

    let _ = shutdown_tx.send(());
    let _ = server_handle.await;
}
