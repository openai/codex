//! Hook chain for executing multiple hooks in priority order.

use super::HookContext;
use super::RequestHook;
use super::ResponseHook;
use super::StreamHook;
use crate::error::HyperError;
use crate::request::GenerateRequest;
use crate::response::GenerateResponse;
use crate::stream::StreamEvent;
use std::sync::Arc;

/// A chain of hooks executed in priority order.
///
/// The `HookChain` manages collections of request, response, and stream hooks,
/// executing them in priority order (lower priority values run first).
///
/// # Example
///
/// ```no_run
/// use hyper_sdk::hooks::{HookChain, ResponseIdHook, LoggingHook};
/// use std::sync::Arc;
///
/// let mut chain = HookChain::new();
/// chain.add_request_hook(Arc::new(ResponseIdHook));
/// chain.add_request_hook(Arc::new(LoggingHook::info()));
/// ```
#[derive(Debug, Default, Clone)]
pub struct HookChain {
    request_hooks: Vec<Arc<dyn RequestHook>>,
    response_hooks: Vec<Arc<dyn ResponseHook>>,
    stream_hooks: Vec<Arc<dyn StreamHook>>,
}

impl HookChain {
    /// Create a new empty hook chain.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a request hook to the chain.
    ///
    /// Hooks are automatically sorted by priority when executed.
    pub fn add_request_hook(&mut self, hook: Arc<dyn RequestHook>) -> &mut Self {
        self.request_hooks.push(hook);
        self.sort_request_hooks();
        self
    }

    /// Add a response hook to the chain.
    pub fn add_response_hook(&mut self, hook: Arc<dyn ResponseHook>) -> &mut Self {
        self.response_hooks.push(hook);
        self.sort_response_hooks();
        self
    }

    /// Add a stream hook to the chain.
    ///
    /// Hooks are automatically sorted by priority when added (lower values run first).
    pub fn add_stream_hook(&mut self, hook: Arc<dyn StreamHook>) -> &mut Self {
        self.stream_hooks.push(hook);
        self.sort_stream_hooks();
        self
    }

    /// Remove all hooks from the chain.
    pub fn clear(&mut self) {
        self.request_hooks.clear();
        self.response_hooks.clear();
        self.stream_hooks.clear();
    }

    /// Check if the chain has any request hooks.
    pub fn has_request_hooks(&self) -> bool {
        !self.request_hooks.is_empty()
    }

    /// Check if the chain has any response hooks.
    pub fn has_response_hooks(&self) -> bool {
        !self.response_hooks.is_empty()
    }

    /// Check if the chain has any stream hooks.
    pub fn has_stream_hooks(&self) -> bool {
        !self.stream_hooks.is_empty()
    }

    /// Get the number of request hooks.
    pub fn request_hook_count(&self) -> usize {
        self.request_hooks.len()
    }

    /// Get the number of response hooks.
    pub fn response_hook_count(&self) -> usize {
        self.response_hooks.len()
    }

    /// Get the number of stream hooks.
    pub fn stream_hook_count(&self) -> usize {
        self.stream_hooks.len()
    }

    /// Execute all request hooks in priority order.
    ///
    /// Hooks are executed sequentially in order of priority (lower values first).
    /// If any hook returns an error, execution stops and the error is returned.
    pub async fn run_request_hooks(
        &self,
        request: &mut GenerateRequest,
        context: &mut HookContext,
    ) -> Result<(), HyperError> {
        for hook in &self.request_hooks {
            tracing::debug!(hook = hook.name(), "Running request hook");
            hook.on_request(request, context).await?;
        }
        Ok(())
    }

    /// Execute all response hooks in priority order.
    pub async fn run_response_hooks(
        &self,
        response: &mut GenerateResponse,
        context: &HookContext,
    ) -> Result<(), HyperError> {
        for hook in &self.response_hooks {
            tracing::debug!(hook = hook.name(), "Running response hook");
            hook.on_response(response, context).await?;
        }
        Ok(())
    }

    /// Execute all stream hooks for an event.
    ///
    /// Unlike request/response hooks, stream hooks observe events but cannot modify them.
    pub async fn run_stream_hooks(
        &self,
        event: &StreamEvent,
        context: &HookContext,
    ) -> Result<(), HyperError> {
        for hook in &self.stream_hooks {
            hook.on_event(event, context).await?;
        }
        Ok(())
    }

