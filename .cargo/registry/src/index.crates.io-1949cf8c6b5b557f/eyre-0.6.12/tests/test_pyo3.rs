#![cfg(feature = "pyo3")]

use pyo3::prelude::*;

use eyre::{bail, Result, WrapErr};

fn f() -> Result<()> {
    use std::io;
    bail!(io::Error::new(io::ErrorKind::PermissionDenied, "oh no!"));
}

fn g() -> Result<()> {
    f().wrap_err("f failed")
}

fn h() -> Result<()> {
    g().wrap_err("g failed")
}

#[test]
fn test_pyo3_exception_contents() {
    use pyo3::types::IntoPyDict;

    let err = h().unwrap_err();
    let expected_contents = format!("{:?}", err);
    let pyerr = PyErr::from(err);

    Python::with_gil(|py| {
        let locals = [("err", pyerr)].into_py_dict(py);
        let pyerr = py.run("raise err", None, Some(locals)).unwrap_err();
        assert_eq!(pyerr.value(py).to_string(), expected_contents);
    })
}
