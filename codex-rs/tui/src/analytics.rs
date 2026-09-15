//! Account analytics data preparation, staged for the dashboard integration.

mod data;
mod models;
mod normalize;
mod report_data;
mod tokens;

#[cfg(test)]
#[path = "analytics_tests.rs"]
mod tests;
