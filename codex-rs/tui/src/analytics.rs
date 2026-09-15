//! Account analytics data and chart primitives, staged for the dashboard integration.

mod client;
mod data;
mod models;
mod normalize;
mod plot;
mod render;
mod report_data;
mod styles;
mod tokens;

#[cfg(test)]
#[path = "analytics_tests.rs"]
mod tests;
