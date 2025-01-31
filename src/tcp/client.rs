use crate::commands::{Request, Response, STOP_RESPONSE};
use crate::monitor::{as_table, FileMonitor};
use crate::tcp::server::PORT;
use crate::workspace::{Workspace, WorkspaceLoadError};
use serde::Deserialize;
use std::io::prelude::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

/// Open a non-blocking connection to the TCP server
fn open_connection() -> Result<TcpStream, String> {
    // Get the connection information
    let localhost_addr_v4 = Ipv4Addr::LOCALHOST;
    let localhost_addr = IpAddr::V4(localhost_addr_v4);
    let socket_addr = SocketAddr::new(localhost_addr, PORT);

    // Get the TCP stream
    let stream = match TcpStream::connect(socket_addr) {
        Ok(stream) => stream,
        Err(_) => {
            return Err(format!(
                "Could not connect to the server on port {PORT}, is the server running?"
            ))
        }
    };

    // Set the read timeout for the TCP stream, in case the server is down
    let duration = Duration::from_secs(1);
    stream
        .set_read_timeout(Some(duration))
        .expect("Bad duration passed as socket read timeout.");

    // Return newly opened stream
    Ok(stream)
}

/// Communicate a request to the server and receive the response
fn communicate(request: Request) -> Result<Response, String> {
    let mut stream = open_connection()?;
    let raw_request = serde_json::to_string(&request).expect("Could not serialize requiest");
    stream
        .write_all(raw_request.as_bytes())
        .expect("Could not write request");
    let mut serialization = serde_json::Deserializer::from_reader(&stream);
    Ok(Response::deserialize(&mut serialization).expect("Could not deserialize the response"))
}

/// Send a ping request to the server
pub fn ping() -> Result<String, String> {
    match communicate(Request::Ping) {
        Ok(Response::NoData) => Ok(String::from("Ping received!")),
        _ => Err(String::from(
            "ERROR: Did not receive expected ping response",
        )),
    }
}

/// Send an echo request to the server
pub fn echo(message: String) -> Result<String, String> {
    match communicate(Request::Echo { msg: message }) {
        Ok(Response::Message { msg }) => Ok(msg),
        _ => Err(String::from("ERROR: Did not receive echo response")),
    }
}

/// Send a stop server request to the server
pub fn stop_server() -> Result<String, String> {
    match communicate(Request::Shutdown) {
        Ok(Response::Message { msg }) if msg == STOP_RESPONSE => {
            Ok(format!("Server on port {PORT} shutdown"))
        }
        _ => Err(String::from("ERROR: Did not receive expected response")),
    }
}

/// Send a start file monitor request to the server
pub fn start_monitor(
    read_pattern: String,
    write_directory: PathBuf,
    base_directory: PathBuf,
) -> Result<String, String> {
    if write_directory.as_path().is_symlink() || base_directory.as_path().is_symlink() {
        return Err(String::from("ERROR: Symlinks are not allowed"));
    }

    match communicate(Request::StartLink {
        read_pattern,
        write_directory,
        base_directory,
    }) {
        Ok(Response::Message { msg }) => Ok(msg),
        _ => Err(String::from("ERROR: Could not start link")),
    }
}

/// Send a stop file monitor request to the server
pub fn stop_monitor(number: usize) -> Result<String, String> {
    match communicate(Request::StopLink { number }) {
        Ok(Response::Message { msg }) => Ok(msg),
        Ok(Response::ErrorMessage { msg }) => Err(msg),
        _ => Err(String::from("ERROR: Could not stop link")),
    }
}

fn get_monitor_list(number: usize) -> Result<Vec<FileMonitor>, String> {
    // Get the response of the server communication
    let response = match communicate(Request::ViewLink { number }) {
        Ok(Response::Links { json }) => json,
        Ok(Response::ErrorMessage { msg }) => return Err(msg),
        _ => return Err(String::from("ERROR: Could not retrieve link(s)")),
    };

    // Parse the response string into a list of FileMonitors
    let monitors: Vec<FileMonitor> =
        serde_json::from_str(&response).expect("Failed to parse JSON response");
    Ok(monitors)
}

/// Send a view file monitor request to the server
pub fn view_monitor(number: usize, absolute: bool) -> Result<String, String> {
    let monitor_list = match get_monitor_list(number) {
        Ok(file_monitors) => file_monitors,
        Err(error) => return Err(error),
    };

    let table = as_table(&monitor_list, number, absolute);
    Ok(table.to_string())
}

