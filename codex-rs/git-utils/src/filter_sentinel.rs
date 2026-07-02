use std::io;

const MAX_SENTINEL_FILTER_CHILD_PROBES: usize = 16;

#[derive(Debug, Default)]
pub(crate) struct SentinelFilterProbeBudget {
    completed_child_probes: usize,
}

impl SentinelFilterProbeBudget {
    pub(crate) const fn max_probes() -> usize {
        MAX_SENTINEL_FILTER_CHILD_PROBES
    }

    pub(crate) fn ensure_probe_available(&self) -> io::Result<()> {
        if self.completed_child_probes >= MAX_SENTINEL_FILTER_CHILD_PROBES {
            return Err(io::Error::other(format!(
                "refusing to continue Git filter sentinel disambiguation after {} child probes (hard limit: {})",
                self.completed_child_probes, MAX_SENTINEL_FILTER_CHILD_PROBES
            )));
        }
        Ok(())
    }

    pub(crate) fn record_completed_probe(&mut self) {
        debug_assert!(self.completed_child_probes < MAX_SENTINEL_FILTER_CHILD_PROBES);
        self.completed_child_probes += 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SentinelFilterProbeResolution {
    SpecialAttributeState,
    NeedsOptionalProbe,
    LiteralDriver,
    ProbeFailure,
}

pub(crate) fn classify_sentinel_filter_probes(
    required_succeeded: bool,
    optional_succeeded: Option<bool>,
) -> SentinelFilterProbeResolution {
    if required_succeeded {
        SentinelFilterProbeResolution::SpecialAttributeState
    } else {
        match optional_succeeded {
            None => SentinelFilterProbeResolution::NeedsOptionalProbe,
            Some(true) => SentinelFilterProbeResolution::LiteralDriver,
            Some(false) => SentinelFilterProbeResolution::ProbeFailure,
        }
    }
}

#[cfg(test)]
pub(crate) fn sentinel_filter_probe_config_args(
    neutralization_args: &[String],
    driver: &str,
    required: bool,
) -> io::Result<Vec<String>> {
    if !matches!(driver, "set" | "unset" | "unspecified") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Git filter sentinel probe requested for a non-sentinel driver",
        ));
    }
    let mut args = Vec::with_capacity(neutralization_args.len() + 2);
    args.extend_from_slice(neutralization_args);
    args.push("-c".to_string());
    args.push(format!("filter.{driver}.required={required}"));
    Ok(args)
}
