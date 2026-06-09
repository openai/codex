#![cfg(unix)]

use super::*;
use crate::config::STATSIG_OTLP_HTTP_ENDPOINT;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::thread;

#[test]
fn process_exit_upload_contains_final_metric() {
    let temp_dir = tempfile::tempdir().expect("create temporary directory");
    let executable = temp_dir.path().join("upload-helper");
    let output = temp_dir.path().join("upload-helper.output");
    fs::write(&executable, "#!/bin/sh\ncat > \"$0.output\"\n").expect("write upload helper");
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).expect("make upload helper executable");

    let (exporter, process_exit_upload) = build_http_metric_exporter(
        STATSIG_OTLP_HTTP_ENDPOINT.to_string(),
        HashMap::new(),
        OtelHttpProtocol::Json,
        None,
        Temporality::Delta,
        /*process_exit_upload*/ true,
    )
    .expect("build Statsig metric exporter");
    let upload = process_exit_upload.expect("process-exit upload state");
    upload.configure_executable(executable);
    assert!(upload.prepare());

    let (provider, meter) = build_provider(
        Resource::builder_empty().build(),
        exporter,
        Some(Duration::from_secs(/*secs*/ 3600)),
        None,
    );
    meter
        .u64_counter("codex.test.process_exit")
        .build()
        .add(1, &[]);

    provider.shutdown().expect("hand off final metric");

    for _ in 0..200 {
        if fs::read(&output).is_ok_and(|body| {
            body.windows(b"codex.test.process_exit".len())
                .any(|window| window == b"codex.test.process_exit")
        }) {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("detached upload payload did not contain the final metric");
}
