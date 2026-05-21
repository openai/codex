use crate::CloudConfigFragment;
use crate::CloudRequirementsFragment;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use futures::future::Shared;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::future::Future;
use thiserror::Error;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudConfigBundle {
    pub config_toml: CloudConfigTomlBundle,
    pub requirements_toml: CloudRequirementsTomlBundle,
}

impl CloudConfigBundle {
    pub fn is_empty(&self) -> bool {
        self.config_toml.enterprise_managed.is_empty()
            && self.requirements_toml.enterprise_managed.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudConfigTomlBundle {
    pub enterprise_managed: Vec<CloudConfigFragment>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudRequirementsTomlBundle {
    pub enterprise_managed: Vec<CloudRequirementsFragment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloudConfigBundleLoadErrorCode {
    Auth,
    Timeout,
    RequestFailed,
    Internal,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct CloudConfigBundleLoadError {
    code: CloudConfigBundleLoadErrorCode,
    message: String,
    status_code: Option<u16>,
}

impl CloudConfigBundleLoadError {
    pub fn new(
        code: CloudConfigBundleLoadErrorCode,
        status_code: Option<u16>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            status_code,
        }
    }

    pub fn code(&self) -> CloudConfigBundleLoadErrorCode {
        self.code
    }

    pub fn status_code(&self) -> Option<u16> {
        self.status_code
    }
}

#[derive(Clone)]
pub struct CloudConfigBundleLoader {
    fut: Shared<BoxFuture<'static, Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>>>,
}

impl CloudConfigBundleLoader {
    pub fn new<F>(fut: F) -> Self
    where
        F: Future<Output = Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>>
            + Send
            + 'static,
    {
        Self {
            fut: fut.boxed().shared(),
        }
    }

    pub async fn get(&self) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        self.fut.clone().await
    }
}

impl fmt::Debug for CloudConfigBundleLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloudConfigBundleLoader").finish()
    }
}

impl Default for CloudConfigBundleLoader {
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
        let loader = CloudConfigBundleLoader::new(async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(Some(CloudConfigBundle::default()))
        });

        let (first, second) = tokio::join!(loader.get(), loader.get());
        assert_eq!(first, second);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
