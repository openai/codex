use alloc::borrow::Cow;
use alloc::str;
use alloc::string::String;
use core::cell::Cell;
use core::convert::Infallible;
use core::fmt::{self, Write};
use core::ops::Deref;
use core::pin::Pin;

use super::MAX_LEN;
use super::escape::HtmlSafeOutput;
use crate::{FastWritable, Result};

/// Return an ephemeral `&str` for `$src: impl fmt::Display`
///
/// If `$str` is `&str` or `String`, this macro simply passes on its content.
/// If it is neither, then the formatted data is collection into `&buffer`.
///
/// `return`s with an error if the formatting failed.
macro_rules! try_to_str {
    ($src:expr => $buffer:ident) => {
        match format_args!("{}", $src) {
            args => {
                if let Some(s) = args.as_str() {
                    s
                } else {
                    $buffer = String::new();
                    $buffer.write_fmt(args)?;
                    &$buffer
                }
            }
        }
    };
}

/// Formats arguments according to the specified format
///
/// The *second* argument to this filter must be a string literal (as in normal
/// Rust). The two arguments are passed through to the `format!()`
/// [macro](https://doc.rust-lang.org/stable/std/macro.format.html) by
/// the Askama code generator, but the order is swapped to support filter
/// composition.
///
/// ```ignore
/// {{ value|fmt("{:?}") }}
/// ```
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ value|fmt("{:?}") }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example {
///     value: (usize, usize),
/// }
///
/// assert_eq!(
///     Example { value: (3, 4) }.to_string(),
///     "<div>(3, 4)</div>"
/// );
/// # }
/// ```
///
/// Compare with [format](./fn.format.html).
pub fn fmt() {}

/// Formats arguments according to the specified format
///
/// The first argument to this filter must be a string literal (as in normal
/// Rust). All arguments are passed through to the `format!()`
/// [macro](https://doc.rust-lang.org/stable/std/macro.format.html) by
/// the Askama code generator.
///
/// ```ignore
/// {{ "{:?}{:?}"|format(value, other_value) }}
/// ```
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ "{:?}"|format(value) }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example {
///     value: (usize, usize),
/// }
///
/// assert_eq!(
///     Example { value: (3, 4) }.to_string(),
///     "<div>(3, 4)</div>"
/// );
/// # }
/// ```
///
/// Compare with [fmt](./fn.fmt.html).
pub fn format() {}

/// Replaces line breaks in plain text with appropriate HTML
///
/// A single newline becomes an HTML line break `<br>` and a new line
/// followed by a blank line becomes a paragraph break `<p>`.
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ example|linebreaks }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     example: &'a str,
/// }
///
/// assert_eq!(
///     Example { example: "Foo\nBar\n\nBaz" }.to_string(),
///     "<div><p>Foo<br/>Bar</p><p>Baz</p></div>"
/// );
/// # }
/// ```
#[inline]
pub fn linebreaks<S: fmt::Display>(source: S) -> Result<HtmlSafeOutput<Linebreaks<S>>, Infallible> {
    Ok(HtmlSafeOutput(Linebreaks(source)))
}

pub struct Linebreaks<S>(S);

impl<S: fmt::Display> fmt::Display for Linebreaks<S> {
    #[inline]
    fn fmt(&self, dest: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer;
        flush_linebreaks(dest, try_to_str!(self.0 => buffer))
    }
}

impl<S: FastWritable> FastWritable for Linebreaks<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        let mut buffer = String::new();
        self.0.write_into(&mut buffer, values)?;
        Ok(flush_linebreaks(dest, &buffer)?)
    }
}

fn flush_linebreaks(dest: &mut (impl fmt::Write + ?Sized), s: &str) -> fmt::Result {
    let linebroken = s.replace("\n\n", "</p><p>").replace('\n', "<br/>");
    write!(dest, "<p>{linebroken}</p>")
}

/// Converts all newlines in a piece of plain text to HTML line breaks
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ lines|linebreaksbr }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     lines: &'a str,
/// }
///
/// assert_eq!(
///     Example { lines: "a\nb\nc" }.to_string(),
///     "<div>a<br/>b<br/>c</div>"
/// );
/// # }
/// ```
#[inline]
pub fn linebreaksbr<S: fmt::Display>(
    source: S,
) -> Result<HtmlSafeOutput<Linebreaksbr<S>>, Infallible> {
    Ok(HtmlSafeOutput(Linebreaksbr(source)))
}

