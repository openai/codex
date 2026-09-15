//! Account analytics data preparation, staged for the dashboard integration.

mod client;
mod data;
mod models;
mod normalize;
mod render;
mod report_data;
mod tokens;

#[cfg(test)]
#[path = "analytics_tests.rs"]
mod tests;
