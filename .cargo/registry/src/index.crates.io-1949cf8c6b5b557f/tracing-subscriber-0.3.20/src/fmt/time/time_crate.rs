use crate::fmt::{format::Writer, time::FormatTime, writer::WriteAdaptor};
use std::fmt;
use time::{format_description::well_known, formatting::Formattable, OffsetDateTime, UtcOffset};

/// Formats the current [local time] using a [formatter] from the [`time` crate].
///
/// To format the current [UTC time] instead, use the [`UtcTime`] type.
///
/// <div class="example-wrap" style="display:inline-block">
/// <pre class="compile_fail" style="white-space:normal;font:inherit;">
///     <strong>Warning</strong>: The <a href = "https://docs.rs/time/0.3/time/"><code>time</code>
///     crate</a> must be compiled with <code>--cfg unsound_local_offset</code> in order to use
///     local timestamps. When this cfg is not enabled, local timestamps cannot be recorded, and
///     events will be logged without timestamps.
///
///    Alternatively, [`OffsetTime`] can log with a local offset if it is initialized early.
///
///    See the <a href="https://docs.rs/time/0.3.4/time/#feature-flags"><code>time</code>
///    documentation</a> for more details.
/// </pre></div>
///
/// [local time]: time::OffsetDateTime::now_local
/// [UTC time]:     time::OffsetDateTime::now_utc
/// [formatter]:    time::formatting::Formattable
/// [`time` crate]: time
#[derive(Clone, Debug)]
#[cfg_attr(
    docsrs,
    doc(cfg(all(unsound_local_offset, feature = "time", feature = "local-time")))
)]
#[cfg(feature = "local-time")]
pub struct LocalTime<F> {
    format: F,
}

/// Formats the current [UTC time] using a [formatter] from the [`time` crate].
///
/// To format the current [local time] instead, use the [`LocalTime`] type.
///
/// [local time]: time::OffsetDateTime::now_local
/// [UTC time]:     time::OffsetDateTime::now_utc
/// [formatter]:    time::formatting::Formattable
/// [`time` crate]: time
#[cfg_attr(docsrs, doc(cfg(feature = "time")))]
#[derive(Clone, Debug)]
pub struct UtcTime<F> {
    format: F,
}

/// Formats the current time using a fixed offset and a [formatter] from the [`time` crate].
///
/// This is typically used as an alternative to [`LocalTime`]. `LocalTime` determines the offset
/// every time it formats a message, which may be unsound or fail. With `OffsetTime`, the offset is
/// determined once. This makes it possible to do so while the program is still single-threaded and
/// handle any errors. However, this also means the offset cannot change while the program is
/// running (the offset will not change across DST changes).
///
/// [formatter]: time::formatting::Formattable
/// [`time` crate]: time
#[derive(Clone, Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "time")))]
pub struct OffsetTime<F> {
    offset: time::UtcOffset,
    format: F,
}

// === impl LocalTime ===

#[cfg(feature = "local-time")]
impl LocalTime<well_known::Rfc3339> {
    /// Returns a formatter that formats the current [local time] in the
    /// [RFC 3339] format (a subset of the [ISO 8601] timestamp format).
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time};
    ///
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(time::LocalTime::rfc_3339());
    /// # drop(subscriber);
    /// ```
    ///
    /// [local time]: time::OffsetDateTime::now_local
    /// [RFC 3339]: https://datatracker.ietf.org/doc/html/rfc3339
    /// [ISO 8601]: https://en.wikipedia.org/wiki/ISO_8601
    pub fn rfc_3339() -> Self {
        Self::new(well_known::Rfc3339)
    }
}

