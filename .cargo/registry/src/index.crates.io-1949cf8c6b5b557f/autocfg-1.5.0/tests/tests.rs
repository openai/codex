extern crate autocfg;

use autocfg::AutoCfg;

mod support;

fn core_std(ac: &AutoCfg, path: &str) -> String {
    let krate = if ac.no_std() { "core" } else { "std" };
    format!("{}::{}", krate, path)
}

fn assert_std(ac: &AutoCfg, probe_result: bool) {
    assert_eq!(!ac.no_std(), probe_result);
}

fn assert_min(ac: &AutoCfg, major: usize, minor: usize, probe_result: bool) {
    assert_eq!(ac.probe_rustc_version(major, minor), probe_result);
}

fn autocfg_for_test() -> AutoCfg {
    AutoCfg::with_dir(support::out_dir().as_ref()).unwrap()
}

#[test]
fn autocfg_version() {
    let ac = autocfg_for_test();
    assert!(ac.probe_rustc_version(1, 0));
}

#[test]
fn probe_add() {
    let ac = autocfg_for_test();
    let add = core_std(&ac, "ops::Add");
    let add_rhs = add.clone() + "<i32>";
    let add_rhs_output = add.clone() + "<i32, Output = i32>";
    let dyn_add_rhs_output = "dyn ".to_string() + &*add_rhs_output;
    assert!(ac.probe_path(&add));
    assert!(ac.probe_trait(&add));
    assert!(ac.probe_trait(&add_rhs));
    assert!(ac.probe_trait(&add_rhs_output));
    assert_min(&ac, 1, 27, ac.probe_type(&dyn_add_rhs_output));
}

#[test]
fn probe_as_ref() {
    let ac = autocfg_for_test();
    let as_ref = core_std(&ac, "convert::AsRef");
    let as_ref_str = as_ref.clone() + "<str>";
    let dyn_as_ref_str = "dyn ".to_string() + &*as_ref_str;
    assert!(ac.probe_path(&as_ref));
    assert!(ac.probe_trait(&as_ref_str));
    assert!(ac.probe_type(&as_ref_str));
    assert_min(&ac, 1, 27, ac.probe_type(&dyn_as_ref_str));
}

#[test]
fn probe_i128() {
    let ac = autocfg_for_test();
    let i128_path = core_std(&ac, "i128");
    assert_min(&ac, 1, 26, ac.probe_path(&i128_path));
    assert_min(&ac, 1, 26, ac.probe_type("i128"));
}

#[test]
fn probe_sum() {
    let ac = autocfg_for_test();
    let sum = core_std(&ac, "iter::Sum");
    let sum_i32 = sum.clone() + "<i32>";
    let dyn_sum_i32 = "dyn ".to_string() + &*sum_i32;
    assert_min(&ac, 1, 12, ac.probe_path(&sum));
    assert_min(&ac, 1, 12, ac.probe_trait(&sum));
    assert_min(&ac, 1, 12, ac.probe_trait(&sum_i32));
    assert_min(&ac, 1, 12, ac.probe_type(&sum_i32));
    assert_min(&ac, 1, 27, ac.probe_type(&dyn_sum_i32));
}

#[test]
fn probe_std() {
    let ac = autocfg_for_test();
    assert_std(&ac, ac.probe_sysroot_crate("std"));
}

#[test]
fn probe_alloc() {
    let ac = autocfg_for_test();
    assert_min(&ac, 1, 36, ac.probe_sysroot_crate("alloc"));
}

#[test]
fn probe_bad_sysroot_crate() {
    let ac = autocfg_for_test();
    assert!(!ac.probe_sysroot_crate("doesnt_exist"));
}

#[test]
fn probe_no_std() {
    let ac = autocfg_for_test();
    assert!(ac.probe_type("i32"));
    assert!(ac.probe_type("[i32]"));
    assert_std(&ac, ac.probe_type("Vec<i32>"));
}

#[test]
fn probe_expression() {
    let ac = autocfg_for_test();
    assert!(ac.probe_expression(r#""test".trim_left()"#));
    assert_min(&ac, 1, 30, ac.probe_expression(r#""test".trim_start()"#));
    assert_std(&ac, ac.probe_expression("[1, 2, 3].to_vec()"));
}

#[test]
fn probe_constant() {
    let ac = autocfg_for_test();
    assert!(ac.probe_constant("1 + 2 + 3"));
    assert_min(
        &ac,
        1,
        33,
        ac.probe_constant("{ let x = 1 + 2 + 3; x * x }"),
    );
    assert_min(&ac, 1, 39, ac.probe_constant(r#""test".len()"#));
}

#[test]
fn probe_raw() {
    let ac = autocfg_for_test();
    let prefix = if ac.no_std() { "#![no_std]\n" } else { "" };
    let f = |s| format!("{}{}", prefix, s);

    // This attribute **must** be used at the crate level.
    assert!(ac.probe_raw(&f("#![no_builtins]")).is_ok());

    assert!(ac.probe_raw(&f("#![deny(dead_code)] fn x() {}")).is_err());
    assert!(ac.probe_raw(&f("#![allow(dead_code)] fn x() {}")).is_ok());
    assert!(ac
        .probe_raw(&f("#![deny(dead_code)] pub fn x() {}"))
        .is_ok());
}

#[test]
fn probe_cleanup() {
    let dir = support::out_dir().join("autocfg_test_probe_cleanup");
    std::fs::create_dir(&dir).unwrap();

    let ac = AutoCfg::with_dir(&dir).unwrap();
    assert!(ac.probe_type("i32"));

    // NB: this is not `remove_dir_all`, so it will only work if the directory
    // is empty -- i.e. the probe should have removed any output files.
    std::fs::remove_dir(&dir).unwrap();
}

#[test]
fn editions() {
    let mut ac = autocfg_for_test();
    assert!(ac.edition().is_none());
    assert!(ac.probe_raw("").is_ok());

    for (edition, minor) in vec![(2015, 27), (2018, 31), (2021, 56), (2024, 85)] {
        let edition = edition.to_string();
        ac.set_edition(Some(edition.clone()));
        assert_eq!(ac.edition(), Some(&*edition));
        assert_min(&ac, 1, minor, ac.probe_raw("").is_ok());
    }

    ac.set_edition(Some("invalid".into()));
    assert_eq!(ac.edition(), Some("invalid"));
    assert!(ac.probe_raw("").is_err());

    ac.set_edition(None);
    assert!(ac.edition().is_none());
    assert!(ac.probe_raw("").is_ok());
}

#[test]
fn edition_keyword_try() {
    let mut ac = autocfg_for_test();

    if ac.probe_rustc_version(1, 27) {
        ac.set_edition(Some(2015.to_string()));
    }
    assert!(ac.probe_expression("{ let try = 0; try }"));

    if ac.probe_rustc_version(1, 31) {
        ac.set_edition(Some(2018.to_string()));
        assert!(!ac.probe_expression("{ let try = 0; try }"));
        assert!(ac.probe_expression("{ let r#try = 0; r#try }"));
    }
}
