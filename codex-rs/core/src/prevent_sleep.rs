#[derive(Debug)]
pub(crate) struct PreventSleepGuard {
    #[cfg(target_os = "macos")]
    child: Option<std::process::Child>,
}

impl PreventSleepGuard {
    pub(crate) fn activate_if_supported(enabled: bool) -> Option<Self> {
        if !enabled {
            return None;
        }

        #[cfg(target_os = "macos")]
        {
            Self::start_caffeinate()
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = enabled;
            return None;
        }
    }

    #[cfg(target_os = "macos")]
    fn start_caffeinate() -> Option<Self> {
        use std::path::Path;
        use std::process::Command;

        const CAFFEINATE_PATH: &str = "/usr/bin/caffeinate";
        let caffeinate_path = Path::new(CAFFEINATE_PATH);
        if !caffeinate_path.exists() {
            tracing::debug!(
                "prevent sleep requested but caffeinate not found at {CAFFEINATE_PATH}"
            );
            return None;
        }

        match Command::new(caffeinate_path).arg("-i").spawn() {
            Ok(child) => Some(Self { child: Some(child) }),
            Err(err) => {
                tracing::warn!("Failed to launch caffeinate: {err}");
                None
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for PreventSleepGuard {
    fn drop(&mut self) {
        use std::io::ErrorKind;

        if let Some(mut child) = self.child.take() {
            match child.kill() {
                Ok(_) => {}
                Err(err) if err.kind() == ErrorKind::InvalidInput => {}
                Err(err) => {
                    tracing::debug!("Failed to terminate caffeinate process: {err}");
                }
            }

            if let Err(err) = child.wait() {
                tracing::debug!("Failed to reap caffeinate process: {err}");
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl Drop for PreventSleepGuard {
    fn drop(&mut self) {}
}
