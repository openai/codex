#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use std::{
    collections::VecDeque,
    fmt,
    future::Future,
    marker::PhantomData,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};

use deadpool_runtime::Runtime;
use tokio::sync::{Semaphore, TryAcquireError};

use crate::{
    managed::{
        dropguard::DropGuard, hooks::Hooks, object::ObjectInner, Manager, Metrics, Object,
        PoolBuilder, PoolConfig, PoolError, QueueMode, TimeoutType, Timeouts,
    },
    Status,
};

/// Generic object and connection pool.
///
/// This struct can be cloned and transferred across thread boundaries and uses
/// reference counting for its internal state.
pub struct Pool<M: Manager, W: From<Object<M>> = Object<M>> {
    pub(crate) inner: Arc<PoolInner<M>>,
    pub(crate) _wrapper: PhantomData<fn() -> W>,
}

// Implemented manually to avoid unnecessary trait bound on `W` type parameter.
impl<M, W> fmt::Debug for Pool<M, W>
where
    M: fmt::Debug + Manager,
    M::Type: fmt::Debug,
    W: From<Object<M>>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pool")
            .field("inner", &self.inner)
            .field("wrapper", &self._wrapper)
            .finish()
    }
}

impl<M: Manager, W: From<Object<M>>> Clone for Pool<M, W> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _wrapper: PhantomData,
        }
    }
}

impl<M: Manager, W: From<Object<M>>> Pool<M, W> {
    /// Instantiates a builder for a new [`Pool`].
    ///
    /// This is the only way to create a [`Pool`] instance.
    pub fn builder(manager: M) -> PoolBuilder<M, W> {
        PoolBuilder::new(manager)
    }

