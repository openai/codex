use std::io;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_utils_path_uri::PathUri;

use super::METHOD_NOT_FOUND_ERROR_CODE;
use super::RemoteFileSystem;
use super::map_remote_error;
use super::remote_sandbox_context;
use crate::ExecServerError;
use crate::ExecutorFileSystem;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use crate::TextFilePrefix;
use crate::protocol::FsReadTextPrefixesBatchParams;
use crate::protocol::FsReadTextPrefixesBatchResponse;
use crate::protocol::FsReadTextPrefixesBatchResult;

impl RemoteFileSystem {
    pub(super) async fn read_text_prefixes_batch(
        &self,
        paths: &[PathUri],
        max_bytes: usize,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<FileSystemResult<TextFilePrefix>>> {
        let client = self.client.get().await.map_err(map_remote_error)?;
        let response = match client
            .fs_read_text_prefixes_batch(FsReadTextPrefixesBatchParams {
                paths: paths.to_vec(),
                prefix_byte_limit: max_bytes,
                sandbox: remote_sandbox_context(sandbox),
            })
            .await
        {
            Ok(response) => response,
            Err(ExecServerError::Server {
                code: METHOD_NOT_FOUND_ERROR_CODE,
                ..
            }) => {
                return self
                    .read_text_prefixes_batch_via_reads(paths, max_bytes, sandbox)
                    .await;
            }
            Err(error) => return Err(map_remote_error(error)),
        };
        decode_response(response, paths.len(), max_bytes)
    }
}

fn decode_response(
    response: FsReadTextPrefixesBatchResponse,
    expected_results: usize,
    max_bytes: usize,
) -> FileSystemResult<Vec<FileSystemResult<TextFilePrefix>>> {
    if response.results.len() != expected_results {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "remote fs/readTextPrefixesBatch returned {} results for {expected_results} paths",
                response.results.len()
            ),
        ));
    }
    response
        .results
        .into_iter()
        .map(|result| match result {
            FsReadTextPrefixesBatchResult::Data {
                data_base64,
                complete,
            } => {
                let bytes = STANDARD.decode(data_base64).map_err(|error| {
                    io::Error::new(io::ErrorKind::InvalidData, format!(
                        "remote fs/readTextPrefixesBatch returned invalid base64 dataBase64: {error}"
                    ))
                })?;
                if bytes.len() > max_bytes {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "remote fs/readTextPrefixesBatch returned an oversized prefix",
                    ));
                }
                String::from_utf8(bytes)
                    .map(|text| Ok(TextFilePrefix { text, complete }))
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
            }
            FsReadTextPrefixesBatchResult::Error { error } => {
                Ok(Err(map_remote_error(ExecServerError::Server {
                    code: error.code,
                    message: error.message,
                })))
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "remote_text_prefix_tests.rs"]
mod tests;
