use std::time::Instant;

use crate::status_indicator_shimmer::StatusShimmer;

#[doc(hidden)]
pub fn entertainment_header_from_arc(arc: Vec<&str>) -> String {
    let now = Instant::now();
    let mut shimmer = StatusShimmer::new(now, true);
    let arc: Vec<String> = arc.into_iter().map(|text| text.to_string()).collect();
    shimmer.set_entertainment_arcs(vec![arc]);
    shimmer.render_header(now).text
}
