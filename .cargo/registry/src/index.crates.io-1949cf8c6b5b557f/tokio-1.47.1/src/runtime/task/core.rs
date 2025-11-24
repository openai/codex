//! Core task module.
//!
//! # Safety
//!
//! The functions in this module are private to the `task` module. All of them
//! should be considered `unsafe` to use, but are not marked as such since it
//! would be too noisy.
//!
//! Make sure to consult the relevant safety section of each function before
//! use.

use crate::future::Future;
use crate::loom::cell::UnsafeCell;
use crate::runtime::context;
use crate::runtime::task::raw::{self, Vtable};
use crate::runtime::task::state::State;
use crate::runtime::task::{Id, Schedule, TaskHarnessScheduleHooks};
use crate::util::linked_list;

use std::num::NonZeroU64;
#[cfg(tokio_unstable)]
use std::panic::Location;
use std::pin::Pin;
use std::ptr::NonNull;
use std::task::{Context, Poll, Waker};

/// The task cell. Contains the components of the task.
///
/// It is critical for `Header` to be the first field as the task structure will
/// be referenced by both *mut Cell and *mut Header.
///
/// Any changes to the layout of this struct _must_ also be reflected in the
/// `const` fns in raw.rs.
///
// # This struct should be cache padded to avoid false sharing. The cache padding rules are copied
// from crossbeam-utils/src/cache_padded.rs
//
// Starting from Intel's Sandy Bridge, spatial prefetcher is now pulling pairs of 64-byte cache
// lines at a time, so we have to align to 128 bytes rather than 64.
//
// Sources:
// - https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-optimization-manual.pdf
// - https://github.com/facebook/folly/blob/1b5288e6eea6df074758f877c849b6e73bbb9fbb/folly/lang/Align.h#L107
//
// ARM's big.LITTLE architecture has asymmetric cores and "big" cores have 128-byte cache line size.
//
// Sources:
// - https://www.mono-project.com/news/2016/09/12/arm64-icache/
//
// powerpc64 has 128-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_ppc64x.go#L9
#[cfg_attr(
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
    ),
    repr(align(128))
)]
// arm, mips, mips64, sparc, and hexagon have 32-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_arm.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mips.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mipsle.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mips64x.go#L9
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/sparc/include/asm/cache.h#L17
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/hexagon/include/asm/cache.h#L12
#[cfg_attr(
    any(
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc",
        target_arch = "hexagon",
    ),
    repr(align(32))
)]
// m68k has 16-byte cache line size.
//
// Sources:
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/m68k/include/asm/cache.h#L9
#[cfg_attr(target_arch = "m68k", repr(align(16)))]
// s390x has 256-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_s390x.go#L7
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/s390/include/asm/cache.h#L13
#[cfg_attr(target_arch = "s390x", repr(align(256)))]
// x86, riscv, wasm, and sparc64 have 64-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/dda2991c2ea0c5914714469c4defc2562a907230/src/internal/cpu/cpu_x86.go#L9
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_wasm.go#L7
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/sparc/include/asm/cache.h#L19
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/riscv/include/asm/cache.h#L10
//
// All others are assumed to have 64-byte cache line size.
#[cfg_attr(
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc",
        target_arch = "hexagon",
        target_arch = "m68k",
        target_arch = "s390x",
    )),
    repr(align(64))
)]
#[repr(C)]
pub(super) struct Cell<T: Future, S> {
    /// Hot task state data
    pub(super) header: Header,

    /// Either the future or output, depending on the execution stage.
    pub(super) core: Core<T, S>,

    /// Cold data
    pub(super) trailer: Trailer,
}

pub(super) struct CoreStage<T: Future> {
    stage: UnsafeCell<Stage<T>>,
}

/// The core of the task.
///
/// Holds the future or output, depending on the stage of execution.
///
/// Any changes to the layout of this struct _must_ also be reflected in the
/// `const` fns in raw.rs.
#[repr(C)]
pub(super) struct Core<T: Future, S> {
    /// Scheduler used to drive this future.
    pub(super) scheduler: S,

    /// The task's ID, used for populating `JoinError`s.
    pub(super) task_id: Id,

    /// The source code location where the task was spawned.
    ///
    /// This is used for populating the `TaskMeta` passed to the task runtime
    /// hooks.
    #[cfg(tokio_unstable)]
    pub(super) spawned_at: &'static Location<'static>,

    /// Either the future or the output.
    pub(super) stage: CoreStage<T>,
}

