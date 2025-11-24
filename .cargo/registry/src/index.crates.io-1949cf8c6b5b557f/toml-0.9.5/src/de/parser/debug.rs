pub(crate) struct TraceScope {
    text: String,
    style: anstyle::Style,
    guard: DebugDepthGuard,
}

impl TraceScope {
    pub(crate) fn new(text: impl core::fmt::Display) -> Self {
        let text = text.to_string();
        let style = anstyle::Style::new();
        trace(&format!("> {text}"), style);
        Self {
            text,
            style,
            guard: DEBUG_DEPTH.scoped(),
        }
    }
}

impl Drop for TraceScope {
    fn drop(&mut self) {
        let text = &self.text;
        let style = self.style;
        drop(self.guard.take());
        trace(&format!("< {text}"), style);
    }
}

pub(crate) fn trace(text: &str, style: anstyle::Style) {
    #![allow(unexpected_cfgs)] // HACK: fixed in newer versions
    let depth = DEBUG_DEPTH.depth();
    anstream::eprintln!("{:depth$}{style}{text}{style:#}", "");
}

pub(crate) struct DebugDepth(core::sync::atomic::AtomicUsize);

impl DebugDepth {
    pub(crate) fn scoped(&self) -> DebugDepthGuard {
        DebugDepthGuard::new()
    }

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

pub(crate) struct DebugDepthGuard {
    depth: usize,
    inc: bool,
}

impl DebugDepthGuard {
    pub(crate) fn new() -> Self {
        let depth = DEBUG_DEPTH.enter_unchecked();
        Self { depth, inc: true }
    }

    fn take(&mut self) -> Self {
        let depth = self.depth;
        let inc = self.inc;
        self.inc = false;
        Self { depth, inc }
    }
}

impl Drop for DebugDepthGuard {
    fn drop(&mut self) {
        if self.inc {
            DEBUG_DEPTH.exit_unchecked();
        }
    }
}

impl AsRef<usize> for DebugDepthGuard {
    #[inline(always)]
    fn as_ref(&self) -> &usize {
        &self.depth
    }
}

impl core::ops::Deref for DebugDepthGuard {
    type Target = usize;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.depth
    }
}
