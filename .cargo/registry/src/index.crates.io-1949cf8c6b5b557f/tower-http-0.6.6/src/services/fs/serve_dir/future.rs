use super::{
    open_file::{FileOpened, FileRequestExtent, OpenFileOutput},
    DefaultServeDirFallback, ResponseBody,
};
use crate::{
    body::UnsyncBoxBody, content_encoding::Encoding, services::fs::AsyncReadBody, BoxError,
};
use bytes::Bytes;
use futures_core::future::BoxFuture;
use futures_util::future::{FutureExt, TryFutureExt};
use http::{
    header::{self, ALLOW},
    HeaderValue, Request, Response, StatusCode,
};
use http_body_util::{BodyExt, Empty, Full};
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    future::Future,
    io,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tower_service::Service;

pin_project! {
    /// Response future of [`ServeDir::try_call()`][`super::ServeDir::try_call()`].
    pub struct ResponseFuture<ReqBody, F = DefaultServeDirFallback> {
        #[pin]
        pub(super) inner: ResponseFutureInner<ReqBody, F>,
    }
}

impl<ReqBody, F> ResponseFuture<ReqBody, F> {
    pub(super) fn open_file_future(
        future: BoxFuture<'static, io::Result<OpenFileOutput>>,
        fallback_and_request: Option<(F, Request<ReqBody>)>,
    ) -> Self {
        Self {
            inner: ResponseFutureInner::OpenFileFuture {
                future,
                fallback_and_request,
            },
        }
    }

    pub(super) fn invalid_path(fallback_and_request: Option<(F, Request<ReqBody>)>) -> Self {
        Self {
            inner: ResponseFutureInner::InvalidPath {
                fallback_and_request,
            },
        }
    }

    pub(super) fn method_not_allowed() -> Self {
        Self {
            inner: ResponseFutureInner::MethodNotAllowed,
        }
    }
}

pin_project! {
    #[project = ResponseFutureInnerProj]
    pub(super) enum ResponseFutureInner<ReqBody, F> {
        OpenFileFuture {
            #[pin]
            future: BoxFuture<'static, io::Result<OpenFileOutput>>,
            fallback_and_request: Option<(F, Request<ReqBody>)>,
        },
        FallbackFuture {
            future: BoxFuture<'static, Result<Response<ResponseBody>, Infallible>>,
        },
        InvalidPath {
            fallback_and_request: Option<(F, Request<ReqBody>)>,
        },
        MethodNotAllowed,
    }
}

impl<F, ReqBody, ResBody> Future for ResponseFuture<ReqBody, F>
where
    F: Service<Request<ReqBody>, Response = Response<ResBody>, Error = Infallible> + Clone,
    F::Future: Send + 'static,
    ResBody: http_body::Body<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Output = io::Result<Response<ResponseBody>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();

            let new_state = match this.inner.as_mut().project() {
                ResponseFutureInnerProj::OpenFileFuture {
                    future: open_file_future,
                    fallback_and_request,
                } => match ready!(open_file_future.poll(cx)) {
                    Ok(OpenFileOutput::FileOpened(file_output)) => {
                        break Poll::Ready(Ok(build_response(*file_output)));
                    }

                    Ok(OpenFileOutput::Redirect { location }) => {
                        let mut res = response_with_status(StatusCode::TEMPORARY_REDIRECT);
                        res.headers_mut().insert(http::header::LOCATION, location);
                        break Poll::Ready(Ok(res));
                    }

                    Ok(OpenFileOutput::FileNotFound) => {
                        if let Some((mut fallback, request)) = fallback_and_request.take() {
                            call_fallback(&mut fallback, request)
                        } else {
                            break Poll::Ready(Ok(not_found()));
                        }
                    }

                    Ok(OpenFileOutput::PreconditionFailed) => {
                        break Poll::Ready(Ok(response_with_status(
                            StatusCode::PRECONDITION_FAILED,
                        )));
                    }

                    Ok(OpenFileOutput::NotModified) => {
                        break Poll::Ready(Ok(response_with_status(StatusCode::NOT_MODIFIED)));
                    }

                    Ok(OpenFileOutput::InvalidRedirectUri) => {
                        break Poll::Ready(Ok(response_with_status(
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )));
                    }

                    Err(err) => {
                        #[cfg(unix)]
                        // 20 = libc::ENOTDIR => "not a directory
                        // when `io_error_more` landed, this can be changed
                        // to checking for `io::ErrorKind::NotADirectory`.
                        // https://github.com/rust-lang/rust/issues/86442
                        let error_is_not_a_directory = err.raw_os_error() == Some(20);
                        #[cfg(not(unix))]
                        let error_is_not_a_directory = false;

                        if matches!(
                            err.kind(),
                            io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                        ) || error_is_not_a_directory
                        {
                            if let Some((mut fallback, request)) = fallback_and_request.take() {
                                call_fallback(&mut fallback, request)
                            } else {
                                break Poll::Ready(Ok(not_found()));
                            }
                        } else {
                            break Poll::Ready(Err(err));
                        }
                    }
                },

                ResponseFutureInnerProj::FallbackFuture { future } => {
                    break Pin::new(future).poll(cx).map_err(|err| match err {})
                }

                ResponseFutureInnerProj::InvalidPath {
                    fallback_and_request,
                } => {
                    if let Some((mut fallback, request)) = fallback_and_request.take() {
                        call_fallback(&mut fallback, request)
                    } else {
                        break Poll::Ready(Ok(not_found()));
                    }
                }

                ResponseFutureInnerProj::MethodNotAllowed => {
                    let mut res = response_with_status(StatusCode::METHOD_NOT_ALLOWED);
                    res.headers_mut()
                        .insert(ALLOW, HeaderValue::from_static("GET,HEAD"));
                    break Poll::Ready(Ok(res));
                }
            };

            this.inner.set(new_state);
        }
    }
}

