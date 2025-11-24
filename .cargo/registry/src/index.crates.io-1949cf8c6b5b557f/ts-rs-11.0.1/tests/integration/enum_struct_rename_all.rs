#[cfg(feature = "serde-compat")]
use serde::Serialize;
use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "enum_struct_rename_all/")]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[cfg_attr(feature = "serde-compat", serde(rename_all = "camelCase"))]
#[cfg_attr(not(feature = "serde-compat"), ts(rename_all = "camelCase"))]
pub enum TaskStatus {
    #[cfg_attr(feature = "serde-compat", serde(rename_all = "camelCase"))]
    #[cfg_attr(not(feature = "serde-compat"), ts(rename_all = "camelCase"))]
    Running { started_time: String },

    #[cfg_attr(feature = "serde-compat", serde(rename_all = "camelCase"))]
    #[cfg_attr(not(feature = "serde-compat"), ts(rename_all = "camelCase"))]
    Terminated {
        status: i32,
        stdout: String,
        stderr: String,
    },
}

#[test]
pub fn enum_struct_rename_all() {
    assert_eq!(
        TaskStatus::inline(),
        r#"{ "running": { startedTime: string, } } | { "terminated": { status: number, stdout: string, stderr: string, } }"#
    )
}

#[derive(TS, Clone)]
#[ts(export, export_to = "enum_struct_rename_all/")]
#[cfg_attr(feature = "serde-compat", derive(Serialize))]
#[cfg_attr(feature = "serde-compat", serde(rename_all_fields = "kebab-case"))]
#[cfg_attr(not(feature = "serde-compat"), ts(rename_all_fields = "kebab-case"))]
pub enum TaskStatus2 {
    Running {
        started_time: String,
    },

    Terminated {
        status: i32,
        stdout: String,
        stderr: String,
    },

    A(i32),
    B(i32, i32),
    C,
}

#[test]
pub fn enum_struct_rename_all_fields() {
    assert_eq!(
        TaskStatus2::inline(),
        r#"{ "Running": { "started-time": string, } } | { "Terminated": { status: number, stdout: string, stderr: string, } } | { "A": number } | { "B": [number, number] } | "C""#
    )
}