#[cfg(feature = "local-time")]
impl<F: Formattable> LocalTime<F> {
    /// Returns a formatter that formats the current [local time] using the
    /// [`time` crate] with the provided provided format. The format may be any
    /// type that implements the [`Formattable`] trait.
    ///
    ///
    /// <div class="example-wrap" style="display:inline-block">
    /// <pre class="compile_fail" style="white-space:normal;font:inherit;">
    ///     <strong>Warning</strong>: The <a href = "https://docs.rs/time/0.3/time/">
    ///     <code>time</code> crate</a> must be compiled with <code>--cfg
    ///     unsound_local_offset</code> in order to use local timestamps. When this
    ///     cfg is not enabled, local timestamps cannot be recorded, and
    ///     events will be logged without timestamps.
    ///
    ///    See the <a href="https://docs.rs/time/0.3.4/time/#feature-flags">
    ///    <code>time</code> documentation</a> for more details.
    /// </pre></div>
    ///
    /// Typically, the format will be a format description string, or one of the
    /// `time` crate's [well-known formats].
    ///
    /// If the format description is statically known, then the
    /// [`format_description!`] macro should be used. This is identical to the
    /// [`time::format_description::parse`] method, but runs at compile-time,
    /// throwing an error if the format description is invalid. If the desired format
    /// is not known statically (e.g., a user is providing a format string), then the
    /// [`time::format_description::parse`] method should be used. Note that this
    /// method is fallible.
    ///
    /// See the [`time` book] for details on the format description syntax.
    ///
    /// # Examples
    ///
    /// Using the [`format_description!`] macro:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::LocalTime};
    /// use time::macros::format_description;
    ///
    /// let timer = LocalTime::new(format_description!("[hour]:[minute]:[second]"));
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(subscriber);
    /// ```
    ///
    /// Using [`time::format_description::parse`]:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::LocalTime};
    ///
    /// let time_format = time::format_description::parse("[hour]:[minute]:[second]")
    ///     .expect("format string should be valid!");
    /// let timer = LocalTime::new(time_format);
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(subscriber);
    /// ```
    ///
    /// Using the [`format_description!`] macro requires enabling the `time`
    /// crate's "macros" feature flag.
    ///
    /// Using a [well-known format][well-known formats] (this is equivalent to
    /// [`LocalTime::rfc_3339`]):
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::LocalTime};
    ///
    /// let timer = LocalTime::new(time::format_description::well_known::Rfc3339);
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(subscriber);
    /// ```
    ///
    /// [local time]: time::OffsetDateTime::now_local()
    /// [`time` crate]: time
    /// [`Formattable`]: time::formatting::Formattable
    /// [well-known formats]: time::format_description::well_known
    /// [`format_description!`]: https://docs.rs/time/0.3/time/macros/macro.format_description.html
    /// [`time::format_description::parse`]: time::format_description::parse()
    /// [`time` book]: https://time-rs.github.io/book/api/format-description.html
    pub fn new(format: F) -> Self {
        Self { format }
    }
}

#[cfg(feature = "local-time")]
impl<F> FormatTime for LocalTime<F>
where
    F: Formattable,
{
    fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
        let now = OffsetDateTime::now_local().map_err(|_| fmt::Error)?;
        format_datetime(now, w, &self.format)
    }
}

#[cfg(feature = "local-time")]
impl<F> Default for LocalTime<F>
where
    F: Formattable + Default,
{
    fn default() -> Self {
        Self::new(F::default())
    }
}

// === impl UtcTime ===

impl UtcTime<well_known::Rfc3339> {
    /// Returns a formatter that formats the current [UTC time] in the
    /// [RFC 3339] format, which is a subset of the [ISO 8601] timestamp format.
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time};
    ///
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(time::UtcTime::rfc_3339());
    /// # drop(subscriber);
    /// ```
    ///
    /// [local time]: time::OffsetDateTime::now_utc
    /// [RFC 3339]: https://datatracker.ietf.org/doc/html/rfc3339
    /// [ISO 8601]: https://en.wikipedia.org/wiki/ISO_8601
    pub fn rfc_3339() -> Self {
        Self::new(well_known::Rfc3339)
    }
}

