use codex_protocol::ThreadId;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct InheritedThreadState {
    prompt_cache_key: Option<ThreadId>,
}

impl InheritedThreadState {
    pub(crate) fn builder() -> InheritedThreadStateBuilder {
        InheritedThreadStateBuilder::default()
    }

    pub(crate) fn prompt_cache_key(&self) -> Option<ThreadId> {
        self.prompt_cache_key
    }
}

#[derive(Default)]
pub(crate) struct InheritedThreadStateBuilder {
    prompt_cache_key: Option<ThreadId>,
}

impl InheritedThreadStateBuilder {
    pub(crate) fn prompt_cache_key(mut self, prompt_cache_key: Option<ThreadId>) -> Self {
        self.prompt_cache_key = prompt_cache_key;
        self
    }

    pub(crate) fn build(self) -> InheritedThreadState {
        InheritedThreadState {
            prompt_cache_key: self.prompt_cache_key,
        }
    }
}
