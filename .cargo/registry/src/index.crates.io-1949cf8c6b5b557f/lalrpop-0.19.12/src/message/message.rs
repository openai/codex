use ascii_canvas::AsciiView;
use crate::grammar::parse_tree::Span;
use crate::message::Content;
use std::cmp;
use std::fmt::{Debug, Error, Formatter};
use crate::style::Style;
use crate::tls::Tls;

/// The top-level message display like this:
///
/// ```
/// <span>: <heading>
///
/// <body>
/// ```
///
/// This is equivalent to a
///
/// ```
/// Vert[separate=2] {
///     Horiz[separate=1] {
///         Horiz[separate=0] {
///             Citation { span },
///             Text { ":" },
///         },
///         <heading>,
///     },
///     <body>
/// }
/// ```
pub struct Message {
    span: Span,
    heading: Box<dyn Content>,
    body: Box<dyn Content>,
}

impl Message {
    pub fn new(span: Span, heading: Box<dyn Content>, body: Box<dyn Content>) -> Self {
        Message {
            span,
            heading,
            body,
        }
    }
}

impl Content for Message {
    fn min_width(&self) -> usize {
        let file_text = Tls::file_text();
        let span = file_text.span_str(self.span).chars().count();
        let heading = self.heading.min_width();
        let body = self.body.min_width();
        cmp::max(span + heading + 2, body + 2)
    }

    fn emit(&self, view: &mut dyn AsciiView) {
        let session = Tls::session();
        let file_text = Tls::file_text();

        let span = file_text.span_str(self.span);
        view.write_chars(0, 0, span.chars(), Style::new());
        let count = span.chars().count();
        view.write_chars(0, count, ":".chars(), Style::new());

        let (row, _) = self
            .heading
            .emit_at(&mut view.styled(session.heading), 0, count + 2);

        self.body.emit_at(view, row + 2, 2);
    }

    fn into_wrap_items(self: Box<Self>, wrap_items: &mut Vec<Box<dyn Content>>) {
        wrap_items.push(self);
    }
}

impl Debug for Message {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        fmt.debug_struct("Message")
            .field("span", &self.span)
            .field("heading", &self.heading)
            .field("body", &self.body)
            .finish()
    }
}
