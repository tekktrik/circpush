use crate::commands::{Request, Response, STOP_RESPONSE};
use serde::Deserialize;
use std::io::{prelude::*, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;

#[cfg(target_family = "unix")]
pub fn start_server() {
    Command::new("circpush")
        .arg("server")
        .arg("run")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Could not spawn server process");
}

#[cfg(target_family = "windows")]
pub fn start_server() {
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
    let port = 61533; // TODO: Use input or settings port later
    let socket_addr = SocketAddr::new(localhost_addr, port);
    let listener = TcpListener::bind(socket_addr).expect("Could not bind server socket");
    listener
        .set_nonblocking(true)
        .expect("Could not set the socket to non-blocking");
    listener
}

fn handle_connection(mut stream: TcpStream) -> bool {
    let mut serialization = serde_json::Deserializer::from_reader(&stream);
    let request =
        Request::deserialize(&mut serialization).expect("Unable to deserialize the request");
    let response = match &request {
        Request::Ping => Response::NoData,
        Request::Echo { msg } => Response::Message { msg: msg.clone() },
        Request::Shutdown => Response::Message {
            msg: String::from_str(STOP_RESPONSE).unwrap(),
        },
    };
    let raw_response = serde_json::to_string(&response).expect("Could not serialize the response");
    stream
        .write_all(raw_response.as_bytes())
        .expect("Could not write reponse");
    !matches!(&request, Request::Shutdown)
}

pub fn run_server() {
    let listener = bind_socket();
    let sleep_duration = Duration::from_millis(10);
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let keep_running = handle_connection(stream);
                if !keep_running {
                    return;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                // TODO: Move on to file updates
            }
            Err(_e) => panic!("Could not accept incoming connection"),
        }
        sleep(sleep_duration); // TODO: Remove later?
    }
}
