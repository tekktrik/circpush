use crate::commands::{Request, Response, STOP_RESPONSE};
use crate::link::FileLink;
use crate::monitor::FileMonitor;
use serde::Deserialize;
use serde_json::to_string;
use std::io::prelude::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::str::FromStr;
use tabled::{Tabled, Table};
use std::time::Duration;
use tabled::builder::Builder;
use crate::tcp::server::PORT;

fn open_connection() -> Result<TcpStream, String> {
    let localhost_addr_v4 = Ipv4Addr::LOCALHOST;
    let localhost_addr = IpAddr::V4(localhost_addr_v4);
    // let PORT = 61533; // TODO: Use input or settings port later
    let socket_addr = SocketAddr::new(localhost_addr, PORT);
    let stream = match TcpStream::connect(socket_addr) {
        Ok(stream) => stream,
        Err(_) => return Err(format!("Could not connect to the server on port {PORT}, is the server running?"))
    };
    let duration = Duration::from_secs(1);
    stream
        .set_read_timeout(Some(duration))
        .expect("Bad duration passed as socket read timeout.");
    Ok(stream)
}

fn communicate(request: Request) -> Result<Response, String> {
    let mut stream = open_connection()?;
    let raw_request = serde_json::to_string(&request).expect("Could not serialize requiest");
    stream
        .write_all(raw_request.as_bytes())
        .expect("Could not write request");
    let mut serialization = serde_json::Deserializer::from_reader(&stream);
    Ok(Response::deserialize(&mut serialization).expect("Could not deserialize the response"))
}

pub fn ping() -> Result<String, String> {
    match communicate(Request::Ping) {
        Ok(Response::NoData) => Ok(String::from("Ping receieved!")),
        Err(error) => Err(error),
        _ => Err(String::from("ERROR: Did not receive expected ping response")),
    }
}

pub fn echo(message: String) -> Result<String, String> {
    match communicate(Request::Echo { msg: message }) {
        Ok(Response::Message { msg }) => Ok(msg),
        Err(error) => Err(error),
        _ => Err(String::from("ERROR: Did not receive expected echo response")),
    }
}

pub fn stop_server() -> Result<String, String> {
    match communicate(Request::Shutdown) {
        Ok(Response::Message { msg }) if msg == STOP_RESPONSE => Ok(format!("Server on port {PORT} shutdown")),
        Err(error) => Err(error),
        _ => Err(String::from("ERROR: Did not receive expected response")),
    }
}

pub fn start_link(read_pattern: String, write_directory: PathBuf, base_directory: PathBuf) -> Result<String, String> {
    match communicate(Request::StartLink { read_pattern, write_directory, base_directory }) {
        Ok(Response::Message { msg }) => Ok(msg),
        Err(error) => Err(error),
        _ => Err(String::from("ERROR: Could not start link"))
    }
}

pub fn stop_link(number: usize) -> Result<String, String> {
    match communicate(Request::StopLink { number }) {
        Ok(Response::Message { msg }) => Ok(msg),
        Err(error) => Err(error),
        _ => Err(String::from("ERROR: Could not stop link")),
    }
}

pub fn view_link(number: usize, absolute: bool) -> Result<String, String> {
    let response = match communicate(Request::ViewLink { number }) {
        Ok(Response::Links { json }) => json,
        Err(error) => return Err(error),
        _ => return Err(String::from("ERROR: Could not retrieve link")),
    };

    let monitor_list: Vec<FileMonitor> = serde_json::from_str(&response).expect("Failed to parse JSON response");

    let mut table_builder = Builder::default();
    table_builder.push_record(FileMonitor::table_header());

    for (index, monitor) in monitor_list.iter().enumerate()  {
        let mut record = monitor.to_table_record(absolute);
        let monitor_num = index + 1;
        record.insert(0, monitor_num.to_string());
        table_builder.push_record(record);
    }

    Ok(table_builder.build().to_string())
}
