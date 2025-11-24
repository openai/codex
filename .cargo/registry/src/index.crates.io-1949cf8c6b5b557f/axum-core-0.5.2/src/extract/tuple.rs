use super::{FromRequest, FromRequestParts, Request};
use crate::response::{IntoResponse, Response};
use http::request::Parts;
use std::{convert::Infallible, future::Future};

impl<S> FromRequestParts<S> for ()
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(_: &mut Parts, _: &S) -> Result<(), Self::Rejection> {
        Ok(())
    }
}

macro_rules! impl_from_request {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut, unused_variables)]
        impl<S, $($ty,)* $last> FromRequestParts<S> for ($($ty,)* $last,)
        where
            $( $ty: FromRequestParts<S> + Send, )*
            $last: FromRequestParts<S> + Send,
            S: Send + Sync,
        {
            type Rejection = Response;

            async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
                $(
                    let $ty = $ty::from_request_parts(parts, state)
                        .await
                        .map_err(|err| err.into_response())?;
                )*
                let $last = $last::from_request_parts(parts, state)
                    .await
                    .map_err(|err| err.into_response())?;

                Ok(($($ty,)* $last,))
            }
        }

        // This impl must not be generic over M, otherwise it would conflict with the blanket
        // implementation of `FromRequest<S, Mut>` for `T: FromRequestParts<S>`.
        #[allow(non_snake_case, unused_mut, unused_variables)]
        impl<S, $($ty,)* $last> FromRequest<S> for ($($ty,)* $last,)
        where
            $( $ty: FromRequestParts<S> + Send, )*
            $last: FromRequest<S> + Send,
            S: Send + Sync,
        {
            type Rejection = Response;

            fn from_request(req: Request, state: &S) -> impl Future<Output = Result<Self, Self::Rejection>> {
                let (mut parts, body) = req.into_parts();

                async move {
                    $(
                        let $ty = $ty::from_request_parts(&mut parts, state).await.map_err(|err| err.into_response())?;
                    )*

                    let req = Request::from_parts(parts, body);

                    let $last = $last::from_request(req, state).await.map_err(|err| err.into_response())?;

                    Ok(($($ty,)* $last,))
                }
            }
        }
    };
}

all_the_tuples!(impl_from_request);

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http::Method;

    use crate::extract::{FromRequest, FromRequestParts};

    fn assert_from_request<M, T>()
    where
        T: FromRequest<(), M>,
    {
    }

    fn assert_from_request_parts<T: FromRequestParts<()>>() {}

    #[test]
    fn unit() {
        assert_from_request_parts::<()>();
        assert_from_request::<_, ()>();
    }

    #[test]
    fn tuple_of_one() {
        assert_from_request_parts::<(Method,)>();
        assert_from_request::<_, (Method,)>();
        assert_from_request::<_, (Bytes,)>();
    }

    #[test]
    fn tuple_of_two() {
        assert_from_request_parts::<((), ())>();
        assert_from_request::<_, ((), ())>();
        assert_from_request::<_, (Method, Bytes)>();
    }

    #[test]
    fn nested_tuple() {
        assert_from_request_parts::<(((Method,),),)>();
        assert_from_request::<_, ((((Bytes,),),),)>();
    }
}
