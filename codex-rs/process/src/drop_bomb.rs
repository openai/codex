#[derive(Debug)]
pub(crate) struct DropBomb {
    armed: bool,
}

impl DropBomb {
    pub(crate) fn new() -> Self {
        Self { armed: true }
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }

    #[cfg(test)]
    pub(crate) fn is_armed(&self) -> bool {
        self.armed
    }
}

impl Drop for DropBomb {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }

        const UNJOINED_CHILD_MESSAGE: &str = "managed child process dropped without being joined";

        if cfg!(debug_assertions) && !std::thread::panicking() {
            panic!("{UNJOINED_CHILD_MESSAGE}");
        }

        tracing::error!("{UNJOINED_CHILD_MESSAGE}");
    }
}

#[cfg(test)]
#[path = "drop_bomb_tests.rs"]
mod tests;
