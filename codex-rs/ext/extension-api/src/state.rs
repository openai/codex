use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

type ErasedData = Arc<dyn Any + Send + Sync>;

/// Typed extension-owned data attached to one host object.
#[derive(Debug)]
pub struct ExtensionData {
    level_id: String,
    entries: Mutex<HashMap<TypeId, ErasedData>>,
}

impl ExtensionData {
    /// Creates an empty attachment map for one host-owned scope.
    pub fn new(level_id: impl Into<String>) -> Self {
        Self {
            level_id: level_id.into(),
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the host identity for the scope this data is attached to.
    pub fn level_id(&self) -> &str {
        &self.level_id
    }

    /// Returns the attached value of type `T`, if one exists.
    pub fn get<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync,
    {
        let value = self.entries().get(&TypeId::of::<T>())?.clone();
        Some(downcast_data(value))
    }

    /// Returns the attached value of type `T`, inserting one from `init` when absent.
    ///
    /// The initializer runs while this map is locked, so it should stay cheap;
    /// heavyweight lazy work belongs inside the attached value itself.
    pub fn get_or_init<T>(&self, init: impl FnOnce() -> T) -> Arc<T>
    where
        T: Any + Send + Sync,
    {
        let mut entries = self.entries();
        let value = entries
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Arc::new(init()));
        downcast_data(Arc::clone(value))
    }

    /// Stores `value` as the attachment of type `T`, returning any previous value.
    pub fn insert<T>(&self, value: T) -> Option<Arc<T>>
    where
        T: Any + Send + Sync,
    {
        self.entries()
            .insert(TypeId::of::<T>(), Arc::new(value))
            .map(downcast_data)
    }

    /// Removes and returns the attached value of type `T`, if one exists.
    pub fn remove<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync,
    {
        self.entries().remove(&TypeId::of::<T>()).map(downcast_data)
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<TypeId, ErasedData>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn downcast_data<T>(value: ErasedData) -> Arc<T>
where
    T: Any + Send + Sync,
{
    let Ok(value) = value.downcast::<T>() else {
        unreachable!("typed extension data stored an incompatible value");
    };
    value
}
