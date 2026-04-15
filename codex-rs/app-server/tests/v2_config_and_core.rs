// Core v2 app-server integration tests that do not depend on the thread/turn
// analytics or websocket helper modules.
#[path = "suite/v2/account.rs"]
mod account;
#[path = "suite/v2/app_list.rs"]
mod app_list;
#[path = "suite/v2/client_metadata.rs"]
mod client_metadata;
#[path = "suite/v2/collaboration_mode_list.rs"]
mod collaboration_mode_list;
#[path = "suite/v2/compaction.rs"]
mod compaction;
#[path = "suite/v2/config_rpc.rs"]
mod config_rpc;
#[path = "suite/v2/dynamic_tools.rs"]
mod dynamic_tools;
#[path = "suite/v2/experimental_api.rs"]
mod experimental_api;
#[path = "suite/v2/experimental_feature_list.rs"]
mod experimental_feature_list;
#[path = "suite/v2/fs.rs"]
mod fs;
#[path = "suite/v2/initialize.rs"]
mod initialize;
#[path = "suite/v2/memory_reset.rs"]
mod memory_reset;
#[path = "suite/v2/model_list.rs"]
mod model_list;
#[path = "suite/v2/output_schema.rs"]
mod output_schema;
#[path = "suite/v2/plan_item.rs"]
mod plan_item;
#[path = "suite/v2/rate_limits.rs"]
mod rate_limits;
#[path = "suite/v2/request_permissions.rs"]
mod request_permissions;
#[path = "suite/v2/request_user_input.rs"]
mod request_user_input;
#[path = "suite/v2/review.rs"]
mod review;
#[path = "suite/v2/safety_check_downgrade.rs"]
mod safety_check_downgrade;
#[path = "suite/v2/skills_list.rs"]
mod skills_list;
#[path = "suite/v2/windows_sandbox_setup.rs"]
mod windows_sandbox_setup;
