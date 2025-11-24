//! Contains high-level interface for an events-based XML emitter.

use std::borrow::Cow;
use std::io::{self, Write};

use crate::encoding::UTF8_BOM;
use crate::events::{attributes::Attribute, BytesCData, BytesPI, BytesStart, BytesText, Event};

#[cfg(feature = "async-tokio")]
mod async_tokio;

/// XML writer. Writes XML [`Event`]s to a [`std::io::Write`] or [`tokio::io::AsyncWrite`] implementor.
#[cfg(feature = "serialize")]
use {crate::se::SeError, serde::Serialize};

/// XML writer. Writes XML [`Event`]s to a [`std::io::Write`] implementor.
///
/// # Examples
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use quick_xml::events::{Event, BytesEnd, BytesStart};
/// use quick_xml::reader::Reader;
/// use quick_xml::writer::Writer;
/// use std::io::Cursor;
///
/// let xml = r#"<this_tag k1="v1" k2="v2"><child>text</child></this_tag>"#;
/// let mut reader = Reader::from_str(xml);
/// let mut writer = Writer::new(Cursor::new(Vec::new()));
/// loop {
///     match reader.read_event() {
///         Ok(Event::Start(e)) if e.name().as_ref() == b"this_tag" => {
///
///             // crates a new element ... alternatively we could reuse `e` by calling
///             // `e.into_owned()`
///             let mut elem = BytesStart::new("my_elem");
///
///             // collect existing attributes
///             elem.extend_attributes(e.attributes().map(|attr| attr.unwrap()));
///
///             // copy existing attributes, adds a new my-key="some value" attribute
///             elem.push_attribute(("my-key", "some value"));
///
///             // writes the event to the writer
///             assert!(writer.write_event(Event::Start(elem)).is_ok());
///         },
///         Ok(Event::End(e)) if e.name().as_ref() == b"this_tag" => {
///             assert!(writer.write_event(Event::End(BytesEnd::new("my_elem"))).is_ok());
///         },
///         Ok(Event::Eof) => break,
///         // we can either move or borrow the event to write, depending on your use-case
///         Ok(e) => assert!(writer.write_event(e.borrow()).is_ok()),
///         Err(e) => panic!("Error at position {}: {:?}", reader.error_position(), e),
///     }
/// }
///
/// let result = writer.into_inner().into_inner();
/// let expected = r#"<my_elem k1="v1" k2="v2" my-key="some value"><child>text</child></my_elem>"#;
/// assert_eq!(result, expected.as_bytes());
/// ```
#[derive(Clone)]
pub struct Writer<W> {
    /// underlying writer
    writer: W,
    indent: Option<Indentation>,
}

impl<W> Writer<W> {
    /// Creates a `Writer` from a generic writer.
    pub const fn new(inner: W) -> Writer<W> {
        Writer {
            writer: inner,
            indent: None,
        }
    }

    /// Creates a `Writer` with configured indents from a generic writer.
    pub fn new_with_indent(inner: W, indent_char: u8, indent_size: usize) -> Writer<W> {
        Writer {
            writer: inner,
            indent: Some(Indentation::new(indent_char, indent_size)),
        }
    }

    /// Consumes this `Writer`, returning the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Get a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Get a reference to the underlying writer.
    pub const fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Provides a simple, high-level API for writing XML elements.
    ///
    /// Returns an [`ElementWriter`] that simplifies setting attributes and writing
    /// content inside the element.
    ///
    /// # Example
    ///
    /// ```
    /// # use quick_xml::Result;
    /// # fn main() -> Result<()> {
    /// use quick_xml::events::{BytesStart, BytesText, Event};
    /// use quick_xml::writer::Writer;
    /// use quick_xml::Error;
    /// use std::io::Cursor;
    ///
    /// let mut writer = Writer::new(Cursor::new(Vec::new()));
    ///
    /// // writes <tag attr1="value1"/>
    /// writer.create_element("tag")
    ///     .with_attribute(("attr1", "value1"))  // chain `with_attribute()` calls to add many attributes
    ///     .write_empty()?;
    ///
    /// // writes <tag attr1="value1" attr2="value2">with some text inside</tag>
    /// writer.create_element("tag")
    ///     .with_attributes(vec![("attr1", "value1"), ("attr2", "value2")].into_iter())  // or add attributes from an iterator
    ///     .write_text_content(BytesText::new("with some text inside"))?;
    ///
    /// // writes <tag><fruit quantity="0">apple</fruit><fruit quantity="1">orange</fruit></tag>
    /// writer.create_element("tag")
    ///     // We need to provide error type, because it is not named somewhere explicitly
    ///     .write_inner_content(|writer| {
    ///         let fruits = ["apple", "orange"];
    ///         for (quant, item) in fruits.iter().enumerate() {
    ///             writer
    ///                 .create_element("fruit")
    ///                 .with_attribute(("quantity", quant.to_string().as_str()))
    ///                 .write_text_content(BytesText::new(item))?;
    ///         }
    ///         Ok(())
    ///     })?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn create_element<'a, N>(&'a mut self, name: N) -> ElementWriter<'a, W>
    where
        N: Into<Cow<'a, str>>,
    {
        ElementWriter {
            writer: self,
            start_tag: BytesStart::new(name),
            state: AttributeIndent::NoneAttributesWritten,
            spaces: Vec::new(),
        }
    }
}

