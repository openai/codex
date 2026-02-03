use crate::config_loader::ConfigRequirementsToml;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use futures::future::Shared;
use std::fmt;
use std::future::Future;

#[derive(Clone)]
pub struct CloudRequirementsLoader {
    // TODO(gt): This should return a Result once we can fail-closed.
    fut: Shared<BoxFuture<'static, Option<ConfigRequirementsToml>>>,
    warning_fut: Option<Shared<BoxFuture<'static, Option<String>>>>,
}

impl CloudRequirementsLoader {
    pub fn new<F>(fut: F) -> Self
    where
        F: Future<Output = Option<ConfigRequirementsToml>> + Send + 'static,
    {
        Self {
            fut: fut.boxed().shared(),
            warning_fut: None,
        }
    }

    pub fn new_with_warning<F, W>(fut: F, warning_fut: W) -> Self
    where
        F: Future<Output = Option<ConfigRequirementsToml>> + Send + 'static,
        W: Future<Output = Option<String>> + Send + 'static,
    {
        Self {
            fut: fut.boxed().shared(),
            warning_fut: Some(warning_fut.boxed().shared()),
        }
    }

    pub async fn get(&self) -> Option<ConfigRequirementsToml> {
        self.fut.clone().await
    }

    pub async fn warning(&self) -> Option<String> {
        match &self.warning_fut {
            Some(warning_fut) => warning_fut.clone().await,
            None => None,
        }
    }
}

impl fmt::Debug for CloudRequirementsLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloudRequirementsLoader").finish()
    }
}

impl Default for CloudRequirementsLoader {
    fn default() -> Self {
        Self::new(async { None })
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
            Some(ConfigRequirementsToml::default())
        });

        let (first, second) = tokio::join!(loader.get(), loader.get());
        assert_eq!(first, second);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn shared_warning_future_runs_once() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let loader = CloudRequirementsLoader::new_with_warning(async { None }, async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Some("warning".to_string())
        });

        let (first, second) = tokio::join!(loader.warning(), loader.warning());
        assert_eq!(first, second);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