/// Send a save file monitors request to the server
pub fn save_workspace(name: &str, desc: &str, force: bool) -> Result<String, String> {
    // Get the response of the server communication
    let monitor_list = match get_monitor_list(0) {
        Ok(file_monitors) => file_monitors,
        Err(error) => return Err(error),
    };

    // If there are no file monitors, return an error
    if monitor_list.is_empty() {
        return Err(String::from("No file monitors are active to save"));
    }

    // Create the new workspace object
    let workspace = Workspace::new(desc, &monitor_list);

    // Save the workspace
    match workspace.save_as_name(name, force) {
        Ok(_) => Ok(format!(
            "Saved the current set of file monitors as workspace '{name}'"
        )),
        Err(_) => Err(format!(
            "Workspace '{name}' already exists, use --force to overwrite it"
        )),
    }
}

/// Sets the workspace name
pub fn set_workspace_name(name: &str) -> Result<String, String> {
    match communicate(Request::SetWorkspaceName {
        name: name.to_owned(),
    }) {
        Ok(Response::NoData) => Ok(format!("Workspace name set to '{name}'")),
        _ => Err(String::from("ERROR: Did not receive expected response")),
    }
}

/// Load the given workspace
pub fn load_workspace(name: &str) -> Result<String, String> {
    // Stop current file monitors
    if stop_monitor(0).is_err() {
        return Err(String::from("ERROR: Could not load the workspace"));
    }

    // Load the workspace from the name
    let workspace = match Workspace::from_name(name) {
        Ok(workspace) => workspace,
        Err(WorkspaceLoadError::UnexpectedFormat) => {
            return Err(format!("Could not parse the format of workspace '{name}'"))
        }
        Err(WorkspaceLoadError::DoesNotExist) => {
            return Err(format!("Workspace '{name}' does not exist"))
        }
    };

    // Start the file monitors from the workspace
    for file_monitor in workspace.monitors {
        start_monitor(
            file_monitor.read_pattern,
            file_monitor.write_directory,
            file_monitor.base_directory,
        )
        .expect("Could not start all file monitors");
    }

    // Set the workspace name for the server
    set_workspace_name(name).expect("Could not set the name for the workspace");

    // Retutnr that the workspace was successfully started
    Ok(format!("Started workspace '{name}'"))
}

/// View the current workspace
pub fn get_current_workspace() -> Result<String, String> {
    // Get the response of the server communication
    let mut msg = match communicate(Request::ViewWorkspaceName) {
        Ok(Response::Message { msg }) => msg,
        _ => return Err(String::from("ERROR: Could not retrieve workspace name")),
    };

    // If there is no name, instead return a message saying no workspace is active
    if msg.is_empty() {
        msg = String::from("No workspace is currently active");
    }

    // Return the message
    Ok(msg)
}

#[cfg(test)]
mod test {

    use super::*;

    /// Tests that the ping function returns an error if the server is not running
    #[test]
    #[serial_test::serial]
    fn ping_error() {
        // Get the expected error message
        let expected_err = "ERROR: Did not receive expected ping response";

        // Get the response of the command
        let response = ping();

        // Check the error response
        let err_msg = response.unwrap_err();
        assert_eq!(&err_msg, expected_err);
    }

    /// Tests that the echo function returns an error if the server is not running  
    #[test]
    #[serial_test::serial]
    fn echo_error() {
        // Get the expected error message
        let expected_err = "ERROR: Did not receive echo response";

        // Get the response of the command
        let response = echo(String::from("testmsg"));

        // Check the error response
        let err_msg = response.unwrap_err();
        assert_eq!(&err_msg, expected_err);
    }

    /// Tests that the stop server function returns an error if the server is not running
    #[test]
    #[serial_test::serial]
    fn stop_server_error() {
        // Get the expected error message
        let expected_err = "ERROR: Did not receive expected response";

        // Get the response of the command
        let response = stop_server();

        // Check the error response
        let err_msg = response.unwrap_err();
        assert_eq!(&err_msg, expected_err);
    }

    mod start_monitor {

        use super::*;

        #[cfg(target_family = "unix")]
        use std::os::unix::fs::symlink;
        #[cfg(target_family = "unix")]
	use std::fs::remove_file as remove_symlink;

