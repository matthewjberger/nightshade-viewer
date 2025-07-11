use pyo3::{exceptions::PyConnectionError, prelude::*};

/// Runs the native viewer
#[pyfunction]
fn run() -> PyResult<()> {
    match nightshade_core::start() {
        Err(error) => Err(PyConnectionError::new_err(error.to_string())),
        _ => PyResult::Ok(()),
    }
}

#[pymodule]
fn nightshade(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)?;
    Ok(())
}
