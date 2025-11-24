use bytes::Bytes;
use http_body_util::BodyExt;

use http_body_util::Empty;
use hyper_tls::HttpsConnector;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use tokio::io::{self, AsyncWriteExt as _};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let https = HttpsConnector::new();

    let client = Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(https);

    let mut res = client.get("https://hyper.rs".parse()?).await?;

    println!("Status: {}", res.status());
    println!("Headers:\n{:#?}", res.headers());

    while let Some(frame) = res.body_mut().frame().await {
        let frame = frame?;

        if let Some(d) = frame.data_ref() {
            io::stdout().write_all(d).await?;
        }
    }

    Ok(())
}
