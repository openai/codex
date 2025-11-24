use axum_core::extract::{FromRef, FromRequestParts};
use http::request::Parts;
use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

/// Extractor for state.
///
/// See ["Accessing state in middleware"][state-from-middleware] for how to
/// access state in middleware.
///
/// State is global and used in every request a router with state receives.
/// For accessing data derived from requests, such as authorization data, see [`Extension`].
///
/// [state-from-middleware]: crate::middleware#accessing-state-in-middleware
/// [`Extension`]: crate::Extension
///
/// # With `Router`
///
/// ```
/// use axum::{Router, routing::get, extract::State};
///
/// // the application state
/// //
/// // here you can put configuration, database connection pools, or whatever
/// // state you need
/// #[derive(Clone)]
/// struct AppState {}
///
/// let state = AppState {};
///
/// // create a `Router` that holds our state
/// let app = Router::new()
///     .route("/", get(handler))
///     // provide the state so the router can access it
///     .with_state(state);
///
/// async fn handler(
///     // access the state via the `State` extractor
///     // extracting a state of the wrong type results in a compile error
///     State(state): State<AppState>,
/// ) {
///     // use `state`...
/// }
/// # let _: axum::Router = app;
/// ```
///
/// Note that `State` is an extractor, so be sure to put it before any body
/// extractors, see ["the order of extractors"][order-of-extractors].
///
/// [order-of-extractors]: crate::extract#the-order-of-extractors
///
/// ## Combining stateful routers
///
/// Multiple [`Router`]s can be combined with [`Router::nest`] or [`Router::merge`]
/// When combining [`Router`]s with one of these methods, the [`Router`]s must have
/// the same state type. Generally, this can be inferred automatically:
///
/// ```
/// use axum::{Router, routing::get, extract::State};
///
/// #[derive(Clone)]
/// struct AppState {}
///
/// let state = AppState {};
///
/// // create a `Router` that will be nested within another
/// let api = Router::new()
///     .route("/posts", get(posts_handler));
///
/// let app = Router::new()
///     .nest("/api", api)
///     .with_state(state);
///
/// async fn posts_handler(State(state): State<AppState>) {
///     // use `state`...
/// }
/// # let _: axum::Router = app;
/// ```
///
/// However, if you are composing [`Router`]s that are defined in separate scopes,
/// you may need to annotate the [`State`] type explicitly:
///
/// ```
/// use axum::{Router, routing::get, extract::State};
///
/// #[derive(Clone)]
/// struct AppState {}
///
/// fn make_app() -> Router {
///     let state = AppState {};
///
///     Router::new()
///         .nest("/api", make_api())
///         .with_state(state) // the outer Router's state is inferred
/// }
///
/// // the inner Router must specify its state type to compose with the
/// // outer router
/// fn make_api() -> Router<AppState> {
///     Router::new()
///         .route("/posts", get(posts_handler))
/// }
///
/// async fn posts_handler(State(state): State<AppState>) {
///     // use `state`...
/// }
/// # let _: axum::Router = make_app();
/// ```
///
/// In short, a [`Router`]'s generic state type defaults to `()`
/// (no state) unless [`Router::with_state`] is called or the value
/// of the generic type is given explicitly.
///
/// [`Router`]: crate::Router
/// [`Router::merge`]: crate::Router::merge
/// [`Router::nest`]: crate::Router::nest
/// [`Router::with_state`]: crate::Router::with_state
///
/// # With `MethodRouter`
///
/// ```
/// use axum::{routing::get, extract::State};
///
/// #[derive(Clone)]
/// struct AppState {}
///
/// let state = AppState {};
///
/// let method_router_with_state = get(handler)
///     // provide the state so the handler can access it
///     .with_state(state);
/// # let _: axum::routing::MethodRouter = method_router_with_state;
///
/// async fn handler(State(state): State<AppState>) {
///     // use `state`...
/// }
/// ```
///
/// # With `Handler`
///
/// ```
/// use axum::{routing::get, handler::Handler, extract::State};
///
/// #[derive(Clone)]
/// struct AppState {}
///
/// let state = AppState {};
///
/// async fn handler(State(state): State<AppState>) {
///     // use `state`...
/// }
///
/// // provide the state so the handler can access it
/// let handler_with_state = handler.with_state(state);
///
/// # async {
/// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
/// axum::serve(listener, handler_with_state.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// # Substates
///
/// [`State`] only allows a single state type but you can use [`FromRef`] to extract "substates":
///
/// ```
/// use axum::{Router, routing::get, extract::{State, FromRef}};
///
/// // the application state
/// #[derive(Clone)]
/// struct AppState {
///     // that holds some api specific state
///     api_state: ApiState,
/// }
///
/// // the api specific state
/// #[derive(Clone)]
/// struct ApiState {}
///
/// // support converting an `AppState` in an `ApiState`
/// impl FromRef<AppState> for ApiState {
///     fn from_ref(app_state: &AppState) -> ApiState {
///         app_state.api_state.clone()
///     }
/// }
///
/// let state = AppState {
///     api_state: ApiState {},
/// };
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .route("/api/users", get(api_users))
///     .with_state(state);
///
/// async fn api_users(
///     // access the api specific state
///     State(api_state): State<ApiState>,
/// ) {
/// }
///
/// async fn handler(
///     // we can still access to top level state
///     State(state): State<AppState>,
/// ) {
/// }
/// # let _: axum::Router = app;
/// ```
///
/// For convenience `FromRef` can also be derived using `#[derive(FromRef)]`.
///
/// # For library authors
///
/// If you're writing a library that has an extractor that needs state, this is the recommended way
/// to do it:
///
/// ```rust
/// use axum_core::extract::{FromRequestParts, FromRef};
/// use http::request::Parts;
/// use std::convert::Infallible;
///
/// // the extractor your library provides
/// struct MyLibraryExtractor;
///
/// impl<S> FromRequestParts<S> for MyLibraryExtractor
/// where
///     // keep `S` generic but require that it can produce a `MyLibraryState`
///     // this means users will have to implement `FromRef<UserState> for MyLibraryState`
///     MyLibraryState: FromRef<S>,
///     S: Send + Sync,
/// {
///     type Rejection = Infallible;
///
///     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
///         // get a `MyLibraryState` from a reference to the state
///         let state = MyLibraryState::from_ref(state);
///
///         // ...
///         # todo!()
///     }
/// }
///
/// // the state your library needs
/// struct MyLibraryState {
///     // ...
/// }
/// ```
///
/// # Shared mutable state
///
/// [As state is global within a `Router`][global] you can't directly get a mutable reference to
/// the state.
///
/// The most basic solution is to use an `Arc<Mutex<_>>`. Which kind of mutex you need depends on
/// your use case. See [the tokio docs] for more details.
///
/// Note that holding a locked `std::sync::Mutex` across `.await` points will result in `!Send`
/// futures which are incompatible with axum. If you need to hold a mutex across `.await` points,
/// consider using a `tokio::sync::Mutex` instead.
///
/// ## Example
///
/// ```
/// use axum::{Router, routing::get, extract::State};
/// use std::sync::{Arc, Mutex};
///
/// #[derive(Clone)]
/// struct AppState {
///     data: Arc<Mutex<String>>,
/// }
///
/// async fn handler(State(state): State<AppState>) {
///     {
///         let mut data = state.data.lock().expect("mutex was poisoned");
///         *data = "updated foo".to_owned();
///     }
///
///     // ...
/// }
///
/// let state = AppState {
///     data: Arc::new(Mutex::new("foo".to_owned())),
/// };
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .with_state(state);
/// # let _: Router = app;
/// ```
///
/// [global]: crate::Router::with_state
/// [the tokio docs]: https://docs.rs/tokio/1.25.0/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
#[derive(Debug, Default, Clone, Copy)]
pub struct State<S>(pub S);

impl<OuterState, InnerState> FromRequestParts<OuterState> for State<InnerState>
where
    InnerState: FromRef<OuterState>,
    OuterState: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &OuterState,
    ) -> Result<Self, Self::Rejection> {
        let inner_state = InnerState::from_ref(state);
        Ok(Self(inner_state))
    }
}

impl<S> Deref for State<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> DerefMut for State<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
