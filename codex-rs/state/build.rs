fn main() {
    println!("cargo:rerun-if-changed=vendor/sqlite-recover/dbdata.c");
    println!("cargo:rerun-if-changed=vendor/sqlite-recover/sqlite3.h");
    println!("cargo:rerun-if-changed=vendor/sqlite-recover/sqlite3recover.c");
    println!("cargo:rerun-if-changed=vendor/sqlite-recover/sqlite3recover.h");

    cc::Build::new()
        .file("vendor/sqlite-recover/dbdata.c")
        .file("vendor/sqlite-recover/sqlite3recover.c")
        .include("vendor/sqlite-recover")
        .warnings(false)
        .compile("codex_sqlite_recover");
}
