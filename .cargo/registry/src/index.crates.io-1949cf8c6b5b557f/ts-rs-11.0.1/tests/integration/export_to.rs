use std::path::Path;

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "export_to/with_str_to_file.ts")]
struct WithStrToFile;

#[derive(TS)]
#[ts(export, export_to = "export_to/")]
struct WithStrToDir;

// --

#[derive(TS)]
#[ts(export, export_to = &"export_to/with_str_ref_to_file.ts")]
struct WithStrRefToFile;

#[derive(TS)]
#[ts(export, export_to = &"export_to/")]
struct WithStrRefToDir;

// --

#[derive(TS)]
#[ts(export, export_to = format!("export_to/with_string_to_file.ts"))]
struct WithStringToFile;

#[derive(TS)]
#[ts(export, export_to = format!("export_to/"))]
struct WithStringToDir;

// --

#[derive(TS)]
#[ts(export, export_to = &format!("export_to/with_string_ref_to_file.ts"))]
struct WithStringRefToFile;

#[derive(TS)]
#[ts(export, export_to = &format!("export_to/"))]
struct WithStringRefToDir;

// --

#[derive(TS)]
#[ts(export, export_to = {
    let dir = WithStrToFile::default_output_path().unwrap();
    let dir = dir.parent().unwrap();
    let file = dir.join("to_absolute_file_path.ts");
    let file = std::path::absolute(file).unwrap();
    file.display().to_string()
})]
struct ToAbsoluteFilePath(WithStrToDir, WithStrToFile);

#[derive(TS)]
#[ts(export, export_to = {
    let dir = WithStrToFile::default_output_path().unwrap();
    let dir = dir.parent().unwrap();
    let dir = std::path::absolute(dir).unwrap();
    let dir = dir.display();
    format!("{dir}/")
})]
struct ToAbsoluteDirPath(WithStrToDir, WithStrToFile, ToAbsoluteFilePath);

// --

#[test]
#[cfg(test)]
fn check_export_complete() {
    export_bindings_withstrtofile();
    export_bindings_withstrtodir();
    export_bindings_withstrreftofile();
    export_bindings_withstrreftodir();
    export_bindings_withstringtofile();
    export_bindings_withstringtodir();
    export_bindings_withstringreftofile();
    export_bindings_withstringreftodir();
    export_bindings_toabsolutefilepath();
    export_bindings_toabsolutedirpath();

    let files = [
        "with_str_to_file.ts",
        "WithStrToDir.ts",
        "with_str_ref_to_file.ts",
        "WithStrRefToDir.ts",
        "with_string_to_file.ts",
        "WithStringToDir.ts",
        "with_string_ref_to_file.ts",
        "WithStringRefToDir.ts",
        "to_absolute_file_path.ts",
        "ToAbsoluteDirPath.ts",
    ];

    let dir = std::env::var("TS_RS_EXPORT_DIR").unwrap_or_else(|_| "./bindings".to_owned());
    let dir = Path::new(&dir).join("export_to");

    files
        .iter()
        .map(|file| dir.join(file))
        .for_each(|file| assert!(file.is_file(), "{file:?}"));
}