pub struct Linebreaksbr<S>(S);

impl<S: fmt::Display> fmt::Display for Linebreaksbr<S> {
    #[inline]
    fn fmt(&self, dest: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer;
        flush_linebreaksbr(dest, try_to_str!(self.0 => buffer))
    }
}

impl<S: FastWritable> FastWritable for Linebreaksbr<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        let mut buffer = String::new();
        self.0.write_into(&mut buffer, values)?;
        Ok(flush_linebreaksbr(dest, &buffer)?)
    }
}

fn flush_linebreaksbr(dest: &mut (impl fmt::Write + ?Sized), s: &str) -> fmt::Result {
    dest.write_str(&s.replace('\n', "<br/>"))
}

/// Replaces only paragraph breaks in plain text with appropriate HTML
///
/// A new line followed by a blank line becomes a paragraph break `<p>`.
/// Paragraph tags only wrap content; empty paragraphs are removed.
/// No `<br/>` tags are added.
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// {{ lines|paragraphbreaks }}
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     lines: &'a str,
/// }
///
/// assert_eq!(
///     Example { lines: "Foo\nBar\n\nBaz" }.to_string(),
///     "<p>Foo\nBar</p><p>Baz</p>"
/// );
/// # }
/// ```
#[inline]
pub fn paragraphbreaks<S: fmt::Display>(
    source: S,
) -> Result<HtmlSafeOutput<Paragraphbreaks<S>>, Infallible> {
    Ok(HtmlSafeOutput(Paragraphbreaks(source)))
}

pub struct Paragraphbreaks<S>(S);

impl<S: fmt::Display> fmt::Display for Paragraphbreaks<S> {
    #[inline]
    fn fmt(&self, dest: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer;
        flush_paragraphbreaks(dest, try_to_str!(self.0 => buffer))
    }
}

impl<S: FastWritable> FastWritable for Paragraphbreaks<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        let mut buffer = String::new();
        self.0.write_into(&mut buffer, values)?;
        Ok(flush_paragraphbreaks(dest, &buffer)?)
    }
}

fn flush_paragraphbreaks(dest: &mut (impl fmt::Write + ?Sized), s: &str) -> fmt::Result {
    let linebroken = s.replace("\n\n", "</p><p>").replace("<p></p>", "");
    write!(dest, "<p>{linebroken}</p>")
}

/// Converts to lowercase
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ word|lower }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     word: &'a str,
/// }
///
/// assert_eq!(
///     Example { word: "FOO" }.to_string(),
///     "<div>foo</div>"
/// );
///
/// assert_eq!(
///     Example { word: "FooBar" }.to_string(),
///     "<div>foobar</div>"
/// );
/// # }
/// ```
#[inline]
pub fn lower<S: fmt::Display>(source: S) -> Result<Lower<S>, Infallible> {
    Ok(Lower(source))
}

pub struct Lower<S>(S);

impl<S: fmt::Display> fmt::Display for Lower<S> {
    #[inline]
    fn fmt(&self, dest: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer;
        flush_lower(dest, try_to_str!(self.0 => buffer))
    }
}

impl<S: FastWritable> FastWritable for Lower<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        let mut buffer = String::new();
        self.0.write_into(&mut buffer, values)?;
        Ok(flush_lower(dest, &buffer)?)
    }
}

fn flush_lower(dest: &mut (impl fmt::Write + ?Sized), s: &str) -> fmt::Result {
    dest.write_str(&s.to_lowercase())
}

/// Converts to lowercase, alias for the `|lower` filter
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ word|lowercase }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     word: &'a str,
/// }
///
/// assert_eq!(
///     Example { word: "FOO" }.to_string(),
///     "<div>foo</div>"
/// );
///
/// assert_eq!(
///     Example { word: "FooBar" }.to_string(),
///     "<div>foobar</div>"
/// );
/// # }
/// ```
#[inline]
pub fn lowercase<S: fmt::Display>(source: S) -> Result<Lower<S>, Infallible> {
    lower(source)
}

