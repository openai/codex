use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParsedCommand {
    Read {
        cmd: String,
        name: String,
    },
    ListFiles {
        cmd: String,
        path: Option<String>,
    },
    Search {
        cmd: String,
        query: Option<String>,
        path: Option<String>,
    },
    Format {
        cmd: String,
        tool: Option<String>,
        targets: Option<Vec<String>>,
    },
    Test {
        cmd: String,
    },
    Lint {
        cmd: String,
        tool: Option<String>,
        targets: Option<Vec<String>>,
    },
    Noop {
        cmd: String,
    },
    Unknown {
        cmd: String,
    },
}
