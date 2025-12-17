mod orchestrator;
mod q_and_a;
mod reviewer;
mod worker;

use std::collections::HashMap;

use crate::agents::AgentDefinition;

pub(super) fn builtin_agents() -> HashMap<String, AgentDefinition> {
    let mut agents = HashMap::new();

    for agent in [
        orchestrator::definition(),
        worker::definition(),
        reviewer::definition(),
        q_and_a::definition(),
    ] {
        agents.insert(agent.name.clone(), agent);
    }

    agents
}