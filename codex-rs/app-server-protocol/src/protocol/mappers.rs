use crate::protocol::v1;
use crate::protocol::v2;
impl From<v1::ExecOneOffCommandParams> for v2::CommandExecParams {
    fn from(value: v1::ExecOneOffCommandParams) -> Self {
        Self {
            command: value.command,
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: value
                .timeout_ms
                .map(|timeout| i64::try_from(timeout).unwrap_or(60_000)),
            cwd: value.cwd,
            env: None,
            size: None,
            sandbox_policy: value.sandbox_policy.map(std::convert::Into::into),
            permission_profile: None,
        }
    }
}
