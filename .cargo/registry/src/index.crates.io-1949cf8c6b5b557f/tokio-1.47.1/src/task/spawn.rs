use crate::runtime::BOX_FUTURE_THRESHOLD;
use crate::task::JoinHandle;
use crate::util::trace::SpawnMeta;

use std::future::Future;

cfg_rt! {
    /// Spawns a new asynchronous task, returning a
    /// [`JoinHandle`](JoinHandle) for it.
    ///
    /// The provided future will start running in the background immediately
    /// when `spawn` is called, even if you don't await the returned
    /// `JoinHandle`.
    ///
    /// Spawning a task enables the task to execute concurrently to other tasks. The
    /// spawned task may execute on the current thread, or it may be sent to a
    /// different thread to be executed. The specifics depend on the current
    /// [`Runtime`](crate::runtime::Runtime) configuration.
    ///
    /// It is guaranteed that spawn will not synchronously poll the task being spawned.
    /// This means that calling spawn while holding a lock does not pose a risk of
    /// deadlocking with the spawned task.
    ///
    /// There is no guarantee that a spawned task will execute to completion.
    /// When a runtime is shutdown, all outstanding tasks are dropped,
    /// regardless of the lifecycle of that task.
    ///
    /// This function must be called from the context of a Tokio runtime. Tasks running on
    /// the Tokio runtime are always inside its context, but you can also enter the context
    /// using the [`Runtime::enter`](crate::runtime::Runtime::enter()) method.
    ///
    /// # Examples
    ///
    /// In this example, a server is started and `spawn` is used to start a new task
    /// that processes each received connection.
    ///
    /// ```no_run
    /// use tokio::net::{TcpListener, TcpStream};
    ///
    /// use std::io;
    ///
    /// async fn process(socket: TcpStream) {
    ///     // ...
    /// # drop(socket);
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let listener = TcpListener::bind("127.0.0.1:8080").await?;
    ///
    ///     loop {
    ///         let (socket, _) = listener.accept().await?;
    ///
    ///         tokio::spawn(async move {
    ///             // Process each socket concurrently.
    ///             process(socket).await
    ///         });
    ///     }
    /// }
    /// ```
    ///
    /// To run multiple tasks in parallel and receive their results, join
    /// handles can be stored in a vector.
    /// ```
    /// # #[tokio::main(flavor = "current_thread")] async fn main() {
    /// async fn my_background_op(id: i32) -> String {
    ///     let s = format!("Starting background task {}.", id);
    ///     println!("{}", s);
    ///     s
    /// }
    ///
    /// let ops = vec![1, 2, 3];
    /// let mut tasks = Vec::with_capacity(ops.len());
    /// for op in ops {
    ///     // This call will make them start running in the background
    ///     // immediately.
    ///     tasks.push(tokio::spawn(my_background_op(op)));
    /// }
    ///
    /// let mut outputs = Vec::with_capacity(tasks.len());
    /// for task in tasks {
    ///     outputs.push(task.await.unwrap());
    /// }
    /// println!("{:?}", outputs);
    /// # }
    /// ```
    /// This example pushes the tasks to `outputs` in the order they were
    /// started in. If you do not care about the ordering of the outputs, then
    /// you can also use a [`JoinSet`].
    ///
    /// [`JoinSet`]: struct@crate::task::JoinSet
    ///
    /// # Panics
    ///
    /// Panics if called from **outside** of the Tokio runtime.
    ///
    /// # Using `!Send` values from a task
    ///
    /// The task supplied to `spawn` must implement `Send`. However, it is
    /// possible to **use** `!Send` values from the task as long as they only
    /// exist between calls to `.await`.
    ///
    /// For example, this will work:
    ///
    /// ```
    /// use tokio::task;
    ///
    /// use std::rc::Rc;
    ///
    /// fn use_rc(rc: Rc<()>) {
    ///     // Do stuff w/ rc
    /// # drop(rc);
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     tokio::spawn(async {
    ///         // Force the `Rc` to stay in a scope with no `.await`
    ///         {
    ///             let rc = Rc::new(());
    ///             use_rc(rc.clone());
    ///         }
    ///
    ///         task::yield_now().await;
    ///     }).await.unwrap();
    /// }
    /// ```
    ///
    /// This will **not** work:
    ///
    /// ```compile_fail
    /// use tokio::task;
    ///
    /// use std::rc::Rc;
    ///
    /// fn use_rc(rc: Rc<()>) {
    ///     // Do stuff w/ rc
    /// # drop(rc);
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     tokio::spawn(async {
    ///         let rc = Rc::new(());
    ///
    ///         task::yield_now().await;
    ///
    ///         use_rc(rc.clone());
    ///     }).await.unwrap();
    /// }
    /// ```
    ///
    /// Holding on to a `!Send` value across calls to `.await` will result in
    /// an unfriendly compile error message similar to:
    ///
    /// ```text
    /// `[... some type ...]` cannot be sent between threads safely
    /// ```
    ///
    /// or:
    ///
    /// ```text
    /// error[E0391]: cycle detected when processing `main`
    /// ```
    #[track_caller]
    pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let fut_size = std::mem::size_of::<F>();
        if fut_size > BOX_FUTURE_THRESHOLD {
            spawn_inner(Box::pin(future), SpawnMeta::new_unnamed(fut_size))
        } else {
            spawn_inner(future, SpawnMeta::new_unnamed(fut_size))
        }
    }

    #[track_caller]
    pub(super) fn spawn_inner<T>(future: T, meta: SpawnMeta<'_>) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        use crate::runtime::{context, task};

        #[cfg(all(
            tokio_unstable,
            tokio_taskdump,
            feature = "rt",
            target_os = "linux",
            any(
                target_arch = "aarch64",
                target_arch = "x86",
                target_arch = "x86_64"
            )
        ))]
        let future = task::trace::Trace::root(future);
        let id = task::Id::next();
        let task = crate::util::trace::task(future, "task", meta, id.as_u64());

        match context::with_current(|handle| handle.spawn(task, id, meta.spawned_at)) {
            Ok(join_handle) => join_handle,
            Err(e) => panic!("{}", e),
        }
    }
}
