# codex-supervisor

Multi-agent coordination system for Codex that breaks down natural language goals into executable plans and distributes work across specialized agents.

## Features

- **Goal Analysis**: Converts natural language goals into structured execution plans
- **Task Assignment**: Intelligently assigns plan steps to appropriate agents based on hints
- **Flexible Execution**: Supports multiple coordination strategies:
  - **Sequential**: Execute tasks one after another
  - **Parallel**: Execute all tasks simultaneously
  - **Hybrid**: Mix of sequential and parallel execution
- **Result Aggregation**: Combines outputs using configurable merge strategies:
  - **Concatenate**: Combine all results
  - **Voting**: Democratic selection based on success rate
  - **FirstSuccess**: Take the first successful result
  - **HighestScore**: Select the best-scoring result
- **Management Styles**: Autocratic, Democratic, Laissez-faire

## Usage

```rust
use codex_supervisor::{Supervisor, types::SupervisorConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = SupervisorConfig::default();
    let supervisor = Supervisor::new(config);

    let result = supervisor.coordinate_goal(
        "Implement secure authentication",
        Some(vec!["Security".to_string(), "Backend".to_string(), "Frontend".to_string()])
    ).await?;

    println!("Goal: {}", result.goal);
    println!("Plan steps: {}", result.plan.steps.len());
    println!("Results: {}", result.results.summary);

    Ok(())
}
```

## CLI Usage

```bash
# Basic usage
codex supervisor "Implement secure auth"

# With specific agents
codex supervisor "Build API" --agents "Backend,Database,Testing"

# With coordination strategy
codex supervisor "Refactor codebase" --strategy parallel

# JSON output
codex supervisor "Deploy service" --format json
```

## Architecture

The supervisor follows a pipeline architecture:

1. **Planner** (`planner.rs`): Analyzes goals and generates execution plans
2. **Assigner** (`assigner.rs`): Maps steps to agents
3. **Executor** (`executor.rs`): Runs tasks according to coordination strategy
4. **Aggregator** (`aggregator.rs`): Combines results using merge strategy

## Testing

Run the test suite:

```bash
cargo test -p codex-supervisor
```

All tests pass with 100% success rate (15 tests).

## Future Enhancements

- LLM integration for intelligent plan generation
- Integration with codex-subagents for real agent execution
- Dynamic replanning based on execution results
- Cost estimation and optimization