fn response_with_status(status: StatusCode) -> Response<ResponseBody> {
    Response::builder()
        .status(status)
        .body(empty_body())
        .unwrap()
}

fn not_found() -> Response<ResponseBody> {
    response_with_status(StatusCode::NOT_FOUND)
}

pub(super) fn call_fallback<F, B, FResBody>(
    fallback: &mut F,
    req: Request<B>,
) -> ResponseFutureInner<B, F>
where
    F: Service<Request<B>, Response = Response<FResBody>, Error = Infallible> + Clone,
    F::Future: Send + 'static,
    FResBody: http_body::Body<Data = Bytes> + Send + 'static,
    FResBody::Error: Into<BoxError>,
{
    let future = fallback
        .call(req)
        .map_ok(|response| {
            response
                .map(|body| {
                    UnsyncBoxBody::new(
                        body.map_err(|err| match err.into().downcast::<io::Error>() {
                            Ok(err) => *err,
                            Err(err) => io::Error::new(io::ErrorKind::Other, err),
                        })
                        .boxed_unsync(),
                    )
                })
                .map(ResponseBody::new)
        })
        .boxed();

    ResponseFutureInner::FallbackFuture { future }
}

fn build_response(output: FileOpened) -> Response<ResponseBody> {
    let (maybe_file, size) = match output.extent {
        FileRequestExtent::Full(file, meta) => (Some(file), meta.len()),
        FileRequestExtent::Head(meta) => (None, meta.len()),
    };

    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, output.mime_header_value)
        .header(header::ACCEPT_RANGES, "bytes");

    if let Some(encoding) = output
        .maybe_encoding
        .filter(|encoding| *encoding != Encoding::Identity)
    {
        builder = builder.header(header::CONTENT_ENCODING, encoding.into_header_value());
    }

    if let Some(last_modified) = output.last_modified {
        builder = builder.header(header::LAST_MODIFIED, last_modified.0.to_string());
    }

    match output.maybe_range {
        Some(Ok(ranges)) => {
            if let Some(range) = ranges.first() {
                if ranges.len() > 1 {
                    builder
                        .header(header::CONTENT_RANGE, format!("bytes */{}", size))
                        .status(StatusCode::RANGE_NOT_SATISFIABLE)
                        .body(body_from_bytes(Bytes::from(
                            "Cannot serve multipart range requests",
                        )))
                        .unwrap()
                } else {
                    let body = if let Some(file) = maybe_file {
                        let range_size = range.end() - range.start() + 1;
                        ResponseBody::new(UnsyncBoxBody::new(
                            AsyncReadBody::with_capacity_limited(
                                file,
                                output.chunk_size,
                                range_size,
                            )
                            .boxed_unsync(),
                        ))
                    } else {
                        empty_body()
                    };

                    let content_length = if size == 0 {
                        0
                    } else {
                        range.end() - range.start() + 1
                    };

                    builder
                        .header(
                            header::CONTENT_RANGE,
                            format!("bytes {}-{}/{}", range.start(), range.end(), size),
                        )
                        .header(header::CONTENT_LENGTH, content_length)
                        .status(StatusCode::PARTIAL_CONTENT)
                        .body(body)
                        .unwrap()
                }
            } else {
                builder
                    .header(header::CONTENT_RANGE, format!("bytes */{}", size))
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .body(body_from_bytes(Bytes::from(
                        "No range found after parsing range header, please file an issue",
                    )))
                    .unwrap()
            }
        }

        Some(Err(_)) => builder
            .header(header::CONTENT_RANGE, format!("bytes */{}", size))
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .body(empty_body())
            .unwrap(),

        // Not a range request
        None => {
            let body = if let Some(file) = maybe_file {
                ResponseBody::new(UnsyncBoxBody::new(
                    AsyncReadBody::with_capacity(file, output.chunk_size).boxed_unsync(),
                ))
            } else {
                empty_body()
            };

            builder
                .header(header::CONTENT_LENGTH, size.to_string())
                .body(body)
                .unwrap()
        }
    }
}

fn body_from_bytes(bytes: Bytes) -> ResponseBody {
    let body = Full::from(bytes).map_err(|err| match err {}).boxed_unsync();
    ResponseBody::new(UnsyncBoxBody::new(body))
}

fn empty_body() -> ResponseBody {
    let body = Empty::new().map_err(|err| match err {}).boxed_unsync();
    ResponseBody::new(UnsyncBoxBody::new(body))
}
