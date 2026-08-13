use std::error::Error;

use tokio::io;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

use crate::fs_helper::FsHelperOpenResponse;
use crate::fs_helper::FsHelperPayload;
use crate::fs_helper::FsHelperRequest;
use crate::fs_helper::FsHelperResponse;
use crate::fs_helper::map_fs_error;
use crate::fs_helper::run_direct_request;
use crate::regular_file;

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
    let mut stdin = BufReader::new(io::stdin());
    let mut input = String::new();
    stdin.read_line(&mut input).await?;
    let request: FsHelperRequest = serde_json::from_str(&input)?;
    let mut opened_file = None;
    let result = match request {
        FsHelperRequest::Open(params) => {
            let result: io::Result<_> = async {
                let path = params.path.to_abs_path()?;
                let file = regular_file::open(path.as_path()).await?;
                // Unix can hand the opened fd directly to the parent.
                #[cfg(unix)]
                crate::sandboxed_file_open::transfer_file(&file)?;
                let response = FsHelperOpenResponse {
                    // Windows duplicates from the helper process instead.
                    #[cfg(windows)]
                    process_id: std::process::id(),
                    // The parent needs the raw handle to duplicate it.
                    #[cfg(windows)]
                    file_handle: {
                        use std::os::windows::io::AsRawHandle;

                        file.as_raw_handle() as usize as u64
                    },
                };
                opened_file = Some(file);
                Ok(FsHelperPayload::Open(response))
            }
            .await;
            result.map_err(map_fs_error)
        }
        request => run_direct_request(request).await,
    };
    let response = match result {
        Ok(payload) => FsHelperResponse::Ok(payload),
        Err(error) => FsHelperResponse::Error(error),
    };
    let mut stdout = io::stdout();
    stdout
        .write_all(serde_json::to_string(&response)?.as_bytes())
        .await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;

    // Keep the Windows handle alive until the parent duplicates it.
    #[cfg(windows)]
    if opened_file.is_some() {
        use tokio::io::AsyncReadExt;

        let mut acknowledgement = Vec::new();
        stdin.read_to_end(&mut acknowledgement).await?;
    }
    drop(opened_file);
    Ok(())
}