impl<W: Write> Writer<W> {
    /// Write a [Byte-Order-Mark] character to the document.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use quick_xml::Result;
    /// # fn main() -> Result<()> {
    /// use quick_xml::events::{BytesStart, BytesText, Event};
    /// use quick_xml::writer::Writer;
    /// use quick_xml::Error;
    /// use std::io::Cursor;
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Writer::new_with_indent(&mut buffer, b' ', 4);
    ///
    /// writer.write_bom()?;
    /// writer
    ///     .create_element("empty")
    ///     .with_attribute(("attr1", "value1"))
    ///     .write_empty()
    ///     .expect("failure");
    ///
    /// assert_eq!(
    ///     std::str::from_utf8(&buffer).unwrap(),
    ///     "\u{FEFF}<empty attr1=\"value1\"/>"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    /// [Byte-Order-Mark]: https://unicode.org/faq/utf_bom.html#BOM
    pub fn write_bom(&mut self) -> io::Result<()> {
        self.write(UTF8_BOM)
    }

    /// Writes the given event to the underlying writer.
    pub fn write_event<'a, E: Into<Event<'a>>>(&mut self, event: E) -> io::Result<()> {
        let mut next_should_line_break = true;
        let result = match event.into() {
            Event::Start(e) => {
                let result = self.write_wrapped(b"<", &e, b">");
                if let Some(i) = self.indent.as_mut() {
                    i.grow();
                }
                result
            }
            Event::End(e) => {
                if let Some(i) = self.indent.as_mut() {
                    i.shrink();
                }
                self.write_wrapped(b"</", &e, b">")
            }
            Event::Empty(e) => self.write_wrapped(b"<", &e, b"/>"),
            Event::Text(e) => {
                next_should_line_break = false;
                self.write(&e)
            }
            Event::Comment(e) => self.write_wrapped(b"<!--", &e, b"-->"),
            Event::CData(e) => {
                next_should_line_break = false;
                self.write(b"<![CDATA[")?;
                self.write(&e)?;
                self.write(b"]]>")
            }
            Event::Decl(e) => self.write_wrapped(b"<?", &e, b"?>"),
            Event::PI(e) => self.write_wrapped(b"<?", &e, b"?>"),
            Event::DocType(e) => self.write_wrapped(b"<!DOCTYPE ", &e, b">"),
            Event::Eof => Ok(()),
        };
        if let Some(i) = self.indent.as_mut() {
            i.should_line_break = next_should_line_break;
        }
        result
    }

    /// Writes bytes
    #[inline]
    pub(crate) fn write(&mut self, value: &[u8]) -> io::Result<()> {
        self.writer.write_all(value).map_err(Into::into)
    }

    #[inline]
    fn write_wrapped(&mut self, before: &[u8], value: &[u8], after: &[u8]) -> io::Result<()> {
        if let Some(ref i) = self.indent {
            if i.should_line_break {
                self.writer.write_all(b"\n")?;
                self.writer.write_all(i.current())?;
            }
        }
        self.write(before)?;
        self.write(value)?;
        self.write(after)?;
        Ok(())
    }