/// Converts to uppercase
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ word|upper }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     word: &'a str,
/// }
///
/// assert_eq!(
///     Example { word: "foo" }.to_string(),
///     "<div>FOO</div>"
/// );
///
/// assert_eq!(
///     Example { word: "FooBar" }.to_string(),
///     "<div>FOOBAR</div>"
/// );
/// # }
/// ```
#[inline]
pub fn upper<S: fmt::Display>(source: S) -> Result<Upper<S>, Infallible> {
    Ok(Upper(source))
}

pub struct Upper<S>(S);

impl<S: fmt::Display> fmt::Display for Upper<S> {
    #[inline]
    fn fmt(&self, dest: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer;
        flush_upper(dest, try_to_str!(self.0 => buffer))
    }
}

impl<S: FastWritable> FastWritable for Upper<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        let mut buffer = String::new();
        self.0.write_into(&mut buffer, values)?;
        Ok(flush_upper(dest, &buffer)?)
    }
}

fn flush_upper(dest: &mut (impl fmt::Write + ?Sized), s: &str) -> fmt::Result {
    dest.write_str(&s.to_uppercase())
}

/// Converts to uppercase, alias for the `|upper` filter
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ word|uppercase }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     word: &'a str,
/// }
///
/// assert_eq!(
///     Example { word: "foo" }.to_string(),
///     "<div>FOO</div>"
/// );
///
/// assert_eq!(
///     Example { word: "FooBar" }.to_string(),
///     "<div>FOOBAR</div>"
/// );
/// # }
/// ```
#[inline]
pub fn uppercase<S: fmt::Display>(source: S) -> Result<Upper<S>, Infallible> {
    upper(source)
}

/// Strip leading and trailing whitespace
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ example|trim }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     example: &'a str,
/// }
///
/// assert_eq!(
///     Example { example: " Hello\tworld\t" }.to_string(),
///     "<div>Hello\tworld</div>"
/// );
/// # }
/// ```
#[inline]
pub fn trim<S: fmt::Display>(source: S) -> Result<Trim<S>, Infallible> {
    Ok(Trim(source))
}

pub struct Trim<S>(S);

impl<S: fmt::Display> fmt::Display for Trim<S> {
    #[inline]
    fn fmt(&self, dest: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut collector = TrimCollector(String::new());
        write!(collector, "{}", self.0)?;
        flush_trim(dest, collector)
    }
}

impl<S: FastWritable> FastWritable for Trim<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        let mut collector = TrimCollector(String::new());
        self.0.write_into(&mut collector, values)?;
        Ok(flush_trim(dest, collector)?)
    }
}

struct TrimCollector(String);

impl fmt::Write for TrimCollector {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        match self.0.is_empty() {
            true => self.0.write_str(s.trim_start()),
            false => self.0.write_str(s),
        }
    }
}

fn flush_trim(dest: &mut (impl fmt::Write + ?Sized), collector: TrimCollector) -> fmt::Result {
    dest.write_str(collector.0.trim_end())
}

/// Indent lines with spaces or a prefix.
///
/// The first line and blank lines are not indented by default.
/// The filter has two optional [`bool`] arguments, `first` and `blank`, that can be set to `true`
/// to indent the first and blank lines, resp.
///
/// ### Example of `indent` with spaces
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ example|indent(4) }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     example: &'a str,
/// }
///
/// assert_eq!(
///     Example { example: "hello\nfoo\nbar" }.to_string(),
///     "<div>hello\n    foo\n    bar</div>"
/// );
/// # }
/// ```
///
/// ### Example of `indent` with prefix a custom prefix
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ example|indent("$$$ ") }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     example: &'a str,
/// }
///
/// assert_eq!(
///     Example { example: "hello\nfoo\nbar" }.to_string(),
///     "<div>hello\n$$$ foo\n$$$ bar</div>"
/// );
/// # }
/// ```
#[inline]
pub fn indent<S, I: AsIndent>(
    source: S,
    indent: I,
    first: bool,
    blank: bool,
) -> Result<Indent<S, I>, Infallible> {
    Ok(Indent {
        source,
        indent,
        first,
        blank,
    })
}

