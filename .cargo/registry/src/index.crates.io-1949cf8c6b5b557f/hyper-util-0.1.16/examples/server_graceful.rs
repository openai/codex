use bytes::Bytes;
use std::convert::Infallible;
use std::pin::pin;
use std::time::Duration;
use tokio::net::TcpListener;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    let server = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    let mut ctrl_c = pin!(tokio::signal::ctrl_c());

    loop {
        tokio::select! {
            conn = listener.accept() => {
                let (stream, peer_addr) = match conn {
                    Ok(conn) => conn,
                    Err(e) => {
                        eprintln!("accept error: {}", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };
                eprintln!("incomming connection accepted: {}", peer_addr);

                let stream = hyper_util::rt::TokioIo::new(Box::pin(stream));

                let conn = server.serve_connection_with_upgrades(stream, hyper::service::service_fn(|_| async move {
                        tokio::time::sleep(Duration::from_secs(5)).await;  // emulate slow request
                        let body = http_body_util::Full::<Bytes>::from("Hello World!".to_owned());
                        Ok::<_, Infallible>(http::Response::new(body))
                    }));

                let conn = graceful.watch(conn.into_owned());

                tokio::spawn(async move {
                    if let Err(err) = conn.await {
                        eprintln!("connection error: {}", err);
                    }
                    eprintln!("connection dropped: {}", peer_addr);
                });
            },

            _ = ctrl_c.as_mut() => {
                drop(listener);
                eprintln!("Ctrl-C received, starting shutdown");
                    break;
            }
        }
    }

    tokio::select! {
        _ = graceful.shutdown() => {
            eprintln!("Gracefully shutdown!");
        },
        _ = tokio::time::sleep(Duration::from_secs(10)) => {
            eprintln!("Waited 10 seconds for graceful shutdown, aborting...");
        }
    }

    Ok(())
}
