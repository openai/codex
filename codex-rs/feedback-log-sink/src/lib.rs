//! Remote feedback log sink protocol helpers.

use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_state::LogEntry;
use codex_state::log_db::LogSinkQueueConfig;
use codex_state::log_db::LogWriter;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tonic::transport::Channel;
use tonic::transport::Endpoint;
use tracing::Event;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::span::Attributes;
use tracing::span::Id;
use tracing::span::Record;
use tracing_subscriber::Layer;
use tracing_subscriber::field::RecordFields;
use tracing_subscriber::fmt::FormatFields;
use tracing_subscriber::fmt::FormattedFields;
use tracing_subscriber::fmt::format::DefaultFields;
use tracing_subscriber::registry::LookupSpan;
use uuid::Uuid;

#[path = "proto/codex.feedback_log_sink.v1.rs"]
pub mod proto;

use proto::feedback_log_sink_client::FeedbackLogSinkClient;

const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrpcFeedbackLogSinkConfig {
    pub endpoint: String,
    pub queue: LogSinkQueueConfig,
    pub rpc_timeout: Duration,
    pub source_process_uuid: Option<String>,
}

impl GrpcFeedbackLogSinkConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            queue: LogSinkQueueConfig::default(),
            rpc_timeout: DEFAULT_RPC_TIMEOUT,
            source_process_uuid: None,
        }
    }

    fn normalized(self) -> Self {
        Self {
            endpoint: self.endpoint,
            queue: self.queue,
            rpc_timeout: if self.rpc_timeout.is_zero() {
                DEFAULT_RPC_TIMEOUT
            } else {
                self.rpc_timeout
            },
            source_process_uuid: self.source_process_uuid,
        }
    }
}

pub struct GrpcFeedbackLogSinkLayer {
    sender: mpsc::Sender<RemoteLogCommand>,
    process_uuid: String,
}

impl GrpcFeedbackLogSinkLayer {
    pub fn start(config: GrpcFeedbackLogSinkConfig) -> Result<Self, tonic::transport::Error> {
        let config = config.normalized();
        let endpoint = Endpoint::from_shared(config.endpoint)?
            .connect_timeout(config.rpc_timeout)
            .timeout(config.rpc_timeout);
        let client = FeedbackLogSinkClient::new(endpoint.connect_lazy());
        let (sender, receiver) = mpsc::channel(config.queue.queue_capacity.max(1));
        let process_uuid = config
            .source_process_uuid
            .unwrap_or_else(|| current_process_log_uuid().to_string());
        tokio::spawn(run_grpc_sink(
            client,
            receiver,
            config.queue,
            config.rpc_timeout,
            process_uuid.clone(),
        ));
        Ok(Self {
            sender,
            process_uuid,
        })
    }

    pub async fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.sender.send(RemoteLogCommand::Flush(tx)).await.is_ok() {
            let _ = rx.await;
        }
    }

    fn try_send(&self, entry: LogEntry) {
        let _ = self
            .sender
            .try_send(RemoteLogCommand::Entry(Box::new(entry)));
    }
}

impl Clone for GrpcFeedbackLogSinkLayer {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            process_uuid: self.process_uuid.clone(),
        }
    }
}

impl<S> Layer<S> for GrpcFeedbackLogSinkLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &Attributes<'_>,
        id: &Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = SpanFieldVisitor::default();
        attrs.record(&mut visitor);

        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SpanLogContext {
                name: span.metadata().name().to_string(),
                formatted_fields: format_fields(attrs),
                thread_id: visitor.thread_id,
            });
        }
    }

    fn on_record(
        &self,
        id: &Id,
        values: &Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = SpanFieldVisitor::default();
        values.record(&mut visitor);

        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if let Some(log_context) = extensions.get_mut::<SpanLogContext>() {
                if let Some(thread_id) = visitor.thread_id {
                    log_context.thread_id = Some(thread_id);
                }
                append_fields(&mut log_context.formatted_fields, values);
            } else {
                extensions.insert(SpanLogContext {
                    name: span.metadata().name().to_string(),
                    formatted_fields: format_fields(values),
                    thread_id: visitor.thread_id,
                });
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let thread_id = visitor
            .thread_id
            .clone()
            .or_else(|| event_thread_id(event, &ctx));
        let feedback_log_body = format_feedback_log_body(event, &ctx);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        let entry = LogEntry {
            ts: now.as_secs() as i64,
            ts_nanos: now.subsec_nanos() as i64,
            level: metadata.level().as_str().to_string(),
            target: metadata.target().to_string(),
            message: visitor.message,
            feedback_log_body: Some(feedback_log_body),
            thread_id,
            process_uuid: Some(self.process_uuid.clone()),
            module_path: metadata.module_path().map(ToString::to_string),
            file: metadata.file().map(ToString::to_string),
            line: metadata.line().map(|line| line as i64),
        };

        self.try_send(entry);
    }
}

