use crate::proxy::reserve_windows_managed_listeners;
use crate::proxy::reserve_windows_managed_socks_listener;
use crate::proxy::windows_managed_loopback_addr;
use crate::windows_tcp_attribution::restricting_sids_for_tcp_connection;
use anyhow::Context;
use anyhow::Result;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::service::BoxService;
use rama_net::stream::Socket;
use rama_tcp::TcpStream;
use rama_tcp::server::TcpListener;
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
#[cfg(test)]
use std::sync::Weak;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tracing::info;

pub(crate) type WindowsRouteService = BoxService<TcpStream, (), BoxError>;

// Production keeps the listeners alive for the process lifetime so their ports remain stable even
// when no routes are registered. Crate tests use independent Tokio runtimes and requested ports, so
// they retain only a weak reference and can tear each ingress down between tests.
#[cfg(not(test))]
static SHARED_INGRESS: LazyLock<Mutex<Option<Arc<WindowsProxyIngress>>>> =
    LazyLock::new(|| Mutex::new(None));
#[cfg(test)]
static SHARED_INGRESS: LazyLock<Mutex<Weak<WindowsProxyIngress>>> =
    LazyLock::new(|| Mutex::new(Weak::new()));

#[derive(Clone)]
struct RouteServices {
    http: WindowsRouteService,
    socks: Option<WindowsRouteService>,
}

type RouteRegistry = Arc<Mutex<HashMap<String, Arc<RouteServices>>>>;

#[derive(Clone, Copy)]
enum ProxyProtocol {
    Http,
    Socks,
}

#[derive(Clone)]
struct IngressDispatcher {
    routes: RouteRegistry,
    protocol: ProxyProtocol,
}

impl Service<TcpStream> for IngressDispatcher {
    type Output = ();
    type Error = BoxError;

    async fn serve(&self, stream: TcpStream) -> Result<(), BoxError> {
        let local_addr = stream.local_addr()?;
        let peer_addr = stream.peer_addr()?;
        let restricting_sids = restricting_sids_for_tcp_connection(local_addr, peer_addr)?;
        let route = {
            let routes = self
                .routes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            registered_route_for_sids(&routes, &restricting_sids)?
        };
        let service = match self.protocol {
            ProxyProtocol::Http => route.http.clone(),
            ProxyProtocol::Socks => route.socks.clone().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "network proxy route does not enable SOCKS5",
                )
            })?,
        };
        service.serve(stream).await
    }
}

pub(crate) struct WindowsProxyIngress {
    http_addr: SocketAddr,
    routes: RouteRegistry,
    runtime: Handle,
    http_task: JoinHandle<()>,
    socks: Mutex<SocksListenerState>,
}

struct SocksListenerState {
    addr: SocketAddr,
    task: Option<JoinHandle<()>>,
}

impl WindowsProxyIngress {
    pub(crate) fn shared(
        requested_http_addr: SocketAddr,
        requested_socks_addr: SocketAddr,
        reserve_socks_listener: bool,
    ) -> Result<Arc<Self>> {
        let requested_http_addr = windows_managed_loopback_addr(requested_http_addr);
        let requested_socks_addr = windows_managed_loopback_addr(requested_socks_addr);
        let mut shared = SHARED_INGRESS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        #[cfg(not(test))]
        if let Some(ingress) = shared.as_ref()
            && ingress.is_running()
        {
            if reserve_socks_listener {
                ingress.ensure_socks_listener(requested_socks_addr)?;
            }
            return Ok(Arc::clone(ingress));
        }
        #[cfg(not(test))]
        shared.take();
        #[cfg(test)]
        if let Some(ingress) = shared.upgrade()
            && ingress.is_running()
        {
            if reserve_socks_listener {
                ingress.ensure_socks_listener(requested_socks_addr)?;
            }
            return Ok(ingress);
        }

        let listeners = reserve_windows_managed_listeners(
            requested_http_addr,
            requested_socks_addr,
            reserve_socks_listener,
        )
        .context("reserve shared managed Windows proxy ingress")?;
        let http_addr = listeners.http_addr()?;
        let socks_addr = listeners.socks_addr(requested_socks_addr)?;
        let (http_listener, socks_listener) = listeners.into_listeners();
        let http_listener =
            TcpListener::try_from(http_listener).context("convert shared HTTP ingress listener")?;
        let socks_listener = socks_listener
            .map(TcpListener::try_from)
            .transpose()
            .context("convert shared SOCKS5 ingress listener")?;
        let runtime =
            Handle::try_current().context("start shared managed Windows proxy ingress")?;
        let routes = Arc::new(Mutex::new(HashMap::new()));
        let http_task = runtime.spawn(run_listener(
            http_listener,
            IngressDispatcher {
                routes: Arc::clone(&routes),
                protocol: ProxyProtocol::Http,
            },
            "HTTP",
            http_addr,
        ));
        let socks_task = socks_listener
            .map(|listener| spawn_socks_listener(&runtime, &routes, listener, socks_addr));
        let ingress = Arc::new(Self {
            http_addr,
            routes,
            runtime,
            http_task,
            socks: Mutex::new(SocksListenerState {
                addr: socks_addr,
                task: socks_task,
            }),
        });
        #[cfg(not(test))]
        {
            *shared = Some(Arc::clone(&ingress));
        }
        #[cfg(test)]
        {
            *shared = Arc::downgrade(&ingress);
        }
        Ok(ingress)
    }

