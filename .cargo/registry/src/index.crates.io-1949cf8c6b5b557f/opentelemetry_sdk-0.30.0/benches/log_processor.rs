/*
    The benchmark results:
    criterion = "0.5.1"
    OS: Ubuntu 22.04.3 LTS (5.15.146.1-microsoft-standard-WSL2)
    Hardware: AMD EPYC 7763 64-Core Processor - 2.44 GHz, 16vCPUs,
    RAM: 64.0 GB
    | Test                                        | Average time|
    |---------------------------------------------|-------------|
    | log_noop_processor                          | 134 ns      |
    | log_cloning_processor                       | 236 ns      |
    | log_clone_and_send_to_channel_processor     | 403 ns      |
*/

use opentelemetry::time::now;
use std::{
    sync::{Arc, Mutex},
    thread::sleep,
};

use criterion::{criterion_group, criterion_main, Criterion};
use opentelemetry::{
    logs::{LogRecord as _, Logger, LoggerProvider, Severity},
    InstrumentationScope,
};
use opentelemetry_sdk::{
    error::OTelSdkResult,
    logs::{LogProcessor, SdkLogRecord, SdkLogger, SdkLoggerProvider},
};

// Run this benchmark with:
// cargo bench --bench log_processor

fn create_log_record(logger: &SdkLogger) -> SdkLogRecord {
    let mut log_record = logger.create_log_record();
    let now = now();
    log_record.set_observed_timestamp(now);
    log_record.set_target("my-target".to_string());
    log_record.set_event_name("CheckoutFailed");
    log_record.set_severity_number(Severity::Warn);
    log_record.set_severity_text("WARN");
    log_record.add_attribute("book_id", "12345");
    log_record.add_attribute("book_title", "Rust Programming Adventures");
    log_record.add_attribute("message", "Unable to process checkout.");
    log_record
}

#[derive(Debug)]
struct NoopProcessor;

impl LogProcessor for NoopProcessor {
    fn emit(&self, _data: &mut SdkLogRecord, _scope: &InstrumentationScope) {}

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }
}

#[derive(Debug)]
struct CloningProcessor;

impl LogProcessor for CloningProcessor {
    fn emit(&self, data: &mut SdkLogRecord, _scope: &InstrumentationScope) {
        let _data_cloned = data.clone();
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }
}

#[derive(Debug)]
struct SendToChannelProcessor {
    sender: std::sync::mpsc::Sender<(SdkLogRecord, InstrumentationScope)>,
    receiver: Arc<Mutex<std::sync::mpsc::Receiver<(SdkLogRecord, InstrumentationScope)>>>,
}

impl SendToChannelProcessor {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let s = Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
        };
        let receiver_cloned = s.receiver.clone();
        let _ = std::thread::spawn(move || loop {
            sleep(std::time::Duration::from_millis(10));
            let data = receiver_cloned.lock().unwrap().recv();
            if data.is_err() {
                println!(
                    "Error receiving log data from channel {0}",
                    data.err().unwrap()
                );
                break;
            }
        });
        s
    }
}

impl LogProcessor for SendToChannelProcessor {
    fn emit(&self, record: &mut SdkLogRecord, scope: &InstrumentationScope) {
        let res = self.sender.send((record.clone(), scope.clone()));
        if res.is_err() {
            println!("Error sending log data to channel {0}", res.err().unwrap());
        }
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    log_noop_processor(c);
    log_cloning_processor(c);
    log_cloning_and_send_to_channel_processor(c);
}

fn log_noop_processor(c: &mut Criterion) {
    let provider = SdkLoggerProvider::builder()
        .with_log_processor(NoopProcessor {})
        .build();
    let logger = provider.logger("benchmark");

    c.bench_function("log_noop_processor", |b| {
        b.iter(|| {
            let log_record = create_log_record(&logger);
            logger.emit(log_record);
        });
    });
}

fn log_cloning_processor(c: &mut Criterion) {
    let provider = SdkLoggerProvider::builder()
        .with_log_processor(CloningProcessor {})
        .build();
    let logger = provider.logger("benchmark");

    c.bench_function("log_cloning_processor", |b| {
        b.iter(|| {
            let log_record = create_log_record(&logger);
            logger.emit(log_record);
        });
    });
}

fn log_cloning_and_send_to_channel_processor(c: &mut Criterion) {
    let provider = SdkLoggerProvider::builder()
        .with_log_processor(SendToChannelProcessor::new())
        .build();
    let logger = provider.logger("benchmark");

    c.bench_function("log_clone_and_send_to_channel_processor", |b| {
        b.iter(|| {
            let log_record = create_log_record(&logger);
            logger.emit(log_record);
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(2));
    targets = criterion_benchmark
}
criterion_main!(benches);