pub struct Indent<S, I> {
    source: S,
    indent: I,
    first: bool,
    blank: bool,
}

impl<S: fmt::Display, I: AsIndent> fmt::Display for Indent<S, I> {
    fn fmt(&self, dest: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indent = self.indent.as_indent();
        if indent.len() >= MAX_LEN || indent.is_empty() {
            write!(dest, "{}", self.source)
        } else {
            let mut buffer;
            let buffer = try_to_str!(self.source => buffer);
            flush_indent(dest, indent, buffer, self.first, self.blank)
        }
    }
}

impl<S: FastWritable, I: AsIndent> FastWritable for Indent<S, I> {
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        let indent = self.indent.as_indent();
        if indent.len() >= MAX_LEN || indent.is_empty() {
            self.source.write_into(dest, values)
        } else {
            let mut buffer = String::new();
            self.source.write_into(&mut buffer, values)?;
            Ok(flush_indent(dest, indent, &buffer, self.first, self.blank)?)
        }
    }
}

fn flush_indent(
    dest: &mut (impl fmt::Write + ?Sized),
    indent: &str,
    s: &str,
    first: bool,
    blank: bool,
) -> fmt::Result {
    if s.len() >= MAX_LEN {
        return dest.write_str(s);
    }

    for (idx, line) in s.split_inclusive('\n').enumerate() {
        if (first || idx > 0) && (blank || !matches!(line, "\n" | "\r\n")) {
            dest.write_str(indent)?;
        }
        dest.write_str(line)?;
    }
    Ok(())
}

/// Capitalize a value. The first character will be uppercase, all others lowercase.
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ example|capitalize }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     example: &'a str,
/// }
///
/// assert_eq!(
///     Example { example: "hello" }.to_string(),
///     "<div>Hello</div>"
/// );
///
/// assert_eq!(
///     Example { example: "hElLO" }.to_string(),
///     "<div>Hello</div>"
/// );
/// # }
/// ```
#[inline]
pub fn capitalize<S: fmt::Display>(source: S) -> Result<Capitalize<S>, Infallible> {
    Ok(Capitalize(source))
}

pub struct Capitalize<S>(S);

impl<S: fmt::Display> fmt::Display for Capitalize<S> {
    #[inline]
    fn fmt(&self, dest: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer;
        flush_capitalize(dest, try_to_str!(self.0 => buffer))
    }
}

impl<S: FastWritable> FastWritable for Capitalize<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        let mut buffer = String::new();
        self.0.write_into(&mut buffer, values)?;
        Ok(flush_capitalize(dest, &buffer)?)
    }
}

fn flush_capitalize(dest: &mut (impl fmt::Write + ?Sized), s: &str) -> fmt::Result {
    let mut chars = s.chars();
    if let Some(c) = chars.next() {
        write!(
            dest,
            "{}{}",
            c.to_uppercase(),
            chars.as_str().to_lowercase()
        )
    } else {
        Ok(())
    }
}

/// Count the words in that string.
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ example|wordcount }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     example: &'a str,
/// }
///
/// assert_eq!(
///     Example { example: "askama is sort of cool" }.to_string(),
///     "<div>5</div>"
/// );
/// # }
/// ```
#[inline]
pub fn wordcount<S>(source: S) -> Wordcount<S> {
    Wordcount(Cell::new(Some(WordcountInner {
        source,
        buffer: String::new(),
    })))
}

pub struct Wordcount<S>(Cell<Option<WordcountInner<S>>>);

struct WordcountInner<S> {
    source: S,
    buffer: String,
}

impl<S: fmt::Display> fmt::Display for Wordcount<S> {
    #[inline]
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(mut inner) = self.0.take() {
            write!(inner.buffer, "{}", inner.source)?;
            self.0.set(Some(inner));
        }
        Ok(())
    }
}

impl<S: FastWritable> FastWritable for Wordcount<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        _: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        if let Some(mut inner) = self.0.take() {
            inner.source.write_into(&mut inner.buffer, values)?;
            self.0.set(Some(inner));
        }
        Ok(())
    }
}

