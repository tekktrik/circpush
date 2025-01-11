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

#[cfg(feature="test-support")]
pub mod test_support {

    use super::*;

    use std::fs;

    pub const TEST_APP_DIRECTORY_NAME: &str = ".circpush-test";

    pub fn stop_server() {
        tcp::client::stop_server().expect("Could not stop server");
    }

    pub fn save_app_directory() -> bool {
        let app_directory = cli::get_app_dir();
        let test_directory = app_directory.with_file_name(TEST_APP_DIRECTORY_NAME);
        let preexists = app_directory.exists();
        if preexists {
            fs::create_dir(&test_directory).expect("Could not create test application directory");
            fs_extra::dir::move_dir(&app_directory, &test_directory, &fs_extra::dir::CopyOptions::new()).expect("Could not rename existing application directory");
        }
        cli::ensure_app_dir();
        preexists
    }

    pub fn restore_app_directory() {
        let app_directory = cli::get_app_dir();
        let test_directory = app_directory.with_file_name(TEST_APP_DIRECTORY_NAME);
        fs_extra::dir::move_dir(&test_directory.join(env!("CARGO_PKG_NAME")), &app_directory.parent().expect("Could not get config folder"), &fs_extra::dir::CopyOptions::new()).expect("Could not restore application directory");
        fs::remove_dir(test_directory).expect("Could not delete test application folder");
    }

    pub fn prepare_fresh_state() -> bool {
        let preexists = save_app_directory();
        tcp::server::start_server();
        while tcp::client::ping().is_err() {}
        preexists
    }

    pub fn restore_previous_state(preexisted: bool) {
        stop_server();
        while tcp::client::ping().is_ok() {}

        fs::remove_dir_all(cli::get_app_dir()).expect("Could not delete test directory");

        if preexisted {
            restore_app_directory();
        }
    }
}
