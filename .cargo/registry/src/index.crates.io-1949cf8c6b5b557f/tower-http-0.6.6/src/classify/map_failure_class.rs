use super::{ClassifiedResponse, ClassifyEos, ClassifyResponse};
use http::{HeaderMap, Response};
use std::fmt;

/// Response classifier that transforms the failure class of some other
/// classifier.
///
/// Created with [`ClassifyResponse::map_failure_class`] or
/// [`ClassifyEos::map_failure_class`].
#[derive(Clone, Copy)]
pub struct MapFailureClass<C, F> {
    inner: C,
    f: F,
}

impl<C, F> MapFailureClass<C, F> {
    pub(super) fn new(classify: C, f: F) -> Self {
        Self { inner: classify, f }
    }
}

impl<C, F> fmt::Debug for MapFailureClass<C, F>
where
    C: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapFailureClass")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

impl<C, F, NewClass> ClassifyResponse for MapFailureClass<C, F>
where
    C: ClassifyResponse,
    F: FnOnce(C::FailureClass) -> NewClass,
{
    type FailureClass = NewClass;
    type ClassifyEos = MapFailureClass<C::ClassifyEos, F>;

    fn classify_response<B>(
        self,
        res: &Response<B>,
    ) -> ClassifiedResponse<Self::FailureClass, Self::ClassifyEos> {
        match self.inner.classify_response(res) {
            ClassifiedResponse::Ready(result) => ClassifiedResponse::Ready(result.map_err(self.f)),
            ClassifiedResponse::RequiresEos(classify_eos) => {
                let mapped_classify_eos = MapFailureClass::new(classify_eos, self.f);
                ClassifiedResponse::RequiresEos(mapped_classify_eos)
            }
        }
    }

    fn classify_error<E>(self, error: &E) -> Self::FailureClass
    where
        E: std::fmt::Display + 'static,
    {
        (self.f)(self.inner.classify_error(error))
    }
}

impl<C, F, NewClass> ClassifyEos for MapFailureClass<C, F>
where
    C: ClassifyEos,
    F: FnOnce(C::FailureClass) -> NewClass,
{
    type FailureClass = NewClass;

    fn classify_eos(self, trailers: Option<&HeaderMap>) -> Result<(), Self::FailureClass> {
        self.inner.classify_eos(trailers).map_err(self.f)
    }

    fn classify_error<E>(self, error: &E) -> Self::FailureClass
    where
        E: std::fmt::Display + 'static,
    {
        (self.f)(self.inner.classify_error(error))
    }
}