impl<F: Formattable> UtcTime<F> {
    /// Returns a formatter that formats the current [UTC time] using the
    /// [`time` crate], with the provided provided format. The format may be any
    /// type that implements the [`Formattable`] trait.
    ///
    /// Typically, the format will be a format description string, or one of the
    /// `time` crate's [well-known formats].
    ///
    /// If the format description is statically known, then the
    /// [`format_description!`] macro should be used. This is identical to the
    /// [`time::format_description::parse`] method, but runs at compile-time,
    /// failing  an error if the format description is invalid. If the desired format
    /// is not known statically (e.g., a user is providing a format string), then the
    /// [`time::format_description::parse`] method should be used. Note that this
    /// method is fallible.
    ///
    /// See the [`time` book] for details on the format description syntax.
    ///
    /// # Examples
    ///
    /// Using the [`format_description!`] macro:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::UtcTime};
    /// use time::macros::format_description;
    ///
    /// let timer = UtcTime::new(format_description!("[hour]:[minute]:[second]"));
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(subscriber);
    /// ```
    ///
    /// Using the [`format_description!`] macro requires enabling the `time`
    /// crate's "macros" feature flag.
    ///
    /// Using [`time::format_description::parse`]:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::UtcTime};
    ///
    /// let time_format = time::format_description::parse("[hour]:[minute]:[second]")
    ///     .expect("format string should be valid!");
    /// let timer = UtcTime::new(time_format);
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(subscriber);
    /// ```
    ///
    /// Using a [well-known format][well-known formats] (this is equivalent to
    /// [`UtcTime::rfc_3339`]):
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::UtcTime};
    ///
    /// let timer = UtcTime::new(time::format_description::well_known::Rfc3339);
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(subscriber);
    /// ```
    ///
    /// [UTC time]: time::OffsetDateTime::now_utc()
    /// [`time` crate]: time
    /// [`Formattable`]: time::formatting::Formattable
    /// [well-known formats]: time::format_description::well_known
    /// [`format_description!`]: https://docs.rs/time/0.3/time/macros/macro.format_description.html
    /// [`time::format_description::parse`]: time::format_description::parse
    /// [`time` book]: https://time-rs.github.io/book/api/format-description.html
    pub fn new(format: F) -> Self {
        Self { format }
    }
}

impl<F> FormatTime for UtcTime<F>
where
    F: Formattable,
{
    fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
        format_datetime(OffsetDateTime::now_utc(), w, &self.format)
    }
}

impl<F> Default for UtcTime<F>
where
    F: Formattable + Default,
{
    fn default() -> Self {
        Self::new(F::default())
    }
}

// === impl OffsetTime ===

#[cfg(feature = "local-time")]
impl OffsetTime<well_known::Rfc3339> {
    /// Returns a formatter that formats the current time using the [local time offset] in the [RFC
    /// 3339] format (a subset of the [ISO 8601] timestamp format).
    ///
    /// Returns an error if the local time offset cannot be determined. This typically occurs in
    /// multithreaded programs. To avoid this problem, initialize `OffsetTime` before forking
    /// threads. When using Tokio, this means initializing `OffsetTime` before the Tokio runtime.
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time};
    ///
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(time::OffsetTime::local_rfc_3339().expect("could not get local offset!"));
    /// # drop(subscriber);
    /// ```
    ///
    /// Using `OffsetTime` with Tokio:
    ///
    /// ```
    /// use tracing_subscriber::fmt::time::OffsetTime;
    ///
    /// #[tokio::main]
    /// async fn run() {
    ///     tracing::info!("runtime initialized");
    ///
    ///     // At this point the Tokio runtime is initialized, and we can use both Tokio and Tracing
    ///     // normally.
    /// }
    ///
    /// fn main() {
    ///     // Because we need to get the local offset before Tokio spawns any threads, our `main`
    ///     // function cannot use `tokio::main`.
    ///     tracing_subscriber::fmt()
    ///         .with_timer(OffsetTime::local_rfc_3339().expect("could not get local time offset"))
    ///         .init();
    ///
    ///     // Even though `run` is written as an `async fn`, because we used `tokio::main` on it
    ///     // we can call it as a synchronous function.
    ///     run();
    /// }
    /// ```
    ///
    /// [local time offset]: time::UtcOffset::current_local_offset
    /// [RFC 3339]: https://datatracker.ietf.org/doc/html/rfc3339
    /// [ISO 8601]: https://en.wikipedia.org/wiki/ISO_8601
    pub fn local_rfc_3339() -> Result<Self, time::error::IndeterminateOffset> {
        Ok(Self::new(
            UtcOffset::current_local_offset()?,
            well_known::Rfc3339,
        ))
    }
}

