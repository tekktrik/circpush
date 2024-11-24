use crate::commands::{Request, Response, STOP_RESPONSE};
use serde::Deserialize;
use std::io::prelude::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

#[allow(dead_code)]
fn open_connection() -> TcpStream {
    let localhost_addr_v4 = Ipv4Addr::LOCALHOST;
    let localhost_addr = IpAddr::V4(localhost_addr_v4);
    let port = 61533; // TODO: Use input or settings port later
    let socket_addr = SocketAddr::new(localhost_addr, port);
    let stream = TcpStream::connect(socket_addr).expect("Could not connect to the server");
    let duration = Duration::from_secs(1);
    stream
        .set_read_timeout(Some(duration))
        .expect("Bad duration passed as socket read timeout.");
    stream
}

fn communicate(request: Request) -> Response {
    let mut stream = open_connection();
    let raw_request = serde_json::to_string(&request).expect("Could not serialize requiest");
    stream
        .write_all(raw_request.as_bytes())
        .expect("Could not write request");
    let mut serialization = serde_json::Deserializer::from_reader(&stream);
    Response::deserialize(&mut serialization).expect("Could not deserialize the response")
    // match Response::deserialize(&mut serialization) {
    //     Ok(response) => Ok(response),
    //     Err(e) => Some(3)
    // }
}

pub fn ping() -> Result<&'static str, &'static str> {
    match communicate(Request::Ping) {
        Response::NoData => Ok("Ping receieved!"),
        _ => Err("ERROR: Did not receive expected ping response"),
    }
}

pub fn echo(message: String) -> Result<String, &'static str> {
    match communicate(Request::Echo { msg: message }) {
        Response::Message { msg } => Ok(msg),
        _ => Err("ERROR: Did not receive expected echoresponse"),
    }
}

pub fn stop_server() -> Result<&'static str, &'static str> {
    match communicate(Request::Shutdown) {
        Response::Message { msg } if msg == STOP_RESPONSE => Ok("Server shutdown"),
        _ => Err("ERROR: Did not receive expected echoresponse"),
    }
}
