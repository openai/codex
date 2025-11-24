use crate::Report;

impl From<Report> for pyo3::PyErr {
    fn from(error: Report) -> Self {
        pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", error))
    }
}
