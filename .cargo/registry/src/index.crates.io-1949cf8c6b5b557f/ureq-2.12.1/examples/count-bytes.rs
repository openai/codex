use std::sync::{Arc, Mutex};

use ureq::{Error, Middleware, MiddlewareNext, Request, Response};

// Some state that could be shared with the main application.
#[derive(Debug, Default)]
struct CounterState {
    request_count: u64,
    total_bytes: u64,
}

// Middleware wrapper working off the shared state.
struct CounterMiddleware(Arc<Mutex<CounterState>>);

pub fn main() -> Result<(), Error> {
    // Shared state for counters.
    let shared_state = Arc::new(Mutex::new(CounterState::default()));

    let agent = ureq::builder()
        // Clone the state into the middleware
        .middleware(CounterMiddleware(shared_state.clone()))
        .build();

    agent.get("https://httpbin.org/bytes/123").call()?;
    agent.get("https://httpbin.org/bytes/123").call()?;

    {
        let state = shared_state.lock().unwrap();

        println!("State after requests:\n\n{:?}\n", state);

        assert_eq!(state.request_count, 2);
        assert_eq!(state.total_bytes, 246);
    }

    Ok(())
}

impl Middleware for CounterMiddleware {
    fn handle(&self, request: Request, next: MiddlewareNext) -> Result<Response, Error> {
        // Get state before request to increase request counter.
        // Extra brackets to release the lock while continuing the chain.
        {
            let mut state = self.0.lock().unwrap();

            state.request_count += 1;
        } // release lock

        // Continue the middleware chain
        let response = next.handle(request)?;

        // Get state after response to increase byte count.
        // Extra brackets not necessary, but there for symmetry with first lock.
        {
            let mut state = self.0.lock().unwrap();

            let len = response
                .header("Content-Length")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap();

            state.total_bytes += len;
        } // release lock

        Ok(response)
    }
}
