mod board;
mod cli;
mod commands;
mod link;
mod monitor;
mod tcp;
mod workspace;

use pyo3::prelude::*;
use std::process::exit;

/// Python module created using PyO3 (circpush)
#[pymodule]
pub mod circpush {

    use super::*;

    /// Function within the module (cli())
    ///
    /// This is essentially just the PyO3 wrappcer around cli::entry(),
    /// that prints out the resulting text exits with the appropriate
    /// exit code.
    #[pyfunction]
    pub fn cli() -> PyResult<()> {
        match crate::cli::entry() {
            Ok(text) => {
                println!("{text}");
                Ok(())
            }
            Err(text) => {
                println!("{text}");
                exit(1);
            }
        }
    }
}
