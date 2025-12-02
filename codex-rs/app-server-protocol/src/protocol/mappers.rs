use crate::protocol::v1;
use crate::protocol::v2;

impl From<v1::ExecOneOffCommandParams> for v2::CommandExecParams {
    fn from(value: v1::ExecOneOffCommandParams) -> Self {
        Self {
            command: value.command,
            timeout_ms: value.timeout_ms,
            cwd: value.cwd,
            sandbox_policy: value.sandbox_policy.map(std::convert::Into::into),
        }
    }
}
