pub mod client;
pub mod server;

#[cfg(all(test, feature = "test-support"))]
mod test {

    use std::{fs, path::Path, thread, time::Duration};

    use tempfile::TempDir;

    use super::*;

    fn with_threaded_server<F>(f: F, delay_ms: u64) -> Result<String, String>
    where F: FnOnce() -> Result<String, String> {
        let preexisted = crate::test_support::save_app_directory();

        let handle = thread::spawn(|| {
            let _resp = server::run_server();
        });
        thread::sleep(Duration::from_millis(delay_ms));
        
        let result = f();
        thread::sleep(Duration::from_millis(delay_ms));
        
        client::stop_server().expect("Server thread not ended");

        handle.join().expect("Could not join with server thread");

        if preexisted {
            crate::test_support::restore_app_directory();
        }

        result
    }

    fn get_start_monitor_closure() -> (impl FnOnce() -> Result<String, String>, TempDir) {
        let tempdir = TempDir::new().expect("Coulkd not create temporary write directory");
        let tempdir_path = tempdir.path().to_path_buf();
        let start_monitor_func = || client::start_monitor(String::from("test*"), tempdir_path.clone(), tempdir_path);
        (start_monitor_func, tempdir)
    }

    #[test]
    #[serial_test::serial]
    fn ping_success() {
        let ping_func = || client::ping();
        let response = with_threaded_server(ping_func, 500);
        
        let msg = response.unwrap();
        assert_eq!(&msg, "Ping received!");
    }

    #[test]
    #[serial_test::serial]
    fn echo_success() {
        let echo_msg = "This is a test message";
        
        let ping_func = || client::echo(echo_msg.to_string());
        let response = with_threaded_server(ping_func, 500);
        
        let msg = response.unwrap();
        assert_eq!(&msg, echo_msg);
    }

    #[test]
    #[serial_test::serial]
    fn stop_server_success() {
        let delay_ms = 500;
        let handle = thread::spawn(|| {
            let _resp = server::run_server();
        });
        thread::sleep(Duration::from_millis(delay_ms));

        let port = server::PORT;
        let expected_msg = format!("Server on port {port} shutdown");
        let response = client::stop_server();
        handle.join().expect("Could not join with server thread");

        let msg = response.unwrap();
        assert_eq!(&msg, &expected_msg);
    }


    #[test]
    #[serial_test::serial]
    fn start_monitor_success() {
        let (start_monitor_func, _tempdir) = get_start_monitor_closure();
        let resp_msg = "Link 1 started!";
        let response = with_threaded_server(start_monitor_func, 500);
        
        let msg = response.unwrap();
        assert_eq!(&msg, resp_msg);
    }

    mod stop_monitor {

        use super::*;

        #[test]
        #[serial_test::serial]
        fn single() {
            let (start_monitor_func, _tempdir) = get_start_monitor_closure();
            
            let resp_msg = "Link removed!";

            let stop_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor");
                client::stop_monitor(1)
            };
            let response = with_threaded_server(stop_monitor_func, 500);

            let msg = response.unwrap();
            assert_eq!(&msg, resp_msg);
        }

        #[test]
        #[serial_test::serial]
        fn all() {
            let (start_monitor_func, _tempdir) = get_start_monitor_closure();
            
            let resp_msg = "All links cleared!";
            
            let stop_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor");
                client::stop_monitor(0)
            };
            let response = with_threaded_server(stop_monitor_func, 500);

