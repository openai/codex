//! Codex-specific process spawning helpers.
//!
//! Spawned children must be explicitly joined before their managed handle is
//! dropped. Debug builds enforce this with a drop bomb, while release builds
//! log an error.

pub mod sync;
pub mod tokio;

const UNJOINED_CHILD_MESSAGE: &str = "managed child process dropped without being joined";

#[derive(Debug)]
struct DropBomb {
    armed: bool,
}

impl DropBomb {
    fn new() -> Self {
        Self { armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    #[cfg(test)]
    fn is_armed(&self) -> bool {
        self.armed
    }
}

impl Drop for DropBomb {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }

        if cfg!(debug_assertions) && !std::thread::panicking() {
            panic!("{UNJOINED_CHILD_MESSAGE}");
        }

        tracing::error!("{UNJOINED_CHILD_MESSAGE}");
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

#[cfg(test)]
mod test_support;