impl<F: time::formatting::Formattable> OffsetTime<F> {
    /// Returns a formatter that formats the current time using the [`time` crate] with the provided
    /// provided format and [timezone offset]. The format may be any type that implements the
    /// [`Formattable`] trait.
    ///
    ///
    /// Typically, the offset will be the [local offset], and format will be a format description
    /// string, or one of the `time` crate's [well-known formats].
    ///
    /// If the format description is statically known, then the
    /// [`format_description!`] macro should be used. This is identical to the
    /// [`time::format_description::parse`] method, but runs at compile-time,
    /// throwing an error if the format description is invalid. If the desired format
    /// is not known statically (e.g., a user is providing a format string), then the
    /// [`time::format_description::parse`] method should be used. Note that this
    /// method is fallible.
    ///
    /// See the [`time` book] for details on the format description syntax.
    ///
    /// # Examples
    ///
    /// Using the [`format_description!`] macro:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::OffsetTime};
    /// use time::macros::format_description;
    /// use time::UtcOffset;
    ///
    /// let offset = UtcOffset::current_local_offset().expect("should get local offset!");
    /// let timer = OffsetTime::new(offset, format_description!("[hour]:[minute]:[second]"));
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(subscriber);
    /// ```
    ///
    /// Using [`time::format_description::parse`]:
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::OffsetTime};
    /// use time::UtcOffset;
    ///
    /// let offset = UtcOffset::current_local_offset().expect("should get local offset!");
    /// let time_format = time::format_description::parse("[hour]:[minute]:[second]")
    ///     .expect("format string should be valid!");
    /// let timer = OffsetTime::new(offset, time_format);
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(subscriber);
    /// ```
    ///
    /// Using the [`format_description!`] macro requires enabling the `time`
    /// crate's "macros" feature flag.
    ///
    /// Using a [well-known format][well-known formats] (this is equivalent to
    /// [`OffsetTime::local_rfc_3339`]):
    ///
    /// ```
    /// use tracing_subscriber::fmt::{self, time::OffsetTime};
    /// use time::UtcOffset;
    ///
    /// let offset = UtcOffset::current_local_offset().expect("should get local offset!");
    /// let timer = OffsetTime::new(offset, time::format_description::well_known::Rfc3339);
    /// let subscriber = tracing_subscriber::fmt()
    ///     .with_timer(timer);
    /// # drop(subscriber);
    /// ```
    ///
    /// [`time` crate]: time
    /// [timezone offset]: time::UtcOffset
    /// [`Formattable`]: time::formatting::Formattable
    /// [local offset]: time::UtcOffset::current_local_offset()
    /// [well-known formats]: time::format_description::well_known
    /// [`format_description!`]: https://docs.rs/time/0.3/time/macros/macro.format_description.html
    /// [`time::format_description::parse`]: time::format_description::parse
    /// [`time` book]: https://time-rs.github.io/book/api/format-description.html
    pub fn new(offset: time::UtcOffset, format: F) -> Self {
        Self { offset, format }
    }
}

impl<F> FormatTime for OffsetTime<F>
where
    F: time::formatting::Formattable,
{
    fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
        let now = OffsetDateTime::now_utc().to_offset(self.offset);
        format_datetime(now, w, &self.format)
    }
}

fn format_datetime(
    now: OffsetDateTime,
    into: &mut Writer<'_>,
    fmt: &impl Formattable,
) -> fmt::Result {
    let mut into = WriteAdaptor::new(into);
    now.format_into(&mut into, fmt)
        .map_err(|_| fmt::Error)
        .map(|_| ())
}
