use super::*;
use flate2::Compression;
use flate2::write::GzEncoder;
use pretty_assertions::assert_eq;
use std::io::Cursor;
use std::io::Write;

#[cfg(unix)]
#[test]
fn materialize_npm_plugin_source_uses_packed_package_root() {
    use std::os::unix::fs::PermissionsExt;

    let codex_home = tempfile::tempdir().expect("create codex home");
    let fake_npm_dir = tempfile::tempdir().expect("create fake npm directory");
    let archive_bytes =
        npm_package_archive_bytes("@acme/plugin", "1.2.0").expect("build fixture archive");
    let archive_path = fake_npm_dir.path().join("fixture.tgz");
    fs::write(&archive_path, &archive_bytes).expect("write fixture archive");
    let fake_npm = fake_npm_dir.path().join("npm");
    fs::write(
        &fake_npm,
        format!(
            r#"#!/bin/sh
destination=""
previous=""
for argument in "$@"; do
  if [ "$previous" = "--pack-destination" ]; then
    destination="$argument"
  fi
  previous="$argument"
done
cp "{}" "$destination/acme-plugin-1.2.0.tgz"
printf '%s\n' "$@" > "$destination/args.txt"
pwd > "$destination/pwd.txt"
"#,
            archive_path.display()
        ),
    )
    .expect("write fake npm");
    let mut permissions = fs::metadata(&fake_npm)
        .expect("read fake npm metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_npm, permissions).expect("make fake npm executable");

    let (plugin_root, tempdir) = materialize_npm_plugin_source_with_command(
        codex_home.path(),
        "@acme/plugin",
        Some("^1.2.0"),
        Some("https://npm.example.com"),
        fake_npm.as_os_str(),
    )
    .expect("materialize npm source");

    assert_eq!(
        plugin_root.as_path(),
        tempdir.path().join("extracted/package")
    );
    assert!(
        plugin_root
            .as_path()
            .join(".codex-plugin/plugin.json")
            .is_file()
    );
    let args = fs::read_to_string(tempdir.path().join("args.txt")).expect("read npm arguments");
    assert!(args.contains("pack"));
    assert!(args.contains("--ignore-scripts"));
    assert!(args.contains("--registry"));
    assert!(args.contains("https://npm.example.com"));
    assert!(args.contains("@acme/plugin@^1.2.0"));
    assert!(!args.contains("install"));
    let npm_working_directory = fs::canonicalize(
        fs::read_to_string(tempdir.path().join("pwd.txt"))
            .expect("read npm working directory")
            .trim(),
    )
    .expect("canonicalize npm working directory");
    assert_eq!(
        npm_working_directory,
        fs::canonicalize(tempdir.path()).expect("canonicalize tempdir")
    );
}

fn npm_package_archive_bytes(package: &str, version: &str) -> std::io::Result<Vec<u8>> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut archive = tar::Builder::new(encoder);
    append_archive_file(
        &mut archive,
        "package/package.json",
        format!(r#"{{"name":"{package}","version":"{version}"}}"#).as_bytes(),
    )?;
    append_archive_file(
        &mut archive,
        "package/.codex-plugin/plugin.json",
        br#"{"name":"plugin"}"#,
    )?;
    let encoder = archive.into_inner()?;
    encoder.finish()
}

fn append_archive_file<W: Write>(
    archive: &mut tar::Builder<W>,
    path: &str,
    contents: &[u8],
) -> std::io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive.append_data(&mut header, path, Cursor::new(contents))
}
