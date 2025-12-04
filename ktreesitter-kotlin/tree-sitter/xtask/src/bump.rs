use std::cmp::Ordering;

use anyhow::{anyhow, Result};
use git2::{DiffOptions, Repository};
use indoc::indoc;
use semver::{BuildMetadata, Prerelease, Version};
use toml::Value;

use crate::{create_commit, BumpVersion};

pub fn get_latest_tag(repo: &Repository) -> Result<String> {
    let mut tags = repo
        .tag_names(None)?
        .into_iter()
        .filter_map(|tag| tag.map(String::from))
        .filter_map(|tag| Version::parse(tag.strip_prefix('v').unwrap_or(&tag)).ok())
        .collect::<Vec<Version>>();

    tags.sort_by(
        |a, b| match (a.pre != Prerelease::EMPTY, b.pre != Prerelease::EMPTY) {
            (true, true) | (false, false) => a.cmp(b),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
        },
    );

    tags.last()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| anyhow!("No tags found"))
}

pub fn run(args: BumpVersion) -> Result<()> {
    let repo = Repository::open(".")?;
    let latest_tag = get_latest_tag(&repo)?;
    let current_version = Version::parse(&latest_tag)?;
    let latest_tag_sha = repo.revparse_single(&format!("v{latest_tag}"))?.id();

    let workspace_toml_version = Version::parse(&fetch_workspace_version()?)?;

    if current_version.major != workspace_toml_version.major
        && current_version.minor != workspace_toml_version.minor
    {
        eprintln!(
            indoc! {"
            Seems like the workspace Cargo.toml ({}) version does not match up with the latest git tag ({}).
            Please ensure you don't change that yourself, this subcommand will handle this for you.
        "},
            workspace_toml_version, latest_tag
        );
        return Ok(());
    }

    let mut revwalk = repo.revwalk()?;
    revwalk.push_range(format!("{latest_tag_sha}..HEAD").as_str())?;
    let mut diff_options = DiffOptions::new();

    let mut should_increment_patch = false;
    let mut should_increment_minor = false;

    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let message = commit.message().unwrap();
        let message = message.trim();

        let diff = {
            let parent = commit.parent(0).unwrap();
            let parent_tree = parent.tree().unwrap();
            let commit_tree = commit.tree().unwrap();
            repo.diff_tree_to_tree(
                Some(&parent_tree),
                Some(&commit_tree),
                Some(&mut diff_options),
            )?
        };

        let mut source_code_changed = false;
        diff.foreach(
            &mut |delta, _| {
                let path = delta.new_file().path().unwrap().to_str().unwrap();
                if path.ends_with("rs") || path.ends_with("js") || path.ends_with('c') {
                    source_code_changed = true;
                }
                true
            },
            None,
            None,
            None,
        )?;

        if source_code_changed {
            should_increment_patch = true;

            let Some((prefix, _)) = message.split_once(':') else {
                continue;
            };

            let convention = if prefix.contains('(') {
                prefix.split_once('(').unwrap().0
            } else {
                prefix
            };

            if ["feat", "feat!"].contains(&convention) || prefix.ends_with('!') {
                should_increment_minor = true;
            }
        }
    }

    let next_version = if let Some(version) = args.version {
        version
    } else {
        let mut next_version = current_version.clone();
        if should_increment_minor {
            next_version.minor += 1;
            next_version.patch = 0;
            next_version.pre = Prerelease::EMPTY;
            next_version.build = BuildMetadata::EMPTY;
        } else if should_increment_patch {
            next_version.patch += 1;
            next_version.pre = Prerelease::EMPTY;
            next_version.build = BuildMetadata::EMPTY;
        } else {
            return Err(anyhow!(format!(
                "No source code changed since {current_version}"
            )));
        }
        next_version
    };
    if next_version <= current_version {
        return Err(anyhow!(format!(
            "Next version {next_version} must be greater than current version {current_version}"
        )));
    }

    println!("Bumping from {current_version} to {next_version}");
    update_crates(&current_version, &next_version)?;
    update_makefile(&next_version)?;
    update_cmake(&next_version)?;
    update_npm(&next_version)?;
    update_zig(&next_version)?;
    tag_next_version(&repo, &next_version)?;

    Ok(())
}

