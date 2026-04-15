// v2 thread and turn integration tests. Keep `analytics` in this shard because
// several thread/turn modules import its helpers via `super::analytics`.
#[path = "suite/v2/analytics.rs"]
mod analytics;
#[path = "suite/v2/thread_archive.rs"]
mod thread_archive;
#[path = "suite/v2/thread_fork.rs"]
mod thread_fork;
#[path = "suite/v2/thread_inject_items.rs"]
mod thread_inject_items;
#[path = "suite/v2/thread_list.rs"]
mod thread_list;
#[path = "suite/v2/thread_loaded_list.rs"]
mod thread_loaded_list;
#[path = "suite/v2/thread_memory_mode_set.rs"]
mod thread_memory_mode_set;
#[path = "suite/v2/thread_metadata_update.rs"]
mod thread_metadata_update;
#[path = "suite/v2/thread_read.rs"]
mod thread_read;
#[path = "suite/v2/thread_resume.rs"]
mod thread_resume;
#[path = "suite/v2/thread_rollback.rs"]
mod thread_rollback;
#[path = "suite/v2/thread_shell_command.rs"]
mod thread_shell_command;
#[path = "suite/v2/thread_start.rs"]
mod thread_start;
#[path = "suite/v2/thread_status.rs"]
mod thread_status;
#[path = "suite/v2/thread_unarchive.rs"]
mod thread_unarchive;
#[path = "suite/v2/thread_unsubscribe.rs"]
mod thread_unsubscribe;
#[path = "suite/v2/turn_interrupt.rs"]
mod turn_interrupt;
#[path = "suite/v2/turn_start.rs"]
mod turn_start;
#[path = "suite/v2/turn_start_zsh_fork.rs"]
mod turn_start_zsh_fork;
#[path = "suite/v2/turn_steer.rs"]
mod turn_steer;
