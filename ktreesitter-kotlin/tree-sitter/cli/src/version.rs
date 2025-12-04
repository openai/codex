use std::{fs, path::PathBuf, process::Command};

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use tree_sitter_loader::TreeSitterJSON;

pub struct Version {
    pub version: String,
    pub current_dir: PathBuf,
}

impl Version {
    #[must_use]
    pub const fn new(version: String, current_dir: PathBuf) -> Self {
        Self {
            version,
            current_dir,
        }
    }

    pub fn run(self) -> Result<()> {
        let tree_sitter_json = self.current_dir.join("tree-sitter.json");

        let tree_sitter_json =
            serde_json::from_str::<TreeSitterJSON>(&fs::read_to_string(tree_sitter_json)?)?;

        let is_multigrammar = tree_sitter_json.grammars.len() > 1;

        self.update_treesitter_json().with_context(|| {
            format!(
                "Failed to update tree-sitter.json at {}",
                self.current_dir.display()
            )
        })?;
        self.update_cargo_toml().with_context(|| {
            format!(
                "Failed to update Cargo.toml at {}",
                self.current_dir.display()
            )
        })?;
        self.update_package_json().with_context(|| {
            format!(
                "Failed to update package.json at {}",
                self.current_dir.display()
            )
        })?;
        self.update_makefile(is_multigrammar).with_context(|| {
            format!(
                "Failed to update Makefile at {}",
                self.current_dir.display()
            )
        })?;
        self.update_cmakelists_txt().with_context(|| {
            format!(
                "Failed to update CMakeLists.txt at {}",
                self.current_dir.display()
            )
        })?;
        self.update_pyproject_toml().with_context(|| {
            format!(
                "Failed to update pyproject.toml at {}",
                self.current_dir.display()
            )
        })?;

        Ok(())
    }

    fn update_treesitter_json(&self) -> Result<()> {
        let tree_sitter_json = &fs::read_to_string(self.current_dir.join("tree-sitter.json"))?;

        let tree_sitter_json = tree_sitter_json
            .lines()
            .map(|line| {
                if line.contains("\"version\":") {
                    let prefix_index = line.find("\"version\":").unwrap() + "\"version\":".len();
                    let start_quote = line[prefix_index..].find('"').unwrap() + prefix_index + 1;
                    let end_quote = line[start_quote + 1..].find('"').unwrap() + start_quote + 1;

                    format!(
                        "{}{}{}",
                        &line[..start_quote],
                        self.version,
                        &line[end_quote..]
                    )
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        fs::write(self.current_dir.join("tree-sitter.json"), tree_sitter_json)?;

        Ok(())
    }

    fn update_cargo_toml(&self) -> Result<()> {
        if !self.current_dir.join("Cargo.toml").exists() {
            return Ok(());
        }

        let cargo_toml = fs::read_to_string(self.current_dir.join("Cargo.toml"))?;

        let cargo_toml = cargo_toml
            .lines()
            .map(|line| {
                if line.starts_with("version =") {
                    format!("version = \"{}\"", self.version)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        fs::write(self.current_dir.join("Cargo.toml"), cargo_toml)?;

        if self.current_dir.join("Cargo.lock").exists() {
            let Ok(cmd) = Command::new("cargo")
                .arg("generate-lockfile")
                .arg("--offline")
                .current_dir(&self.current_dir)
                .output()
            else {
                return Ok(()); // cargo is not `executable`, ignore
            };

            if !cmd.status.success() {
                let stderr = String::from_utf8_lossy(&cmd.stderr);
                return Err(anyhow!(
                    "Failed to run `cargo generate-lockfile`:\n{stderr}"
                ));
            }
        }

        Ok(())
    }

    fn update_package_json(&self) -> Result<()> {
        if !self.current_dir.join("package.json").exists() {
            return Ok(());
        }

        let package_json = &fs::read_to_string(self.current_dir.join("package.json"))?;

        let package_json = package_json
            .lines()
            .map(|line| {
                if line.contains("\"version\":") {
                    let prefix_index = line.find("\"version\":").unwrap() + "\"version\":".len();
                    let start_quote = line[prefix_index..].find('"').unwrap() + prefix_index + 1;
                    let end_quote = line[start_quote + 1..].find('"').unwrap() + start_quote + 1;

                    format!(
                        "{}{}{}",
                        &line[..start_quote],
                        self.version,
                        &line[end_quote..]
                    )
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        fs::write(self.current_dir.join("package.json"), package_json)?;

        if self.current_dir.join("package-lock.json").exists() {
            let Ok(cmd) = Command::new("npm")
                .arg("install")
                .arg("--package-lock-only")
                .current_dir(&self.current_dir)
                .output()
            else {
                return Ok(()); // npm is not `executable`, ignore
            };

            if !cmd.status.success() {
                let stderr = String::from_utf8_lossy(&cmd.stderr);
                return Err(anyhow!("Failed to run `npm install`:\n{stderr}"));
            }
        }

        Ok(())
    }

    fn update_makefile(&self, is_multigrammar: bool) -> Result<()> {
        let makefile = if is_multigrammar {
            if !self.current_dir.join("common").join("common.mak").exists() {
                return Ok(());
            }

            fs::read_to_string(self.current_dir.join("Makefile"))?
        } else {
            if !self.current_dir.join("Makefile").exists() {
                return Ok(());
            }

            fs::read_to_string(self.current_dir.join("Makefile"))?
        };

        let makefile = makefile
            .lines()
            .map(|line| {
                if line.starts_with("VERSION") {
                    format!("VERSION := {}", self.version)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        fs::write(self.current_dir.join("Makefile"), makefile)?;

        Ok(())
    }

    fn update_cmakelists_txt(&self) -> Result<()> {
        if !self.current_dir.join("CMakeLists.txt").exists() {
            return Ok(());
        }

        let cmake = fs::read_to_string(self.current_dir.join("CMakeLists.txt"))?;

        let re = Regex::new(r#"(\s*VERSION\s+)"[0-9]+\.[0-9]+\.[0-9]+""#)?;
        let cmake = re.replace(&cmake, format!(r#"$1"{}""#, self.version));

        fs::write(self.current_dir.join("CMakeLists.txt"), cmake.as_bytes())?;

        Ok(())
    }

    fn update_pyproject_toml(&self) -> Result<()> {
        if !self.current_dir.join("pyproject.toml").exists() {
            return Ok(());
        }

        let pyproject_toml = fs::read_to_string(self.current_dir.join("pyproject.toml"))?;

        let pyproject_toml = pyproject_toml
            .lines()
            .map(|line| {
                if line.starts_with("version =") {
                    format!("version = \"{}\"", self.version)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        fs::write(self.current_dir.join("pyproject.toml"), pyproject_toml)?;

        Ok(())
    }
}
