// SPDX-FileCopyrightText: 2025 Alec Delaney
// SPDX-License-Identifier: MIT

pub mod client;
pub mod server;

#[cfg(all(test, feature = "test-support"))]
mod test {

    use std::{fs, path::Path, thread, time::Duration};

    use tempfile::TempDir;

    use super::*;

    /// Helper function for running a function with server running in a separate thread
    fn with_threaded_server<F>(f: F) -> Result<String, String>
    where
        F: FnOnce() -> Result<String, String>,
    {
        // Create a duration of 100ms for delays between steps
        let delay_ms = Duration::from_millis(200);

        // Save the current state of the application directory
        let preexisted = crate::test_support::save_app_directory();

        // Spawn a thread for the server
        let handle = thread::spawn(|| {
            let _resp = server::run_server(0);
        });

        // Allow the server to start
        thread::sleep(delay_ms);

        // Run the given function
        let result = f();

        // Allow the function to complete, if needed
        thread::sleep(delay_ms);

        // Stop the server
        client::stop_server().expect("Server thread not ended");

        // Wait for the server thread to finish
        handle.join().expect("Could not join with server thread");

        // Restore the previous application directory if it existed
        if preexisted {
            crate::test_support::restore_app_directory();
        }

        // Return the result of the given function
        result
    }

    /// Returns a closure that starts a file monitor
    fn get_start_monitor_closure() -> (impl FnOnce() -> Result<String, String>, TempDir) {
        // Get the path to a temporary directory
        let tempdir = TempDir::new().expect("Coulkd not create temporary write directory");
        let tempdir_path = tempdir.path().to_path_buf();

        // Get a closure that will start a file monitor using the temporary directory
        let start_monitor_func =
            || client::start_monitor(String::from("test*"), tempdir_path.clone(), tempdir_path);

        // Return the closure and temporary directory
        (start_monitor_func, tempdir)
    }

    /// Tests the success of the ping functionality
    #[test]
    #[serial_test::serial]
    fn ping_success() {
        // Store the expected response message
        let ping_msg = "Ping received!";

        // Create a closure for pinging the servers
        let ping_func = || client::ping(None);

        // Run the closure with a server
        let response = with_threaded_server(ping_func);

        // Check that the response message matches the expected message
        let msg = response.unwrap();
        assert_eq!(&msg, ping_msg);
    }

    mod start_server {

        #[test]
        #[serial_test::serial]
        fn success() {
            // Save the current state of the application directory
            let preexisted = crate::test_support::save_app_directory();

            // Start the server and wait to fully spin up
            crate::tcp::server::start_server(0).expect("Could not start server");

            // Check the server is running
            while crate::tcp::client::ping(None).is_err() {}
            assert!(crate::tcp::server::is_server_running());

            // Stop the server and wait to fully shutdown
            crate::tcp::client::stop_server().expect("Could not stop server");
            while crate::tcp::client::ping(None).is_ok() {}

            // Restore the previous application directory if it existed
            if preexisted {
                crate::test_support::restore_app_directory();
            }

            // Check the server is no longer running
            crate::tcp::client::ping(None).expect_err("Successfully pinged server");
            assert!(!crate::tcp::server::is_server_running());
        }
    }

    /// Tests the success of the stop server functionality
    #[test]
    #[serial_test::serial]
    fn stop_server_success() {
        // Save the current state of the application directory
        let preexisted = crate::test_support::save_app_directory();

        // Store the delay duration
        let delay_ms = Duration::from_millis(200);

        // Spawn a thread to run the server
        let handle = thread::spawn(|| {
            let _resp = server::run_server(0);
        });

        // Pause for the delay duration
        thread::sleep(delay_ms);

        // Get expected response message
        let port = client::get_port();
        let expected_msg = format!("Server on port {port} shutdown");

        // Stop the server and get the response message
        let response = client::stop_server();

        // Wait for the server thread to finish
        handle.join().expect("Could not join with server thread");

        // Restore the previous application directory if it existed
        if preexisted {
            crate::test_support::restore_app_directory();
        }

        // Check that the response message matches the expected message
        let msg = response.unwrap();
        assert_eq!(&msg, &expected_msg);
    }