    pub(crate) fn http_addr(&self) -> SocketAddr {
        self.http_addr
    }

    pub(crate) fn socks_addr(&self) -> SocketAddr {
        self.socks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .addr
    }

    pub(crate) fn active_socks_addr(&self) -> Option<SocketAddr> {
        let socks = self
            .socks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        socks
            .task
            .as_ref()
            .filter(|task| !task.is_finished())
            .map(|_| socks.addr)
    }

    pub(crate) fn register_route(
        self: &Arc<Self>,
        http: WindowsRouteService,
        socks: Option<WindowsRouteService>,
    ) -> WindowsProxyRoute {
        let services = Arc::new(RouteServices { http, socks });
        let mut routes = self
            .routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sid = loop {
            let sid = random_restricting_sid();
            if !routes.contains_key(&sid) {
                break sid;
            }
        };
        routes.insert(sid.clone(), Arc::clone(&services));
        WindowsProxyRoute {
            sid,
            services,
            ingress: Arc::clone(self),
        }
    }

    fn is_running(&self) -> bool {
        !self.http_task.is_finished()
            && self
                .socks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .task
                .as_ref()
                .is_none_or(|task| !task.is_finished())
    }

    fn ensure_socks_listener(&self, requested_addr: SocketAddr) -> Result<()> {
        let mut socks = self
            .socks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(task) = socks.task.as_ref() {
            anyhow::ensure!(
                !task.is_finished(),
                "shared managed Windows SOCKS5 ingress stopped"
            );
            return Ok(());
        }

        let listener = reserve_windows_managed_socks_listener(requested_addr)
            .context("reserve shared managed Windows SOCKS5 ingress")?;
        let addr = listener
            .local_addr()
            .context("read shared managed Windows SOCKS5 ingress address")?;
        let listener = {
            let _runtime = self.runtime.enter();
            TcpListener::try_from(listener)
        }
        .context("convert shared SOCKS5 ingress listener")?;
        let task = spawn_socks_listener(&self.runtime, &self.routes, listener, addr);
        socks.addr = addr;
        socks.task = Some(task);
        Ok(())
    }
}

impl Drop for WindowsProxyIngress {
    fn drop(&mut self) {
        self.http_task.abort();
        let socks = self
            .socks
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(socks_task) = socks.task.as_ref() {
            socks_task.abort();
        }
    }
}

fn spawn_socks_listener(
    runtime: &Handle,
    routes: &RouteRegistry,
    listener: TcpListener,
    addr: SocketAddr,
) -> JoinHandle<()> {
    runtime.spawn(run_listener(
        listener,
        IngressDispatcher {
            routes: Arc::clone(routes),
            protocol: ProxyProtocol::Socks,
        },
        "SOCKS5",
        addr,
    ))
}

pub(crate) struct WindowsProxyRoute {
    sid: String,
    services: Arc<RouteServices>,
    ingress: Arc<WindowsProxyIngress>,
}

impl WindowsProxyRoute {
    pub(crate) fn sid(&self) -> &str {
        &self.sid
    }
}

impl Drop for WindowsProxyRoute {
    fn drop(&mut self) {
        let mut routes = self
            .ingress
            .routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if routes
            .get(&self.sid)
            .is_some_and(|services| Arc::ptr_eq(services, &self.services))
        {
            routes.remove(&self.sid);
        }
    }
}

async fn run_listener(
    listener: TcpListener,
    dispatcher: IngressDispatcher,
    protocol: &'static str,
    addr: SocketAddr,
) {
    info!("shared managed Windows {protocol} proxy ingress listening on {addr}");
    listener.serve(dispatcher).await;
}

fn registered_route_for_sids(
    routes: &HashMap<String, Arc<RouteServices>>,
    restricting_sids: &[String],
) -> io::Result<Arc<RouteServices>> {
    let mut matching_routes = restricting_sids
        .iter()
        .filter_map(|sid| routes.get(sid).cloned());
    let route = matching_routes.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "proxy client token has no registered network proxy route SID",
        )
    })?;
    if matching_routes.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "proxy client token has multiple registered network proxy route SIDs",
        ));
    }
    Ok(route)
}

fn random_restricting_sid() -> String {
    let a = rand::random::<u32>();
    let b = rand::random::<u32>();
    let c = rand::random::<u32>();
    let d = rand::random::<u32>();
    format!("S-1-5-21-{a}-{b}-{c}-{d}")
}

#[cfg(test)]
#[path = "windows_proxy_ingress_tests.rs"]
mod tests;