impl<S> Wordcount<S> {
    pub fn into_count(self) -> usize {
        if let Some(inner) = self.0.into_inner() {
            inner.buffer.split_whitespace().count()
        } else {
            0
        }
    }
}

/// Return a title cased version of the value. Words will start with uppercase letters, all
/// remaining characters are lowercase.
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ example|title }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     example: &'a str,
/// }
///
/// assert_eq!(
///     Example { example: "hello WORLD" }.to_string(),
///     "<div>Hello World</div>"
/// );
/// # }
/// ```
#[inline]
pub fn title<S: fmt::Display>(source: S) -> Result<Title<S>, Infallible> {
    Ok(Title(source))
}

pub struct Title<S>(S);

impl<S: fmt::Display> fmt::Display for Title<S> {
    #[inline]
    fn fmt(&self, dest: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer;
        flush_title(dest, try_to_str!(self.0 => buffer))
    }
}

impl<S: FastWritable> FastWritable for Title<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()> {
        let mut buffer = String::new();
        self.0.write_into(&mut buffer, values)?;
        Ok(flush_title(dest, &buffer)?)
    }
}

fn flush_title(dest: &mut (impl fmt::Write + ?Sized), s: &str) -> fmt::Result {
    for word in s.split_inclusive(char::is_whitespace) {
        flush_capitalize(dest, word)?;
    }
    Ok(())
}

/// Return a title cased version of the value. Alias for the [`|title`](title) filter.
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{ example|titlecase }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     example: &'a str,
/// }
///
/// assert_eq!(
///     Example { example: "hello WORLD" }.to_string(),
///     "<div>Hello World</div>"
/// );
/// # }
/// ```
#[inline]
pub fn titlecase<S: fmt::Display>(source: S) -> Result<Title<S>, Infallible> {
    title(source)
}

/// A prefix usable for indenting [prettified JSON data](super::json::json_pretty) and
/// [`|indent`](indent)
///
/// ```
/// # use askama::filters::AsIndent;
/// assert_eq!(4.as_indent(), "    ");
/// assert_eq!(" -> ".as_indent(), " -> ");
/// ```
pub trait AsIndent {
    /// Borrow `self` as prefix to use.
    fn as_indent(&self) -> &str;
}

impl AsIndent for str {
    #[inline]
    fn as_indent(&self) -> &str {
        self
    }
}

#[cfg(feature = "alloc")]
impl AsIndent for alloc::string::String {
    #[inline]
    fn as_indent(&self) -> &str {
        self
    }
}

impl AsIndent for usize {
    #[inline]
    fn as_indent(&self) -> &str {
        spaces(*self)
    }
}

impl AsIndent for core::num::Wrapping<usize> {
    #[inline]
    fn as_indent(&self) -> &str {
        spaces(self.0)
    }
}

impl AsIndent for core::num::NonZeroUsize {
    #[inline]
    fn as_indent(&self) -> &str {
        spaces(self.get())
    }
}

fn spaces(width: usize) -> &'static str {
    const MAX_SPACES: usize = 16;
    const SPACES: &str = match str::from_utf8(&[b' '; MAX_SPACES]) {
        Ok(spaces) => spaces,
        Err(_) => panic!(),
    };

    &SPACES[..width.min(SPACES.len())]
}

#[cfg(feature = "alloc")]
impl<T: AsIndent + alloc::borrow::ToOwned + ?Sized> AsIndent for Cow<'_, T> {
    #[inline]
    fn as_indent(&self) -> &str {
        T::as_indent(self)
    }
}

crate::impl_for_ref! {
    impl AsIndent for T {
        #[inline]
        fn as_indent(&self) -> &str {
            <T>::as_indent(self)
        }
    }
}