    pub(crate) fn from_builder(builder: PoolBuilder<M, W>) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                manager: builder.manager,
                next_id: AtomicUsize::new(0),
                slots: Mutex::new(Slots {
                    vec: VecDeque::with_capacity(builder.config.max_size),
                    size: 0,
                    max_size: builder.config.max_size,
                }),
                users: AtomicUsize::new(0),
                semaphore: Semaphore::new(builder.config.max_size),
                config: builder.config,
                hooks: builder.hooks,
                runtime: builder.runtime,
            }),
            _wrapper: PhantomData,
        }
    }

    /// Retrieves an [`Object`] from this [`Pool`] or waits for one to
    /// become available.
    ///
    /// # Errors
    ///
    /// See [`PoolError`] for details.
    pub async fn get(&self) -> Result<W, PoolError<M::Error>> {
        self.timeout_get(&self.timeouts()).await
    }

    /// Retrieves an [`Object`] from this [`Pool`] using a different `timeout`
    /// than the configured one.
    ///
    /// # Errors
    ///
    /// See [`PoolError`] for details.
    pub async fn timeout_get(&self, timeouts: &Timeouts) -> Result<W, PoolError<M::Error>> {
        let _ = self.inner.users.fetch_add(1, Ordering::Relaxed);
        let users_guard = DropGuard(|| {
            let _ = self.inner.users.fetch_sub(1, Ordering::Relaxed);
        });

        let non_blocking = match timeouts.wait {
            Some(t) => t.as_nanos() == 0,
            None => false,
        };

        let permit = if non_blocking {
            self.inner.semaphore.try_acquire().map_err(|e| match e {
                TryAcquireError::Closed => PoolError::Closed,
                TryAcquireError::NoPermits => PoolError::Timeout(TimeoutType::Wait),
            })?
        } else {
            apply_timeout(
                self.inner.runtime,
                TimeoutType::Wait,
                timeouts.wait,
                async {
                    self.inner
                        .semaphore
                        .acquire()
                        .await
                        .map_err(|_| PoolError::Closed)
                },
            )
            .await?
        };

        let inner_obj = loop {
            let inner_obj = match self.inner.config.queue_mode {
                QueueMode::Fifo => self.inner.slots.lock().unwrap().vec.pop_front(),
                QueueMode::Lifo => self.inner.slots.lock().unwrap().vec.pop_back(),
            };
            let inner_obj = if let Some(inner_obj) = inner_obj {
                self.try_recycle(timeouts, inner_obj).await?
            } else {
                self.try_create(timeouts).await?
            };
            if let Some(inner_obj) = inner_obj {
                break inner_obj;
            }
        };

        users_guard.disarm();
        permit.forget();

        Ok(Object {
            inner: Some(inner_obj),
            pool: self.weak(),
        }
        .into())
    }

    #[inline]
    async fn try_recycle(
        &self,
        timeouts: &Timeouts,
        inner_obj: ObjectInner<M>,
    ) -> Result<Option<ObjectInner<M>>, PoolError<M::Error>> {
        let mut unready_obj = UnreadyObject {
            inner: Some(inner_obj),
            pool: &self.inner,
        };
        let inner = unready_obj.inner();

        // Apply pre_recycle hooks
        if let Err(_e) = self.inner.hooks.pre_recycle.apply(inner).await {
            // TODO log pre_recycle error
            return Ok(None);
        }

        if apply_timeout(
            self.inner.runtime,
            TimeoutType::Recycle,
            timeouts.recycle,
            self.inner.manager.recycle(&mut inner.obj, &inner.metrics),
        )
        .await
        .is_err()
        {
            return Ok(None);
        }

        // Apply post_recycle hooks
        if let Err(_e) = self.inner.hooks.post_recycle.apply(inner).await {
            // TODO log post_recycle error
            return Ok(None);
        }

        inner.metrics.recycle_count += 1;
        #[cfg(not(target_arch = "wasm32"))]
        {
            inner.metrics.recycled = Some(Instant::now());
        }

        Ok(Some(unready_obj.ready()))
    }

    #[inline]
    async fn try_create(
        &self,
        timeouts: &Timeouts,
    ) -> Result<Option<ObjectInner<M>>, PoolError<M::Error>> {
        let mut unready_obj = UnreadyObject {
            inner: Some(ObjectInner {
                obj: apply_timeout(
                    self.inner.runtime,
                    TimeoutType::Create,
                    timeouts.create,
                    self.inner.manager.create(),
                )
                .await?,
                id: self.inner.next_id.fetch_add(1, Ordering::Relaxed),
                metrics: Metrics::default(),
            }),
            pool: &self.inner,
        };

        self.inner.slots.lock().unwrap().size += 1;

        // Apply post_create hooks
        if let Err(e) = self
            .inner
            .hooks
            .post_create
            .apply(unready_obj.inner())
            .await
        {
            return Err(PoolError::PostCreateHook(e));
        }

        Ok(Some(unready_obj.ready()))
    }

    /**
     * Resize the pool. This change the `max_size` of the pool dropping
     * excess objects and/or making space for new ones.
     *
     * If the pool is closed this method does nothing. The [`Pool::status`] method
     * always reports a `max_size` of 0 for closed pools.
     */
    pub fn resize(&self, max_size: usize) {
        if self.inner.semaphore.is_closed() {
            return;
        }
        let mut slots = self.inner.slots.lock().unwrap();
        let old_max_size = slots.max_size;
        slots.max_size = max_size;
        // shrink pool
        if max_size < old_max_size {
            while slots.size > slots.max_size {
                if let Ok(permit) = self.inner.semaphore.try_acquire() {
                    permit.forget();
                    if slots.vec.pop_front().is_some() {
                        slots.size -= 1;
                    }
                } else {
                    break;
                }
            }
            // Create a new VecDeque with a smaller capacity
            let mut vec = VecDeque::with_capacity(max_size);
            for obj in slots.vec.drain(..) {
                vec.push_back(obj);
            }
            slots.vec = vec;
        }
        // grow pool
        if max_size > old_max_size {
            let additional = slots.max_size - old_max_size;
            slots.vec.reserve_exact(additional);
            self.inner.semaphore.add_permits(additional);
        }
    }

    /// Retains only the objects specified by the given function.
    ///
    /// This function is typically used to remove objects from
    /// the pool based on their current state or metrics.
    ///
    /// **Caution:** This function blocks the entire pool while
    /// it is running. Therefore the given function should not
    /// block.
    ///
    /// The following example starts a background task that
    /// runs every 30 seconds and removes objects from the pool
    /// that haven't been used for more than one minute.
    ///
    /// ```rust,ignore
    /// let interval = Duration::from_secs(30);
    /// let max_age = Duration::from_secs(60);
    /// tokio::spawn(async move {
    ///     loop {
    ///         tokio::time::sleep(interval).await;
    ///         pool.retain(|_, metrics| metrics.last_used() < max_age);
    ///     }
    /// });
    /// ```
    pub fn retain(
        &self,
        mut predicate: impl FnMut(&M::Type, Metrics) -> bool,
    ) -> RetainResult<M::Type> {
        let mut removed = Vec::with_capacity(self.status().size);
        let mut guard = self.inner.slots.lock().unwrap();
        let mut i = 0;
        // This code can be simplified once `Vec::extract_if` lands in stable Rust.
        // https://doc.rust-lang.org/std/vec/struct.Vec.html#method.extract_if
        while i < guard.vec.len() {
            let obj = &mut guard.vec[i];
            if predicate(&mut obj.obj, obj.metrics) {
                i += 1;
            } else {
                let mut obj = guard.vec.remove(i).unwrap();
                self.manager().detach(&mut obj.obj);
                removed.push(obj.obj);
            }
        }
        guard.size -= removed.len();
        RetainResult {
            retained: i,
            removed,
        }
    }

    /// Get current timeout configuration
    pub fn timeouts(&self) -> Timeouts {
        self.inner.config.timeouts
    }

    /// Closes this [`Pool`].
    ///
    /// All current and future tasks waiting for [`Object`]s will return
    /// [`PoolError::Closed`] immediately.
    ///
    /// This operation resizes the pool to 0.
    pub fn close(&self) {
        self.resize(0);
        self.inner.semaphore.close();
    }

    /// Indicates whether this [`Pool`] has been closed.
    pub fn is_closed(&self) -> bool {
        self.inner.semaphore.is_closed()
    }

    /// Retrieves [`Status`] of this [`Pool`].
    #[must_use]
    pub fn status(&self) -> Status {
        let slots = self.inner.slots.lock().unwrap();
        let users = self.inner.users.load(Ordering::Relaxed);
        let (available, waiting) = if users < slots.size {
            (slots.size - users, 0)
        } else {
            (0, users - slots.size)
        };
        Status {
            max_size: slots.max_size,
            size: slots.size,
            available,
            waiting,
        }
    }

    /// Returns [`Manager`] of this [`Pool`].
    #[must_use]
    pub fn manager(&self) -> &M {
        &self.inner.manager
    }

    /// Returns a [`WeakPool<T>`] of this [`Pool`].
    pub fn weak(&self) -> WeakPool<M> {
        WeakPool {
            inner: Arc::downgrade(&self.inner),
            _wrapper: PhantomData,
        }
    }
}

