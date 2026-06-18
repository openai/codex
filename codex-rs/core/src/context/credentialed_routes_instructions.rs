use super::ContextualUserFragment;
use std::collections::BTreeSet;

const MAX_INSTRUCTION_CHARS: usize = 1_000;
const MAX_ROUTE_PREFIXES: usize = 50;
const OMITTED_ROUTES: &str = "\n- [additional credentialed routes omitted]";
const OPEN_TAG: &str = "<credentialed_routes>";
const CLOSE_TAG: &str = "</credentialed_routes>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CredentialedRoutesInstructions {
    route_prefixes: Vec<String>,
}

impl CredentialedRoutesInstructions {
    pub(crate) fn new(route_prefixes: &[String]) -> Option<Self> {
        let route_prefixes = route_prefixes
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        (!route_prefixes.is_empty()).then_some(Self { route_prefixes })
    }
}

impl ContextualUserFragment for CredentialedRoutesInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (OPEN_TAG, CLOSE_TAG)
    }

    fn body(&self) -> String {
        let mut instructions = "\nThe managed network proxy automatically attaches stored credentials when you call these HTTPS URL prefixes directly:".to_string();
        let route_count = self.route_prefixes.len();
        let mut omitted = false;
        for (index, route_prefix) in self.route_prefixes.iter().enumerate() {
            let route_prefix = format!("\n- {route_prefix}");
            let omitted_suffix_len = if index + 1 < route_count {
                OMITTED_ROUTES.len()
            } else {
                0
            };
            if index == MAX_ROUTE_PREFIXES
                || instructions.len() + route_prefix.len() + omitted_suffix_len + 1
                    > MAX_INSTRUCTION_CHARS
            {
                omitted = true;
                break;
            }
            instructions.push_str(&route_prefix);
        }
        if omitted {
            instructions.push_str(OMITTED_ROUTES);
        }
        instructions.push('\n');
        instructions
    }
}

#[cfg(test)]
#[path = "credentialed_routes_instructions_tests.rs"]
mod tests;
