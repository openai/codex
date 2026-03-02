use crate::config_requirements::ConfigRequirementsToml;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use futures::future::Shared;
use std::fmt;
use std::future::Future;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct CloudRequirementsLoadError {
    message: String,
}

impl CloudRequirementsLoadError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone)]
pub struct CloudRequirementsLoader {
    fut: Shared<
        BoxFuture<'static, Result<Option<ConfigRequirementsToml>, CloudRequirementsLoadError>>,
    >,
}

impl CloudRequirementsLoader {
    pub fn new<F>(fut: F) -> Self
    where
        F: Future<Output = Result<Option<ConfigRequirementsToml>, CloudRequirementsLoadError>>
            + Send
            + 'static,
    {
        Self {
            fut: fut.boxed().shared(),
        }
    }

    pub async fn get(&self) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsLoadError> {
        self.fut.clone().await
    }
}

impl fmt::Debug for CloudRequirementsLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloudRequirementsLoader").finish()
    }
}

impl Default for CloudRequirementsLoader {
    fn default() -> Self {
        Self::new(async { Ok(None) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn shared_future_runs_once() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let loader = CloudRequirementsLoader::new(async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(Some(ConfigRequirementsToml::default()))
        });

        let (first, second) = tokio::join!(loader.get(), loader.get());
        assert_eq!(first, second);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
