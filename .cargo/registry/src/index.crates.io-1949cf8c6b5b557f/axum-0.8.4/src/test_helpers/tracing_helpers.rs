use std::{
    future::{Future, IntoFuture},
    io,
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex},
};

use serde::{de::DeserializeOwned, Deserialize};
use tracing::instrument::WithSubscriber;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{filter::Targets, fmt::MakeWriter};

#[derive(Deserialize, Eq, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct TracingEvent<T> {
    pub(crate) fields: T,
    pub(crate) target: String,
    pub(crate) level: String,
}

/// Run an async closure and capture the tracing output it produces.
pub(crate) fn capture_tracing<T, F>(f: F) -> CaptureTracing<T, F>
where
    T: DeserializeOwned,
{
    CaptureTracing {
        f,
        filter: None,
        _phantom: PhantomData,
    }
}

pub(crate) struct CaptureTracing<T, F> {
    f: F,
    filter: Option<Targets>,
    _phantom: PhantomData<fn() -> T>,
}

impl<T, F> CaptureTracing<T, F> {
    pub(crate) fn with_filter(mut self, filter_string: &str) -> Self {
        self.filter = Some(filter_string.parse().unwrap());
        self
    }
}

impl<T, F, Fut> IntoFuture for CaptureTracing<T, F>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future + Send,
    T: DeserializeOwned,
{
    type Output = Vec<TracingEvent<T>>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        let Self { f, filter, .. } = self;
        Box::pin(async move {
            let (make_writer, handle) = TestMakeWriter::new();

            let filter = filter.unwrap_or_else(|| "axum=trace".parse().unwrap());
            let subscriber = tracing_subscriber::registry().with(
                tracing_subscriber::fmt::layer()
                    .with_writer(make_writer)
                    .with_target(true)
                    .without_time()
                    .with_ansi(false)
                    .json()
                    .flatten_event(false)
                    .with_filter(filter),
            );

            let guard = tracing::subscriber::set_default(subscriber);

            f().with_current_subscriber().await;

            drop(guard);

            handle
                .take()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect()
        })
    }
}

struct TestMakeWriter {
    write: Arc<Mutex<Option<Vec<u8>>>>,
}

impl TestMakeWriter {
    fn new() -> (Self, Handle) {
        let write = Arc::new(Mutex::new(Some(Vec::<u8>::new())));

        (
            Self {
                write: write.clone(),
            },
            Handle { write },
        )
    }
}

impl<'a> MakeWriter<'a> for TestMakeWriter {
    type Writer = Writer<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        Writer(self)
    }
}

struct Writer<'a>(&'a TestMakeWriter);

impl io::Write for Writer<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut *self.0.write.lock().unwrap() {
            Some(vec) => {
                let len = buf.len();
                vec.extend(buf);
                Ok(len)
            }
            None => Err(io::Error::other("inner writer has been taken")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Handle {
    write: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Handle {
    fn take(self) -> String {
        let vec = self.write.lock().unwrap().take().unwrap();
        String::from_utf8(vec).unwrap()
    }
}
