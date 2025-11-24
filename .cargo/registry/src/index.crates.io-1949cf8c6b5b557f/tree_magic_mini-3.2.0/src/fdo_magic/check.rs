use petgraph::prelude::*;
use std::cmp::min;
use std::iter::zip;

fn from_u8_singlerule(bytes: &[u8], rule: &super::MagicRule) -> bool {
    // Check if we're even in bounds
    let bound_min = rule.start_off as usize;
    if bound_min > bytes.len() {
        return false;
    }
    let bound_max = rule.start_off as usize + rule.val.len() + rule.region_len as usize;
    let bound_max = min(bound_max, bytes.len());

    let testarea = &bytes[bound_min..bound_max];

    testarea.windows(rule.val.len()).any(|window| {
        // Apply mask to value
        match rule.mask {
            None => rule.val == window,
            Some(mask) => {
                assert_eq!(window.len(), mask.len());
                let masked = zip(window, mask).map(|(a, b)| a & b);
                rule.val.iter().copied().eq(masked)
            }
        }
    })
}

/// Test every given rule by walking graph
/// TODO: Not loving the code duplication here.
pub fn from_u8_walker(
    bytes: &[u8],
    graph: &DiGraph<super::MagicRule, u32>,
    node: NodeIndex,
    isroot: bool,
) -> bool {
    let n = graph.neighbors_directed(node, Outgoing);

    if isroot {
        let rule = &graph[node];

        // Check root
        if !from_u8_singlerule(bytes, rule) {
            return false;
        }

        // Return if that was the only test
        if n.clone().count() == 0 {
            return true;
        }

        // Otherwise next indent level is lower, so continue
    }

    // Check subrules recursively
    for y in n {
        let rule = &graph[y];

        if from_u8_singlerule(bytes, rule) {
            // Check next indent level if needed
            if graph.neighbors_directed(y, Outgoing).count() != 0 {
                return from_u8_walker(bytes, graph, y, false);
            // Next indent level is lower, so this must be it
            } else {
                return true;
            }
        }
    }

    false
}