impl<S> LogWriter<S> for GrpcFeedbackLogSinkLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn flush(&self) -> impl Future<Output = ()> + Send + '_ {
        GrpcFeedbackLogSinkLayer::flush(self)
    }
}

enum RemoteLogCommand {
    Entry(Box<LogEntry>),
    Flush(oneshot::Sender<()>),
}

async fn run_grpc_sink(
    mut client: FeedbackLogSinkClient<Channel>,
    mut receiver: mpsc::Receiver<RemoteLogCommand>,
    config: LogSinkQueueConfig,
    rpc_timeout: Duration,
    source_process_uuid: String,
) {
    let batch_size = config.batch_size.max(1);
    let flush_interval = if config.flush_interval.is_zero() {
        LogSinkQueueConfig::default().flush_interval
    } else {
        config.flush_interval
    };
    let mut buffer = Vec::with_capacity(batch_size);
    let mut ticker = tokio::time::interval(flush_interval);
    loop {
        tokio::select! {
            maybe_command = receiver.recv() => {
                match maybe_command {
                    Some(RemoteLogCommand::Entry(entry)) => {
                        buffer.push(*entry);
                        if buffer.len() >= batch_size {
                            flush_remote(&mut client, &mut buffer, &source_process_uuid, rpc_timeout).await;
                        }
                    }
                    Some(RemoteLogCommand::Flush(reply)) => {
                        flush_remote(&mut client, &mut buffer, &source_process_uuid, rpc_timeout).await;
                        let _ = reply.send(());
                    }
                    None => {
                        flush_remote(&mut client, &mut buffer, &source_process_uuid, rpc_timeout).await;
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                flush_remote(&mut client, &mut buffer, &source_process_uuid, rpc_timeout).await;
            }
        }
    }
}

async fn flush_remote(
    client: &mut FeedbackLogSinkClient<Channel>,
    buffer: &mut Vec<LogEntry>,
    source_process_uuid: &str,
    rpc_timeout: Duration,
) {
    if buffer.is_empty() {
        return;
    }
    let entries = buffer.split_off(0);
    let request = append_log_batch_request(entries, source_process_uuid.to_string());
    let _ = tokio::time::timeout(rpc_timeout, client.append_log_batch(request)).await;
}

impl From<LogEntry> for proto::FeedbackLogEntry {
    fn from(entry: LogEntry) -> Self {
        Self {
            ts: entry.ts,
            ts_nanos: entry.ts_nanos,
            level: entry.level,
            target: entry.target,
            message: entry.message,
            feedback_log_body: entry.feedback_log_body,
            thread_id: entry.thread_id,
            process_uuid: entry.process_uuid,
            module_path: entry.module_path,
            file: entry.file,
            line: entry.line,
        }
    }
}

pub fn append_log_batch_request(
    entries: Vec<LogEntry>,
    source_process_uuid: impl Into<String>,
) -> proto::AppendLogBatchRequest {
    proto::AppendLogBatchRequest {
        entries: entries.into_iter().map(Into::into).collect(),
        source_process_uuid: source_process_uuid.into(),
    }
}

fn current_process_log_uuid() -> &'static str {
    static PROCESS_LOG_UUID: OnceLock<String> = OnceLock::new();
    PROCESS_LOG_UUID.get_or_init(|| {
        let pid = std::process::id();
        let process_uuid = Uuid::new_v4();
        format!("pid:{pid}:{process_uuid}")
    })
}

#[derive(Debug)]
struct SpanLogContext {
    name: String,
    formatted_fields: String,
    thread_id: Option<String>,
}

#[derive(Default)]
struct SpanFieldVisitor {
    thread_id: Option<String>,
}

impl SpanFieldVisitor {
    fn record_field(&mut self, field: &Field, value: String) {
        if field.name() == "thread_id" && self.thread_id.is_none() {
            self.thread_id = Some(value);
        }
    }
}

impl Visit for SpanFieldVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_field(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_field(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_field(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_field(field, value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_field(field, value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_field(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_field(field, format!("{value:?}"));
    }
}

fn event_thread_id<S>(
    event: &Event<'_>,
    ctx: &tracing_subscriber::layer::Context<'_, S>,
) -> Option<String>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    let mut thread_id = None;
    if let Some(scope) = ctx.event_scope(event) {
        for span in scope.from_root() {
            let extensions = span.extensions();
            if let Some(log_context) = extensions.get::<SpanLogContext>()
                && log_context.thread_id.is_some()
            {
                thread_id = log_context.thread_id.clone();
            }
        }
    }
    thread_id
}

fn format_feedback_log_body<S>(
    event: &Event<'_>,
    ctx: &tracing_subscriber::layer::Context<'_, S>,
) -> String
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    let mut feedback_log_body = String::new();
    if let Some(scope) = ctx.event_scope(event) {
        for span in scope.from_root() {
            let extensions = span.extensions();
            if let Some(log_context) = extensions.get::<SpanLogContext>() {
                feedback_log_body.push_str(&log_context.name);
                if !log_context.formatted_fields.is_empty() {
                    feedback_log_body.push('{');
                    feedback_log_body.push_str(&log_context.formatted_fields);
                    feedback_log_body.push('}');
                }
            } else {
                feedback_log_body.push_str(span.metadata().name());
            }
            feedback_log_body.push(':');
        }
        if !feedback_log_body.is_empty() {
            feedback_log_body.push(' ');
        }
    }
    feedback_log_body.push_str(&format_fields(event));
    feedback_log_body
}

fn format_fields<R>(fields: R) -> String
where
    R: RecordFields,
{
    let formatter = DefaultFields::default();
    let mut formatted = FormattedFields::<DefaultFields>::new(String::new());
    let _ = formatter.format_fields(formatted.as_writer(), fields);
    formatted.fields
}

fn append_fields(fields: &mut String, values: &Record<'_>) {
    let formatter = DefaultFields::default();
    let mut formatted = FormattedFields::<DefaultFields>::new(std::mem::take(fields));
    let _ = formatter.add_fields(&mut formatted, values);
    *fields = formatted.fields;
}

#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
    thread_id: Option<String>,
}

impl MessageVisitor {
    fn record_field(&mut self, field: &Field, value: String) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(value.clone());
        }
        if field.name() == "thread_id" && self.thread_id.is_none() {
            self.thread_id = Some(value);
        }
    }
}

