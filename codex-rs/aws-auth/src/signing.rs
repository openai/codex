use std::str::FromStr;
use std::time::SystemTime;

use aws_credential_types::Credentials;
use aws_sigv4::http_request::SignableBody;
use aws_sigv4::http_request::SignableRequest;
use aws_sigv4::http_request::SigningSettings;
use aws_sigv4::http_request::sign;
use aws_sigv4::sign::v4;
use http::Request;
use http::Uri;

use crate::AwsAuthError;
use crate::AwsRequestToSign;
use crate::AwsSignedRequest;

pub(crate) fn sign_request(
    credentials: &Credentials,
    region: &str,
    service: &str,
    request: AwsRequestToSign,
    time: SystemTime,
) -> Result<AwsSignedRequest, AwsAuthError> {
    let signable_headers = request
        .headers
        .iter()
        .map(|(name, value)| {
            Ok::<_, AwsAuthError>((
                name.as_str(),
                value.to_str().map_err(AwsAuthError::InvalidHeaderValue)?,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signable_request = SignableRequest::new(
        request.method.as_str(),
        request.url.as_str(),
        signable_headers.into_iter(),
        SignableBody::Bytes(request.body.as_ref()),
    )
    .map_err(AwsAuthError::SigningRequest)?;
    let identity = credentials.clone().into();

    let signing_params = v4::SigningParams::builder()
        .identity(&identity)
        .region(region)
        .name(service)
        .time(time)
        .settings(SigningSettings::default())
        .build()
        .map_err(|err| AwsAuthError::SigningParams(err.to_string()))?;
    let (instructions, _signature) = sign(signable_request, &signing_params.into())
        .map_err(AwsAuthError::SigningFailure)?
        .into_parts();

    let uri = Uri::from_str(&request.url).map_err(AwsAuthError::InvalidUri)?;
    let mut http_request = Request::builder()
        .method(request.method)
        .uri(uri)
        .body(())
        .map_err(AwsAuthError::BuildHttpRequest)?;
    *http_request.headers_mut() = request.headers;
    instructions.apply_to_request_http1x(&mut http_request);

    Ok(AwsSignedRequest {
        url: http_request.uri().to_string(),
        headers: http_request.headers().clone(),
    })
}

#[cfg(test)]
pub(crate) fn header_value(headers: &http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}
