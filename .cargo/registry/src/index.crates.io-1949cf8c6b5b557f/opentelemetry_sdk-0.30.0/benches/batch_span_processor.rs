use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use opentelemetry::time::now;
use opentelemetry::trace::{
    SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState,
};
use opentelemetry_sdk::testing::trace::NoopSpanExporter;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::{
    BatchConfigBuilder, BatchSpanProcessor, SpanEvents, SpanLinks, SpanProcessor,
};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn get_span_data() -> Vec<SpanData> {
    (0..200)
        .map(|_| SpanData {
            span_context: SpanContext::new(
                TraceId::from_u128(12),
                SpanId::from_u64(12),
                TraceFlags::default(),
                false,
                TraceState::default(),
            ),
            parent_span_id: SpanId::from_u64(12),
            span_kind: SpanKind::Client,
            name: Default::default(),
            start_time: now(),
            end_time: now(),
            attributes: Vec::new(),
            dropped_attributes_count: 0,
            events: SpanEvents::default(),
            links: SpanLinks::default(),
            status: Status::Unset,
            instrumentation_scope: Default::default(),
        })
        .collect::<Vec<SpanData>>()
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("BatchSpanProcessor");
    group.sample_size(50);

    for task_num in [1, 2, 4, 8, 16, 32].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("with {} concurrent task", task_num)),
            task_num,
            |b, &task_num| {
                b.iter(|| {
                    let rt = Runtime::new().unwrap();
                    rt.block_on(async move {
                        let span_processor = BatchSpanProcessor::builder(NoopSpanExporter::new())
                            .with_batch_config(
                                BatchConfigBuilder::default()
                                    .with_max_queue_size(10_000)
                                    .build(),
                            )
                            .build();
                        let mut shared_span_processor = Arc::new(span_processor);
                        let mut handles = Vec::with_capacity(10);
                        for _ in 0..task_num {
                            let span_processor = shared_span_processor.clone();
                            let spans = get_span_data();
                            handles.push(tokio::spawn(async move {
                                for span in spans {
                                    span_processor.on_end(span);
                                    tokio::task::yield_now().await;
                                }
                            }));
                        }
                        futures_util::future::join_all(handles).await;
                        let _ = Arc::<BatchSpanProcessor>::get_mut(&mut shared_span_processor)
                            .unwrap()
                            .shutdown();
                    });
                })
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(2));
    targets = criterion_benchmark
}
criterion_main!(benches);
