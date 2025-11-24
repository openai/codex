use crate::{
    extract::{nested_path::SetNestedPath, Request},
    handler::Handler,
};
use axum_core::response::IntoResponse;
use matchit::MatchError;
use std::{borrow::Cow, collections::HashMap, convert::Infallible, fmt, sync::Arc};
use tower_layer::Layer;
use tower_service::Service;

use super::{
    future::RouteFuture, not_found::NotFound, strip_prefix::StripPrefix, url_params, Endpoint,
    MethodRouter, Route, RouteId, FALLBACK_PARAM_PATH, NEST_TAIL_PARAM,
};

pub(super) struct PathRouter<S, const IS_FALLBACK: bool> {
    routes: HashMap<RouteId, Endpoint<S>>,
    node: Arc<Node>,
    prev_route_id: RouteId,
    v7_checks: bool,
}

impl<S> PathRouter<S, true>
where
    S: Clone + Send + Sync + 'static,
{
    pub(super) fn new_fallback() -> Self {
        let mut this = Self::default();
        this.set_fallback(Endpoint::Route(Route::new(NotFound)));
        this
    }

    pub(super) fn set_fallback(&mut self, endpoint: Endpoint<S>) {
        self.replace_endpoint("/", endpoint.clone());
        self.replace_endpoint(FALLBACK_PARAM_PATH, endpoint);
    }
}

fn validate_path(v7_checks: bool, path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("Paths must start with a `/`. Use \"/\" for root routes");
    } else if !path.starts_with('/') {
        return Err("Paths must start with a `/`");
    }

    if v7_checks {
        validate_v07_paths(path)?;
    }

    Ok(())
}

fn validate_v07_paths(path: &str) -> Result<(), &'static str> {
    path.split('/')
        .find_map(|segment| {
            if segment.starts_with(':') {
                Some(Err(
                    "Path segments must not start with `:`. For capture groups, use \
                `{capture}`. If you meant to literally match a segment starting with \
                a colon, call `without_v07_checks` on the router.",
                ))
            } else if segment.starts_with('*') {
                Some(Err(
                    "Path segments must not start with `*`. For wildcard capture, use \
                `{*wildcard}`. If you meant to literally match a segment starting with \
                an asterisk, call `without_v07_checks` on the router.",
                ))
            } else {
                None
            }
        })
        .unwrap_or(Ok(()))
}

