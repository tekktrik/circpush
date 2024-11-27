mod commands;
mod link;
mod tcp;
mod monitor;
mod cli;

use pyo3::prelude::*;
use std::process::exit;

#[pymodule]
pub mod circpush {

    use super::*;

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
