use serde::{Deserialize, Serialize};

pub const STOP_RESPONSE: &str = "@stopping";
// pub const COMMAND_ACK: &str = "@ack";
// pub const COMMAND_NAK: &str = "@nak";

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    Ping,
    Echo { msg: String },
    Shutdown,
    StartLink { read_pattern: String, write_directory: String, base_directory: String },
    StopLink { number: usize},
    ViewLink { number: usize},
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    NoData,
    Message { msg: String },
    Links { json: String },
    ErrorMessage { msg: String },
}
