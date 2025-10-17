// Performance benchmarks for Web Search and DeepResearch
// Measures response time, memory usage, and throughput

use codex_deep_research::DeepResearcher;
use codex_deep_research::DeepResearcherConfig;
use codex_deep_research::MockProvider;
use codex_deep_research::ResearchStrategy;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use std::sync::Arc;
use std::time::Duration;

// Benchmark 1: Web Search response time
fn bench_web_search_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("web_search_speed");
    group.measurement_time(Duration::from_secs(10));

    let rt = tokio::runtime::Runtime::new().unwrap();
    let provider = Arc::new(MockProvider);

    group.bench_function("mock_search", |b| {
        b.to_async(&rt).iter(|| async {
            let config = DeepResearcherConfig {
                max_depth: 1,
                max_sources: 5,
                strategy: ResearchStrategy::Focused,
            };
            let researcher = DeepResearcher::new(config, provider.clone());
            black_box(researcher.research("test query").await.unwrap())
        });
    });

    group.finish();
}

// Benchmark 2: DeepResearch execution time by depth
fn bench_deep_research_by_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("deep_research_depth");
    group.measurement_time(Duration::from_secs(30));

    let rt = tokio::runtime::Runtime::new().unwrap();
    let provider = Arc::new(MockProvider);

    for depth in [1, 2, 3].iter() {
        let provider = Arc::new(MockProvider);
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, &depth| {
            let provider = Arc::clone(&provider);
            b.to_async(&rt).iter(|| async {
                let config = DeepResearcherConfig {
                    max_depth: depth,
                    max_sources: 10,
                    strategy: ResearchStrategy::Comprehensive,
                };
                let researcher = DeepResearcher::new(config, provider.clone());
                black_box(researcher.research("Rust async").await.unwrap())
            });
        });
    }

    group.finish();
}

// Benchmark 3: DeepResearch execution time by strategy
fn bench_deep_research_by_strategy(c: &mut Criterion) {
    let mut group = c.benchmark_group("deep_research_strategy");
    group.measurement_time(Duration::from_secs(30));

    let rt = tokio::runtime::Runtime::new().unwrap();
    let provider = Arc::new(MockProvider);

    let strategies = vec![
        ("comprehensive", ResearchStrategy::Comprehensive),
        ("focused", ResearchStrategy::Focused),
        ("exploratory", ResearchStrategy::Exploratory),
    ];

    for (name, strategy) in strategies.iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            strategy,
            |b, strategy| {
                b.to_async(&rt).iter(|| async {
                    let config = DeepResearcherConfig {
                        max_depth: 2,
                        max_sources: 10,
                        strategy: strategy.clone(),
                    };
                    let researcher = DeepResearcher::new(config, provider.clone());
                    black_box(researcher.research("Rust patterns").await.unwrap())
                });
            },
        );
    }

    group.finish();
}

// Benchmark 4: Memory usage by source count
fn bench_deep_research_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("deep_research_memory");
    group.measurement_time(Duration::from_secs(20));

    let rt = tokio::runtime::Runtime::new().unwrap();
    let provider = Arc::new(MockProvider);

    for max_sources in [5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(max_sources),
            max_sources,
            |b, &max_sources| {
                b.to_async(&rt).iter(|| async {
                    let config = DeepResearcherConfig {
                        max_depth: 2,
                        max_sources,
                        strategy: ResearchStrategy::Comprehensive,
                    };
                    let researcher = DeepResearcher::new(config, provider.clone());
                    black_box(researcher.research("test").await.unwrap())
                });
            },
        );
    }

    group.finish();
}

// Benchmark 5: Parallel execution throughput
fn bench_parallel_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_throughput");
    group.measurement_time(Duration::from_secs(30));

    let rt = tokio::runtime::Runtime::new().unwrap();
    let provider = Arc::new(MockProvider);

    let queries = vec![
        "Rust async",
        "Rust concurrency",
        "Rust ownership",
        "Rust lifetimes",
    ];

    group.bench_function("sequential_4_queries", |b| {
        b.to_async(&rt).iter(|| async {
            let config = DeepResearcherConfig::default();
            let researcher = DeepResearcher::new(config, provider.clone());

            let mut results = Vec::new();
            for query in &queries {
                results.push(researcher.research(query).await.unwrap());
            }
            black_box(results)
        });
    });

    group.bench_function("parallel_4_queries", |b| {
        b.to_async(&rt).iter(|| async {
            let config = DeepResearcherConfig::default();

            let tasks: Vec<_> = queries
                .iter()
                .map(|query| {
                    let researcher = DeepResearcher::new(config.clone(), provider.clone());
                    let query = query.to_string();
                    tokio::spawn(async move { researcher.research(&query).await.unwrap() })
                })
                .collect();

            let mut results = Vec::new();
            for task in tasks {
                results.push(task.await.unwrap());
            }
            black_box(results)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_web_search_speed,
    bench_deep_research_by_depth,
    bench_deep_research_by_strategy,
    bench_deep_research_memory,
    bench_parallel_throughput
);
criterion_main!(benches);
