use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageServerId {
    RustAnalyzer,
    Gopls,
    TypeScriptLanguageServer,
    Pyright,
    Clangd,
}

impl LanguageServerId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RustAnalyzer => "rust-analyzer",
            Self::Gopls => "gopls",
            Self::TypeScriptLanguageServer => "typescript-language-server",
            Self::Pyright => "pyright",
            Self::Clangd => "clangd",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::RustAnalyzer,
            Self::Gopls,
            Self::TypeScriptLanguageServer,
            Self::Pyright,
            Self::Clangd,
        ]
    }
}

impl fmt::Display for LanguageServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LanguageServerId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "rust-analyzer" | "rust_analyzer" => Ok(Self::RustAnalyzer),
            "gopls" => Ok(Self::Gopls),
            "typescript-language-server" | "typescript_language_server" | "tsserver" => {
                Ok(Self::TypeScriptLanguageServer)
            }
            "pyright" | "pyright-langserver" => Ok(Self::Pyright),
            "clangd" => Ok(Self::Clangd),
            other => Err(format!("unknown language server id: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStrategy {
    RustupComponent { component: &'static str },
    GoInstall { module: &'static str },
    Npm { package: &'static str },
    SystemOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSpec {
    pub id: LanguageServerId,
    pub markers: Vec<&'static str>,
    pub extensions: Vec<&'static str>,
    pub bin_name: &'static str,
    pub default_args: Vec<&'static str>,
    pub install: InstallStrategy,
}

impl ServerSpec {
    pub fn language_id_for_extension(&self, extension: &str) -> &'static str {
        match self.id {
            LanguageServerId::RustAnalyzer => "rust",
            LanguageServerId::Gopls => "go",
            LanguageServerId::TypeScriptLanguageServer => match extension {
                "ts" | "tsx" => "typescript",
                _ => "javascript",
            },
            LanguageServerId::Pyright => "python",
            LanguageServerId::Clangd => match extension {
                "c" => "c",
                _ => "cpp",
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerRegistry {
    specs: Vec<ServerSpec>,
}

impl Default for ServerRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

impl ServerRegistry {
    pub fn builtin() -> Self {
        Self {
            specs: vec![
                ServerSpec {
                    id: LanguageServerId::RustAnalyzer,
                    markers: vec!["Cargo.toml"],
                    extensions: vec!["rs"],
                    bin_name: "rust-analyzer",
                    default_args: Vec::new(),
                    install: InstallStrategy::RustupComponent {
                        component: "rust-analyzer",
                    },
                },
                ServerSpec {
                    id: LanguageServerId::Gopls,
                    markers: vec!["go.mod", "go.work"],
                    extensions: vec!["go"],
                    bin_name: "gopls",
                    default_args: Vec::new(),
                    install: InstallStrategy::GoInstall {
                        module: "golang.org/x/tools/gopls@latest",
                    },
                },
                ServerSpec {
                    id: LanguageServerId::TypeScriptLanguageServer,
                    markers: vec!["package.json", "tsconfig.json"],
                    extensions: vec!["ts", "tsx", "js", "jsx"],
                    bin_name: "typescript-language-server",
                    default_args: vec!["--stdio"],
                    install: InstallStrategy::Npm {
                        package: "typescript-language-server",
                    },
                },
                ServerSpec {
                    id: LanguageServerId::Pyright,
                    markers: vec!["pyproject.toml", "requirements.txt"],
                    extensions: vec!["py"],
                    bin_name: "pyright-langserver",
                    default_args: vec!["--stdio"],
                    install: InstallStrategy::Npm { package: "pyright" },
                },
                ServerSpec {
                    id: LanguageServerId::Clangd,
                    markers: vec!["compile_commands.json", "CMakeLists.txt"],
                    extensions: vec!["c", "h", "cc", "cpp", "hpp", "cxx"],
                    bin_name: "clangd",
                    default_args: Vec::new(),
                    install: InstallStrategy::SystemOnly,
                },
            ],
        }
    }

    pub fn specs(&self) -> &[ServerSpec] {
        &self.specs
    }

    pub fn spec(&self, id: LanguageServerId) -> Option<&ServerSpec> {
        self.specs.iter().find(|spec| spec.id == id)
    }
}
