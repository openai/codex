use crate::config_loader::ConfigRequirementsToml;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use futures::future::Shared;
use std::fmt;
use std::future::Future;
use std::io;
use std::sync::Arc;

#[derive(Clone)]
pub struct CloudRequirementsLoader {
    fut: Shared<BoxFuture<'static, Arc<io::Result<Option<ConfigRequirementsToml>>>>>,
}

impl CloudRequirementsLoader {
    pub fn new<F>(fut: F) -> Self
    where
        F: Future<Output = io::Result<Option<ConfigRequirementsToml>>> + Send + 'static,
    {
        Self {
            fut: fut.map(Arc::new).boxed().shared(),
        }
    }

    pub async fn get(&self) -> io::Result<Option<ConfigRequirementsToml>> {
        match self.fut.clone().await.as_ref() {
            Ok(requirements) => Ok(requirements.clone()),
            Err(err) => Err(io::Error::new(err.kind(), err.to_string())),
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
        assert_eq!(first.as_ref().ok(), second.as_ref().ok());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
