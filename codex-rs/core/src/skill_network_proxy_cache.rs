#[cfg(unix)]
use crate::config::SkillNetworkProxyKey;
#[cfg(unix)]
use crate::config::StartedNetworkProxy;
#[cfg(unix)]
use std::collections::HashMap;
#[cfg(unix)]
use std::future::Future;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use tokio::sync::Mutex;

#[derive(Default)]
pub(crate) struct SkillNetworkProxyCache {
    #[cfg(unix)]
    proxies: Mutex<HashMap<SkillNetworkProxyKey, Arc<StartedNetworkProxy>>>,
}

impl SkillNetworkProxyCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[cfg(unix)]
    pub(crate) async fn get_or_start<F, Fut>(
        &self,
        key: SkillNetworkProxyKey,
        start: F,
    ) -> anyhow::Result<Arc<StartedNetworkProxy>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<StartedNetworkProxy>>,
    {
        let mut proxies = self.proxies.lock().await;
        if let Some(proxy) = proxies.get(&key) {
            return Ok(Arc::clone(proxy));
        }

        let proxy = Arc::new(start().await?);
        proxies.insert(key, Arc::clone(&proxy));
        Ok(proxy)
    }
}