    /// Tests the success of the start monitor functionality
    #[test]
    #[serial_test::serial]
    fn start_monitor_success() {
        // Store the expected response message
        let resp_msg = "Link 1 started!";

        // Get the closure for starting the file monitor
        let (start_monitor_func, _tempdir) = get_start_monitor_closure();

        // Run the closure with a server
        let response = with_threaded_server(start_monitor_func);

        // Check that the response message matches the expected message
        let msg = response.unwrap();
        assert_eq!(&msg, resp_msg);
    }

    mod stop_monitor {

        use super::*;

        /// Tests the success of the stop monitor functionality, when:
        ///
        /// - Stopping a single file monitor
        #[test]
        #[serial_test::serial]
        fn single() {
            // Store the expected response message
            let resp_msg = "Link removed!";

            // Get the closure for starting the file monitor
            let (start_monitor_func, _tempdir) = get_start_monitor_closure();

            // Get a closure for stopping a file monitor
            let stop_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor");
                client::stop_monitor(1)
            };

            // Run the closure with a server
            let response = with_threaded_server(stop_monitor_func);

            // Check that the response message matches the expected message
            let msg = response.unwrap();
            assert_eq!(&msg, resp_msg);
        }

        /// Tests the success of the stop monitor functionality, when:
        ///
        /// - Stopping all file monitor
        #[test]
        #[serial_test::serial]
        fn all() {
            // Store the expected response message
            let resp_msg = "All links cleared!";

            // Get the closure for starting the file monitor
            let (start_monitor_func, _tempdir) = get_start_monitor_closure();

            // Get a closure for stopping all file monitors
            let stop_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor");
                client::stop_monitor(0)
            };

            // Run the closure with a server
            let response = with_threaded_server(stop_monitor_func);

