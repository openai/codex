//! Performance benchmarks for parallel agent execution.
//!
//! These benchmarks verify that the supervisor meets performance targets:
//! - Cold start < 80ms
//! - RSS < 30MB (measured separately)
//! - 8 agents parallel < 500ms
//! - 100 req/min with no spikes

use codex_supervisor::AgentManager;
use codex_supervisor::AgentState;
use codex_supervisor::AgentType;
use codex_supervisor::MergeStrategy;
use codex_supervisor::SubAgent;
use codex_supervisor::TaskExecutionResult;
use codex_supervisor::TaskType;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark single agent task execution.
fn bench_single_agent(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("single_agent_task", |b| {
        b.to_async(&rt).iter(|| async {
            let mut agent = SubAgent::new(AgentType::CodeExpert);
            let result = agent
                .process_task(black_box("Simple task".to_string()))
                .await;
            black_box(result)
        });
    });
}

/// Benchmark parallel agent execution (2, 4, 8 agents).
fn bench_parallel_agents(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    for agent_count in [2, 4, 8].iter() {
        c.bench_with_input(
            BenchmarkId::new("parallel_agents", agent_count),
            agent_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async move {
                    let mut manager = AgentManager::new();

                    // Create agents
                    for i in 0..count {
                        let agent_type = match i % 3 {
                            0 => AgentType::CodeExpert,
                            1 => AgentType::Researcher,
                            _ => AgentType::Tester,
                        };
                        manager.create_agent(agent_type);
                    }

                    // Execute tasks in parallel
                    let task_type = TaskType::Parallel {
                        agent_types: (0..count)
                            .map(|i| match i % 3 {
                                0 => AgentType::CodeExpert,
                                1 => AgentType::Researcher,
                                _ => AgentType::Tester,
                            })
                            .collect(),
                        task: black_box("Benchmark task".to_string()),
                        merge_strategy: MergeStrategy::Concatenate,
                    };

                    let result = manager.execute_task(task_type).await;
                    black_box(result)
                });
            },
        );
    }
}

/// Benchmark agent state transitions.
fn bench_state_transitions(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("agent_state_transitions", |b| {
        b.to_async(&rt).iter(|| async {
            let mut agent = SubAgent::new(AgentType::CodeExpert);

            // Idle -> Processing -> Idle
            agent.set_state(AgentState::Processing);
            black_box(agent.get_state());
            agent.set_state(AgentState::Idle);
            black_box(agent.get_state());
        });
    });
}

/// Benchmark agent manager creation and cleanup.
fn bench_manager_lifecycle(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("manager_lifecycle", |b| {
        b.to_async(&rt).iter(|| async {
            let mut manager = AgentManager::new();

            // Create multiple agents
            manager.create_agent(AgentType::CodeExpert);
            manager.create_agent(AgentType::Researcher);
            manager.create_agent(AgentType::Tester);

            // Get states
            black_box(manager.get_all_states());

            // Cleanup happens on drop
            drop(manager);
        });
    });
}

/// Benchmark sequential task execution.
fn bench_sequential_tasks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("sequential_tasks", |b| {
        b.to_async(&rt).iter(|| async {
            let mut manager = AgentManager::new();
            manager.create_agent(AgentType::CodeExpert);

            let task_type = TaskType::Sequential {
                agent_types: vec![AgentType::CodeExpert],
                task: black_box("Sequential task".to_string()),
            };

            let result = manager.execute_task(task_type).await;
            black_box(result)
        });
    });
}

/// Benchmark high-throughput scenario (100 tasks).
fn bench_high_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("high_throughput_100_tasks", |b| {
        b.to_async(&rt).iter(|| async {
            let mut manager = AgentManager::new();
            manager.create_agent(AgentType::CodeExpert);
            manager.create_agent(AgentType::Researcher);

            // Execute 100 tasks rapidly
            for i in 0..100 {
                let task_type = TaskType::Parallel {
                    agent_types: vec![AgentType::CodeExpert, AgentType::Researcher],
                    task: black_box(format!("Task {}", i)),
                    merge_strategy: MergeStrategy::Concatenate,
                };

                let _ = manager.execute_task(task_type).await;
            }
        });
    });
}

/// Benchmark agent response merging strategies.
fn bench_merge_strategies(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let strategies = vec![
        ("concatenate", MergeStrategy::Concatenate),
        ("voting", MergeStrategy::Voting),
        ("highest_score", MergeStrategy::HighestScore),
    ];

    for (name, strategy) in strategies {
        c.bench_with_input(
            BenchmarkId::new("merge_strategy", name),
            &strategy,
            |b, strat| {
                b.to_async(&rt).iter(|| async {
                    let mut manager = AgentManager::new();
                    manager.create_agent(AgentType::CodeExpert);
                    manager.create_agent(AgentType::Researcher);
                    manager.create_agent(AgentType::Tester);

                    let task_type = TaskType::Parallel {
                        agent_types: vec![
                            AgentType::CodeExpert,
                            AgentType::Researcher,
                            AgentType::Tester,
                        ],
                        task: black_box("Merge test".to_string()),
                        merge_strategy: strat.clone(),
                    };

                    let result = manager.execute_task(task_type).await;
                    black_box(result)
                });
            },
        );
    }
}

/// Cold start benchmark (agent creation time).
fn bench_cold_start(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("cold_start", |b| {
        b.to_async(&rt).iter(|| async {
            // Measure time to create and initialize an agent
            let agent = SubAgent::new(black_box(AgentType::CodeExpert));
            black_box(agent)
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets =
        bench_cold_start,
        bench_single_agent,
        bench_parallel_agents,
        bench_state_transitions,
        bench_manager_lifecycle,
        bench_sequential_tasks,
        bench_high_throughput,
        bench_merge_strategies,
}

criterion_main!(benches);
