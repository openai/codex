use crate::{HttpClientError, HttpRequest, HttpResponse};

use http::{
    header::{HeaderValue, CONTENT_TYPE},
    method::Method,
    status::StatusCode,
};

use std::io::Read;

impl crate::SyncHttpClient for ureq::Agent {
    type Error = HttpClientError<ureq::Error>;

    fn call(&self, request: HttpRequest) -> Result<HttpResponse, Self::Error> {
        let mut req = if *request.method() == Method::POST {
            self.post(&request.uri().to_string())
        } else {
            debug_assert_eq!(*request.method(), Method::GET);
            self.get(&request.uri().to_string())
        };

        for (name, value) in request.headers() {
            req = req.set(
                name.as_ref(),
                // TODO: In newer `ureq` it should be easier to convert arbitrary byte sequences
                // without unnecessary UTF-8 fallibility here.
                value.to_str().map_err(|_| {
                    HttpClientError::Other(format!(
                        "invalid `{name}` header value {:?}",
                        value.as_bytes()
                    ))
                })?,
            );
        }

        let response = if let Method::POST = *request.method() {
            req.send_bytes(request.body())
        } else {
            req.call()
        }
        .map_err(Box::new)?;

        let mut builder = http::Response::builder()
            .status(StatusCode::from_u16(response.status()).map_err(http::Error::from)?);

        if let Some(content_type) = response
            .header(CONTENT_TYPE.as_str())
            .map(HeaderValue::from_str)
            .transpose()
            .map_err(http::Error::from)?
        {
            builder = builder.header(CONTENT_TYPE, content_type);
        }

        let mut body = Vec::new();
        response.into_reader().read_to_end(&mut body)?;

        builder.body(body).map_err(HttpClientError::Http)
    }
}
