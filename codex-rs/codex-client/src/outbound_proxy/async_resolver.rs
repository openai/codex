use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;

use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::sync::watch;

use super::RouteFailureClass;
use super::SystemProxyRouteDecision;
use super::cached_system_proxy_decision;
use super::resolve_system_proxy_for_url;
use super::system_proxy_cache_key;

const MAX_CONCURRENT_SYSTEM_PROXY_RESOLUTIONS: usize = 2;

type PlatformResolver = fn(&str) -> SystemProxyRouteDecision;
type ResolutionSender = watch::Sender<Option<SystemProxyRouteDecision>>;

static ASYNC_SYSTEM_PROXY_RESOLVER: OnceLock<AsyncSystemProxyResolver<PlatformResolver>> =
    OnceLock::new();

/// Resolves system proxy/PAC/WPAD routing without blocking an async runtime worker.
pub async fn resolve_system_proxy_for_url_async(request_url: &str) -> SystemProxyRouteDecision {
    if let Some(decision) = cached_system_proxy_decision(request_url) {
        return decision.into();
    }

    ASYNC_SYSTEM_PROXY_RESOLVER
        .get_or_init(|| {
            AsyncSystemProxyResolver::new(
                MAX_CONCURRENT_SYSTEM_PROXY_RESOLUTIONS,
                resolve_system_proxy_for_url as PlatformResolver,
            )
        })
        .resolve(request_url)
        .await
}

struct AsyncSystemProxyResolver<R> {
    permits: Arc<Semaphore>,
    in_flight: Arc<Mutex<HashMap<String, ResolutionSender>>>,
    resolve: Arc<R>,
}

impl<R> AsyncSystemProxyResolver<R>
where
    R: Fn(&str) -> SystemProxyRouteDecision + Send + Sync + 'static,
{
    fn new(max_concurrent_resolutions: usize, resolve: R) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(max_concurrent_resolutions)),
            in_flight: Arc::new(Mutex::new(HashMap::new())),
            resolve: Arc::new(resolve),
        }
    }

    async fn resolve(&self, request_url: &str) -> SystemProxyRouteDecision {
        let cache_key = system_proxy_cache_key(request_url);
        let existing = self
            .in_flight
            .lock()
            .await
            .get(&cache_key)
            .map(ResolutionSender::subscribe);

        let mut receiver = if let Some(receiver) = existing {
            receiver
        } else {
            let Ok(permit) = Arc::clone(&self.permits).acquire_owned().await else {
                return resolver_error();
            };
            let (receiver, leader) = {
                let mut in_flight = self.in_flight.lock().await;
                if let Some(sender) = in_flight.get(&cache_key) {
                    (sender.subscribe(), None)
                } else {
                    let (sender, receiver) = watch::channel(None);
                    in_flight.insert(cache_key.clone(), sender.clone());
                    (receiver, Some((sender, permit)))
                }
            };

            if let Some((sender, permit)) = leader {
                let in_flight = Arc::clone(&self.in_flight);
                let resolve = Arc::clone(&self.resolve);
                let request_url = request_url.to_string();
                tokio::spawn(async move {
                    let decision = match tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        resolve(&request_url)
                    })
                    .await
                    {
                        Ok(decision) => decision,
                        Err(_) => resolver_error(),
                    };
                    let _ = sender.send_replace(Some(decision));
                    let _ = in_flight.lock().await.remove(&cache_key);
                });
            }

            receiver
        };

        loop {
            if let Some(decision) = receiver.borrow().clone() {
                return decision;
            }
            if receiver.changed().await.is_err() {
                return resolver_error();
            }
        }
    }
}

fn resolver_error() -> SystemProxyRouteDecision {
    SystemProxyRouteDecision::Unavailable {
        failure: RouteFailureClass::ResolverError,
    }
}

#[cfg(test)]
#[path = "async_resolver_tests.rs"]
mod tests;