impl<S, const IS_FALLBACK: bool> PathRouter<S, IS_FALLBACK>
where
    S: Clone + Send + Sync + 'static,
{
    pub(super) fn without_v07_checks(&mut self) {
        self.v7_checks = false;
    }

    pub(super) fn route(
        &mut self,
        path: &str,
        method_router: MethodRouter<S>,
    ) -> Result<(), Cow<'static, str>> {
        validate_path(self.v7_checks, path)?;

        let endpoint = if let Some((route_id, Endpoint::MethodRouter(prev_method_router))) = self
            .node
            .path_to_route_id
            .get(path)
            .and_then(|route_id| self.routes.get(route_id).map(|svc| (*route_id, svc)))
        {
            // if we're adding a new `MethodRouter` to a route that already has one just
            // merge them. This makes `.route("/", get(_)).route("/", post(_))` work
            let service = Endpoint::MethodRouter(
                prev_method_router
                    .clone()
                    .merge_for_path(Some(path), method_router)?,
            );
            self.routes.insert(route_id, service);
            return Ok(());
        } else {
            Endpoint::MethodRouter(method_router)
        };

        let id = self.next_route_id();
        self.set_node(path, id)?;
        self.routes.insert(id, endpoint);

        Ok(())
    }

    pub(super) fn method_not_allowed_fallback<H, T>(&mut self, handler: H)
    where
        H: Handler<T, S>,
        T: 'static,
    {
        for (_, endpoint) in self.routes.iter_mut() {
            if let Endpoint::MethodRouter(rt) = endpoint {
                *rt = rt.clone().default_fallback(handler.clone());
            }
        }
    }

    pub(super) fn route_service<T>(
        &mut self,
        path: &str,
        service: T,
    ) -> Result<(), Cow<'static, str>>
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        self.route_endpoint(path, Endpoint::Route(Route::new(service)))
    }

    pub(super) fn route_endpoint(
        &mut self,
        path: &str,
        endpoint: Endpoint<S>,
    ) -> Result<(), Cow<'static, str>> {
        validate_path(self.v7_checks, path)?;

        let id = self.next_route_id();
        self.set_node(path, id)?;
        self.routes.insert(id, endpoint);

        Ok(())
    }

    fn set_node(&mut self, path: &str, id: RouteId) -> Result<(), String> {
        let node = Arc::make_mut(&mut self.node);

        node.insert(path, id)
            .map_err(|err| format!("Invalid route {path:?}: {err}"))
    }

    pub(super) fn merge(
        &mut self,
        other: PathRouter<S, IS_FALLBACK>,
    ) -> Result<(), Cow<'static, str>> {
        let PathRouter {
            routes,
            node,
            prev_route_id: _,
            v7_checks,
        } = other;

        // If either of the two did not allow paths starting with `:` or `*`, do not allow them for the merged router either.
        self.v7_checks |= v7_checks;

        for (id, route) in routes {
            let path = node
                .route_id_to_path
                .get(&id)
                .expect("no path for route id. This is a bug in axum. Please file an issue");

            if IS_FALLBACK && (&**path == "/" || &**path == FALLBACK_PARAM_PATH) {
                // when merging two routers it doesn't matter if you do `a.merge(b)` or
                // `b.merge(a)`. This must also be true for fallbacks.
                //
                // However all fallback routers will have routes for `/` and `/*` so when merging
                // we have to ignore the top level fallbacks on one side otherwise we get
                // conflicts.
                //
                // `Router::merge` makes sure that when merging fallbacks `other` always has the
                // fallback we want to keep. It panics if both routers have a custom fallback. Thus
                // it is always okay to ignore one fallback and `Router::merge` also makes sure the
                // one we can ignore is that of `self`.
                self.replace_endpoint(path, route);
            } else {
                match route {
                    Endpoint::MethodRouter(method_router) => self.route(path, method_router)?,
                    Endpoint::Route(route) => self.route_service(path, route)?,
                }
            }
        }

        Ok(())
    }

    pub(super) fn nest(
        &mut self,
        path_to_nest_at: &str,
        router: PathRouter<S, IS_FALLBACK>,
    ) -> Result<(), Cow<'static, str>> {
        let prefix = validate_nest_path(self.v7_checks, path_to_nest_at);

        let PathRouter {
            routes,
            node,
            prev_route_id: _,
            // Ignore the configuration of the nested router
            v7_checks: _,
        } = router;

        for (id, endpoint) in routes {
            let inner_path = node
                .route_id_to_path
                .get(&id)
                .expect("no path for route id. This is a bug in axum. Please file an issue");

            let path = path_for_nested_route(prefix, inner_path);

            let layer = (
                StripPrefix::layer(prefix),
                SetNestedPath::layer(path_to_nest_at),
            );
            match endpoint.layer(layer) {
                Endpoint::MethodRouter(method_router) => {
                    self.route(&path, method_router)?;
                }
                Endpoint::Route(route) => {
                    self.route_endpoint(&path, Endpoint::Route(route))?;
                }
            }
        }

        Ok(())
    }

    pub(super) fn nest_service<T>(
        &mut self,
        path_to_nest_at: &str,
        svc: T,
    ) -> Result<(), Cow<'static, str>>
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        let path = validate_nest_path(self.v7_checks, path_to_nest_at);
        let prefix = path;

        let path = if path.ends_with('/') {
            format!("{path}{{*{NEST_TAIL_PARAM}}}")
        } else {
            format!("{path}/{{*{NEST_TAIL_PARAM}}}")
        };

        let layer = (
            StripPrefix::layer(prefix),
            SetNestedPath::layer(path_to_nest_at),
        );
        let endpoint = Endpoint::Route(Route::new(layer.layer(svc)));

        self.route_endpoint(&path, endpoint.clone())?;

        // `/{*rest}` is not matched by `/` so we need to also register a router at the
        // prefix itself. Otherwise if you were to nest at `/foo` then `/foo` itself
        // wouldn't match, which it should
        self.route_endpoint(prefix, endpoint.clone())?;
        if !prefix.ends_with('/') {
            // same goes for `/foo/`, that should also match
            self.route_endpoint(&format!("{prefix}/"), endpoint)?;
        }

        Ok(())
    }

    pub(super) fn layer<L>(self, layer: L) -> PathRouter<S, IS_FALLBACK>
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        let routes = self
            .routes
            .into_iter()
            .map(|(id, endpoint)| {
                let route = endpoint.layer(layer.clone());
                (id, route)
            })
            .collect();

        PathRouter {
            routes,
            node: self.node,
            prev_route_id: self.prev_route_id,
            v7_checks: self.v7_checks,
        }
    }

    #[track_caller]
    pub(super) fn route_layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        if self.routes.is_empty() {
            panic!(
                "Adding a route_layer before any routes is a no-op. \
                 Add the routes you want the layer to apply to first."
            );
        }

        let routes = self
            .routes
            .into_iter()
            .map(|(id, endpoint)| {
                let route = endpoint.layer(layer.clone());
                (id, route)
            })
            .collect();

        PathRouter {
            routes,
            node: self.node,
            prev_route_id: self.prev_route_id,
            v7_checks: self.v7_checks,
        }
    }

    pub(super) fn has_routes(&self) -> bool {
        !self.routes.is_empty()
    }

    pub(super) fn with_state<S2>(self, state: S) -> PathRouter<S2, IS_FALLBACK> {
        let routes = self
            .routes
            .into_iter()
            .map(|(id, endpoint)| {
                let endpoint: Endpoint<S2> = match endpoint {
                    Endpoint::MethodRouter(method_router) => {
                        Endpoint::MethodRouter(method_router.with_state(state.clone()))
                    }
                    Endpoint::Route(route) => Endpoint::Route(route),
                };
                (id, endpoint)
            })
            .collect();

        PathRouter {
            routes,
            node: self.node,
            prev_route_id: self.prev_route_id,
            v7_checks: self.v7_checks,
        }
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn call_with_state(
        &self,
        #[cfg_attr(not(feature = "original-uri"), allow(unused_mut))] mut req: Request,
        state: S,
    ) -> Result<RouteFuture<Infallible>, (Request, S)> {
        #[cfg(feature = "original-uri")]
        {
            use crate::extract::OriginalUri;

            if req.extensions().get::<OriginalUri>().is_none() {
                let original_uri = OriginalUri(req.uri().clone());
                req.extensions_mut().insert(original_uri);
            }
        }

        let (mut parts, body) = req.into_parts();

        match self.node.at(parts.uri.path()) {
            Ok(match_) => {
                let id = *match_.value;

                if !IS_FALLBACK {
                    #[cfg(feature = "matched-path")]
                    crate::extract::matched_path::set_matched_path_for_request(
                        id,
                        &self.node.route_id_to_path,
                        &mut parts.extensions,
                    );
                }

                url_params::insert_url_params(&mut parts.extensions, match_.params);

                let endpoint = self
                    .routes
                    .get(&id)
                    .expect("no route for id. This is a bug in axum. Please file an issue");

                let req = Request::from_parts(parts, body);
                match endpoint {
                    Endpoint::MethodRouter(method_router) => {
                        Ok(method_router.call_with_state(req, state))
                    }
                    Endpoint::Route(route) => Ok(route.clone().call_owned(req)),
                }
            }
            // explicitly handle all variants in case matchit adds
            // new ones we need to handle differently
            Err(MatchError::NotFound) => Err((Request::from_parts(parts, body), state)),
        }
    }

    pub(super) fn replace_endpoint(&mut self, path: &str, endpoint: Endpoint<S>) {
        match self.node.at(path) {
            Ok(match_) => {
                let id = *match_.value;
                self.routes.insert(id, endpoint);
            }
            Err(_) => self
                .route_endpoint(path, endpoint)
                .expect("path wasn't matched so endpoint shouldn't exist"),
        }
    }

    fn next_route_id(&mut self) -> RouteId {
        let next_id = self
            .prev_route_id
            .0
            .checked_add(1)
            .expect("Over `u32::MAX` routes created. If you need this, please file an issue.");
        self.prev_route_id = RouteId(next_id);
        self.prev_route_id
    }
}

