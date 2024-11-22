use serde::{Deserialize, Serialize};

pub const STOP_RESPONSE: &str = "@stopping";

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    Ping,
    Echo { msg: String },
    Shutdown,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    NoData,
    Message { msg: String },
}
