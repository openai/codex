//! This is an implementation of [`Reader`] for reading from a `&[u8]` as
//! underlying byte stream. This implementation supports not using an
//! intermediate buffer as the byte slice itself can be used to borrow from.

use std::borrow::Cow;
use std::io;

#[cfg(feature = "encoding")]
use crate::reader::EncodingRef;
#[cfg(feature = "encoding")]
use encoding_rs::{Encoding, UTF_8};

use crate::errors::{Error, Result};
use crate::events::Event;
use crate::name::QName;
use crate::parser::Parser;
use crate::reader::{BangType, ReadTextResult, Reader, Span, XmlSource};
use crate::utils::is_whitespace;

/// This is an implementation for reading from a `&[u8]` as underlying byte stream.
/// This implementation supports not using an intermediate buffer as the byte slice
/// itself can be used to borrow from.
impl<'a> Reader<&'a [u8]> {
    /// Creates an XML reader from a string slice.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &'a str) -> Self {
        // Rust strings are guaranteed to be UTF-8, so lock the encoding
        #[cfg(feature = "encoding")]
        {
            let mut reader = Self::from_reader(s.as_bytes());
            reader.state.encoding = EncodingRef::Explicit(UTF_8);
            reader
        }

        #[cfg(not(feature = "encoding"))]
        Self::from_reader(s.as_bytes())
    }

    /// Read an event that borrows from the input rather than a buffer.
    ///
    /// There is no asynchronous `read_event_async()` version of this function,
    /// because it is not necessary -- the contents are already in memory and no IO
    /// is needed, therefore there is no potential for blocking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::Event;
    /// use quick_xml::reader::Reader;
    ///
    /// let mut reader = Reader::from_str(r#"
    ///     <tag1 att1 = "test">
    ///        <tag2><!--Test comment-->Test</tag2>
    ///        <tag2>Test 2</tag2>
    ///     </tag1>
    /// "#);
    /// reader.config_mut().trim_text(true);
    ///
    /// let mut count = 0;
    /// let mut txt = Vec::new();
    /// loop {
    ///     match reader.read_event().unwrap() {
    ///         Event::Start(e) => count += 1,
    ///         Event::Text(e) => txt.push(e.unescape().unwrap().into_owned()),
    ///         Event::Eof => break,
    ///         _ => (),
    ///     }
    /// }
    /// assert_eq!(count, 3);
    /// assert_eq!(txt, vec!["Test".to_string(), "Test 2".to_string()]);
    /// ```
    #[inline]
    pub fn read_event(&mut self) -> Result<Event<'a>> {
        self.read_event_impl(())
    }

    /// Reads until end element is found. This function is supposed to be called
    /// after you already read a [`Start`] event.
    ///
    /// Returns a span that cover content between `>` of an opening tag and `<` of
    /// a closing tag or an empty slice, if [`expand_empty_elements`] is set and
    /// this method was called after reading expanded [`Start`] event.
    ///
    /// Manages nested cases where parent and child elements have the _literally_
    /// same name.
    ///
    /// If a corresponding [`End`] event is not found, an error of type [`Error::IllFormed`]
    /// will be returned. In particularly, that error will be returned if you call
    /// this method without consuming the corresponding [`Start`] event first.
    ///
    /// The `end` parameter should contain name of the end element _in the reader
    /// encoding_. It is good practice to always get that parameter using
    /// [`BytesStart::to_end()`] method.
    ///
    /// The correctness of the skipped events does not checked, if you disabled
    /// the [`check_end_names`] option.
    ///
    /// There is no asynchronous `read_to_end_async()` version of this function,
    /// because it is not necessary -- the contents are already in memory and no IO
    /// is needed, therefore there is no potential for blocking.
    ///
    /// # Namespaces
    ///
    /// While the `Reader` does not support namespace resolution, namespaces
    /// does not change the algorithm for comparing names. Although the names
    /// `a:name` and `b:name` where both prefixes `a` and `b` resolves to the
    /// same namespace, are semantically equivalent, `</b:name>` cannot close
    /// `<a:name>`, because according to [the specification]
    ///
    /// > The end of every element that begins with a **start-tag** MUST be marked
    /// > by an **end-tag** containing a name that echoes the element's type as
    /// > given in the **start-tag**
    ///
    /// # Examples
    ///
    /// This example shows, how you can skip XML content after you read the
    /// start event.
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::{BytesStart, Event};
    /// use quick_xml::reader::Reader;
    ///
    /// let mut reader = Reader::from_str(r#"
    ///     <outer>
    ///         <inner>
    ///             <inner></inner>
    ///             <inner/>
    ///             <outer></outer>
    ///             <outer/>
    ///         </inner>
    ///     </outer>
    /// "#);
    /// reader.config_mut().trim_text(true);
    ///
    /// let start = BytesStart::new("outer");
    /// let end   = start.to_end().into_owned();
    ///
    /// // First, we read a start event...
    /// assert_eq!(reader.read_event().unwrap(), Event::Start(start));
    ///
    /// // ...then, we could skip all events to the corresponding end event.
    /// // This call will correctly handle nested <outer> elements.
    /// // Note, however, that this method does not handle namespaces.
    /// reader.read_to_end(end.name()).unwrap();
    ///
    /// // At the end we should get an Eof event, because we ate the whole XML
    /// assert_eq!(reader.read_event().unwrap(), Event::Eof);
    /// ```
    ///
    /// [`Start`]: Event::Start
    /// [`End`]: Event::End
    /// [`BytesStart::to_end()`]: crate::events::BytesStart::to_end
    /// [`expand_empty_elements`]: crate::reader::Config::expand_empty_elements
    /// [`check_end_names`]: crate::reader::Config::check_end_names
    /// [the specification]: https://www.w3.org/TR/xml11/#dt-etag
    pub fn read_to_end(&mut self, end: QName) -> Result<Span> {
        Ok(read_to_end!(self, end, (), read_event_impl, {}))
    }

    /// Reads content between start and end tags, including any markup. This
    /// function is supposed to be called after you already read a [`Start`] event.
    ///
    /// Manages nested cases where parent and child elements have the _literally_
    /// same name.
    ///
    /// This method does not unescape read data, instead it returns content
    /// "as is" of the XML document. This is because it has no idea what text
    /// it reads, and if, for example, it contains CDATA section, attempt to
    /// unescape it content will spoil data.
    ///
    /// Any text will be decoded using the XML current [`decoder()`].
    ///
    /// Actually, this method perform the following code:
    ///
    /// ```ignore
    /// let span = reader.read_to_end(end)?;
    /// let text = reader.decoder().decode(&reader.inner_slice[span]);
    /// ```
    ///
    /// # Examples
    ///
    /// This example shows, how you can read a HTML content from your XML document.
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// # use std::borrow::Cow;
    /// use quick_xml::events::{BytesStart, Event};
    /// use quick_xml::reader::Reader;
    ///
    /// let mut reader = Reader::from_str("
    ///     <html>
    ///         <title>This is a HTML text</title>
    ///         <p>Usual XML rules does not apply inside it
    ///         <p>For example, elements not needed to be &quot;closed&quot;
    ///     </html>
    /// ");
    /// reader.config_mut().trim_text(true);
    ///
    /// let start = BytesStart::new("html");
    /// let end   = start.to_end().into_owned();
    ///
    /// // First, we read a start event...
    /// assert_eq!(reader.read_event().unwrap(), Event::Start(start));
    /// // ...and disable checking of end names because we expect HTML further...
    /// reader.config_mut().check_end_names = false;
    ///
    /// // ...then, we could read text content until close tag.
    /// // This call will correctly handle nested <html> elements.
    /// let text = reader.read_text(end.name()).unwrap();
    /// assert_eq!(text, Cow::Borrowed(r#"
    ///         <title>This is a HTML text</title>
    ///         <p>Usual XML rules does not apply inside it
    ///         <p>For example, elements not needed to be &quot;closed&quot;
    ///     "#));
    /// assert!(matches!(text, Cow::Borrowed(_)));
    ///
    /// // Now we can enable checks again
    /// reader.config_mut().check_end_names = true;
    ///
    /// // At the end we should get an Eof event, because we ate the whole XML
    /// assert_eq!(reader.read_event().unwrap(), Event::Eof);
    /// ```
    ///
    /// [`Start`]: Event::Start
    /// [`decoder()`]: Self::decoder()
    pub fn read_text(&mut self, end: QName) -> Result<Cow<'a, str>> {
        // self.reader will be changed, so store original reference
        let buffer = self.reader;
        let span = self.read_to_end(end)?;

        let len = span.end - span.start;
        // SAFETY: `span` can only contain indexes up to usize::MAX because it
        // was created from offsets from a single &[u8] slice
        Ok(self.decoder().decode(&buffer[0..len as usize])?)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Implementation of `XmlSource` for `&[u8]` reader using a `Self` as buffer
