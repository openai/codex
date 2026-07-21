use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use pretty_assertions::assert_eq;

use super::*;
use crate::OutboundProxyPolicy;

#[test]
fn request_builder_debug_redacts_url_secrets() {
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
    );
    let request = pool.get(
        "https://username:password@private.example/secret-path?sig=query-secret#fragment-secret",
    );

    assert_eq!(
        format!("{request:?}"),
        concat!(
            "RouteAwareRequestBuilder { pool: RouteAwareClientPool { ",
            "http_client_factory: HttpClientFactory { outbound_proxy_policy: ReqwestDefault }, ",
            "route_class: Api, request_logging: Enabled, .. }, method: Some(GET), ",
            "url: Some(\"<redacted>\"), .. }"
        )
    );
}

#[tokio::test]
async fn forwards_exact_urls_and_reuses_clients_by_resolved_route() {
    let builder_count = Arc::new(AtomicUsize::new(0));
    let observed_builder_count = Arc::clone(&builder_count);
    let pool = RouteAwareClientPool::with_builder_factory(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
        move || {
            observed_builder_count.fetch_add(1, Ordering::SeqCst);
            reqwest::Client::builder()
        },
        PoolRequestLogging::Enabled,
    );

    let direct_url = "https://example.com/first?target=direct";
    let same_route_url = "https://example.com/second?target=direct%202";
    let proxy_url = "https://example.com/third?target=proxy";
    let resolver = FakeRouteResolver::new(HashMap::from([
        (direct_url.to_string(), OutboundProxyRoute::Direct),
        (same_route_url.to_string(), OutboundProxyRoute::Direct),
        (
            proxy_url.to_string(),
            OutboundProxyRoute::Proxy {
                url: "http://proxy.example".to_string(),
                no_proxy: None,
            },
        ),
    ]));

    resolve_with(&pool, &resolver, direct_url)
        .await
        .expect("first client should build");
    resolve_with(&pool, &resolver, same_route_url)
        .await
        .expect("second client should reuse the route");
    resolve_with(&pool, &resolver, proxy_url)
        .await
        .expect("proxy client should build separately");

    assert_eq!(builder_count.load(Ordering::SeqCst), 2);
    assert_eq!(
        resolver.observed_urls(),
        vec![
            direct_url.to_string(),
            same_route_url.to_string(),
            proxy_url.to_string(),
        ]
    );
}

#[tokio::test]
async fn reqwest_default_route_preserves_transport_redirects() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("redirect listener should bind");
    let address = listener
        .local_addr()
        .expect("redirect listener should have an address");
    listener
        .set_nonblocking(true)
        .expect("redirect listener should become nonblocking");
    let server = std::thread::spawn(move || {
        let mut request_lines = Vec::new();
        for response in [
            "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
        ] {
            let deadline = Instant::now() + Duration::from_secs(2);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "redirect server should receive the next request"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("redirect server should accept: {error}"),
                }
            };
            let mut buffer = [0_u8; 1024];
            let size = stream
                .read(&mut buffer)
                .expect("redirect server should read request");
            let request = String::from_utf8_lossy(&buffer[..size]);
            request_lines.push(
                request
                    .lines()
                    .next()
                    .expect("request should have a request line")
                    .to_string(),
            );
            stream
                .write_all(response.as_bytes())
                .expect("redirect server should write response");
        }
        request_lines
    });
    let pool = RouteAwareClientPool::with_builder_factory(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
        || reqwest::Client::builder().no_proxy(),
        PoolRequestLogging::Enabled,
    );
    let initial_url = format!("http://{address}/start");

    let response = pool
        .get(initial_url)
        .send()
        .await
        .expect("default-routed request should follow redirect");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.url().as_str(), format!("http://{address}/final"));
    assert_eq!(
        server.join().expect("redirect server should finish"),
        vec![
            "GET /start HTTP/1.1".to_string(),
            "GET /final HTTP/1.1".to_string(),
        ]
    );
}

#[tokio::test]
async fn bounds_cached_routes_and_rebuilds_an_evicted_route() {
    let builder_count = Arc::new(AtomicUsize::new(0));
    let observed_builder_count = Arc::clone(&builder_count);
    let pool = RouteAwareClientPool::with_builder_factory(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
        move || {
            observed_builder_count.fetch_add(1, Ordering::SeqCst);
            reqwest::Client::builder()
        },
        PoolRequestLogging::Enabled,
    );
    let routes = (0..=MAX_CACHED_ROUTES)
        .map(|index| {
            (
                format!("https://target-{index}.example"),
                OutboundProxyRoute::Proxy {
                    url: format!("http://proxy-{index}.example"),
                    no_proxy: None,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let resolver = FakeRouteResolver::new(routes.clone());

    for request_url in routes.keys() {
        resolve_with(&pool, &resolver, request_url)
            .await
            .expect("client should build");
    }
    let evicted_route = {
        let clients = pool.clients.lock().expect("client cache lock");
        assert_eq!(clients.len(), MAX_CACHED_ROUTES);
        routes
            .iter()
            .find(|(_, route)| !clients.contains_key(*route))
            .map(|(request_url, _)| request_url.clone())
            .expect("one route should have been evicted")
    };

    resolve_with(&pool, &resolver, &evicted_route)
        .await
        .expect("evicted client should rebuild");

    assert_eq!(builder_count.load(Ordering::SeqCst), MAX_CACHED_ROUTES + 2);
    assert_eq!(
        pool.clients.lock().expect("client cache lock").len(),
        MAX_CACHED_ROUTES
    );
}

#[derive(Clone)]
struct FakeRouteResolver {
    routes: Arc<HashMap<String, OutboundProxyRoute>>,
    observed_urls: Arc<Mutex<Vec<String>>>,
}

impl FakeRouteResolver {
    fn new(routes: HashMap<String, OutboundProxyRoute>) -> Self {
        Self {
            routes: Arc::new(routes),
            observed_urls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn resolve(&self, request_url: String) -> io::Result<OutboundProxyRoute> {
        self.observed_urls
            .lock()
            .expect("observed URL lock")
            .push(request_url.clone());
        self.routes
            .get(&request_url)
            .cloned()
            .ok_or_else(|| io::Error::other(format!("no route for {request_url}")))
    }

    fn observed_urls(&self) -> Vec<String> {
        self.observed_urls
            .lock()
            .expect("observed URL lock")
            .clone()
    }
}

async fn resolve_with(
    pool: &RouteAwareClientPool,
    resolver: &FakeRouteResolver,
    request_url: &str,
) -> Result<HttpClient, RouteAwareClientPoolError> {
    let resolver = resolver.clone();
    pool.client_for_url_with_resolver(request_url, move |request_url| async move {
        resolver.resolve(request_url).await
    })
    .await
}
