#[derive(Clone)]
pub(crate) struct AnyValue {
    inner: std::sync::Arc<dyn std::any::Any + Send + Sync + 'static>,
    // While we can extract `TypeId` from `inner`, the debug repr is of a number, so let's track
    // the type_name in debug builds.
    id: AnyValueId,
}

impl AnyValue {
    pub(crate) fn new<V: std::any::Any + Clone + Send + Sync + 'static>(inner: V) -> Self {
        let id = AnyValueId::of::<V>();
        let inner = std::sync::Arc::new(inner);
        Self { inner, id }
    }

    pub(crate) fn downcast_ref<T: std::any::Any + Clone + Send + Sync + 'static>(
        &self,
    ) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }

    pub(crate) fn downcast_into<T: std::any::Any + Clone + Send + Sync>(self) -> Result<T, Self> {
        let id = self.id;
        let value =
            ok!(std::sync::Arc::downcast::<T>(self.inner).map_err(|inner| Self { inner, id }));
        let value = std::sync::Arc::try_unwrap(value).unwrap_or_else(|arc| (*arc).clone());
        Ok(value)
    }

    pub(crate) fn type_id(&self) -> AnyValueId {
        self.id
    }
}

impl std::fmt::Debug for AnyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("AnyValue").field("inner", &self.id).finish()
    }
}

#[derive(Copy, Clone)]
pub struct AnyValueId {
    type_id: std::any::TypeId,
    #[cfg(debug_assertions)]
    type_name: &'static str,
}

impl AnyValueId {
    pub(crate) fn of<A: ?Sized + 'static>() -> Self {
        Self {
            type_id: std::any::TypeId::of::<A>(),
            #[cfg(debug_assertions)]
            type_name: std::any::type_name::<A>(),
        }
    }
}

impl PartialEq for AnyValueId {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}

impl Eq for AnyValueId {}

impl PartialOrd for AnyValueId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq<std::any::TypeId> for AnyValueId {
    fn eq(&self, other: &std::any::TypeId) -> bool {
        self.type_id == *other
    }
}

impl Ord for AnyValueId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.type_id.cmp(&other.type_id)
    }
}

impl std::hash::Hash for AnyValueId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
    }
}

impl std::fmt::Debug for AnyValueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        #[cfg(not(debug_assertions))]
        {
            self.type_id.fmt(f)
        }
        #[cfg(debug_assertions)]
        {
            f.debug_struct(self.type_name).finish()
        }
    }
}

impl<'a, A: ?Sized + 'static> From<&'a A> for AnyValueId {
    fn from(_: &'a A) -> Self {
        Self::of::<A>()
    }
}

#[cfg(test)]
mod test {
    #[test]
    #[cfg(debug_assertions)]
    fn debug_impl() {
        use super::*;

        assert_eq!(format!("{:?}", AnyValue::new(5)), "AnyValue { inner: i32 }");
    }

    #[test]
    fn eq_to_type_id() {
        use super::*;

        let any_value_id = AnyValueId::of::<i32>();
        let type_id = std::any::TypeId::of::<i32>();
        assert_eq!(any_value_id, type_id);
    }
}
