pub(crate) mod request;
pub(crate) mod request_parts;

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use crate::extract::{FromRef, FromRequestParts};
    use http::request::Parts;

    #[derive(Debug, Default, Clone, Copy)]
    pub(crate) struct State<S>(pub(crate) S);

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

    // some extractor that requires the state, such as `SignedCookieJar`
    #[allow(dead_code)]
    pub(crate) struct RequiresState(pub(crate) String);

    impl<S> FromRequestParts<S> for RequiresState
    where
        S: Send + Sync,
        String: FromRef<S>,
    {
        type Rejection = Infallible;

        async fn from_request_parts(
            _parts: &mut Parts,
            state: &S,
        ) -> Result<Self, Self::Rejection> {
            Ok(Self(String::from_ref(state)))
        }
    }
}
