use crate::render::truncate_utf8_to_bytes;

const MAX_WARNINGS: usize = 4;
const MAX_WARNING_BYTES: usize = 256;

pub(crate) fn bounded_warnings(warnings: &[String]) -> Vec<String> {
    warnings
        .iter()
        .take(MAX_WARNINGS)
        .map(|warning| truncate_utf8_to_bytes(warning, MAX_WARNING_BYTES).0)
        .collect()
}
