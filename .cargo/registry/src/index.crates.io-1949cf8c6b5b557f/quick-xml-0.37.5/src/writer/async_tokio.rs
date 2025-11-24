use std::future::Future;
use std::result::Result as StdResult;

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::errors::{Error, Result};
use crate::events::{BytesCData, BytesPI, BytesText, Event};
use crate::{ElementWriter, Writer};

impl<W: AsyncWrite + Unpin> Writer<W> {
    /// Writes the given event to the underlying writer. Async version of [`Writer::write_event`].
    pub async fn write_event_async<'a, E: Into<Event<'a>>>(&mut self, event: E) -> Result<()> {
        let mut next_should_line_break = true;
        let result = match event.into() {
            Event::Start(e) => {
                let result = self.write_wrapped_async(b"<", &e, b">").await;
                if let Some(i) = self.indent.as_mut() {
                    i.grow();
                }
                result
            }
            Event::End(e) => {
                if let Some(i) = self.indent.as_mut() {
                    i.shrink();
                }
                self.write_wrapped_async(b"</", &e, b">").await
            }
            Event::Empty(e) => self.write_wrapped_async(b"<", &e, b"/>").await,
            Event::Text(e) => {
                next_should_line_break = false;
                self.write_async(&e).await
            }
            Event::Comment(e) => self.write_wrapped_async(b"<!--", &e, b"-->").await,
            Event::CData(e) => {
                next_should_line_break = false;
                self.write_async(b"<![CDATA[").await?;
                self.write_async(&e).await?;
                self.write_async(b"]]>").await
            }
            Event::Decl(e) => self.write_wrapped_async(b"<?", &e, b"?>").await,
            Event::PI(e) => self.write_wrapped_async(b"<?", &e, b"?>").await,
            Event::DocType(e) => self.write_wrapped_async(b"<!DOCTYPE ", &e, b">").await,
            Event::Eof => Ok(()),
        };
        if let Some(i) = self.indent.as_mut() {
            i.should_line_break = next_should_line_break;
        }
        result
    }

    /// Manually write a newline and indentation at the proper level. Async version of
    /// [`Writer::write_indent`].
    ///
    /// This method will do nothing if `Writer` was not constructed with [`Writer::new_with_indent`].
    pub async fn write_indent_async(&mut self) -> Result<()> {
        if let Some(ref i) = self.indent {
            self.writer.write_all(b"\n").await?;
            self.writer.write_all(i.current()).await?;
        }
        Ok(())
    }

    #[inline]
    async fn write_async(&mut self, value: &[u8]) -> Result<()> {
        self.writer.write_all(value).await.map_err(Into::into)
    }

    #[inline]
    async fn write_wrapped_async(
        &mut self,
        before: &[u8],
        value: &[u8],
        after: &[u8],
    ) -> Result<()> {
        if let Some(ref i) = self.indent {
            if i.should_line_break {
                self.writer.write_all(b"\n").await?;
                self.writer.write_all(i.current()).await?;
            }
        }
        self.write_async(before).await?;
        self.write_async(value).await?;
        self.write_async(after).await?;
        Ok(())
    }
}

impl<'a, W: AsyncWrite + Unpin> ElementWriter<'a, W> {
    /// Write some text inside the current element.
    ///
    /// # Example
    ///
    /// ```
    /// # use quick_xml::writer::Writer;
    /// # use quick_xml::events::BytesText;
    /// # use tokio::io::AsyncWriteExt;
    /// # #[tokio::main(flavor = "current_thread")] async fn main() {
    /// let mut buffer = Vec::new();
    /// let mut tokio_buffer = tokio::io::BufWriter::new(&mut buffer);
    /// let mut writer = Writer::new_with_indent(&mut tokio_buffer, b' ', 4);
    ///
    /// writer
    ///     .create_element("paired")
    ///     .with_attribute(("attr1", "value1"))
    ///     .with_attribute(("attr2", "value2"))
    ///     .write_text_content_async(BytesText::new("text"))
    ///     .await
    ///     .expect("cannot write content");
    ///
    /// tokio_buffer.flush().await.expect("flush failed");
    ///
    /// assert_eq!(
    ///     std::str::from_utf8(&buffer).unwrap(),
    ///     r#"<paired attr1="value1" attr2="value2">text</paired>"#
    /// );
    /// # }
    pub async fn write_text_content_async(self, text: BytesText<'_>) -> Result<&'a mut Writer<W>> {
        self.writer
            .write_event_async(Event::Start(self.start_tag.borrow()))
            .await?;
        self.writer.write_event_async(Event::Text(text)).await?;
        self.writer
            .write_event_async(Event::End(self.start_tag.to_end()))
            .await?;
        Ok(self.writer)
    }

    /// Write a CData event `<![CDATA[...]]>` inside the current element.
    ///
    /// # Example
    ///
    /// ```
    /// # use quick_xml::writer::Writer;
    /// # use quick_xml::events::BytesCData;
    /// # use tokio::io::AsyncWriteExt;
    /// # #[tokio::main(flavor = "current_thread")] async fn main() {
    /// let mut buffer = Vec::new();
    /// let mut tokio_buffer = tokio::io::BufWriter::new(&mut buffer);
    /// let mut writer = Writer::new_with_indent(&mut tokio_buffer, b' ', 4);
    ///
    /// writer
    ///     .create_element("paired")
    ///     .with_attribute(("attr1", "value1"))
    ///     .with_attribute(("attr2", "value2"))
    ///     .write_cdata_content_async(BytesCData::new("text & content"))
    ///     .await
    ///     .expect("cannot write content");
    ///
    /// tokio_buffer.flush().await.expect("flush failed");
    ///
    /// assert_eq!(
    ///     std::str::from_utf8(&buffer).unwrap(),
    ///     r#"<paired attr1="value1" attr2="value2"><![CDATA[text & content]]></paired>"#
    /// );
    /// # }
    pub async fn write_cdata_content_async(
        self,
        text: BytesCData<'_>,
    ) -> Result<&'a mut Writer<W>> {
        self.writer
            .write_event_async(Event::Start(self.start_tag.borrow()))
            .await?;
        self.writer.write_event_async(Event::CData(text)).await?;
        self.writer
            .write_event_async(Event::End(self.start_tag.to_end()))
            .await?;
        Ok(self.writer)
    }

