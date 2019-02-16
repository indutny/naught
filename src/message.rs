extern crate serde;
extern crate tokio;

pub mod rpc {
    use super::*;

    use std::time::Instant;

    use serde::Serialize;
    use tokio::sync::oneshot;

    #[derive(Debug)]
    pub enum ResponseMessage {
        Info(response::Info),
        Ping(response::Ping),
    }

    #[derive(Debug)]
    pub struct ResponsePacket {
        pub poll_at: Option<Instant>,
        pub message: ResponseMessage,
    }

    #[derive(Serialize, Debug)]
    pub struct ResponseError {
        pub error: crate::error::Error,
    }

    #[derive(Debug)]
    pub enum RequestMessage {
        Info,
        Ping(request::Ping),
    }

    #[derive(Debug)]
    pub struct RequestHTTPPacket {
        pub response_tx: oneshot::Sender<ResponsePacket>,
        pub message: RequestMessage,
    }

    #[derive(Debug)]
    pub enum RequestPacket {
        HTTP(RequestHTTPPacket),
        Poll,
    }
}

pub mod request {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Ping {
        pub sender: String,
        pub peers: Vec<String>,
    }
}

pub mod response {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Info {
        pub hash_seed: Vec<u64>,
        pub replicate: u32,

        pub uri: String,
        pub peers: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Ping {
        pub peers: Vec<String>,
    }
}
