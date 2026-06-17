use thiserror::Error;

#[derive(Debug, Error)]
pub enum BoundedResponseBodyError {
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error("response body exceeds the configured size limit")]
    TooLarge,
}

pub async fn read_bounded_response_body(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, BoundedResponseBodyError> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(BoundedResponseBodyError::TooLarge);
    }

    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if chunk.len() > max_bytes.saturating_sub(body.len()) {
            return Err(BoundedResponseBodyError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