/// A weak reference to a [`Pool<T>`], used to avoid keeping the pool alive.
///
/// `WeakPool<T>` is analogous to [`std::sync::Weak<T>`] for [`Pool<T>`], and
/// is typically used in situations where you need a non-owning reference to a pool,
/// such as in background tasks, managers, or callbacks that should not extend
/// the lifetime of the pool.
///
/// This allows components to retain a reference to the pool while avoiding
/// reference cycles or prolonging its lifetime unnecessarily.
///
/// To access the pool, use [`WeakPool::upgrade`] to attempt to get a strong reference.
#[derive(Debug)]
pub struct WeakPool<M: Manager, W: From<Object<M>> = Object<M>> {
    inner: Weak<PoolInner<M>>,
    _wrapper: PhantomData<fn() -> W>,
}

impl<M: Manager, W: From<Object<M>>> WeakPool<M, W> {
    /// Attempts to upgrade the `WeakPool` to a strong [`Pool<T>`] reference.
    ///
    /// If the pool has already been dropped (i.e., no strong references remain),
    /// this returns `None`.
    pub fn upgrade(&self) -> Option<Pool<M, W>> {
        Some(Pool {
            inner: self.inner.upgrade()?,
            _wrapper: PhantomData,
        })
    }
}

pub(crate) struct PoolInner<M: Manager> {
    manager: M,
    next_id: AtomicUsize,
    slots: Mutex<Slots<ObjectInner<M>>>,
    /// Number of ['Pool'] users. A user is both a future which is waiting for an ['Object'] or one
    /// with an ['Object'] which hasn't been returned, yet.
    users: AtomicUsize,
    semaphore: Semaphore,
    config: PoolConfig,
    runtime: Option<Runtime>,
    hooks: Hooks<M>,
}

