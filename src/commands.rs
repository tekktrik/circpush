use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const STOP_RESPONSE: &str = "@stopping";

/// Various types of requests from the TCP client for the server
///
/// These can be serialized into JSON for communication.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    Ping,
    Echo {
        msg: String,
    },
    Shutdown,
    StartLink {
        read_pattern: String,
        write_directory: PathBuf,
        base_directory: PathBuf,
    },
    StopLink {
        number: usize,
    },
    ViewLink {
        number: usize,
    },
    ViewWorkspaceName,
    SetWorkspaceName {
        name: String,
    },
}

/// Various types of responses from the TCP server to the client
///
/// These can be serialized into JSON for communication.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    NoData,
    Number { number: usize },
    Message { msg: String },
    Links { json: String },
    ErrorMessage { msg: String },
}
