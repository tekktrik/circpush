use crate::commands::{Request, Response, STOP_RESPONSE};
use crate::monitor::FileMonitor;
use serde::Deserialize;
use std::io::{prelude::*, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;
use std::ops::Index;

pub const PORT: u16 = 61553;

#[cfg(target_family = "unix")]
pub fn start_server() -> String {
    Command::new("circpush")
        .arg("server")
        .arg("run")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Could not spawn server process");
    format!("Server started on port {PORT}")
}

#[cfg(target_family = "windows")]
pub fn start_server(verbose: bool) {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
    Command::new("circpush")
        .arg("server")
        .arg("run")
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .expect("Could not spawn server process");
}

fn bind_socket() -> TcpListener {
    let localhost_addr_v4 = Ipv4Addr::LOCALHOST;
    let localhost_addr = IpAddr::V4(localhost_addr_v4);
    // let PORT = 61533; // TODO: Use input or settings port later
    let socket_addr = SocketAddr::new(localhost_addr, PORT);
    let listener = TcpListener::bind(socket_addr).expect("Could not bind server socket");
    listener
        .set_nonblocking(true)
        .expect("Could not set the socket to non-blocking");
    listener
}

fn handle_connection(mut stream: TcpStream, monitors: &mut Vec<FileMonitor>) -> bool {
    let mut serialization = serde_json::Deserializer::from_reader(&stream);
    let request =
        Request::deserialize(&mut serialization).expect("Unable to deserialize the request");
    let response = match &request {
        Request::Ping => Response::NoData,
        Request::Echo { msg } => Response::Message { msg: msg.clone() },
        Request::Shutdown => Response::Message {
            msg: String::from_str(STOP_RESPONSE).unwrap(),
        },
        Request::StartLink {read_pattern, write_directory, base_directory } => {
            let new_monitor = FileMonitor::new(
                read_pattern.clone(),
                write_directory.clone(),
                base_directory.clone(),
            ).expect("Path error occurred!");
            monitors.push(new_monitor);
            let new_link_number = monitors.len();
            Response::Message { msg: format!("Link {new_link_number} started!") }
        }
        Request::StopLink { number} => {
            if *number == 0 {
                monitors.clear();
                Response::Message { msg: String::from("All links cleared!") }
            }
            else if monitors.len() == 0 {
                Response::ErrorMessage { msg: String::from("No links are active") }
            }
            else if *number > monitors.len() {
                Response::ErrorMessage { msg: String::from(format!("Link {number} does not exist!")) }
            }
            else {
                let index = number - 1;
                monitors.remove(index);
                Response::Message { msg: String::from("Link removed!") }
            }
        },
        Request::ViewLink { number} => {
            if *number == 0 {
                let all_monitors_json = serde_json::to_string(&monitors).expect("Could not convert FileMonitors to JSON");
                Response::Links { json: all_monitors_json }
            }
            else if monitors.len() == 0 {
                Response::ErrorMessage { msg: String::from("No links are active") }
            }
            else if *number > monitors.len() {
                Response::ErrorMessage { msg: format!("Link {number} does not exist!") }
            }
            else {
                let index = number - 1;
                let specific_monitor = monitors.index(index);
                let monitor_json = serde_json::to_string(specific_monitor).expect("Could not convert the link to JSON");
                Response::Links { json: monitor_json }
            }
        },
    };
    let raw_response = serde_json::to_string(&response).expect("Could not serialize the response");
    stream
        .write_all(raw_response.as_bytes())
        .expect("Could not write reponse");
    !matches!(&request, Request::Shutdown)
}

pub fn run_server() -> String{
    let listener = bind_socket();
    let sleep_duration = Duration::from_millis(10);
    let mut monitors: Vec<FileMonitor> = Vec::new();
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let keep_running = handle_connection(stream, &mut monitors);
                if !keep_running {
                    break
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                for monitor in &mut monitors {
                    monitor.update_links().expect("Could not update links");
                }
            }
            Err(_e) => panic!("Could not accept incoming connection"),
        }
        sleep(sleep_duration); // TODO: Remove later?
    }
    String::from("Server process ended")
}
