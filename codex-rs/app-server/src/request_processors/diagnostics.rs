use codex_app_server_protocol::ServerDiagnosticsGauge;
use codex_app_server_protocol::ServerDiagnosticsProcess;
use codex_app_server_protocol::ServerDiagnosticsResponse;

pub(crate) fn read_server_diagnostics() -> ServerDiagnosticsResponse {
    let diagnostics = codex_diagnostics::snapshot();

    ServerDiagnosticsResponse {
        process: ServerDiagnosticsProcess {
            id: diagnostics.process.id,
            resident_memory_bytes: diagnostics.process.resident_memory_bytes,
            physical_footprint_bytes: diagnostics.process.physical_footprint_bytes,
        },
        gauges: diagnostics
            .gauges
            .into_iter()
            .map(|gauge| ServerDiagnosticsGauge {
                name: gauge.name.to_string(),
                value: gauge.value,
            })
            .collect(),
    }
}
