use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParsedCommand {
    Read {
        cmd: String,
        name: String,
        /// (Best effort) Path to the file being read by the command. When
        /// possible, this is an absolute path, though when relative, it should
        /// be resolved against the `cwd`` that will be used to run the command
        /// to derive the absolute path.
        path: PathBuf,
    },
    ListFiles {
        cmd: String,
        #[ts(optional = nullable)]
        path: Option<String>,
    },
    Search {
        cmd: String,
        #[ts(optional = nullable)]
        query: Option<String>,
        #[ts(optional = nullable)]
        path: Option<String>,
    },
    Unknown {
        cmd: String,
    },
}
