pub mod installed_marketplaces;
pub mod loader;
pub mod manifest;
pub mod marketplace;
pub mod marketplace_add;
pub mod marketplace_remove;
pub mod marketplace_upgrade;
pub mod remote;
pub mod remote_bundle;
pub mod remote_legacy;
pub mod startup_sync;
pub mod store;
pub mod toggles;

pub const OPENAI_CURATED_MARKETPLACE_NAME: &str = "openai-curated";
pub const OPENAI_BUNDLED_MARKETPLACE_NAME: &str = "openai-bundled";

pub const TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST: &[&str] = &[
    "github@openai-curated",
    "notion@openai-curated",
    "slack@openai-curated",
    "gmail@openai-curated",
    "google-calendar@openai-curated",
    "google-drive@openai-curated",
    "teams@openai-curated",
    "sharepoint@openai-curated",
    "outlook-email@openai-curated",
    "outlook-calendar@openai-curated",
    "linear@openai-curated",
    "figma@openai-curated",
    "computer-use@openai-bundled",
];