/// Crate public as this is also needed by the pool.
#[repr(C)]
pub(crate) struct Header {
    /// Task state.
    pub(super) state: State,

    /// Pointer to next task, used with the injection queue.
    pub(super) queue_next: UnsafeCell<Option<NonNull<Header>>>,

    /// Table of function pointers for executing actions on the task.
    pub(super) vtable: &'static Vtable,

    /// This integer contains the id of the `OwnedTasks` or `LocalOwnedTasks`
    /// that this task is stored in. If the task is not in any list, should be
    /// the id of the list that it was previously in, or `None` if it has never
    /// been in any list.
    ///
    /// Once a task has been bound to a list, it can never be bound to another
    /// list, even if removed from the first list.
    ///
    /// The id is not unset when removed from a list because we want to be able
    /// to read the id without synchronization, even if it is concurrently being
    /// removed from the list.
    pub(super) owner_id: UnsafeCell<Option<NonZeroU64>>,

    /// The tracing ID for this instrumented task.
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    pub(super) tracing_id: Option<tracing::Id>,
}

unsafe impl Send for Header {}
unsafe impl Sync for Header {}

/// Cold data is stored after the future. Data is considered cold if it is only
/// used during creation or shutdown of the task.
pub(super) struct Trailer {
    /// Pointers for the linked list in the `OwnedTasks` that owns this task.
    pub(super) owned: linked_list::Pointers<Header>,
    /// Consumer task waiting on completion of this task.
    pub(super) waker: UnsafeCell<Option<Waker>>,
    /// Optional hooks needed in the harness.
    pub(super) hooks: TaskHarnessScheduleHooks,
}

generate_addr_of_methods! {
    impl<> Trailer {
        pub(super) unsafe fn addr_of_owned(self: NonNull<Self>) -> NonNull<linked_list::Pointers<Header>> {
            &self.owned
        }
    }
}

/// Either the future or the output.
#[repr(C)] // https://github.com/rust-lang/miri/issues/3780
pub(super) enum Stage<T: Future> {
    Running(T),
    Finished(super::Result<T::Output>),
    Consumed,
}

impl<T: Future, S: Schedule> Cell<T, S> {
    /// Allocates a new task cell, containing the header, trailer, and core
    /// structures.
    pub(super) fn new(
        future: T,
        scheduler: S,
        state: State,
        task_id: Id,
        #[cfg(tokio_unstable)] spawned_at: &'static Location<'static>,
    ) -> Box<Cell<T, S>> {
        // Separated into a non-generic function to reduce LLVM codegen
        fn new_header(
            state: State,
            vtable: &'static Vtable,
            #[cfg(all(tokio_unstable, feature = "tracing"))] tracing_id: Option<tracing::Id>,
        ) -> Header {
            Header {
                state,
                queue_next: UnsafeCell::new(None),
                vtable,
                owner_id: UnsafeCell::new(None),
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                tracing_id,
            }
        }

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let tracing_id = future.id();
        let vtable = raw::vtable::<T, S>();
        let result = Box::new(Cell {
            trailer: Trailer::new(scheduler.hooks()),
            header: new_header(
                state,
                vtable,
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                tracing_id,
            ),
            core: Core {
                scheduler,
                stage: CoreStage {
                    stage: UnsafeCell::new(Stage::Running(future)),
                },
                task_id,
                #[cfg(tokio_unstable)]
                spawned_at,
            },
        });

        #[cfg(debug_assertions)]
        {
            // Using a separate function for this code avoids instantiating it separately for every `T`.
            unsafe fn check<S>(
                header: &Header,
                trailer: &Trailer,
                scheduler: &S,
                task_id: &Id,
                #[cfg(tokio_unstable)] spawn_location: &&'static Location<'static>,
            ) {
                let trailer_addr = trailer as *const Trailer as usize;
                let trailer_ptr = unsafe { Header::get_trailer(NonNull::from(header)) };
                assert_eq!(trailer_addr, trailer_ptr.as_ptr() as usize);

                let scheduler_addr = scheduler as *const S as usize;
                let scheduler_ptr = unsafe { Header::get_scheduler::<S>(NonNull::from(header)) };
                assert_eq!(scheduler_addr, scheduler_ptr.as_ptr() as usize);

                let id_addr = task_id as *const Id as usize;
                let id_ptr = unsafe { Header::get_id_ptr(NonNull::from(header)) };
                assert_eq!(id_addr, id_ptr.as_ptr() as usize);

                #[cfg(tokio_unstable)]
                {
                    let spawn_location_addr =
                        spawn_location as *const &'static Location<'static> as usize;
                    let spawn_location_ptr =
                        unsafe { Header::get_spawn_location_ptr(NonNull::from(header)) };
                    assert_eq!(spawn_location_addr, spawn_location_ptr.as_ptr() as usize);
                }
            }
            unsafe {
                check(
                    &result.header,
                    &result.trailer,
                    &result.core.scheduler,
                    &result.core.task_id,
                    #[cfg(tokio_unstable)]
                    &result.core.spawned_at,
                );
            }
        }

        result
    }
}

