// SPDX-FileCopyrightText: 2025 Alec Delaney
// SPDX-License-Identifier: MIT

use crate::commands::{Request, Response, STOP_RESPONSE};
use crate::filetree::get_port_dir;
use crate::monitor::FileMonitor;
use serde::Deserialize;
use std::fs;
use std::io::prelude::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::ops::Index;
use std::process::Command;
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;

#[cfg(target_family = "unix")]
use std::process::Stdio;

/// State of the server, consisting of the file monitors and the current
/// workspace name, if any
struct ServerState {
    monitors: Vec<FileMonitor>,
    workspace_name: String,
}

/// Checks to see if server is already running
pub fn is_server_running() -> bool {
    crate::tcp::client::get_port() != 0
}

/// Starts the server in a seperate process by using `circpush run`
#[cfg(target_family = "unix")]
pub fn start_server(port: u16) -> Result<String, String> {
    let _daemon = Command::new("circpush")
        .arg("server")
        .arg("run")
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    Ok(String::from("Starting server"))
}

/// Starts the server in a seperate process by using `circpush run`
#[cfg(target_family = "windows")]
pub fn start_server(port: u16) -> String {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
    let _daemon = Command::new("circpush")
        .arg("server")
        .arg("run")
        .arg("--port")
        .arg(port.to_string())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn();
    format!("Starting server")
}

/// Binds to the associated port on localhost as non-blocking
fn bind_socket(port: u16) -> Result<TcpListener, String> {
    // Get the connection information
    let localhost_addr_v4 = Ipv4Addr::LOCALHOST;
    let localhost_addr = IpAddr::V4(localhost_addr_v4);
    let socket_addr = SocketAddr::new(localhost_addr, port);

    // Bind to the necessary port
    let listener = match TcpListener::bind(socket_addr) {
        Ok(listener) => listener,
        Err(_) => return Err(String::from("Could not bind to port")),
    };

    // Save the allocated port
    let assigned_port = listener.local_addr().unwrap().port();
    let assigned_port_str = assigned_port.to_string();
    let port_file = get_port_dir().join(assigned_port_str);
    fs::File::create_new(port_file).expect("Could not create port file");

    // Set the TCP listener to non-blocking mode
    listener
        .set_nonblocking(true)
        .expect("Could not set the socket to non-blocking");

    // Return the TCP listener
    Ok(listener)
}

/// Handle the TCP stream connection and modify the list of monitors accordingly
fn handle_connection(mut stream: TcpStream, state: &mut ServerState) -> bool {
    // Get the monitors and workspace name as their own references
    let monitors = &mut state.monitors;
    let workspace_name = &mut state.workspace_name;

    // Get the request associated with the TCP connection
    let mut serialization = serde_json::Deserializer::from_reader(&stream);
    let request =
        Request::deserialize(&mut serialization).expect("Unable to deserialize the request");

    // Handle the request and create the associated response
    let response = match &request {
        Request::Ping => Response::NoData,
        Request::Shutdown => Response::Message {
            msg: String::from_str(STOP_RESPONSE).unwrap(),
        },
        Request::StartLink {
            read_pattern,
            write_directory,
            base_directory,
        } => {
            // Create a new FileMonitor
            let new_monitor = FileMonitor::new(read_pattern, write_directory, base_directory);

            // Push the new FileMonitor to the lists
            monitors.push(new_monitor);
            *workspace_name = String::from("");

            // Get the new link number and send it with the response
            let new_link_number = monitors.len();
            Response::Message {
                msg: format!("Link {new_link_number} started!"),
            }
        }
        Request::StopLink { number } => {
            // If the link number is 0, stop all monitors
            if *number == 0 {
                monitors.clear();
                *workspace_name = String::from("");
                Response::Message {
                    msg: String::from("All links cleared!"),
                }
            }
            // Error if there are no links
            else if monitors.is_empty() {
                Response::ErrorMessage {
                    msg: String::from("No links are active"),
                }
            }
            // Error if an out-of-bounds monitor is requested
            else if *number > monitors.len() {
                Response::ErrorMessage {
                    msg: format!("Link {number} does not exist!"),
                }
            }
            // Remove a specific FileMonitor
            else {
                let index = number - 1;
                monitors.remove(index);
                *workspace_name = String::from("");
                Response::Message {
                    msg: String::from("Link removed!"),
                }
            }
        }
        Request::ViewLink { number } => {
            // If the link number is 0, view all monitors
            if *number == 0 {
                let all_monitors_json = serde_json::to_string(&monitors)
                    .expect("Could not convert FileMonitors to JSON");
                Response::Links {
                    json: all_monitors_json,
                }
            }
            // Error if there are no monitors
            else if monitors.is_empty() {
                Response::ErrorMessage {
                    msg: String::from("No links are active"),
                }
            }
            // Error if an out-of-bounds monitor is requested
            else if *number > monitors.len() {
                Response::ErrorMessage {
                    msg: format!("Link {number} does not exist!"),
                }
            }
            // View a specific monitor
            else {
                let index = number - 1;
                let specific_monitor = monitors.index(index);
                let monitor_json = serde_json::to_string(&[specific_monitor])
                    .expect("Could not convert the link to JSON");
                Response::Links { json: monitor_json }
            }
        }
        Request::ViewWorkspaceName => Response::Message {
            msg: workspace_name.clone(),
        },
        Request::SetWorkspaceName { name } => {
            *workspace_name = name.clone();
            Response::NoData
        }
    };

    // Send the response back to the client
    let raw_response = serde_json::to_string(&response).expect("Could not serialize the response");
    stream
        .write_all(raw_response.as_bytes())
        .expect("Could not write reponse");

    // Return whether the request received was for server shutdown
    !matches!(&request, Request::Shutdown)
}

/// Run the server loop
pub fn run_server(port: u16) -> Result<String, String> {
    // Get the TCP listener
    let listener = bind_socket(port)?;

    // Get the duration to pause  in between checking for connections
    let sleep_duration = Duration::from_millis(10);

    // Create the initial list for FileMonitors (empty)
    let mut state = ServerState {
        monitors: Vec::new(),
        workspace_name: String::new(),
    };

    // Handle incoming connections
    for connection in listener.incoming() {
        match connection {
            // Incoming connection received
            Ok(stream) => {
                let keep_running = handle_connection(stream, &mut state);
                if !keep_running {
                    break;
                }
            }
            // No connection received before non-blocking timeout
            _ => {
                let mut has_broken_monitors = false;
                for monitor in &mut state.monitors {
                    if monitor.update_links().is_err() {
                        has_broken_monitors = true;
                        break;
                    }
                }
                if has_broken_monitors {
                    state
                        .monitors
                        .retain(|monitor| monitor.write_directory_exists());
                }
            }
        }
        sleep(sleep_duration); // TODO: Remove later?
    }
    Ok(String::from("Server process ended"))
}

mod test {

    #[test]
    #[serial_test::serial]
    fn bind_error() {
        // Save the current state of the application directory
        let preexisted = crate::test_support::save_app_directory();

        // Attempt to run the server on TCP port 1
        let response = crate::tcp::server::run_server(1);

        // Restore the previous application directory if it existed
        if preexisted {
            crate::test_support::restore_app_directory();
        }

        // Store the expected error message
        let expected = "Could not bind to port";

        // Check running the server on TCP port 1 returns the expected error message
        let err_msg = response.expect_err("Successfully started server");
        assert_eq!(&err_msg, expected);
    }
}