impl<T> AsIndent for Pin<T>
where
    T: Deref,
    <T as Deref>::Target: AsIndent,
{
    #[inline]
    fn as_indent(&self) -> &str {
        self.as_ref().get_ref().as_indent()
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use std::borrow::ToOwned;

    use super::*;
    use crate::NO_VALUES;

    #[test]
    fn test_linebreaks() {
        assert_eq!(
            linebreaks("Foo\nBar Baz").unwrap().to_string(),
            "<p>Foo<br/>Bar Baz</p>"
        );
        assert_eq!(
            linebreaks("Foo\nBar\n\nBaz").unwrap().to_string(),
            "<p>Foo<br/>Bar</p><p>Baz</p>"
        );
    }

    #[test]
    fn test_linebreaksbr() {
        assert_eq!(linebreaksbr("Foo\nBar").unwrap().to_string(), "Foo<br/>Bar");
        assert_eq!(
            linebreaksbr("Foo\nBar\n\nBaz").unwrap().to_string(),
            "Foo<br/>Bar<br/><br/>Baz"
        );
    }

    #[test]
    fn test_paragraphbreaks() {
        assert_eq!(
            paragraphbreaks("Foo\nBar Baz").unwrap().to_string(),
            "<p>Foo\nBar Baz</p>"
        );
        assert_eq!(
            paragraphbreaks("Foo\nBar\n\nBaz").unwrap().to_string(),
            "<p>Foo\nBar</p><p>Baz</p>"
        );
        assert_eq!(
            paragraphbreaks("Foo\n\n\n\n\nBar\n\nBaz")
                .unwrap()
                .to_string(),
            "<p>Foo</p><p>\nBar</p><p>Baz</p>"
        );
    }

    #[test]
    fn test_lower() {
        assert_eq!(lower("Foo").unwrap().to_string(), "foo");
        assert_eq!(lower("FOO").unwrap().to_string(), "foo");
        assert_eq!(lower("FooBar").unwrap().to_string(), "foobar");
        assert_eq!(lower("foo").unwrap().to_string(), "foo");
    }

    #[test]
    fn test_upper() {
        assert_eq!(upper("Foo").unwrap().to_string(), "FOO");
        assert_eq!(upper("FOO").unwrap().to_string(), "FOO");
        assert_eq!(upper("FooBar").unwrap().to_string(), "FOOBAR");
        assert_eq!(upper("foo").unwrap().to_string(), "FOO");
    }

    #[test]
    fn test_trim() {
        assert_eq!(trim(" Hello\tworld\t").unwrap().to_string(), "Hello\tworld");
    }

    #[test]
    fn test_indent() {
        assert_eq!(
            indent("hello", 2, false, false).unwrap().to_string(),
            "hello"
        );
        assert_eq!(
            indent("hello\n", 2, false, false).unwrap().to_string(),
            "hello\n"
        );
        assert_eq!(
            indent("hello\nfoo", 2, false, false).unwrap().to_string(),
            "hello\n  foo"
        );
        assert_eq!(
            indent("hello\nfoo\n bar", 4, false, false)
                .unwrap()
                .to_string(),
            "hello\n    foo\n     bar"
        );
        assert_eq!(
            indent("hello", 267_332_238_858, false, false)
                .unwrap()
                .to_string(),
            "hello"
        );

        assert_eq!(
            indent("hello\n\n bar", 4, false, false)
                .unwrap()
                .to_string(),
            "hello\n\n     bar"
        );
        assert_eq!(
            indent("hello\n\n bar", 4, false, true).unwrap().to_string(),
            "hello\n    \n     bar"
        );
        assert_eq!(
            indent("hello\n\n bar", 4, true, false).unwrap().to_string(),
            "    hello\n\n     bar"
        );
        assert_eq!(
            indent("hello\n\n bar", 4, true, true).unwrap().to_string(),
            "    hello\n    \n     bar"
        );
    }

    #[test]
    fn test_indent_str() {
        assert_eq!(
            indent("hello\n\n bar", "❗❓", false, false)
                .unwrap()
                .to_string(),
            "hello\n\n❗❓ bar"
        );
        assert_eq!(
            indent("hello\n\n bar", "❗❓", false, true)
                .unwrap()
                .to_string(),
            "hello\n❗❓\n❗❓ bar"
        );
        assert_eq!(
            indent("hello\n\n bar", "❗❓", true, false)
                .unwrap()
                .to_string(),
            "❗❓hello\n\n❗❓ bar"
        );
        assert_eq!(
            indent("hello\n\n bar", "❗❓", true, true)
                .unwrap()
                .to_string(),
            "❗❓hello\n❗❓\n❗❓ bar"
        );
    }

    #[test]
    #[allow(clippy::arc_with_non_send_sync)] // it's only a test, it does not have to make sense
    #[allow(clippy::type_complexity)] // it's only a test, it does not have to be pretty
    fn test_indent_complicated() {
        use std::boxed::Box;
        use std::cell::{RefCell, RefMut};
        use std::rc::Rc;
        use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockWriteGuard};

        let prefix = Mutex::new(Box::pin("❗❓".to_owned()));
        let prefix = RefCell::new(Arc::new(prefix.try_lock().unwrap()));
        let prefix = RwLock::new(Rc::new(prefix.borrow_mut()));
        let prefix: RwLockWriteGuard<'_, Rc<RefMut<'_, Arc<MutexGuard<'_, Pin<Box<String>>>>>>> =
            prefix.try_write().unwrap();

        assert_eq!(
            indent("hello\n\n bar", &prefix, false, false)
                .unwrap()
                .to_string(),
            "hello\n\n❗❓ bar"
        );
        assert_eq!(
            indent("hello\n\n bar", &prefix, false, true)
                .unwrap()
                .to_string(),
            "hello\n❗❓\n❗❓ bar"
        );
        assert_eq!(
            indent("hello\n\n bar", &prefix, true, false)
                .unwrap()
                .to_string(),
            "❗❓hello\n\n❗❓ bar"
        );
        assert_eq!(
            indent("hello\n\n bar", &prefix, true, true)
                .unwrap()
                .to_string(),
            "❗❓hello\n❗❓\n❗❓ bar"
        );
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("foo").unwrap().to_string(), "Foo".to_string());
        assert_eq!(capitalize("f").unwrap().to_string(), "F".to_string());
        assert_eq!(capitalize("fO").unwrap().to_string(), "Fo".to_string());
        assert_eq!(capitalize("").unwrap().to_string(), String::new());
        assert_eq!(capitalize("FoO").unwrap().to_string(), "Foo".to_string());
        assert_eq!(
            capitalize("foO BAR").unwrap().to_string(),
            "Foo bar".to_string()
        );
        assert_eq!(
            capitalize("äØÄÅÖ").unwrap().to_string(),
            "Äøäåö".to_string()
        );
        assert_eq!(capitalize("ß").unwrap().to_string(), "SS".to_string());
        assert_eq!(capitalize("ßß").unwrap().to_string(), "SSß".to_string());
    }

    #[test]
    fn test_wordcount() {
        for &(word, count) in &[
            ("", 0),
            (" \n\t", 0),
            ("foo", 1),
            ("foo bar", 2),
            ("foo  bar", 2),
        ] {
            let w = wordcount(word);
            let _ = w.to_string();
            assert_eq!(w.into_count(), count, "fmt: {word:?}");

            let w = wordcount(word);
            w.write_into(&mut String::new(), NO_VALUES).unwrap();
            assert_eq!(w.into_count(), count, "FastWritable: {word:?}");
        }
    }

    #[test]
    fn test_title() {
        assert_eq!(&title("").unwrap().to_string(), "");
        assert_eq!(&title(" \n\t").unwrap().to_string(), " \n\t");
        assert_eq!(&title("foo").unwrap().to_string(), "Foo");
        assert_eq!(&title(" foo").unwrap().to_string(), " Foo");
        assert_eq!(&title("foo bar").unwrap().to_string(), "Foo Bar");
        assert_eq!(&title("foo  bar ").unwrap().to_string(), "Foo  Bar ");
        assert_eq!(&title("fOO").unwrap().to_string(), "Foo");
        assert_eq!(&title("fOo BaR").unwrap().to_string(), "Foo Bar");
        assert_eq!(&title("foo\r\nbar").unwrap().to_string(), "Foo\r\nBar");
        assert_eq!(
            &title("Fo\x0boo\x0coO\u{2002}OO\u{3000}baR")
                .unwrap()
                .to_string(),
            "Fo\x0bOo\x0cOo\u{2002}Oo\u{3000}Bar"
        );
    }

    #[test]
    fn fuzzed_indent_filter() {
        let s = "hello\nfoo\nbar".to_string().repeat(1024);
        assert_eq!(indent(s.clone(), 4, false, false).unwrap().to_string(), s);
    }
}