    /// Write a processing instruction `<?...?>` inside the current element.
    ///
    /// # Example
    ///
    /// ```
    /// # use quick_xml::writer::Writer;
    /// # use quick_xml::events::BytesPI;
    /// # use tokio::io::AsyncWriteExt;
    /// # #[tokio::main(flavor = "current_thread")] async fn main() {
    /// let mut buffer = Vec::new();
    /// let mut tokio_buffer = tokio::io::BufWriter::new(&mut buffer);
    /// let mut writer = Writer::new_with_indent(&mut tokio_buffer, b' ', 4);
    ///
    /// writer
    ///     .create_element("paired")
    ///     .with_attribute(("attr1", "value1"))
    ///     .with_attribute(("attr2", "value2"))
    ///     .write_pi_content_async(BytesPI::new(r#"xml-stylesheet href="style.css""#))
    ///     .await
    ///     .expect("cannot write content");
    ///
    /// tokio_buffer.flush().await.expect("flush failed");
    ///
    /// assert_eq!(
    ///     std::str::from_utf8(&buffer).unwrap(),
    ///     r#"<paired attr1="value1" attr2="value2">
    ///     <?xml-stylesheet href="style.css"?>
    /// </paired>"#
    /// );
    /// # }
    pub async fn write_pi_content_async(self, text: BytesPI<'_>) -> Result<&'a mut Writer<W>> {
        self.writer
            .write_event_async(Event::Start(self.start_tag.borrow()))
            .await?;
        self.writer.write_event_async(Event::PI(text)).await?;
        self.writer
            .write_event_async(Event::End(self.start_tag.to_end()))
            .await?;
        Ok(self.writer)
    }

    /// Write an empty (self-closing) tag.
    ///
    /// # Example
    ///
    /// ```
    /// # use quick_xml::writer::Writer;
    /// # use quick_xml::events::BytesText;
    /// # use tokio::io::AsyncWriteExt;
    /// # #[tokio::main(flavor = "current_thread")] async fn main() {
    /// let mut buffer = Vec::new();
    /// let mut tokio_buffer = tokio::io::BufWriter::new(&mut buffer);
    /// let mut writer = Writer::new_with_indent(&mut tokio_buffer, b' ', 4);
    ///
    /// writer
    ///     .create_element("empty")
    ///     .with_attribute(("attr1", "value1"))
    ///     .with_attribute(("attr2", "value2"))
    ///     .write_empty_async()
    ///     .await
    ///     .expect("cannot write content");
    ///
    /// tokio_buffer.flush().await.expect("flush failed");
    ///
    /// assert_eq!(
    ///     std::str::from_utf8(&buffer).unwrap(),
    ///     r#"<empty attr1="value1" attr2="value2"/>"#
    /// );
    /// # }
    pub async fn write_empty_async(self) -> Result<&'a mut Writer<W>> {
        self.writer
            .write_event_async(Event::Empty(self.start_tag))
            .await?;
        Ok(self.writer)
    }

