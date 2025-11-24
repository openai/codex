use async_stream::stream;
use futures_util::pin_mut;
use futures_util::stream::StreamExt;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let incoming = stream! {
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            yield socket;
        }
    };
    pin_mut!(incoming);

    while let Some(v) = incoming.next().await {
        println!("handle = {:?}", v);
    }
}