impl<T: Future> CoreStage<T> {
    pub(super) fn with_mut<R>(&self, f: impl FnOnce(*mut Stage<T>) -> R) -> R {
        self.stage.with_mut(f)
    }
}

/// Set and clear the task id in the context when the future is executed or
/// dropped, or when the output produced by the future is dropped.
pub(crate) struct TaskIdGuard {
    parent_task_id: Option<Id>,
}

impl TaskIdGuard {
    fn enter(id: Id) -> Self {
        TaskIdGuard {
            parent_task_id: context::set_current_task_id(Some(id)),
        }
    }
}

impl Drop for TaskIdGuard {
    fn drop(&mut self) {
        context::set_current_task_id(self.parent_task_id);
    }
}

impl<T: Future, S: Schedule> Core<T, S> {
    /// Polls the future.
    ///
    /// # Safety
    ///
    /// The caller must ensure it is safe to mutate the `state` field. This
    /// requires ensuring mutual exclusion between any concurrent thread that
    /// might modify the future or output field.
    ///
    /// The mutual exclusion is implemented by `Harness` and the `Lifecycle`
    /// component of the task state.
    ///
    /// `self` must also be pinned. This is handled by storing the task on the
    /// heap.
    pub(super) fn poll(&self, mut cx: Context<'_>) -> Poll<T::Output> {
        let res = {
            self.stage.stage.with_mut(|ptr| {
                // Safety: The caller ensures mutual exclusion to the field.
                let future = match unsafe { &mut *ptr } {
                    Stage::Running(future) => future,
                    _ => unreachable!("unexpected stage"),
                };

                // Safety: The caller ensures the future is pinned.
                let future = unsafe { Pin::new_unchecked(future) };

                let _guard = TaskIdGuard::enter(self.task_id);
                future.poll(&mut cx)
            })
        };

        if res.is_ready() {
            self.drop_future_or_output();
        }

        res
    }

    /// Drops the future.
    ///
    /// # Safety
    ///
    /// The caller must ensure it is safe to mutate the `stage` field.
    pub(super) fn drop_future_or_output(&self) {
        // Safety: the caller ensures mutual exclusion to the field.
        unsafe {
            self.set_stage(Stage::Consumed);
        }
    }

    /// Stores the task output.
    ///
    /// # Safety
    ///
    /// The caller must ensure it is safe to mutate the `stage` field.
    pub(super) fn store_output(&self, output: super::Result<T::Output>) {
        // Safety: the caller ensures mutual exclusion to the field.
        unsafe {
            self.set_stage(Stage::Finished(output));
        }
    }

    /// Takes the task output.
    ///
    /// # Safety
    ///
    /// The caller must ensure it is safe to mutate the `stage` field.
    pub(super) fn take_output(&self) -> super::Result<T::Output> {
        use std::mem;

        self.stage.stage.with_mut(|ptr| {
            // Safety:: the caller ensures mutual exclusion to the field.
            match mem::replace(unsafe { &mut *ptr }, Stage::Consumed) {
                Stage::Finished(output) => output,
                _ => panic!("JoinHandle polled after completion"),
            }
        })
    }

    unsafe fn set_stage(&self, stage: Stage<T>) {
        let _guard = TaskIdGuard::enter(self.task_id);
        self.stage.stage.with_mut(|ptr| *ptr = stage);
    }
}

impl Header {
    pub(super) unsafe fn set_next(&self, next: Option<NonNull<Header>>) {
        self.queue_next.with_mut(|ptr| *ptr = next);
    }

    // safety: The caller must guarantee exclusive access to this field, and
    // must ensure that the id is either `None` or the id of the OwnedTasks
    // containing this task.
    pub(super) unsafe fn set_owner_id(&self, owner: NonZeroU64) {
        self.owner_id.with_mut(|ptr| *ptr = Some(owner));
    }

