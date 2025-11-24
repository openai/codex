use crate::decoder::Encoding;
use crate::ErrorSink;
use crate::Span;

pub(crate) struct DebugDepth(core::sync::atomic::AtomicUsize);

impl DebugDepth {
    pub(crate) fn enter_unchecked(&self) -> usize {
        self.0.fetch_add(1, core::sync::atomic::Ordering::SeqCst)
    }

    pub(crate) fn exit_unchecked(&self) {
        let _ = self.0.fetch_sub(1, core::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn depth(&self) -> usize {
        self.0.load(core::sync::atomic::Ordering::SeqCst)
    }
}

static DEBUG_DEPTH: DebugDepth = DebugDepth(core::sync::atomic::AtomicUsize::new(0));

fn render_event(span: impl Into<Option<Span>>, text: &str, style: anstyle::Style) {
    #![allow(unexpected_cfgs)] // HACK: fixed in newer versions
    let span = span.into();
    let depth = DEBUG_DEPTH.depth().min(20);
    anstream::eprintln!("{:depth$}{style}{text}: {span:?}{style:#}", "");
}

pub(crate) struct DebugErrorSink<'s> {
    sink: &'s mut dyn ErrorSink,
}

impl<'s> DebugErrorSink<'s> {
    pub(crate) fn new(sink: &'s mut dyn ErrorSink) -> Self {
        Self { sink }
    }
}

impl ErrorSink for DebugErrorSink<'_> {
    fn report_error(&mut self, error: crate::ParseError) {
        render_event(
            error.unexpected(),
            &format!("{error:?}"),
            anstyle::AnsiColor::Red.on_default(),
        );
        self.sink.report_error(error);
    }
}

pub(crate) struct DebugEventReceiver<'r> {
    receiver: &'r mut dyn crate::parser::EventReceiver,
}

impl<'r> DebugEventReceiver<'r> {
    pub(crate) fn new(receiver: &'r mut dyn crate::parser::EventReceiver) -> Self {
        Self { receiver }
    }
}

impl crate::parser::EventReceiver for DebugEventReceiver<'_> {
    fn std_table_open(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.std_table_open(span, error);
        render_event(span, "[", anstyle::Style::new() | anstyle::Effects::DIMMED);
        DEBUG_DEPTH.enter_unchecked();
    }
    fn std_table_close(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.std_table_close(span, error);
        DEBUG_DEPTH.exit_unchecked();
        render_event(span, "]", anstyle::Style::new() | anstyle::Effects::DIMMED);
    }
    fn array_table_open(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.array_table_open(span, error);
        render_event(span, "[[", anstyle::Style::new() | anstyle::Effects::DIMMED);
        DEBUG_DEPTH.enter_unchecked();
    }
    fn array_table_close(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.array_table_close(span, error);
        DEBUG_DEPTH.exit_unchecked();
        render_event(span, "]]", anstyle::Style::new() | anstyle::Effects::DIMMED);
    }
    fn inline_table_open(&mut self, span: Span, error: &mut dyn ErrorSink) -> bool {
        let allowed = self.receiver.inline_table_open(span, error);
        render_event(span, "{", anstyle::Style::new() | anstyle::Effects::DIMMED);
        DEBUG_DEPTH.enter_unchecked();
        allowed
    }
    fn inline_table_close(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.inline_table_close(span, error);
        DEBUG_DEPTH.exit_unchecked();
        render_event(span, "}", anstyle::Style::new() | anstyle::Effects::DIMMED);
    }
    fn array_open(&mut self, span: Span, error: &mut dyn ErrorSink) -> bool {
        let allowed = self.receiver.array_open(span, error);
        render_event(span, "[", anstyle::Style::new() | anstyle::Effects::DIMMED);
        DEBUG_DEPTH.enter_unchecked();
        allowed
    }
    fn array_close(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.array_close(span, error);
        DEBUG_DEPTH.exit_unchecked();
        render_event(span, "]", anstyle::Style::new() | anstyle::Effects::DIMMED);
    }
    fn simple_key(&mut self, span: Span, encoding: Option<Encoding>, error: &mut dyn ErrorSink) {
        self.receiver.simple_key(span, encoding, error);
        render_event(span, "<key>", anstyle::AnsiColor::Magenta.on_default());
    }
    fn key_sep(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.key_sep(span, error);
        render_event(span, ".", anstyle::Style::new() | anstyle::Effects::DIMMED);
    }
    fn key_val_sep(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.key_val_sep(span, error);
        render_event(span, "=", anstyle::Style::new() | anstyle::Effects::DIMMED);
    }
    fn scalar(&mut self, span: Span, encoding: Option<Encoding>, error: &mut dyn ErrorSink) {
        self.receiver.scalar(span, encoding, error);
        render_event(span, "<scalar>", anstyle::AnsiColor::Green.on_default());
    }
    fn value_sep(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.value_sep(span, error);
        render_event(span, ",", anstyle::Style::new() | anstyle::Effects::DIMMED);
    }
    fn whitespace(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.whitespace(span, error);
        render_event(span, "<whitespace>", anstyle::AnsiColor::Cyan.on_default());
    }
    fn comment(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.comment(span, error);
        render_event(span, "<comment>", anstyle::AnsiColor::Cyan.on_default());
    }
    fn newline(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.newline(span, error);
        render_event(span, "<newline>", anstyle::AnsiColor::Cyan.on_default());
    }
    fn error(&mut self, span: Span, error: &mut dyn ErrorSink) {
        self.receiver.error(span, error);
        render_event(span, "<error>", anstyle::AnsiColor::Red.on_default());
    }
}