    /// Manually write a newline and indentation at the proper level.
    ///
    /// This can be used when the heuristic to line break and indent after any
    /// [`Event`] apart from [`Text`] fails such as when a [`Start`] occurs directly
    /// after [`Text`].
    ///
    /// This method will do nothing if `Writer` was not constructed with [`new_with_indent`].
    ///
    /// [`Text`]: Event::Text
    /// [`Start`]: Event::Start
    /// [`new_with_indent`]: Self::new_with_indent
    pub fn write_indent(&mut self) -> io::Result<()> {
        if let Some(ref i) = self.indent {
            self.writer.write_all(b"\n")?;
            self.writer.write_all(i.current())?;
        }
        Ok(())
    }

    /// Write an arbitrary serializable type
    ///
    /// Note: If you are attempting to write XML in a non-UTF-8 encoding, this may not
    /// be safe to use. Rust basic types assume UTF-8 encodings.
    ///
    /// ```rust
    /// # use pretty_assertions::assert_eq;
    /// # use serde::Serialize;
    /// # use quick_xml::events::{BytesStart, Event};
    /// # use quick_xml::writer::Writer;
    /// # use quick_xml::se::SeError;
    /// # fn main() -> Result<(), SeError> {
    /// #[derive(Debug, PartialEq, Serialize)]
    /// struct MyData {
    ///     question: String,
    ///     answer: u32,
    /// }
    ///
    /// let data = MyData {
    ///     question: "The Ultimate Question of Life, the Universe, and Everything".into(),
    ///     answer: 42,
    /// };
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Writer::new_with_indent(&mut buffer, b' ', 4);
    ///
    /// let start = BytesStart::new("root");
    /// let end = start.to_end();
    ///
    /// writer.write_event(Event::Start(start.clone()))?;
    /// writer.write_serializable("my_data", &data)?;
    /// writer.write_event(Event::End(end))?;
    ///
    /// assert_eq!(
    ///     std::str::from_utf8(&buffer)?,
    ///     r#"<root>
    ///     <my_data>
    ///         <question>The Ultimate Question of Life, the Universe, and Everything</question>
    ///         <answer>42</answer>
    ///     </my_data>
    /// </root>"#
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "serialize")]
    pub fn write_serializable<T: Serialize>(
        &mut self,
        tag_name: &str,
        content: &T,
    ) -> Result<(), SeError> {
        use crate::se::{Indent, Serializer};

        self.write_indent()?;
        let mut fmt = ToFmtWrite(&mut self.writer);
        let mut serializer = Serializer::with_root(&mut fmt, Some(tag_name))?;

        if let Some(indent) = &mut self.indent {
            serializer.set_indent(Indent::Borrow(indent));
        }

        content.serialize(serializer)?;

        Ok(())
    }
}

/// Track indent inside elements state
///
/// ```mermaid
/// stateDiagram-v2
///     [*] --> NoneAttributesWritten
///     NoneAttributesWritten --> Spaces : .with_attribute()
///     NoneAttributesWritten --> WriteConfigured : .new_line()
///
///     Spaces --> Spaces : .with_attribute()
///     Spaces --> WriteSpaces : .new_line()
///
///     WriteSpaces --> Spaces : .with_attribute()
///     WriteSpaces --> WriteSpaces : .new_line()
///
///     Configured --> Configured : .with_attribute()
///     Configured --> WriteConfigured : .new_line()
///
///     WriteConfigured --> Configured : .with_attribute()
///     WriteConfigured --> WriteConfigured : .new_line()
/// ```
#[derive(Debug)]
enum AttributeIndent {
    /// Initial state. `ElementWriter` was just created and no attributes written yet
    NoneAttributesWritten,
    /// Write specified count of spaces to indent before writing attribute in `with_attribute()`
    WriteSpaces(usize),
    /// Keep space indent that should be used if `new_line()` would be called
    Spaces(usize),
    /// Write specified count of indent characters before writing attribute in `with_attribute()`
    WriteConfigured(usize),
    /// Keep indent that should be used if `new_line()` would be called
    Configured(usize),
}

/// A struct to write an element. Contains methods to add attributes and inner
/// elements to the element
pub struct ElementWriter<'a, W> {
    writer: &'a mut Writer<W>,
    start_tag: BytesStart<'a>,
    state: AttributeIndent,
    /// Contains spaces used to write space indents of attributes
    spaces: Vec<u8>,
}

impl<'a, W> ElementWriter<'a, W> {
    /// Adds an attribute to this element.
    pub fn with_attribute<'b, I>(mut self, attr: I) -> Self
    where
        I: Into<Attribute<'b>>,
    {
        self.write_attr(attr.into());
        self
    }

