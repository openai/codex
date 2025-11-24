//! This example runs a server that responds to any request with "Hello, world!"

use std::{convert::Infallible, error::Error};

use bytes::Bytes;
use http::{header::CONTENT_TYPE, Request, Response};
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use tokio::{net::TcpListener, task::JoinSet};

/// Function from an incoming request to an outgoing response
///
/// This function gets turned into a [`hyper::service::Service`] later via
/// [`service_fn`]. Instead of doing this, you could also write a type that
/// implements [`hyper::service::Service`] directly and pass that in place of
/// writing a function like this and calling [`service_fn`].
///
/// This function could use [`Full`] as the body type directly since that's
/// the only type that can be returned in this case, but this uses [`BoxBody`]
/// anyway for demonstration purposes, since this is what's usually used when
/// writing a more complex webserver library.
async fn handle_request(
    _request: Request<Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, Infallible> {
    let response = Response::builder()
        .header(CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from("Hello, world!\n")).boxed())
        .expect("values provided to the builder should be valid");

    Ok(response)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let listen_addr = "127.0.0.1:8000";
    let tcp_listener = TcpListener::bind(listen_addr).await?;
    println!("listening on http://{listen_addr}");

    let mut join_set = JoinSet::new();
    loop {
        let (stream, addr) = match tcp_listener.accept().await {
            Ok(x) => x,
            Err(e) => {
                eprintln!("failed to accept connection: {e}");
                continue;
            }
        };

        let serve_connection = async move {
            println!("handling a request from {addr}");

            let result = Builder::new(TokioExecutor::new())
                .serve_connection(TokioIo::new(stream), service_fn(handle_request))
                .await;

            if let Err(e) = result {
                eprintln!("error serving {addr}: {e}");
            }

            println!("handled a request from {addr}");
        };

        join_set.spawn(serve_connection);
    }

    // If you add a method for breaking the above loop (i.e. graceful shutdown),
    // then you may also want to wait for all existing connections to finish
    // being served before terminating the program, which can be done like this:
    //
    // while let Some(_) = join_set.join_next().await {}
}