    /// Create a new scope for writing XML inside the current element.
    ///
    /// # Example
    ///
    /// ```
    /// # use quick_xml::writer::Writer;
    /// # use quick_xml::events::BytesText;
    /// # use tokio::io::AsyncWriteExt;
    /// use quick_xml::Error;
    ///
    /// # #[tokio::main(flavor = "current_thread")] async fn main() {
    /// let mut buffer = Vec::new();
    /// let mut tokio_buffer = tokio::io::BufWriter::new(&mut buffer);
    /// let mut writer = Writer::new_with_indent(&mut tokio_buffer, b' ', 4);
    ///
    /// writer
    ///     .create_element("outer")
    ///     .with_attributes([("attr1", "value1"), ("attr2", "value2")])
    ///     // We need to provide error type, because it is not named somewhere explicitly
    ///     .write_inner_content_async::<_, _, Error>(|writer| async move {
    ///         let fruits = ["apple", "orange", "banana"];
    ///         for (quant, item) in fruits.iter().enumerate() {
    ///             writer
    ///                 .create_element("fruit")
    ///                 .with_attributes([("quantity", quant.to_string().as_str())])
    ///                 .write_text_content_async(BytesText::new(item))
    ///                 .await?;
    ///         }
    ///         writer
    ///             .create_element("inner")
    ///             .write_inner_content_async(|writer| async move {
    ///                 writer.create_element("empty").write_empty_async().await
    ///             })
    ///             .await?;
    ///
    ///         Ok(writer)
    ///     })
    ///     .await
    ///     .expect("cannot write content");
    ///
    /// tokio_buffer.flush().await.expect("flush failed");
    /// assert_eq!(
    ///     std::str::from_utf8(&buffer).unwrap(),
    ///     r#"<outer attr1="value1" attr2="value2">
    ///     <fruit quantity="0">apple</fruit>
    ///     <fruit quantity="1">orange</fruit>
    ///     <fruit quantity="2">banana</fruit>
    ///     <inner>
    ///         <empty/>
    ///     </inner>
    /// </outer>"#
    /// );
    /// # }
    pub async fn write_inner_content_async<F, Fut, E>(
        mut self,
        closure: F,
    ) -> StdResult<&'a mut Writer<W>, E>
    where
        F: FnOnce(&'a mut Writer<W>) -> Fut,
        Fut: Future<Output = StdResult<&'a mut Writer<W>, E>>,
        E: From<Error>,
    {
        self.writer
            .write_event_async(Event::Start(self.start_tag.borrow()))
            .await?;
        self.writer = closure(self.writer).await?;
        self.writer
            .write_event_async(Event::End(self.start_tag.to_end()))
            .await?;
        Ok(self.writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::*;
    use pretty_assertions::assert_eq;

    macro_rules! test {
        ($name: ident, $event: expr, $expected: expr) => {
            #[tokio::test]
            async fn $name() {
                let mut buffer = Vec::new();
                let mut writer = Writer::new(&mut buffer);

                writer
                    .write_event_async($event)
                    .await
                    .expect("write event failed");

                assert_eq!(std::str::from_utf8(&buffer).unwrap(), $expected,);
            }
        };
    }

    test!(
        xml_header,
        Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), Some("no"))),
        r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>"#
    );

    test!(empty_tag, Event::Empty(BytesStart::new("tag")), r#"<tag/>"#);

    test!(
        comment,
        Event::Comment(BytesText::new("this is a comment")),
        r#"<!--this is a comment-->"#
    );

    test!(
        cdata,
        Event::CData(BytesCData::new("this is a cdata")),
        r#"<![CDATA[this is a cdata]]>"#
    );

    test!(
        pi,
        Event::PI(BytesPI::new("this is a processing instruction")),
        r#"<?this is a processing instruction?>"#
    );

    test!(
        doctype,
        Event::DocType(BytesText::new("this is a doctype")),
        r#"<!DOCTYPE this is a doctype>"#
    );

    #[tokio::test]
    async fn full_tag() {
        let mut buffer = Vec::new();
        let mut writer = Writer::new(&mut buffer);

        let start = Event::Start(BytesStart::new("tag"));
        let text = Event::Text(BytesText::new("inner text"));
        let end = Event::End(BytesEnd::new("tag"));
        for i in [start, text, end] {
            writer.write_event_async(i).await.expect("write tag failed");
        }

        assert_eq!(
            std::str::from_utf8(&buffer).unwrap(),
            r#"<tag>inner text</tag>"#
        );
    }
}

#[cfg(test)]
mod indentation_async {
    use super::*;
    use crate::events::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn self_closed() {
        let mut buffer = Vec::new();
        let mut writer = Writer::new_with_indent(&mut buffer, b' ', 4);

        let tag = BytesStart::new("self-closed")
            .with_attributes(vec![("attr1", "value1"), ("attr2", "value2")].into_iter());
        writer
            .write_event_async(Event::Empty(tag))
            .await
            .expect("write tag failed");

        assert_eq!(
            std::str::from_utf8(&buffer).unwrap(),
            r#"<self-closed attr1="value1" attr2="value2"/>"#
        );
    }