    /// Sort request hooks by priority.
    fn sort_request_hooks(&mut self) {
        self.request_hooks.sort_by_key(|h| h.priority());
    }

    /// Sort response hooks by priority.
    fn sort_response_hooks(&mut self) {
        self.response_hooks.sort_by_key(|h| h.priority());
    }

    /// Sort stream hooks by priority.
    fn sort_stream_hooks(&mut self) {
        self.stream_hooks.sort_by_key(|h| h.priority());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::Message;
    use async_trait::async_trait;
    use std::sync::atomic::AtomicI32;
    use std::sync::atomic::Ordering;

    #[derive(Debug)]
    struct OrderTrackingHook {
        name: String,
        priority: i32,
        counter: Arc<AtomicI32>,
        recorded_order: Arc<std::sync::Mutex<Vec<i32>>>,
    }

    impl OrderTrackingHook {
        fn new(
            name: &str,
            priority: i32,
            counter: Arc<AtomicI32>,
            recorded_order: Arc<std::sync::Mutex<Vec<i32>>>,
        ) -> Self {
            Self {
                name: name.to_string(),
                priority,
                counter,
                recorded_order,
            }
        }
    }

    #[async_trait]
    impl RequestHook for OrderTrackingHook {
        async fn on_request(
            &self,
            _request: &mut GenerateRequest,
            _context: &mut HookContext,
        ) -> Result<(), HyperError> {
            let order = self.counter.fetch_add(1, Ordering::SeqCst);
            self.recorded_order.lock().unwrap().push(order);
            Ok(())
        }

        fn priority(&self) -> i32 {
            self.priority
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[derive(Debug)]
    struct ModifyTempHook {
        temp: f64,
    }

    #[async_trait]
    impl RequestHook for ModifyTempHook {
        async fn on_request(
            &self,
            request: &mut GenerateRequest,
            _context: &mut HookContext,
        ) -> Result<(), HyperError> {
            request.temperature = Some(self.temp);
            Ok(())
        }

        fn priority(&self) -> i32 {
            100
        }

        fn name(&self) -> &str {
            "modify_temp"
        }
    }

    #[tokio::test]
    async fn test_hook_chain_priority_order() {
        let counter = Arc::new(AtomicI32::new(0));
        let recorded = Arc::new(std::sync::Mutex::new(Vec::new()));

        let mut chain = HookChain::new();

        // Add hooks in reverse priority order
        chain.add_request_hook(Arc::new(OrderTrackingHook::new(
            "low_priority",
            200,
            counter.clone(),
            recorded.clone(),
        )));
        chain.add_request_hook(Arc::new(OrderTrackingHook::new(
            "high_priority",
            10,
            counter.clone(),
            recorded.clone(),
        )));
        chain.add_request_hook(Arc::new(OrderTrackingHook::new(
            "medium_priority",
            100,
            counter.clone(),
            recorded.clone(),
        )));

        let mut request = GenerateRequest::new(vec![Message::user("test")]);
        let mut context = HookContext::new();

        chain
            .run_request_hooks(&mut request, &mut context)
            .await
            .unwrap();

        // Hooks should have run in priority order (10, 100, 200)
        let order = recorded.lock().unwrap();
        assert_eq!(*order, vec![0, 1, 2]);
    }

    #[tokio::test]
    async fn test_hook_chain_modifies_request() {
        let mut chain = HookChain::new();
        chain.add_request_hook(Arc::new(ModifyTempHook { temp: 0.42 }));

        let mut request = GenerateRequest::new(vec![Message::user("test")]);
        assert!(request.temperature.is_none());

        let mut context = HookContext::new();
        chain
            .run_request_hooks(&mut request, &mut context)
            .await
            .unwrap();

        assert_eq!(request.temperature, Some(0.42));
    }

    #[test]
    fn test_hook_chain_counts() {
        let mut chain = HookChain::new();
        assert!(!chain.has_request_hooks());
        assert_eq!(chain.request_hook_count(), 0);

        chain.add_request_hook(Arc::new(ModifyTempHook { temp: 0.5 }));
        assert!(chain.has_request_hooks());
        assert_eq!(chain.request_hook_count(), 1);

        chain.clear();
        assert!(!chain.has_request_hooks());
    }
}
