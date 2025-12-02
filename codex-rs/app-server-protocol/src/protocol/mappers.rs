use crate::protocol::v1;
use crate::protocol::v2;

impl From<v1::ExecOneOffCommandParams> for v2::CommandExecParams {
    fn from(value: v1::ExecOneOffCommandParams) -> Self {
        Self {
            command: value.command,
            timeout_ms: value
                .timeout_ms
                .map(|timeout| i64::try_from(timeout).unwrap_or(60_000)),
            cwd: value.cwd,
            sandbox_policy: value.sandbox_policy.map(std::convert::Into::into),
        }
    }
}