/// that will be borrowed by events. This implementation provides a zero-copy deserialization
impl<'a> XmlSource<'a, ()> for &'a [u8] {
    #[cfg(not(feature = "encoding"))]
    #[inline]
    fn remove_utf8_bom(&mut self) -> io::Result<()> {
        if self.starts_with(crate::encoding::UTF8_BOM) {
            *self = &self[crate::encoding::UTF8_BOM.len()..];
        }
        Ok(())
    }

    #[cfg(feature = "encoding")]
    #[inline]
    fn detect_encoding(&mut self) -> io::Result<Option<&'static Encoding>> {
        if let Some((enc, bom_len)) = crate::encoding::detect_encoding(self) {
            *self = &self[bom_len..];
            return Ok(Some(enc));
        }
        Ok(None)
    }

    #[inline]
    fn read_text(&mut self, _buf: (), position: &mut u64) -> ReadTextResult<'a, ()> {
        match memchr::memchr(b'<', self) {
            Some(0) => {
                *position += 1;
                *self = &self[1..];
                ReadTextResult::Markup(())
            }
            Some(i) => {
                *position += i as u64 + 1;
                let bytes = &self[..i];
                *self = &self[i + 1..];
                ReadTextResult::UpToMarkup(bytes)
            }
            None => {
                *position += self.len() as u64;
                let bytes = &self[..];
                *self = &[];
                ReadTextResult::UpToEof(bytes)
            }
        }
    }

    #[inline]
    fn read_with<P>(&mut self, mut parser: P, _buf: (), position: &mut u64) -> Result<&'a [u8]>
    where
        P: Parser,
    {
        if let Some(i) = parser.feed(self) {
            // +1 for `>` which we do not include
            *position += i as u64 + 1;
            let bytes = &self[..i];
            *self = &self[i + 1..];
            return Ok(bytes);
        }

        *position += self.len() as u64;
        Err(Error::Syntax(P::eof_error()))
    }

    #[inline]
    fn read_bang_element(&mut self, _buf: (), position: &mut u64) -> Result<(BangType, &'a [u8])> {
        // Peeked one bang ('!') before being called, so it's guaranteed to
        // start with it.
        debug_assert_eq!(self[0], b'!');

        let mut bang_type = BangType::new(self[1..].first().copied())?;

        if let Some((bytes, i)) = bang_type.parse(&[], self) {
            *position += i as u64;
            *self = &self[i..];
            return Ok((bang_type, bytes));
        }

        *position += self.len() as u64;
        Err(bang_type.to_err().into())
    }

    #[inline]
    fn skip_whitespace(&mut self, position: &mut u64) -> io::Result<()> {
        let whitespaces = self
            .iter()
            .position(|b| !is_whitespace(*b))
            .unwrap_or(self.len());
        *position += whitespaces as u64;
        *self = &self[whitespaces..];
        Ok(())
    }

    #[inline]
    fn peek_one(&mut self) -> io::Result<Option<u8>> {
        Ok(self.first().copied())
    }
}

#[cfg(test)]
mod test {
    use crate::reader::test::check;
    use crate::reader::XmlSource;

    /// Default buffer constructor just pass the byte array from the test
    fn identity<T>(input: T) -> T {
        input
    }

    check!(
        #[test]
        read_event_impl,
        read_until_close,
        identity,
        ()
    );
}