    pub(super) fn get_owner_id(&self) -> Option<NonZeroU64> {
        // safety: If there are concurrent writes, then that write has violated
        // the safety requirements on `set_owner_id`.
        unsafe { self.owner_id.with(|ptr| *ptr) }
    }

    /// Gets a pointer to the `Trailer` of the task containing this `Header`.
    ///
    /// # Safety
    ///
    /// The provided raw pointer must point at the header of a task.
    pub(super) unsafe fn get_trailer(me: NonNull<Header>) -> NonNull<Trailer> {
        let offset = me.as_ref().vtable.trailer_offset;
        let trailer = me.as_ptr().cast::<u8>().add(offset).cast::<Trailer>();
        NonNull::new_unchecked(trailer)
    }

    /// Gets a pointer to the scheduler of the task containing this `Header`.
    ///
    /// # Safety
    ///
    /// The provided raw pointer must point at the header of a task.
    ///
    /// The generic type S must be set to the correct scheduler type for this
    /// task.
    pub(super) unsafe fn get_scheduler<S>(me: NonNull<Header>) -> NonNull<S> {
        let offset = me.as_ref().vtable.scheduler_offset;
        let scheduler = me.as_ptr().cast::<u8>().add(offset).cast::<S>();
        NonNull::new_unchecked(scheduler)
    }

    /// Gets a pointer to the id of the task containing this `Header`.
    ///
    /// # Safety
    ///
    /// The provided raw pointer must point at the header of a task.
    pub(super) unsafe fn get_id_ptr(me: NonNull<Header>) -> NonNull<Id> {
        let offset = me.as_ref().vtable.id_offset;
        let id = me.as_ptr().cast::<u8>().add(offset).cast::<Id>();
        NonNull::new_unchecked(id)
    }

    /// Gets the id of the task containing this `Header`.
    ///
    /// # Safety
    ///
    /// The provided raw pointer must point at the header of a task.
    pub(super) unsafe fn get_id(me: NonNull<Header>) -> Id {
        let ptr = Header::get_id_ptr(me).as_ptr();
        *ptr
    }

    /// Gets a pointer to the source code location where the task containing
    /// this `Header` was spawned.
    ///
    /// # Safety
    ///
    /// The provided raw pointer must point at the header of a task.
    #[cfg(tokio_unstable)]
    pub(super) unsafe fn get_spawn_location_ptr(
        me: NonNull<Header>,
    ) -> NonNull<&'static Location<'static>> {
        let offset = me.as_ref().vtable.spawn_location_offset;
        let spawned_at = me
            .as_ptr()
            .cast::<u8>()
            .add(offset)
            .cast::<&'static Location<'static>>();
        NonNull::new_unchecked(spawned_at)
    }

    /// Gets the source code location where the task containing
    /// this `Header` was spawned
    ///
    /// # Safety
    ///
    /// The provided raw pointer must point at the header of a task.
    #[cfg(tokio_unstable)]
    pub(super) unsafe fn get_spawn_location(me: NonNull<Header>) -> &'static Location<'static> {
        let ptr = Header::get_spawn_location_ptr(me).as_ptr();
        *ptr
    }

    /// Gets the tracing id of the task containing this `Header`.
    ///
    /// # Safety
    ///
    /// The provided raw pointer must point at the header of a task.
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    pub(super) unsafe fn get_tracing_id(me: &NonNull<Header>) -> Option<&tracing::Id> {
        me.as_ref().tracing_id.as_ref()
    }
}

impl Trailer {
    fn new(hooks: TaskHarnessScheduleHooks) -> Self {
        Trailer {
            waker: UnsafeCell::new(None),
            owned: linked_list::Pointers::new(),
            hooks,
        }
    }

    pub(super) unsafe fn set_waker(&self, waker: Option<Waker>) {
        self.waker.with_mut(|ptr| {
            *ptr = waker;
        });
    }

    pub(super) unsafe fn will_wake(&self, waker: &Waker) -> bool {
        self.waker
            .with(|ptr| (*ptr).as_ref().unwrap().will_wake(waker))
    }

    pub(super) fn wake_join(&self) {
        self.waker.with(|ptr| match unsafe { &*ptr } {
            Some(waker) => waker.wake_by_ref(),
            None => panic!("waker missing"),
        });
    }
}

#[test]
#[cfg(not(loom))]
fn header_lte_cache_line() {
    assert!(std::mem::size_of::<Header>() <= 8 * std::mem::size_of::<*const ()>());
}
