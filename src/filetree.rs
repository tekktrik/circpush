// SPDX-FileCopyrightText: 2025 Alec Delaney
// SPDX-License-Identifier: MIT

use std::{fs, path::PathBuf};

/// The application directory name
pub const APP_DIRNAME: &str = env!("CARGO_PKG_NAME");

/// The workspace directory name
pub const WORKSPACE_DIRNAME: &str = "workspaces";

/// The port directory name
pub const PORT_DIRNAME: &str = "port";

/// Get the application directory path
pub fn get_app_dir() -> PathBuf {
    let config_dir = dirs::config_dir().expect("Could not locate config directory");
    config_dir.join(APP_DIRNAME)
}

/// Ensure the application directory exists
pub fn ensure_app_dir() {
    let dir = get_app_dir();
    fs::create_dir_all(dir).expect("Could not create application directory");
}

/// Get the port directory path
pub fn get_port_dir() -> PathBuf {
    get_app_dir().join(PORT_DIRNAME)
}

/// Ensure the port directory exists
pub fn ensure_port_dir() {
    let dir = get_port_dir();
    fs::create_dir_all(dir).expect("Could not create port directory");
}

/// Get the workspace directory path
pub fn get_workspace_dir() -> PathBuf {
    get_app_dir().join(WORKSPACE_DIRNAME)
}

/// Ensure the workspace directory exists
pub fn ensure_workspace_dir() {
    let dir = get_workspace_dir();
    fs::create_dir_all(dir).expect("Could not create workspace directory");
}

#[cfg(all(feature = "test-support", test))]
mod test {
    use std::path::PathBuf;

    use super::*;

    /// Tests that the application directory is approrpiate
    #[test]
    fn get_app_dir() {
        let app_dir = super::get_app_dir();
        assert!(app_dir.ends_with(env!("CARGO_PKG_NAME")))
    }

    /// Tests that ensuring the application directory works
    #[test]
    #[serial_test::serial]
    fn ensure_app_dir() {
        // Prepare a fresh state for the test
        // This function already ensures the application directory exists
        let preexisted = crate::test_support::prepare_fresh_state();

        // Get the application directory and check that it exists
        let app_dir = super::get_app_dir();
        assert!(app_dir.as_path().is_dir());

        // Restore the previous state after the test
        crate::test_support::restore_previous_state(preexisted);
    }

    /// Tests that the application workspace directory is approrpiate
    #[test]
    fn get_workspace_dir() {
        let app_dir = super::get_workspace_dir();
        let endpath = PathBuf::from(env!("CARGO_PKG_NAME")).join(WORKSPACE_DIRNAME);
        assert!(app_dir.ends_with(endpath))
    }

    /// Tests that ensuring the application workspace directory works
    #[test]
    #[serial_test::serial]
    fn ensure_workspace_dir() {
        // Prepare a fresh state for the test
        // This function already ensures the application workspace directory exists
        let preexisted = crate::test_support::prepare_fresh_state();

        // Get the application workspace directory and check that it exists
        let app_dir = super::get_workspace_dir();
        assert!(app_dir.as_path().is_dir());

        // Restore the previous state after the test
        crate::test_support::restore_previous_state(preexisted);
    }
}