    /// Add additional attributes to this element using an iterator.
    ///
    /// The yielded items must be convertible to [`Attribute`] using `Into`.
    pub fn with_attributes<'b, I>(mut self, attributes: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Attribute<'b>>,
    {
        let mut iter = attributes.into_iter();
        if let Some(attr) = iter.next() {
            self.write_attr(attr.into());
            self.start_tag.extend_attributes(iter);
        }
        self
    }

    /// Push a new line inside an element between attributes. Note, that this
    /// method does nothing if [`Writer`] was created without indentation support.
    ///
    /// # Examples
    ///
    /// The following code
    ///
    /// ```
    /// # use quick_xml::writer::Writer;
    /// let mut buffer = Vec::new();
    /// let mut writer = Writer::new_with_indent(&mut buffer, b' ', 2);
    /// writer
    ///   .create_element("element")
    ///     //.new_line() (1)
    ///     .with_attribute(("first", "1"))
    ///     .with_attribute(("second", "2"))
    ///     .new_line()
    ///     .with_attributes([
    ///         ("third", "3"),
    ///         ("fourth", "4"),
    ///     ])
    ///     //.new_line() (2)
    ///     .write_empty();
    /// ```
    /// will produce the following XMLs:
    /// ```xml
    /// <!-- result of the code above. Spaces always is used -->
    /// <element first="1" second="2"
    ///          third="3" fourth="4"/>
    ///
    /// <!-- if uncomment only (1) - indent depends on indentation
    ///      settings - 2 spaces here -->
    /// <element
    ///   first="1" second="2"
    ///   third="3" fourth="4"/>
    ///
    /// <!-- if uncomment only (2). Spaces always is used  -->
    /// <element first="1" second="2"
    ///          third="3" fourth="4"
    /// />
    /// ```
    pub fn new_line(mut self) -> Self {
        if let Some(i) = self.writer.indent.as_mut() {
            match self.state {
                // .new_line() called just after .create_element().
                // Use element indent to additionally indent attributes
                AttributeIndent::NoneAttributesWritten => {
                    self.state = AttributeIndent::WriteConfigured(i.indent_size)
                }

                AttributeIndent::WriteSpaces(_) => {}
                // .new_line() called when .with_attribute() was called at least once.
                // The spaces should be used to indent
                // Plan saved indent
                AttributeIndent::Spaces(indent) => {
                    self.state = AttributeIndent::WriteSpaces(indent)
                }

                AttributeIndent::WriteConfigured(_) => {}
                // .new_line() called when .with_attribute() was called at least once.
                // The configured indent characters should be used to indent
                // Plan saved indent
                AttributeIndent::Configured(indent) => {
                    self.state = AttributeIndent::WriteConfigured(indent)
                }
            }
            self.start_tag.push_newline();
        };
        self
    }

    /// Writes attribute and maintain indentation state
    fn write_attr<'b>(&mut self, attr: Attribute<'b>) {
        if let Some(i) = self.writer.indent.as_mut() {
            // Save the indent that we should use next time when .new_line() be called
            self.state = match self.state {
                // Neither .new_line() or .with_attribute() yet called
                // If newline inside attributes will be requested, we should indent them
                // by the length of tag name and +1 for `<` and +1 for one space
                AttributeIndent::NoneAttributesWritten => {
                    self.start_tag.push_attribute(attr);
                    AttributeIndent::Spaces(self.start_tag.name().as_ref().len() + 2)
                }

                // Indent was requested by previous call to .new_line(), write it
                // New line was already written
                AttributeIndent::WriteSpaces(indent) => {
                    if self.spaces.len() < indent {
                        self.spaces.resize(indent, b' ');
                    }
                    self.start_tag.push_indent(&self.spaces[..indent]);
                    self.start_tag.push_attr(attr.into());
                    AttributeIndent::Spaces(indent)
                }
                // .new_line() was not called, but .with_attribute() was.
                // use the previously calculated indent
                AttributeIndent::Spaces(indent) => {
                    self.start_tag.push_attribute(attr);
                    AttributeIndent::Spaces(indent)
                }

                // Indent was requested by previous call to .new_line(), write it
                // New line was already written
                AttributeIndent::WriteConfigured(indent) => {
                    self.start_tag.push_indent(i.additional(indent));
                    self.start_tag.push_attr(attr.into());
                    AttributeIndent::Configured(indent)
                }
                // .new_line() was not called, but .with_attribute() was.
                // use the previously calculated indent
                AttributeIndent::Configured(indent) => {
                    self.start_tag.push_attribute(attr);
                    AttributeIndent::Configured(indent)
                }
            };
        } else {
            self.start_tag.push_attribute(attr);
        }
    }
}