#[derive(Debug)]
struct Slots<T> {
    vec: VecDeque<T>,
    size: usize,
    max_size: usize,
}

// Implemented manually to avoid unnecessary trait bound on the struct.
impl<M> fmt::Debug for PoolInner<M>
where
    M: fmt::Debug + Manager,
    M::Type: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolInner")
            .field("manager", &self.manager)
            .field("slots", &self.slots)
            .field("used", &self.users)
            .field("semaphore", &self.semaphore)
            .field("config", &self.config)
            .field("runtime", &self.runtime)
            .field("hooks", &self.hooks)
            .finish()
    }
}

impl<M: Manager> PoolInner<M> {
    pub(crate) fn return_object(&self, mut inner: ObjectInner<M>) {
        let _ = self.users.fetch_sub(1, Ordering::Relaxed);
        let mut slots = self.slots.lock().unwrap();
        if slots.size <= slots.max_size {
            slots.vec.push_back(inner);
            drop(slots);
            self.semaphore.add_permits(1);
        } else {
            slots.size -= 1;
            drop(slots);
            self.manager.detach(&mut inner.obj);
        }
    }
    pub(crate) fn detach_object(&self, obj: &mut M::Type) {
        let _ = self.users.fetch_sub(1, Ordering::Relaxed);
        let mut slots = self.slots.lock().unwrap();
        let add_permits = slots.size <= slots.max_size;
        slots.size -= 1;
        drop(slots);
        if add_permits {
            self.semaphore.add_permits(1);
        }
        self.manager.detach(obj);
    }
}

struct UnreadyObject<'a, M: Manager> {
    inner: Option<ObjectInner<M>>,
    pool: &'a PoolInner<M>,
}

impl<M: Manager> UnreadyObject<'_, M> {
    fn ready(mut self) -> ObjectInner<M> {
        self.inner.take().unwrap()
    }
    fn inner(&mut self) -> &mut ObjectInner<M> {
        self.inner.as_mut().unwrap()
    }
}

impl<M: Manager> Drop for UnreadyObject<'_, M> {
    fn drop(&mut self) {
        if let Some(mut inner) = self.inner.take() {
            self.pool.slots.lock().unwrap().size -= 1;
            self.pool.manager.detach(&mut inner.obj);
        }
    }
}

async fn apply_timeout<O, E>(
    runtime: Option<Runtime>,
    timeout_type: TimeoutType,
    duration: Option<Duration>,
    future: impl Future<Output = Result<O, impl Into<PoolError<E>>>>,
) -> Result<O, PoolError<E>> {
    match (runtime, duration) {
        (_, None) => future.await.map_err(Into::into),
        (Some(runtime), Some(duration)) => runtime
            .timeout(duration, future)
            .await
            .ok_or(PoolError::Timeout(timeout_type))?
            .map_err(Into::into),
        (None, Some(_)) => Err(PoolError::NoRuntimeSpecified),
    }
}

#[derive(Debug)]
/// This is the result returned by `Pool::retain`
pub struct RetainResult<T> {
    /// Number of retained objects
    pub retained: usize,
    /// Objects that were removed from the pool
    pub removed: Vec<T>,
}

impl<T> Default for RetainResult<T> {
    fn default() -> Self {
        Self {
            retained: Default::default(),
            removed: Default::default(),
        }
    }
}
