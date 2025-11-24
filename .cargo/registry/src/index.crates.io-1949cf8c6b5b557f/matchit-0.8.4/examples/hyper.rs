use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response};
use tower::util::BoxCloneService;
use tower::Service as _;

// GET /
async fn index(_req: Request<Body>) -> hyper::Result<Response<Body>> {
    Ok(Response::new(Body::from("Hello, world!")))
}

// GET /blog
async fn blog(_req: Request<Body>) -> hyper::Result<Response<Body>> {
    Ok(Response::new(Body::from("...")))
}

// 404 handler
async fn not_found(_req: Request<Body>) -> hyper::Result<Response<Body>> {
    Ok(Response::builder().status(404).body(Body::empty()).unwrap())
}

// We can use `BoxCloneService` to erase the type of each handler service.
//
// We still need a `Mutex` around each service because `BoxCloneService` doesn't
// require the service to implement `Sync`.
type Service = Mutex<BoxCloneService<Request<Body>, Response<Body>, hyper::Error>>;

// We use a `HashMap` to hold a `Router` for each HTTP method. This allows us
// to register the same route for multiple methods.
type Router = HashMap<Method, matchit::Router<Service>>;

async fn route(router: Arc<Router>, req: Request<Body>) -> hyper::Result<Response<Body>> {
    // find the subrouter for this request method
    let router = match router.get(req.method()) {
        Some(router) => router,
        // if there are no routes for this method, respond with 405 Method Not Allowed
        None => return Ok(Response::builder().status(405).body(Body::empty()).unwrap()),
    };

    // find the service for this request path
    match router.at(req.uri().path()) {
        Ok(found) => {
            // lock the service for a very short time, just to clone the service
            let mut service = found.value.lock().unwrap().clone();
            service.call(req).await
        }
        // if we there is no matching service, call the 404 handler
        Err(_) => not_found(req).await,
    }
}

#[tokio::main]
async fn main() {
    // Create a router and register our routes.
    let mut router = Router::new();

    // GET / => `index`
    router
        .entry(Method::GET)
        .or_default()
        .insert("/", BoxCloneService::new(service_fn(index)).into())
        .unwrap();

    // GET /blog => `blog`
    router
        .entry(Method::GET)
        .or_default()
        .insert("/blog", BoxCloneService::new(service_fn(blog)).into())
        .unwrap();

    // boilerplate for the hyper service
    let router = Arc::new(router);
    let make_service = make_service_fn(|_| {
        let router = router.clone();
        async { Ok::<_, Infallible>(service_fn(move |request| route(router.clone(), request))) }
    });

    // run the server
    Server::bind(&([127, 0, 0, 1], 3000).into())
        .serve(make_service)
        .await
        .unwrap()
}
