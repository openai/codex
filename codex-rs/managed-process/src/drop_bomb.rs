use std::ffi::OsString;
use std::panic::Location;

#[derive(Debug)]
pub(crate) struct DebugDropBomb {
    armed: bool,
    program: OsString,
    spawn_location: &'static Location<'static>,
}

impl DebugDropBomb {
    pub(crate) fn new(program: OsString, spawn_location: &'static Location<'static>) -> Self {
        Self {
            armed: true,
            program,
            spawn_location,
        }
    }

    pub(crate) fn defuse(&mut self) {
        self.armed = false;
    }
}

impl Drop for DebugDropBomb {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }

        #[cfg(debug_assertions)]
        {
            panic!(
                "managed Tokio child for {:?} spawned at {} dropped without explicit teardown",
                self.program, self.spawn_location
            );
        }

        #[cfg(not(debug_assertions))]
        tracing::error!(
            program = ?self.program,
            spawn_location = %self.spawn_location,
            "managed Tokio child dropped without explicit teardown"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defused_bomb_drops() {
        let mut bomb = DebugDropBomb::new("test".into(), Location::caller());
        bomb.defuse();
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "dropped without explicit teardown")]
    fn armed_bomb_panics_in_debug() {
        drop(DebugDropBomb::new("test".into(), Location::caller()));
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn armed_bomb_does_not_panic_in_release() {
        let result = std::panic::catch_unwind(|| {
            drop(DebugDropBomb::new("test".into(), Location::caller()));
        });

        assert!(result.is_ok());
    }
}