impl Visit for MessageVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_field(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_field(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_field(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_field(field, value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_field(field, value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_field(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_field(field, format!("{value:?}"));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    use codex_state::LogEntry;
    use codex_state::log_db::LogSinkQueueConfig;
    use pretty_assertions::assert_eq;
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::Response;
    use tonic::Status;
    use tonic::transport::Server;
    use tracing_subscriber::Layer;
    use tracing_subscriber::filter::Targets;
    use tracing_subscriber::layer::SubscriberExt;

    use super::GrpcFeedbackLogSinkConfig;
    use super::GrpcFeedbackLogSinkLayer;
    use super::append_log_batch_request;
    use super::proto;
    use super::proto::feedback_log_sink_server::FeedbackLogSink;
    use super::proto::feedback_log_sink_server::FeedbackLogSinkServer;

    #[test]
    fn log_entry_to_proto_preserves_all_fields() {
        let entry = populated_log_entry();

        let actual = proto::FeedbackLogEntry::from(entry);

        assert_eq!(actual, populated_feedback_log_entry());
    }

    #[test]
    fn log_entry_to_proto_preserves_absent_optional_fields() {
        let entry = LogEntry {
            ts: 1700000000,
            ts_nanos: 42,
            level: "WARN".to_string(),
            target: "codex::target".to_string(),
            message: None,
            feedback_log_body: None,
            thread_id: None,
            process_uuid: None,
            module_path: None,
            file: None,
            line: None,
        };

        let actual = proto::FeedbackLogEntry::from(entry);

        assert_eq!(
            actual,
            proto::FeedbackLogEntry {
                ts: 1700000000,
                ts_nanos: 42,
                level: "WARN".to_string(),
                target: "codex::target".to_string(),
                message: None,
                feedback_log_body: None,
                thread_id: None,
                process_uuid: None,
                module_path: None,
                file: None,
                line: None,
            }
        );
    }

    #[test]
    fn append_log_batch_request_sets_source_process_uuid_and_entries() {
        let actual = append_log_batch_request(vec![populated_log_entry()], "source-process");

        assert_eq!(
            actual,
            proto::AppendLogBatchRequest {
                entries: vec![populated_feedback_log_entry()],
                source_process_uuid: "source-process".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn grpc_layer_sends_tracing_events_on_flush() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let service = RecordingFeedbackLogSink {
            requests: Arc::clone(&requests),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fake sink server");
        let endpoint = format!("http://{}", listener.local_addr().expect("local addr"));
        let server = tokio::spawn(
            Server::builder()
                .add_service(FeedbackLogSinkServer::new(service))
                .serve_with_incoming(TcpListenerStream::new(listener)),
        );
        let layer = GrpcFeedbackLogSinkLayer::start(GrpcFeedbackLogSinkConfig {
            endpoint,
            queue: LogSinkQueueConfig {
                queue_capacity: 8,
                batch_size: 8,
                flush_interval: Duration::from_secs(60),
            },
            rpc_timeout: Duration::from_secs(2),
            source_process_uuid: Some("source-process".to_string()),
        })
        .expect("start grpc feedback log sink");

        let subscriber = tracing_subscriber::registry().with(
            layer
                .clone()
                .with_filter(Targets::new().with_default(tracing::Level::TRACE)),
        );
        let dispatch = tracing::Dispatch::new(subscriber);
        tracing::dispatcher::with_default(&dispatch, || {
            tracing::info_span!("remote-feedback-thread", thread_id = "thread-1", turn = 7)
                .in_scope(|| {
                    tracing::info!(foo = 2, "remote-log");
                });
        });
        layer.flush().await;

        let requests = requests.lock().expect("requests mutex poisoned");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.source_process_uuid, "source-process");
        assert_eq!(request.entries.len(), 1);
        let entry = &request.entries[0];
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.message.as_deref(), Some("remote-log"));
        assert_eq!(
            entry.feedback_log_body.as_deref(),
            Some("remote-feedback-thread{thread_id=\"thread-1\" turn=7}: remote-log foo=2")
        );
        assert_eq!(entry.thread_id.as_deref(), Some("thread-1"));
        assert_eq!(entry.process_uuid.as_deref(), Some("source-process"));

        server.abort();
    }

    #[derive(Clone)]
    struct RecordingFeedbackLogSink {
        requests: Arc<Mutex<Vec<proto::AppendLogBatchRequest>>>,
    }

    #[tonic::async_trait]
    impl FeedbackLogSink for RecordingFeedbackLogSink {
        async fn append_log_batch(
            &self,
            request: tonic::Request<proto::AppendLogBatchRequest>,
        ) -> Result<Response<proto::AppendLogBatchResponse>, Status> {
            self.requests
                .lock()
                .expect("requests mutex poisoned")
                .push(request.into_inner());
            Ok(Response::new(proto::AppendLogBatchResponse {}))
        }
    }

    fn populated_log_entry() -> LogEntry {
        LogEntry {
            ts: 1700000000,
            ts_nanos: 123456789,
            level: "INFO".to_string(),
            target: "codex::feedback".to_string(),
            message: Some("captured message".to_string()),
            feedback_log_body: Some("structured body".to_string()),
            thread_id: Some("thread-1".to_string()),
            process_uuid: Some("process-entry".to_string()),
            module_path: Some("codex_state::log_db".to_string()),
            file: Some("state/src/log_db.rs".to_string()),
            line: Some(123),
        }
    }

    fn populated_feedback_log_entry() -> proto::FeedbackLogEntry {
        proto::FeedbackLogEntry {
            ts: 1700000000,
            ts_nanos: 123456789,
            level: "INFO".to_string(),
            target: "codex::feedback".to_string(),
            message: Some("captured message".to_string()),
            feedback_log_body: Some("structured body".to_string()),
            thread_id: Some("thread-1".to_string()),
            process_uuid: Some("process-entry".to_string()),
            module_path: Some("codex_state::log_db".to_string()),
            file: Some("state/src/log_db.rs".to_string()),
            line: Some(123),
        }
    }
}
