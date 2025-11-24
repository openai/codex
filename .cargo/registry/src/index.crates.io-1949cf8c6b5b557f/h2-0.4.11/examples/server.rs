use std::error::Error;

use bytes::Bytes;
use h2::server::{self, SendResponse};
use h2::RecvStream;
use http::Request;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = env_logger::try_init();

    let listener = TcpListener::bind("127.0.0.1:5928").await?;

    println!("listening on {:?}", listener.local_addr());

    loop {
        if let Ok((socket, _peer_addr)) = listener.accept().await {
            tokio::spawn(async move {
                if let Err(e) = serve(socket).await {
                    println!("  -> err={:?}", e);
                }
            });
        }
    }
}

async fn serve(socket: TcpStream) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connection = server::handshake(socket).await?;
    println!("H2 connection bound");

    while let Some(result) = connection.accept().await {
        let (request, respond) = result?;
        tokio::spawn(async move {
            if let Err(e) = handle_request(request, respond).await {
                println!("error while handling request: {}", e);
            }
        });
    }

    println!("~~~~~~~~~~~ H2 connection CLOSE !!!!!! ~~~~~~~~~~~");
    Ok(())
}

async fn handle_request(
    mut request: Request<RecvStream>,
    mut respond: SendResponse<Bytes>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("GOT request: {:?}", request);

    let body = request.body_mut();
    while let Some(data) = body.data().await {
        let data = data?;
        println!("<<<< recv {:?}", data);
        let _ = body.flow_control().release_capacity(data.len());
    }

    let response = http::Response::new(());
    let mut send = respond.send_response(response, false)?;
    println!(">>>> send");
    send.send_data(Bytes::from_static(b"hello "), false)?;
    send.send_data(Bytes::from_static(b"world\n"), true)?;

    Ok(())
}