            let msg = response.unwrap();
            assert_eq!(&msg, resp_msg);
        }

        #[test]
        #[serial_test::serial]
        fn none_active() {
            let err_msg = "No links are active";
            
            let stop_monitor_func = || client::stop_monitor(1);
            
            let response = with_threaded_server(stop_monitor_func, 500);

            let msg = response.unwrap_err();
            assert_eq!(&msg, err_msg);
        }

        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            let linknum = 2;
            let err_msg = format!("Link {linknum} does not exist!");
            
            let (start_monitor_func, _tempdir) = get_start_monitor_closure();
            let stop_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor");
                client::stop_monitor(linknum)
            };
            let response = with_threaded_server(stop_monitor_func, 500);

            let msg = response.unwrap_err();
            assert_eq!(&msg, &err_msg);
        }
    }

    mod view_monitor {

        use std::env;

        use pathdiff::diff_paths;

        use super::*;

        fn test_view_monitor(relative: bool, link_num: usize) {
            let (start_monitor_func1, tempdir1) = get_start_monitor_closure();
            let (start_monitor_func2, tempdir2) = get_start_monitor_closure();

            let current_dir = env::current_dir().expect("Could not get current directory");

            let mut tempdir1_path = tempdir1.path().to_path_buf();
            let mut tempdir2_path = tempdir2.path().to_path_buf();

            if relative {
                tempdir1_path = diff_paths(&tempdir1_path, &current_dir).expect("Could not get relative path");
                tempdir2_path = diff_paths(&tempdir2_path, &current_dir).expect("Could not get relative path");
            }

            let tempdir1_comps = (&tempdir1_path, &tempdir1_path);
            let tempdir2_comps = (&tempdir2_path, &tempdir2_path);

            let expecteds = if link_num == 0 {
                vec![tempdir1_comps, tempdir2_comps]
            } else {
                vec![tempdir2_comps]
            };

            let expected_parts = crate::test_support::generate_expected_parts(&expecteds, link_num, None);

            let view_monitor_func = || {
                start_monitor_func1().expect("Could not start file monitor 1");
                start_monitor_func2().expect("Could not start file monitor 1");
                client::view_monitor(link_num, !relative)
            };
            let response = with_threaded_server(view_monitor_func, 500);
            
            let msg = response.unwrap();
            assert_eq!(crate::test_support::parse_contents(&msg, false), expected_parts);
        }

        #[test]
        #[serial_test::serial]
        fn single_absolute() {
            test_view_monitor(false, 2);
        }

        #[test]
        #[serial_test::serial]
        fn single_relative() {
            test_view_monitor(true, 2);
        }

        #[test]
        #[serial_test::serial]
        fn all_absolute() {
            test_view_monitor(false, 0);
        }

        #[test]
        #[serial_test::serial]
        fn all_relative() {
            test_view_monitor(true, 0);
        }

        #[test]
        #[serial_test::serial]
        fn none_active() {
            let view_monitor_func = || client::view_monitor(2, true);
            let response = with_threaded_server(view_monitor_func, 500);
            
            let expected_msg = "No links are active";

            let msg = response.unwrap_err();
            assert_eq!(&msg, expected_msg);
        }

        #[test]
        #[serial_test::serial]
        fn does_not_exist() {
            let link_num = 2;

            let (start_monitor_func, _tempdir) = get_start_monitor_closure();

            let view_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor 1");
                client::view_monitor(link_num, true)
            };
            let response = with_threaded_server(view_monitor_func, 500);
            
            let expected_msg = format!("Link {link_num} does not exist!");

            let msg = response.unwrap_err();
            assert_eq!(msg, expected_msg);
        }
    }

    mod save_workspace {

        use std::fs;

        use crate::workspace::Workspace;

        use super::*;

        #[test]
        #[serial_test::serial]
        fn success() {
            let name = "testworkspace";
            let description = "A test description";

            let (start_monitor_func, _tempdir) = get_start_monitor_closure();

            let save_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor 1");
                client::save_workspace(&name, &description, false)
            };
            let response = with_threaded_server(save_monitor_func, 500);
        
            let expected_msg = format!("Saved the current set of file monitors as workspace '{name}'");

            let msg = response.unwrap();
            assert_eq!(msg, expected_msg);
        }

        #[test]
        #[serial_test::serial]
        fn no_monitors_active() {
            let name = "testworkspace";
            let description = "A test description";

            let view_monitor_func = || {
                client::save_workspace(&name, &description, false)
            };
            let response = with_threaded_server(view_monitor_func, 500);
        
            let expected_msg = "No file monitors are active to save";

            let msg = response.unwrap_err();
            assert_eq!(&msg, expected_msg);
        }

        #[test]
        #[serial_test::serial]
        fn already_exists_error() {
            let name = "testworkspace";
            let description = "A test description";

            let (start_monitor_func, _tempdir) = get_start_monitor_closure();

            let save_monitor_func = || {
                start_monitor_func().expect("Could not start file monitor 1");
                let filepath = Workspace::get_filepath_for_name(&name);
                fs::File::create(&filepath).expect("Could not create new file");
                client::save_workspace(&name, &description, false)
            };
            let response = with_threaded_server(save_monitor_func, 500);
        
            let expected_msg = format!("Workspace '{name}' already exists, use --force to overwrite it");

            let msg = response.unwrap_err();
            assert_eq!(msg, expected_msg);
        }
    }

    #[test]
    #[serial_test::serial]
    fn set_workspace_name_success() {
        let name = "testworkspace";

        let save_monitor_func = || {
            client::set_workspace_name(name)
        };
        let response = with_threaded_server(save_monitor_func, 500);
    
        let expected_msg = format!("Workspace name set to '{name}'");

        let msg = response.unwrap();
        assert_eq!(msg, expected_msg);
    }

    mod load_workspace {

        use std::path::PathBuf;

        use crate::workspace::Workspace;

        use super::*;

        #[test]
        #[serial_test::serial]
        fn success() {
            let name = "nodescription";

            let save_monitor_func = || {
                let src_filepath = PathBuf::from(format!("tests/assets/workspaces/{name}.json"));
                
                let filepath = Workspace::get_filepath_for_name(name);

                fs::File::create_new(&filepath).expect("Could not create new file");
                fs::copy(&src_filepath, &filepath).expect("Could not copy file contents");
                
                client::load_workspace(name)
            };
            let response = with_threaded_server(save_monitor_func, 500);
        
            let expected_msg = format!("Started workspace '{name}'");

            let msg = response.unwrap();
            assert_eq!(msg, expected_msg);
        }

        #[test]
        #[serial_test::serial]
        fn unexpected_format_error() {
            let name = "badformat";

            let save_monitor_func = || {                
                let filepath = Workspace::get_filepath_for_name(name);
                fs::File::create_new(&filepath).expect("Could not create new file");
                client::load_workspace(name)
            };
            let response = with_threaded_server(save_monitor_func, 500);
        
            let expected_msg = format!("Could not parse the format of workspace '{name}'");

            let msg = response.unwrap_err();
            assert_eq!(msg, expected_msg);
        }

        #[test]
        #[serial_test::serial]
        fn does_not_exist_error() {
            let name = "doesnotexist";

            let save_monitor_func = || {                
                client::load_workspace(name)
            };
            let response = with_threaded_server(save_monitor_func, 500);
        
            let expected_msg = format!("Workspace '{name}' does not exist");

            let msg = response.unwrap_err();
            assert_eq!(msg, expected_msg);
        }

        #[test]
        #[serial_test::serial]
        fn start_monitor_error() {
            assert!(true);
        }
    }

    #[test]
    #[serial_test::serial]
    fn view_workspace() {
        let view_monitor_func = || client::get_current_workspace();
        let response = with_threaded_server(view_monitor_func, 500);
        
        let expected_msg = "No workspace is currently active";

        let msg = response.unwrap();
        assert_eq!(msg, expected_msg);
    }

    mod run_server {

        use std::fs;

        use super::*;

        #[test]
        #[serial_test::serial]
        fn broken_monitors() {
            let path_components: Vec<(&Path, &Path)> = Vec::new();
            let expected_msg = crate::test_support::generate_expected_parts(&path_components, 0, None);

            let (start_monitor_func, tempdir) = get_start_monitor_closure();

            let ping_func = move || {
                fs::File::create_new(tempdir.path().join("test*")).expect("Could not create new file");
                start_monitor_func().expect("Could not start file monitor 1");
                thread::sleep(Duration::from_millis(500));
                fs::remove_dir_all(tempdir.path()).expect("Could not remove temporary directory");
                thread::sleep(Duration::from_millis(500));
                client::view_monitor(0, true)
            };

            let response = with_threaded_server(ping_func, 500);
            
            let msg = response.unwrap();
            assert_eq!(crate::test_support::parse_contents(&msg, false), expected_msg);
        }
    }
}