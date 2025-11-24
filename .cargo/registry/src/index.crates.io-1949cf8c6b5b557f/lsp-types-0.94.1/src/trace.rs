use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct SetTraceParams {
    /// The new value that should be assigned to the trace setting.
    pub value: TraceValue,
}

/// A TraceValue represents the level of verbosity with which the server systematically
/// reports its execution trace using `LogTrace` notifications.
///
/// The initial trace value is set by the client at initialization and can be modified
/// later using the `SetTrace` notification.
#[derive(Debug, Eq, PartialEq, Clone, Copy, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TraceValue {
    /// The server should not send any `$/logTrace` notification
    #[default]
    Off,
    /// The server should not add the 'verbose' field in the `LogTraceParams`
    Messages,
    Verbose,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogTraceParams {
    /// The message to be logged.
    pub message: String,
    /// Additional information that can be computed if the `trace` configuration
    /// is set to `'verbose'`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbose: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_serialization;

    #[test]
    fn test_set_trace_params() {
        test_serialization(
            &SetTraceParams {
                value: TraceValue::Off,
            },
            r#"{"value":"off"}"#,
        );
    }

    #[test]
    fn test_log_trace_params() {
        test_serialization(
            &LogTraceParams {
                message: "message".into(),
                verbose: None,
            },
            r#"{"message":"message"}"#,
        );

        test_serialization(
            &LogTraceParams {
                message: "message".into(),
                verbose: Some("verbose".into()),
            },
            r#"{"message":"message","verbose":"verbose"}"#,
        );
    }

    #[test]
    fn test_trace_value() {
        test_serialization(
            &vec![TraceValue::Off, TraceValue::Messages, TraceValue::Verbose],
            r#"["off","messages","verbose"]"#,
        );
    }
}
