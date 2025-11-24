use std::sync::{Arc, Mutex};
use tracing::subscriber::with_default;
use tracing_core::span::{Attributes, Record};
use tracing_core::{span, Event, Level, LevelFilter, Metadata, Subscriber};
use tracing_log::{LogTracer, NormalizeEvent};

struct State {
    last_normalized_metadata: Mutex<(bool, Option<OwnedMetadata>)>,
}

#[derive(PartialEq, Debug)]
struct OwnedMetadata {
    name: String,
    target: String,
    level: Level,
    module_path: Option<String>,
    file: Option<String>,
    line: Option<u32>,
}

struct TestSubscriber(Arc<State>);

impl Subscriber for TestSubscriber {
    fn enabled(&self, meta: &Metadata<'_>) -> bool {
        dbg!(meta);
        true
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(LevelFilter::from_level(Level::INFO))
    }

    fn new_span(&self, _span: &Attributes<'_>) -> span::Id {
        span::Id::from_u64(42)
    }

    fn record(&self, _span: &span::Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

    fn event(&self, event: &Event<'_>) {
        dbg!(event);
        *self.0.last_normalized_metadata.lock().unwrap() = (
            event.is_log(),
            event.normalized_metadata().map(|normalized| OwnedMetadata {
                name: normalized.name().to_string(),
                target: normalized.target().to_string(),
                level: *normalized.level(),
                module_path: normalized.module_path().map(String::from),
                file: normalized.file().map(String::from),
                line: normalized.line(),
            }),
        )
    }

    fn enter(&self, _span: &span::Id) {}

    fn exit(&self, _span: &span::Id) {}
}

#[test]
fn normalized_metadata() {
    LogTracer::init().unwrap();
    let me = Arc::new(State {
        last_normalized_metadata: Mutex::new((false, None)),
    });
    let state = me.clone();

    with_default(TestSubscriber(me), || {
        log::info!("expected info log");
        log::debug!("unexpected debug log");
        let log = log::Record::builder()
            .args(format_args!("Error!"))
            .level(log::Level::Info)
            .build();
        log::logger().log(&log);
        last(
            &state,
            true,
            Some(OwnedMetadata {
                name: "log event".to_string(),
                target: "".to_string(),
                level: Level::INFO,
                module_path: None,
                file: None,
                line: None,
            }),
        );

        let log = log::Record::builder()
            .args(format_args!("Error!"))
            .level(log::Level::Info)
            .target("log_tracer_target")
            .file(Some("server.rs"))
            .line(Some(144))
            .module_path(Some("log_tracer"))
            .build();
        log::logger().log(&log);
        last(
            &state,
            true,
            Some(OwnedMetadata {
                name: "log event".to_string(),
                target: "log_tracer_target".to_string(),
                level: Level::INFO,
                module_path: Some("log_tracer".to_string()),
                file: Some("server.rs".to_string()),
                line: Some(144),
            }),
        );

        tracing::info!("test with a tracing info");
        last(&state, false, None);
    })
}

fn last(state: &State, should_be_log: bool, expected: Option<OwnedMetadata>) {
    let lock = state.last_normalized_metadata.lock().unwrap();
    let (is_log, metadata) = &*lock;
    dbg!(&metadata);
    assert_eq!(dbg!(*is_log), should_be_log);
    assert_eq!(metadata.as_ref(), expected.as_ref());
}
