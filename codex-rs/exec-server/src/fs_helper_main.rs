use std::error::Error;

use tokio::io;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::fs_helper::FsHelperRequest;
use crate::fs_helper::FsHelperResponse;
use crate::fs_helper::run_direct_request;

pub fn main() -> ! {
    let exit_code = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => match runtime.block_on(run_main()) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("fs sandbox helper failed: {err}");
                1
            }
        },
        Err(err) => {
            eprintln!("failed to start fs sandbox helper runtime: {err}");
            1
        }
    };
    std::process::exit(exit_code);
}

async fn run_main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).await?;
    let request: FsHelperRequest = serde_json::from_slice(&input)?;
    let response = match run_direct_request(request).await {
        Ok(payload) => FsHelperResponse::Ok(payload),
        Err(error) => FsHelperResponse::Error(error),
    };
    let mut stdout = io::stdout();
    stdout
        .write_all(serde_json::to_string(&response)?.as_bytes())
        .await?;
    stdout.write_all(b"\n").await?;
    Ok(())
}
