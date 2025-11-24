#[cfg(feature = "test-util")]
use crate::runtime::scheduler;
use crate::runtime::task::{self, Task, TaskHarnessScheduleHooks};
use crate::runtime::Handle;

/// `task::Schedule` implementation that does nothing (except some bookkeeping
/// in test-util builds). This is unique to the blocking scheduler as tasks
/// scheduled are not really futures but blocking operations.
///
/// We avoid storing the task by forgetting it in `bind` and re-materializing it
/// in `release`.
pub(crate) struct BlockingSchedule {
    #[cfg(feature = "test-util")]
    handle: Handle,
    hooks: TaskHarnessScheduleHooks,
}

impl BlockingSchedule {
    #[cfg_attr(not(feature = "test-util"), allow(unused_variables))]
    pub(crate) fn new(handle: &Handle) -> Self {
        #[cfg(feature = "test-util")]
        {
            match &handle.inner {
                scheduler::Handle::CurrentThread(handle) => {
                    handle.driver.clock.inhibit_auto_advance();
                }
                #[cfg(feature = "rt-multi-thread")]
                scheduler::Handle::MultiThread(_) => {}
            }
        }
        BlockingSchedule {
            #[cfg(feature = "test-util")]
            handle: handle.clone(),
            hooks: TaskHarnessScheduleHooks {
                task_terminate_callback: handle.inner.hooks().task_terminate_callback.clone(),
            },
        }
    }
}

impl task::Schedule for BlockingSchedule {
    fn release(&self, _task: &Task<Self>) -> Option<Task<Self>> {
        #[cfg(feature = "test-util")]
        {
            match &self.handle.inner {
                scheduler::Handle::CurrentThread(handle) => {
                    handle.driver.clock.allow_auto_advance();
                    handle.driver.unpark();
                }
                #[cfg(feature = "rt-multi-thread")]
                scheduler::Handle::MultiThread(_) => {}
            }
        }
        None
    }

    fn schedule(&self, _task: task::Notified<Self>) {
        unreachable!();
    }

    fn hooks(&self) -> TaskHarnessScheduleHooks {
        TaskHarnessScheduleHooks {
            task_terminate_callback: self.hooks.task_terminate_callback.clone(),
        }
    }
}
