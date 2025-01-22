use std::{fs, path::PathBuf};

pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
pub const WORKSPACE_DIRNAME: &str = "workspaces";

pub fn get_app_dir() -> PathBuf {
    let config_dir = dirs::config_dir().expect("Could not locate config directory");
    config_dir.join(APP_NAME)
}

pub fn ensure_app_dir() {
    let dir = get_app_dir();
    fs::create_dir_all(dir).expect("Could not create application directory");
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

    #[test]
    fn get_app_dir() {
        let app_dir = super::get_app_dir();
        assert!(app_dir.ends_with(env!("CARGO_PKG_NAME")))
    }

    #[test]
    #[serial_test::serial]
    fn ensure_app_dir() {
        // test_support::prepare_fresh_start() runs ensure_app_dir()
        let preexisted = crate::test_support::prepare_fresh_state();
        let app_dir = super::get_app_dir();
        assert!(app_dir.exists());
        crate::test_support::restore_previous_state(preexisted);
    }

    #[test]
    fn get_workspace_dir() {
        let app_dir = super::get_workspace_dir();
        let endpath = PathBuf::from(env!("CARGO_PKG_NAME")).join(WORKSPACE_DIRNAME);
        assert!(app_dir.ends_with(endpath))
    }

    #[test]
    #[serial_test::serial]
    fn ensure_workspace_dir() {
        // test_support::prepare_fresh_start() runs ensure_workspace_dir()
        let preexisted = crate::test_support::prepare_fresh_state();
        let app_dir = super::get_workspace_dir();
        assert!(app_dir.exists());
        crate::test_support::restore_previous_state(preexisted);
    }
}
