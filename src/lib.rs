mod commands;
mod tcp;


use pyo3::prelude::*;

#[pymodule]
pub mod circpush {

    use super::*;

    #[pymodule]
    pub mod server {

        use super::*;

        #[pyfunction]
        pub fn start() -> PyResult<()> {
            crate::tcp::server::start_server();
            Ok(())
        }

        #[pyfunction]
        pub fn run() -> PyResult<()> {
            crate::tcp::server::run_server();
            Ok(())
        }
    }

    #[pymodule]
    pub mod client {

        use pyo3::exceptions::PyRuntimeError;

        use super::*;

        #[pyfunction]
        pub fn ping() -> PyResult<&'static str> {
            match crate::tcp::client::ping() {
                Ok(t) => Ok(t),
                Err(e) => Err(PyRuntimeError::new_err(e)),
            }
        }

        #[pyfunction]
        pub fn echo(message: String) -> PyResult<String> {
            match crate::tcp::client::echo(message) {
                Ok(t) => Ok(t),
                Err(e) => Err(PyRuntimeError::new_err(e)),
            }
        }

        #[pyfunction]
        pub fn stop_server() -> PyResult<&'static str> {
            match crate::tcp::client::stop_server() {
                Ok(t) => Ok(t),
                Err(e) => Err(PyRuntimeError::new_err(e)),
            }
        }
    }
}