impl<S, const IS_FALLBACK: bool> Default for PathRouter<S, IS_FALLBACK> {
    fn default() -> Self {
        Self {
            routes: Default::default(),
            node: Default::default(),
            prev_route_id: RouteId(0),
            v7_checks: true,
        }
    }
}

impl<S, const IS_FALLBACK: bool> fmt::Debug for PathRouter<S, IS_FALLBACK> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PathRouter")
            .field("routes", &self.routes)
            .field("node", &self.node)
            .finish()
    }
}

impl<S, const IS_FALLBACK: bool> Clone for PathRouter<S, IS_FALLBACK> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            node: self.node.clone(),
            prev_route_id: self.prev_route_id,
            v7_checks: self.v7_checks,
        }
    }
}

/// Wrapper around `matchit::Router` that supports merging two `Router`s.
#[derive(Clone, Default)]
struct Node {
    inner: matchit::Router<RouteId>,
    route_id_to_path: HashMap<RouteId, Arc<str>>,
    path_to_route_id: HashMap<Arc<str>, RouteId>,
}

impl Node {
    fn insert(
        &mut self,
        path: impl Into<String>,
        val: RouteId,
    ) -> Result<(), matchit::InsertError> {
        let path = path.into();

        self.inner.insert(&path, val)?;

        let shared_path: Arc<str> = path.into();
        self.route_id_to_path.insert(val, shared_path.clone());
        self.path_to_route_id.insert(shared_path, val);

        Ok(())
    }

    fn at<'n, 'p>(
        &'n self,
        path: &'p str,
    ) -> Result<matchit::Match<'n, 'p, &'n RouteId>, MatchError> {
        self.inner.at(path)
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("paths", &self.route_id_to_path)
            .finish()
    }
}

#[track_caller]
fn validate_nest_path(v7_checks: bool, path: &str) -> &str {
    assert!(path.starts_with('/'));
    assert!(path.len() > 1);

    if path.split('/').any(|segment| {
        segment.starts_with("{*") && segment.ends_with('}') && !segment.ends_with("}}")
    }) {
        panic!("Invalid route: nested routes cannot contain wildcards (*)");
    }

    if v7_checks {
        validate_v07_paths(path).unwrap();
    }

    path
}

pub(crate) fn path_for_nested_route<'a>(prefix: &'a str, path: &'a str) -> Cow<'a, str> {
    debug_assert!(prefix.starts_with('/'));
    debug_assert!(path.starts_with('/'));

    if prefix.ends_with('/') {
        format!("{prefix}{}", path.trim_start_matches('/')).into()
    } else if path == "/" {
        prefix.into()
    } else {
        format!("{prefix}{path}").into()
    }
}
