//! A bounded, non-retrying host observation. No session binding or recovery is
//! involved: a failed RPC never explicitly retires the execution connection.
//! Connection establishment and reconnection remain the transport's policy.

use std::time::Duration;

use codex_code_mode_protocol::CodeModeProcessMetrics;
use codex_code_mode_protocol::CodeModeProcessMetricsSample;
use codex_code_mode_protocol::grpc;
use tonic::Code;

use super::GrpcCodeModeSessionProvider;

pub(super) async fn sample(
    provider: &GrpcCodeModeSessionProvider,
) -> Result<CodeModeProcessMetricsSample, String> {
    let deadline = Duration::from_secs(/*secs*/ 1);
    tokio::time::timeout(deadline, async {
        let mut client = provider.transport.client().await?;
        let mut request = tonic::Request::new(grpc::GetProcessMetricsRequest {});
        request.set_timeout(deadline);
        match client.get_process_metrics(request).await {
            Ok(response) => {
                let snapshot = response.into_inner();
                if snapshot.process_id == 0
                    || !snapshot.process_start_time_seconds.is_finite()
                    || snapshot.process_start_time_seconds <= 0.0
                    || !snapshot.cpu_seconds_total.is_finite()
                    || snapshot.cpu_seconds_total < 0.0
                {
                    return Err("invalid code-mode process metrics snapshot".to_string());
                }
                Ok(CodeModeProcessMetricsSample::Snapshot(
                    CodeModeProcessMetrics {
                        process_id: snapshot.process_id,
                        process_start_time_seconds: snapshot.process_start_time_seconds,
                        cpu_seconds_total: snapshot.cpu_seconds_total,
                        resident_memory_bytes: snapshot.resident_memory_bytes,
                    },
                ))
            }
            Err(status) if status.code() == Code::Unimplemented => {
                Ok(CodeModeProcessMetricsSample::Unsupported)
            }
            Err(status) => Err(format!("code-mode process metrics failed: {status}")),
        }
    })
    .await
    .map_err(|_| "code-mode process metrics timed out".to_string())?
}