    #[tokio::test]
    async fn empty_paired() {
        let mut buffer = Vec::new();
        let mut writer = Writer::new_with_indent(&mut buffer, b' ', 4);

        let start = BytesStart::new("paired")
            .with_attributes(vec![("attr1", "value1"), ("attr2", "value2")].into_iter());
        let end = start.to_end();
        writer
            .write_event_async(Event::Start(start.clone()))
            .await
            .expect("write start tag failed");
        writer
            .write_event_async(Event::End(end))
            .await
            .expect("write end tag failed");

        assert_eq!(
            std::str::from_utf8(&buffer).unwrap(),
            r#"<paired attr1="value1" attr2="value2">
</paired>"#
        );
    }

    #[tokio::test]
    async fn paired_with_inner() {
        let mut buffer = Vec::new();
        let mut writer = Writer::new_with_indent(&mut buffer, b' ', 4);

        let start = BytesStart::new("paired")
            .with_attributes(vec![("attr1", "value1"), ("attr2", "value2")].into_iter());
        let end = start.to_end();
        let inner = BytesStart::new("inner");

        writer
            .write_event_async(Event::Start(start.clone()))
            .await
            .expect("write start tag failed");
        writer
            .write_event_async(Event::Empty(inner))
            .await
            .expect("write inner tag failed");
        writer
            .write_event_async(Event::End(end))
            .await
            .expect("write end tag failed");

        assert_eq!(
            std::str::from_utf8(&buffer).unwrap(),
            r#"<paired attr1="value1" attr2="value2">
    <inner/>
</paired>"#
        );
    }

    #[tokio::test]
    async fn paired_with_text() {
        let mut buffer = Vec::new();
        let mut writer = Writer::new_with_indent(&mut buffer, b' ', 4);

        let start = BytesStart::new("paired")
            .with_attributes(vec![("attr1", "value1"), ("attr2", "value2")].into_iter());
        let end = start.to_end();
        let text = BytesText::new("text");

        writer
            .write_event_async(Event::Start(start.clone()))
            .await
            .expect("write start tag failed");
        writer
            .write_event_async(Event::Text(text))
            .await
            .expect("write text failed");
        writer
            .write_event_async(Event::End(end))
            .await
            .expect("write end tag failed");

        assert_eq!(
            std::str::from_utf8(&buffer).unwrap(),
            r#"<paired attr1="value1" attr2="value2">text</paired>"#
        );
    }

    #[tokio::test]
    async fn mixed_content() {
        let mut buffer = Vec::new();
        let mut writer = Writer::new_with_indent(&mut buffer, b' ', 4);

        let start = BytesStart::new("paired")
            .with_attributes(vec![("attr1", "value1"), ("attr2", "value2")].into_iter());
        let end = start.to_end();
        let text = BytesText::new("text");
        let inner = BytesStart::new("inner");

        writer
            .write_event_async(Event::Start(start.clone()))
            .await
            .expect("write start tag failed");
        writer
            .write_event_async(Event::Text(text))
            .await
            .expect("write text failed");
        writer
            .write_event_async(Event::Empty(inner))
            .await
            .expect("write inner tag failed");
        writer
            .write_event_async(Event::End(end))
            .await
            .expect("write end tag failed");

        assert_eq!(
            std::str::from_utf8(&buffer).unwrap(),
            r#"<paired attr1="value1" attr2="value2">text<inner/>
</paired>"#
        );
    }

    #[tokio::test]
    async fn nested() {
        let mut buffer = Vec::new();
        let mut writer = Writer::new_with_indent(&mut buffer, b' ', 4);

        let start = BytesStart::new("paired")
            .with_attributes(vec![("attr1", "value1"), ("attr2", "value2")].into_iter());
        let end = start.to_end();
        let inner = BytesStart::new("inner");

        writer
            .write_event_async(Event::Start(start.clone()))
            .await
            .expect("write start 1 tag failed");
        writer
            .write_event_async(Event::Start(start.clone()))
            .await
            .expect("write start 2 tag failed");
        writer
            .write_event_async(Event::Empty(inner))
            .await
            .expect("write inner tag failed");
        writer
            .write_event_async(Event::End(end.clone()))
            .await
            .expect("write end tag 2 failed");
        writer
            .write_event_async(Event::End(end))
            .await
            .expect("write end tag 1 failed");

        assert_eq!(
            std::str::from_utf8(&buffer).unwrap(),
            r#"<paired attr1="value1" attr2="value2">
    <paired attr1="value1" attr2="value2">
        <inner/>
    </paired>
</paired>"#
        );
    }
}