        #[cfg(target_family = "windows")]
        use std::os::windows::fs::symlink_dir as symlink;
        #[cfg(target_family = "windows")]
        use std::fs::remove_dir as remove_symlink;

        /// Tests attempting to use symlinks for the base and write directory of a file monitor
        #[test]
        #[serial_test::serial]
        fn symlink_use() {
            // Store the expected response message
            let resp_msg = "ERROR: Symlinks are not allowed";

            // Store the symbolic and pointed-to filepaths
            let symbolic = PathBuf::from("tests/assets/temporary");
            let pointed = PathBuf::from("tests/assets/monitors");

            // Check that the intended symlink does not already exist
            assert!(!symbolic.as_path().is_symlink());

            // Check that the pointed to directory exists
            assert!(pointed.as_path().is_dir());

            // Create a symlink to the pointed to directory
            symlink(&pointed, &symbolic).expect("Could not create a symlink");

            // Check that the symlink now exists
            assert!(symbolic.as_path().is_symlink());

            // Attempt to start the monitor with symlinks
            let error = start_monitor(String::from("test*"), symbolic.clone(), symbolic.clone())
                .expect_err("Successfully started file monitor when it should have been prevented");

            // Remove the symlink
            remove_symlink(&symbolic).expect("Could not remove symlink");

            // Check that the returned and expected response messages match
            assert_eq!(&error, resp_msg);
        }

        /// Tests that the start monitor function returns an error if the server is not running
        #[test]
        #[serial_test::serial]
        fn server_not_running() {
            // Get the expected error message
            let resp_msg = "ERROR: Could not start link";

            // Get the response of the command
            let response = start_monitor(
                String::from("test"),
                PathBuf::from("test"),
                PathBuf::from("test"),
            );

            // Check the error response
            let msg = response.unwrap_err();
            assert_eq!(&msg, resp_msg);
        }
    }

    /// Tests that the stop monitor function returns an error if the server is not running
    #[test]
    #[serial_test::serial]
    fn stop_monitor_error() {
        // Get the expected error message
        let resp_msg = "ERROR: Could not stop link";

        // Get the response of the command
        let response = stop_monitor(0);

        // Check the error response
        let msg = response.unwrap_err();
        assert_eq!(&msg, resp_msg);
    }

    /// Tests that the get monitor list function returns an error if the server is not running
    #[test]
    #[serial_test::serial]
    fn get_monitor_list_error() {
        // Get the expected error message
        let resp_msg = "ERROR: Could not retrieve link(s)";

        // Get the response of the command
        let response = get_monitor_list(1);

        // Check the error response
        let msg = response.unwrap_err();
        assert_eq!(&msg, resp_msg);
    }

    /// Tests that the save workspace function returns an error if the server is not running
    #[test]
    #[serial_test::serial]
    fn save_workspace_error() {
        // Get the expected error message
        let resp_msg = "ERROR: Could not retrieve link(s)";

        // Get the response of the command
        let response = save_workspace("test", "test", false);

        // Check the error response
        let msg = response.unwrap_err();
        assert_eq!(&msg, resp_msg);
    }

    /// Tests that the set workspace name function returns an error if the server is not runnin
    #[test]
    #[serial_test::serial]
    fn set_workspace_name_error() {
        // Get the expected error message
        let resp_msg = "ERROR: Did not receive expected response";

        // Get the response of the command
        let response = set_workspace_name("test");

        // Check the error response
        let msg = response.unwrap_err();
        assert_eq!(&msg, resp_msg);
    }

    /// Tests that the load workspace function returns an error if the server is not runnin
    #[test]
    #[serial_test::serial]
    fn load_workspace_error() {
        // Get the expected error message
        let resp_msg = "ERROR: Could not load the workspace";

        // Get the response of the command
        let response = load_workspace("doesnotexist");

        // Check the error response
        let msg = response.unwrap_err();
        assert_eq!(&msg, resp_msg);
    }

    /// Tests that the get current workspace function returns an error if the server is not runnin
    #[test]
    #[serial_test::serial]
    fn get_current_workspace_error() {
        // Get the expected error message
        let resp_msg = "ERROR: Could not retrieve workspace name";

        // Get the response of the command
        let response = get_current_workspace();

        // Check the error response
        let msg = response.unwrap_err();
        assert_eq!(&msg, resp_msg);
    }
}