fn tag_next_version(repo: &Repository, next_version: &Version) -> Result<()> {
    let signature = repo.signature()?;

    let commit_id = create_commit(
        repo,
        &format!("{next_version}"),
        &[
            "Cargo.toml",
            "Cargo.lock",
            "cli/Cargo.toml",
            "cli/config/Cargo.toml",
            "cli/loader/Cargo.toml",
            "lib/Cargo.toml",
            "highlight/Cargo.toml",
            "tags/Cargo.toml",
            "cli/npm/package.json",
            "lib/binding_web/package.json",
            "Makefile",
            "lib/CMakeLists.txt",
            "build.zig.zon",
        ],
    )?;

    let tag = repo.tag(
        &format!("v{next_version}"),
        &repo.find_object(commit_id, None)?,
        &signature,
        &format!("v{next_version}"),
        false,
    )?;

    println!("Tagged commit {commit_id} with tag {tag}");

    Ok(())
}

fn update_makefile(next_version: &Version) -> Result<()> {
    let makefile = std::fs::read_to_string("Makefile")?;
    let makefile = makefile
        .lines()
        .map(|line| {
            if line.starts_with("VERSION") {
                format!("VERSION := {next_version}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    std::fs::write("Makefile", makefile)?;

    Ok(())
}

fn update_cmake(next_version: &Version) -> Result<()> {
    let cmake = std::fs::read_to_string("lib/CMakeLists.txt")?;
    let cmake = cmake
        .lines()
        .map(|line| {
            if line.contains(" VERSION") {
                let start_quote = line.find('"').unwrap();
                let end_quote = line.rfind('"').unwrap();
                format!(
                    "{}{next_version}{}",
                    &line[..=start_quote],
                    &line[end_quote..]
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    std::fs::write("lib/CMakeLists.txt", cmake)?;

    Ok(())
}

fn update_crates(current_version: &Version, next_version: &Version) -> Result<()> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("workspaces").arg("version");

    if next_version.minor > current_version.minor {
        cmd.arg("minor");
    } else {
        cmd.arg("patch");
    }

    cmd.arg("--no-git-commit")
        .arg("--yes")
        .arg("--force")
        .arg("tree-sitter{,-cli,-config,-generate,-loader,-highlight,-tags}")
        .arg("--ignore-changes")
        .arg("lib/language/*");

    let status = cmd.status()?;

    if !status.success() {
        return Err(anyhow!("Failed to update crates"));
    }

    Ok(())
}

fn update_npm(next_version: &Version) -> Result<()> {
    for path in ["lib/binding_web/package.json", "cli/npm/package.json"] {
        let package_json =
            serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(path)?)?;

        let mut package_json = package_json
            .as_object()
            .ok_or_else(|| anyhow!("Invalid package.json"))?
            .clone();
        package_json.insert(
            "version".to_string(),
            serde_json::Value::String(next_version.to_string()),
        );

        let package_json = serde_json::to_string_pretty(&package_json)? + "\n";

        std::fs::write(path, package_json)?;
    }

    Ok(())
}

fn update_zig(next_version: &Version) -> Result<()> {
    let zig = std::fs::read_to_string("build.zig.zon")?;

    let zig = zig
        .lines()
        .map(|line| {
            if line.starts_with("  .version") {
                format!("  .version = \"{next_version}\",")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    std::fs::write("build.zig.zon", zig)?;

    Ok(())
}

/// read Cargo.toml and get the version
fn fetch_workspace_version() -> Result<String> {
    let cargo_toml = toml::from_str::<Value>(&std::fs::read_to_string("Cargo.toml")?)?;

    Ok(cargo_toml["workspace"]["package"]["version"]
        .as_str()
        .unwrap()
        .trim_matches('"')
        .to_string())
}
