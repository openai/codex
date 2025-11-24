use crate::util::AnyValue;
use crate::util::AnyValueId;
use crate::util::FlatMap;

#[derive(Default, Clone, Debug)]
pub(crate) struct Extensions {
    extensions: FlatMap<AnyValueId, AnyValue>,
}

impl Extensions {
    #[allow(dead_code)]
    pub(crate) fn get<T: Extension>(&self) -> Option<&T> {
        let id = AnyValueId::of::<T>();
        self.extensions.get(&id).map(|e| {
            e.downcast_ref::<T>()
                .expect("`Extensions` tracks values by type")
        })
    }

    #[allow(dead_code)]
    pub(crate) fn set<T: Extension>(&mut self, tagged: T) -> bool {
        let value = AnyValue::new(tagged);
        let id = value.type_id();
        self.extensions.insert(id, value).is_some()
    }

    #[allow(dead_code)]
    pub(crate) fn remove<T: Extension>(&mut self) -> Option<T> {
        let id = AnyValueId::of::<T>();
        self.extensions.remove(&id).map(|e| {
            e.downcast_into::<T>()
                .expect("`Extensions` tracks values by type")
        })
    }

    pub(crate) fn update(&mut self, other: &Self) {
        for (key, value) in other.extensions.iter() {
            self.extensions.insert(*key, value.clone());
        }
    }
}

#[allow(unreachable_pub)]
pub trait Extension: std::fmt::Debug + Clone + std::any::Any + Send + Sync + 'static {}

impl<T> Extension for T where T: std::fmt::Debug + Clone + std::any::Any + Send + Sync + 'static {}
