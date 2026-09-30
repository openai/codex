mod action;
mod approval;
mod authorization;
mod classification;
mod config;
mod coverage;
mod extension;
mod metrics;
mod observation;
mod parent_compaction;
// The treatment backend wires this builder into sampling in the next PR.
#[allow(dead_code)]
mod request;
mod sampler;
mod score;
mod startup;
mod transcript;
mod truncation;
mod trusted_skills;
mod trusted_tools;
mod wrapper_lag;

pub(crate) use extension::install;
