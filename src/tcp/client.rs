use crate::commands::{Request, Response, STOP_RESPONSE};
use crate::monitor::FileMonitor;
use crate::tcp::server::PORT;
use serde::Deserialize;
use std::io::prelude::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;
use tabled::builder::Builder;

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
        Ok(Response::NoData) => Ok(String::from("Ping receieved!")),
        Err(error) => Err(error),
        _ => Err(String::from(
            "ERROR: Did not receive expected ping response",
        )),
    }
}

/// Send an echo request to the server
pub fn echo(message: String) -> Result<String, String> {
    match communicate(Request::Echo { msg: message }) {
        Ok(Response::Message { msg }) => Ok(msg),
        Err(error) => Err(error),
        _ => Err(String::from(
            "ERROR: Did not receive expected echo response",
        )),
    }
}

/// Send a stop server request to the server
pub fn stop_server() -> Result<String, String> {
    match communicate(Request::Shutdown) {
        Ok(Response::Message { msg }) if msg == STOP_RESPONSE => {
            Ok(format!("Server on port {PORT} shutdown"))
        }
        Err(error) => Err(error),
        _ => Err(String::from("ERROR: Did not receive expected response")),
    }
}

/// Send a start file monitor request to the server
pub fn start_monitor(
    read_pattern: String,
    write_directory: PathBuf,
    base_directory: PathBuf,
) -> Result<String, String> {
    match communicate(Request::StartLink {
        read_pattern,
        write_directory,
        base_directory,
    }) {
        Ok(Response::Message { msg }) => Ok(msg),
        Err(error) => Err(error),
        _ => Err(String::from("ERROR: Could not start link")),
    }
}

/// Send a stop file monitor request to the server
pub fn stop_monitor(number: usize) -> Result<String, String> {
    match communicate(Request::StopLink { number }) {
        Ok(Response::Message { msg }) => Ok(msg),
        Err(error) => Err(error),
        _ => Err(String::from("ERROR: Could not stop link")),
    }
}

/// Send a view file monitor request to the server
pub fn view_monitor(number: usize, absolute: bool) -> Result<String, String> {
    // Get the response of the server communication
    let response = match communicate(Request::ViewLink { number }) {
        Ok(Response::Links { json }) => json,
        Err(error) => return Err(error),
        _ => return Err(String::from("ERROR: Could not retrieve link")),
    };

    // Parse the response string into a list of FileMonitors
    let monitor_list: Vec<FileMonitor> =
        serde_json::from_str(&response).expect("Failed to parse JSON response");

    // Create a tabled table to be built and add the header row
    let mut table_builder = Builder::default();
    table_builder.push_record(FileMonitor::table_header());

    // For each FileMonitor returned, get the associated table record and add it along with the associated monitor number
    for (index, monitor) in monitor_list.iter().enumerate() {
        let mut record = monitor.to_table_record(absolute);
        let record_number = if number == 0 { index + 1 } else { number };
        record.insert(0, record_number.to_string());
        table_builder.push_record(record);
    }

    Ok(table_builder.build().to_string())
}