            // Check that the response message matches the expected message
            let msg = response.unwrap();
            assert_eq!(&msg, resp_msg);
        }

        /// Tests the success of the stop monitor functionality, when:
        ///
        /// - No file monitors are active
        #[test]
        #[serial_test::serial]
        fn none_active() {
            // Store the expected response message
            let err_msg = "No links are active";

            // Get a closure for stopping a file monitor without any being started
            let stop_monitor_func = || client::stop_monitor(1);

            // Run the closure with a server
            let response = with_threaded_server(stop_monitor_func);

            // Check that the response message matches the expected message
            let msg = response.unwrap_err();
            assert_eq!(&msg, err_msg);
        }

        /// Tests the success of the stop monitor functionality, when:
        ///
        /// - The requested file monitor does not exist
        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            // Store the expected response message
            let linknum = 2;
            let err_msg = format!("Link {linknum} does not exist!");

            // Get the closure for starting the file monitor
            let (start_monitor_func, _tempdir) = get_start_monitor_closure();

            // Get a closure for stopping the non-existent file monitor
            let stop_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor");
                client::stop_monitor(linknum)
            };

            // Run the closure with a server
            let response = with_threaded_server(stop_monitor_func);

            // Check that the response message matches the expected message
            let msg = response.unwrap_err();
            assert_eq!(&msg, &err_msg);
        }
    }

    mod view_monitor {

        use std::env;

        use pathdiff::diff_paths;

        use super::*;

        /// Helper function for testing viewing a file monitor with a given set of viewing parameters
        fn test_view_monitor(relative: bool, link_num: usize) {
            // Get closures for starting file monitors
            let (start_monitor_func1, tempdir1) = get_start_monitor_closure();
            let (start_monitor_func2, tempdir2) = get_start_monitor_closure();

            // Get the current working directory
            let current_dir = env::current_dir().expect("Could not get current directory");

            // Get the path the the temporary directories used by the file monitors
            let mut tempdir1_path = tempdir1.path().to_path_buf();
            let mut tempdir2_path = tempdir2.path().to_path_buf();

            // Use relative paths for the temporary directories if requested
            if relative {
                tempdir1_path =
                    diff_paths(&tempdir1_path, &current_dir).expect("Could not get relative path");
                tempdir2_path =
                    diff_paths(&tempdir2_path, &current_dir).expect("Could not get relative path");
            }

            // Get the path components to be shown in the expected response message
            let tempdir1_comps = (&tempdir1_path, &tempdir1_path);
            let tempdir2_comps = (&tempdir2_path, &tempdir2_path);

            // Use only the components requested for the expected response message
            let expecteds = if link_num == 0 {
                vec![tempdir1_comps, tempdir2_comps]
            } else {
                vec![tempdir2_comps]
            };

            // Generate the full set of parsed parts of the expected response message
            let expected_parts =
                crate::test_support::generate_expected_parts(&expecteds, link_num, None);

            // Get a closure for viewing the file monitor(s)
            let view_monitor_func = || {
                start_monitor_func1().expect("Could not start file monitor 1");
                start_monitor_func2().expect("Could not start file monitor 1");
                client::view_monitor(link_num, !relative)
            };

            // Run the closure with a server
            let response = with_threaded_server(view_monitor_func);

            // Check that the response message matches the expected message
            let msg = response.unwrap();
            let response_parts = crate::test_support::parse_contents(&msg, false);
            assert_eq!(response_parts, expected_parts);
        }

        /// Tests viewing a file monitor, when:
        ///
        /// - A single file monitor is requested
        /// - The paths are requested as absolute
        #[test]
        #[serial_test::serial]
        fn single_absolute() {
            test_view_monitor(false, 2);
        }

        /// Tests viewing a file monitor, when:
        ///
        /// - A single file monitor is requested
        /// - The paths are requested as relative
        #[test]
        #[serial_test::serial]
        fn single_relative() {
            test_view_monitor(true, 2);
        }

        /// Tests viewing a file monitor, when:
        ///
        /// - All file monitors are requested
        /// - The paths are requested as absolute
        #[test]
        #[serial_test::serial]
        fn all_absolute() {
            test_view_monitor(false, 0);
        }

        /// Tests viewing a file monitor, when:
        ///
        /// - All file monitors are requested
        /// - The paths are requested as relative
        #[test]
        #[serial_test::serial]
        fn all_relative() {
            test_view_monitor(true, 0);
        }

        /// Tests viewing a file monitor, when:
        ///
        /// - No file monitors are active
        #[test]
        #[serial_test::serial]
        fn none_active() {
            // Store the expected response message
            let expected_msg = "No links are active";

            // Get a closure for viewing a file monitor without any being started
            let view_monitor_func = || client::view_monitor(2, true);

            // Run the closure with a server
            let response = with_threaded_server(view_monitor_func);

            // Check that the response message matches the expected message
            let msg = response.unwrap_err();
            assert_eq!(&msg, expected_msg);
        }

        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            // Store the expected response message
            let link_num = 2;
            let expected_msg = format!("Link {link_num} does not exist!");

            // Get the closure for starting the file monitor
            let (start_monitor_func, _tempdir) = get_start_monitor_closure();

            // Get a closure for viewing the non-existent file monitor
            let view_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor 1");
                client::view_monitor(link_num, true)
            };

            // Run the closure with a server
            let response = with_threaded_server(view_monitor_func);

            // Check that the response message matches the expected message
            let msg = response.unwrap_err();
            assert_eq!(msg, expected_msg);
        }
    }

    mod save_workspace {

        use std::fs;

        use crate::workspace::Workspace;

        use super::*;

        /// Tests the successful saving a workspace
        #[test]
        #[serial_test::serial]
        fn success() {
            // Store the workspace details
            let name = "testworkspace";
            let description = "A test description";

            // Store the expected response message
            let expected_msg =
                format!("Saved the current set of file monitors as workspace '{name}'");

            // Get the closure for starting the file monitor
            let (start_monitor_func, _tempdir) = get_start_monitor_closure();

            // Get a closure for saving a workspace
            let save_workspace_func = || {
                start_monitor_func().expect("Could not start file monitor 1");
                client::save_workspace(&name, &description, false)
            };

            // Run the closure with a server
            let response = with_threaded_server(save_workspace_func);

            // Check that the response message matches the expected message
            let msg = response.unwrap();
            assert_eq!(msg, expected_msg);
        }

        /// Tests saving a workspace when no file monitors are active
        #[test]
        #[serial_test::serial]
        fn none_active() {
            // Store the expected response message
            let expected_msg = "No file monitors are active to save";

            // Store the workspace details
            let name = "testworkspace";
            let description = "A test description";

            // Get a closure for saving a workspace without any file monitors being started
            let save_workspace_func = || client::save_workspace(&name, &description, false);

            // Run the closure with a server
            let response = with_threaded_server(save_workspace_func);

            // Check that the response error message matches the expected message
            let msg = response.unwrap_err();
            assert_eq!(&msg, expected_msg);
        }

        /// Tests saving a workspace when a workspace with that name has already been saved
        #[test]
        #[serial_test::serial]
        fn already_exists_error() {
            // Store the workspace details
            let name = "testworkspace";
            let description = "A test description";

            // Store the expected response message
            let expected_msg =
                format!("Workspace '{name}' already exists, use --force to overwrite it");

            // Get the closure for starting the file monitor
            let (start_monitor_func, _tempdir) = get_start_monitor_closure();

            // Get a closure for saving a workspace when a file with that workspace name already exists
            let save_workspace_func = || {
                // Start the file monitor
                start_monitor_func().expect("Could not start file monitor 1");

                // Create a file to occupy the space of the workspace to be saved
                let filepath = Workspace::get_filepath_for_name(&name);
                fs::File::create(&filepath).expect("Could not create new file");

                // Attempt to save the workspace
                client::save_workspace(&name, &description, false)
            };

            // Run the closure with a server
            let response = with_threaded_server(save_workspace_func);

            // Check that the response error message matches the expected message
            let msg = response.unwrap_err();
            assert_eq!(msg, expected_msg);
        }
    }

    /// Tests the setting of a workspace name on the server (used when loading a workspace)
    #[test]
    #[serial_test::serial]
    fn set_workspace_name_success() {
        // Store the workspace name
        let name = "testworkspace";

        // Store the expected response message
        let expected_msg = format!("Workspace name set to '{name}'");

        // Get the closure for setting the workspace name for the server
        let set_workspace_name_func = || client::set_workspace_name(name);

        // Run the closure with a server
        let response = with_threaded_server(set_workspace_name_func);

        // Check that the response error message matches the expected message
        let msg = response.unwrap();
        assert_eq!(msg, expected_msg);
    }

    mod load_workspace {

        use std::path::PathBuf;

        use crate::workspace::Workspace;

        use super::*;

        /// Tests the successful loading of a workspace
        #[test]
        #[serial_test::serial]
        fn success() {
            // Store the workspace name
            let name = "nodescription";

            // Store the expected response message
            let expected_msg = format!("Started workspace '{name}'");

            // Get a closure for loading a workspace
            let load_workspace_func = || {
                // Get the filepath of the workspace file to load
                let src_filepath = PathBuf::from(format!("tests/assets/workspaces/{name}.json"));

                // Get the filepath for where the intended workspace file will be loaded
                let filepath = Workspace::get_filepath_for_name(name);

                // Copy the test asset workspace file to the necessary location
                fs::File::create_new(&filepath).expect("Could not create new file");
                fs::copy(&src_filepath, &filepath).expect("Could not copy file contents");

                // Load the workspace
                client::load_workspace(name)
            };

            // Run the closure with a server
            let response = with_threaded_server(load_workspace_func);

            // Check that the response message matches the expected message
            let msg = response.unwrap();
            assert_eq!(msg, expected_msg);
        }

        /// Tests attempting to load a workspace when the workspace file is formatted incorrectly
        #[test]
        #[serial_test::serial]
        fn unexpected_format_error() {
            // Store the workspace name
            let name = "badformat";

            // Store the expected response message
            let expected_msg = format!("Could not parse the format of workspace '{name}'");

            // Get a closure for loading a workspace when the workspace file is formatted incorrectly
            let load_workspace_func = || {
                // Get the filepath for where the intended workspace file will be loaded
                let filepath = Workspace::get_filepath_for_name(name);

                // Create a blank file in the necessary location
                fs::File::create_new(&filepath).expect("Could not create new file");

                // Load the workspace
                client::load_workspace(name)
            };

            // Run the closure with a server
            let response = with_threaded_server(load_workspace_func);

            // Check that the response error message matches the expected message
            let msg = response.unwrap_err();
            assert_eq!(msg, expected_msg);
        }

        /// Tests attempting to load a workspace when the saved workspace does not exist
        #[test]
        #[serial_test::serial]
        fn does_not_exist_error() {
            // Store the workspace name
            let name = "doesnotexist";

            // Store the expected response message
            let expected_msg = format!("Workspace '{name}' does not exist");

            // Get a closure for loading a workspace when the workspace file is formatted incorrectly
            let load_workspace_func = || client::load_workspace(name);

            // Run the closure with a server
            let response = with_threaded_server(load_workspace_func);

            // Check that the response error message matches the expected message
            let msg = response.unwrap_err();
            assert_eq!(msg, expected_msg);
        }
    }

    /// Tests view a workspace (specifically when none are active)
    #[test]
    #[serial_test::serial]
    fn view_workspace() {
        // Store the expected response message
        let expected_msg = "No workspace is currently active";

        // Get a closure for viewing a workspace
        let view_workspace_func = || client::get_current_workspace();
        let response = with_threaded_server(view_workspace_func);

        // Check that the response message matches the expected message
        let msg = response.unwrap();
        assert_eq!(msg, expected_msg);
    }

    mod run_server {

        use std::fs;

        use super::*;

        /// Tests running the server when broken file monitors are detected
        #[test]
        #[serial_test::serial]
        fn broken_monitors() {
            // Get the expected response message for just a table with the header row
            let path_components: Vec<(&Path, &Path)> = Vec::new();
            let expected_msg =
                crate::test_support::generate_expected_parts(&path_components, 0, None);

            // Get the closure for starting the file monitor
            let (start_monitor_func, tempdir) = get_start_monitor_closure();

            // Get a closure for pinging the server when a file monitor is broken
            let ping_func = move || {
                // Create a new file
                fs::File::create_new(tempdir.path().join("test_todelete"))
                    .expect("Could not create new file");

                // Start the file monitor
                start_monitor_func().expect("Could not start file monitor 1");

                // Wait for the server to track the newly created file
                thread::sleep(Duration::from_millis(200));

                // Check that the file is being tracked by storing the response of client::view_monitor()
                let existing_view = client::view_monitor(0, true);

                // Remove the temporary director housing the created file
                fs::remove_dir_all(tempdir.path()).expect("Could not remove temporary directory");

                // Wait for the server to find the broken file monitor and stop it
                thread::sleep(Duration::from_millis(200));

                // Check that the file is no longer being tracked by storing the response of client::view_monitor()
                let deleted_view = client::view_monitor(0, true);

                // Check that the responses of client::view_monitor() don't match before and after attempted deletion
                assert_ne!(existing_view, deleted_view);

                // Return the response after the file was deleted
                deleted_view
            };

            // Run the closure with a server
            let response = with_threaded_server(ping_func);

            // Parse the reponse
            let msg = response.unwrap();
            let parsed_msg = crate::test_support::parse_contents(&msg, false);

            // Check that the response message matches the expected message
            assert_eq!(parsed_msg, expected_msg);
        }
    }
}
