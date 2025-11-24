use crate::{HttpClientError, HttpRequest, HttpResponse, SyncHttpClient};

use curl::easy::Easy;
use http::header::{HeaderValue, CONTENT_TYPE};
use http::method::Method;
use http::status::StatusCode;

use std::io::Read;

/// A synchronous HTTP client using [`curl`].
pub struct CurlHttpClient;
impl SyncHttpClient for CurlHttpClient {
    type Error = HttpClientError<curl::Error>;

    fn call(&self, request: HttpRequest) -> Result<HttpResponse, Self::Error> {
        let mut easy = Easy::new();
        easy.url(&request.uri().to_string()[..]).map_err(Box::new)?;

        let mut headers = curl::easy::List::new();
        for (name, value) in request.headers() {
            headers
                .append(&format!(
                    "{}: {}",
                    name,
                    // TODO: Unnecessary fallibility, curl uses a CString under the hood
                    value.to_str().map_err(|_| HttpClientError::Other(format!(
                        "invalid `{name}` header value {:?}",
                        value.as_bytes()
                    )))?
                ))
                .map_err(Box::new)?
        }

        easy.http_headers(headers).map_err(Box::new)?;

        if let Method::POST = *request.method() {
            easy.post(true).map_err(Box::new)?;
            easy.post_field_size(request.body().len() as u64)
                .map_err(Box::new)?;
        } else {
            assert_eq!(*request.method(), Method::GET);
        }

        let mut form_slice = &request.body()[..];
        let mut data = Vec::new();
        {
            let mut transfer = easy.transfer();

            transfer
                .read_function(|buf| Ok(form_slice.read(buf).unwrap_or(0)))
                .map_err(Box::new)?;

            transfer
                .write_function(|new_data| {
                    data.extend_from_slice(new_data);
                    Ok(new_data.len())
                })
                .map_err(Box::new)?;

            transfer.perform().map_err(Box::new)?;
        }

        let mut builder = http::Response::builder().status(
            StatusCode::from_u16(easy.response_code().map_err(Box::new)? as u16)
                .map_err(http::Error::from)?,
        );

        if let Some(content_type) = easy
            .content_type()
            .map_err(Box::new)?
            .map(HeaderValue::from_str)
            .transpose()
            .map_err(http::Error::from)?
        {
            builder = builder.header(CONTENT_TYPE, content_type);
        }

        builder.body(data).map_err(HttpClientError::Http)
    }
}