impl<'a, W: Write> ElementWriter<'a, W> {
    /// Write some text inside the current element.
    pub fn write_text_content(self, text: BytesText) -> io::Result<&'a mut Writer<W>> {
        self.writer
            .write_event(Event::Start(self.start_tag.borrow()))?;
        self.writer.write_event(Event::Text(text))?;
        self.writer
            .write_event(Event::End(self.start_tag.to_end()))?;
        Ok(self.writer)
    }

    /// Write a CData event `<![CDATA[...]]>` inside the current element.
    pub fn write_cdata_content(self, text: BytesCData) -> io::Result<&'a mut Writer<W>> {
        self.writer
            .write_event(Event::Start(self.start_tag.borrow()))?;
        self.writer.write_event(Event::CData(text))?;
        self.writer
            .write_event(Event::End(self.start_tag.to_end()))?;
        Ok(self.writer)
    }

    /// Write a processing instruction `<?...?>` inside the current element.
    pub fn write_pi_content(self, pi: BytesPI) -> io::Result<&'a mut Writer<W>> {
        self.writer
            .write_event(Event::Start(self.start_tag.borrow()))?;
        self.writer.write_event(Event::PI(pi))?;
        self.writer
            .write_event(Event::End(self.start_tag.to_end()))?;
        Ok(self.writer)
    }

    /// Write an empty (self-closing) tag.
    pub fn write_empty(self) -> io::Result<&'a mut Writer<W>> {
        self.writer.write_event(Event::Empty(self.start_tag))?;
        Ok(self.writer)
    }

    /// Create a new scope for writing XML inside the current element.
    pub fn write_inner_content<F>(self, closure: F) -> io::Result<&'a mut Writer<W>>
    where
        F: FnOnce(&mut Writer<W>) -> io::Result<()>,
    {
        self.writer
            .write_event(Event::Start(self.start_tag.borrow()))?;
        closure(self.writer)?;
        self.writer
            .write_event(Event::End(self.start_tag.to_end()))?;
        Ok(self.writer)
    }
}
#[cfg(feature = "serialize")]
pub(crate) struct ToFmtWrite<T>(pub T);

#[cfg(feature = "serialize")]
impl<T> std::fmt::Write for ToFmtWrite<T>
where
    T: std::io::Write,
{
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

#[derive(Clone)]
pub(crate) struct Indentation {
    /// todo: this is an awkward fit as it has no impact on indentation logic, but it is
    /// only applicable when an indentation exists. Potentially refactor later
    should_line_break: bool,
    /// The character code to be used for indentations (e.g. ` ` or `\t`)
    indent_char: u8,
    /// How many instances of the indent character ought to be used for each level of indentation
    indent_size: usize,
    /// Used as a cache for the bytes used for indentation
    indents: Vec<u8>,
    /// The current amount of indentation
    current_indent_len: usize,
}

impl Indentation {
    pub fn new(indent_char: u8, indent_size: usize) -> Self {
        Self {
            should_line_break: false,
            indent_char,
            indent_size,
            indents: vec![indent_char; 128],
            current_indent_len: 0, // invariant - needs to remain less than indents.len()
        }
    }

    /// Increase indentation by one level
    pub fn grow(&mut self) {
        self.current_indent_len += self.indent_size;
        self.ensure(self.current_indent_len);
    }

    /// Decrease indentation by one level. Do nothing, if level already zero
    pub fn shrink(&mut self) {
        self.current_indent_len = self.current_indent_len.saturating_sub(self.indent_size);
    }

    /// Returns indent string for current level
    pub fn current(&self) -> &[u8] {
        &self.indents[..self.current_indent_len]
    }

    /// Returns indent with current indent plus additional indent
    pub fn additional(&mut self, additional_indent: usize) -> &[u8] {
        let new_len = self.current_indent_len + additional_indent;
        self.ensure(new_len);
        &self.indents[..new_len]
    }

    fn ensure(&mut self, new_len: usize) {
        if self.indents.len() < new_len {
            self.indents.resize(new_len, self.indent_char);
        }
    }
}
