use std::env;
use std::time::Duration;

use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use tokio::io::{self, AsyncWriteExt};

use hyper_tls::HttpsConnector;

use hyper_timeout::TimeoutConnector;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            println!("Example: client https://example.com");
            return Ok(());
        }
    };

    let url = url.parse::<hyper::Uri>().unwrap();

    // This example uses `HttpsConnector`, but you can also use hyper `HttpConnector`
    //let h = hyper_util::client::legacy::connect::HttpConnector::new();
    let h = HttpsConnector::new();
    let mut connector = TimeoutConnector::new(h);
    connector.set_connect_timeout(Some(Duration::from_secs(5)));
    connector.set_read_timeout(Some(Duration::from_secs(5)));
    connector.set_write_timeout(Some(Duration::from_secs(5)));
    let client = Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(connector);

    let mut res = client.get(url).await?;

    println!("Status: {}", res.status());
    println!("Headers:\n{:#?}", res.headers());

    while let Some(frame) = res.body_mut().frame().await {
        let bytes = frame?
            .into_data()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Error when consuming frame"))?;
        io::stdout().write_all(&bytes).await?;
    }

    Ok(())
}
