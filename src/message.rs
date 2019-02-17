extern crate serde;
extern crate tokio;

pub mod rpc {
    use super::*;

    use serde::Serialize;
    use tokio::sync::oneshot;

    #[derive(Debug)]
    pub enum ResponseMessage {
        Info(response::Info),
        RecvPing(response::Ping),
        GetPingURIs(response::GetPingURIs),
        RecvPingList,
    }

    #[derive(Debug)]
    pub struct ResponsePacket {
        pub message: ResponseMessage,
    }

    #[derive(Serialize, Debug)]
    pub struct ResponseError {
        pub error: crate::error::Error,
    }

    #[derive(Debug)]
    pub enum RequestMessage {
        Info,
        RecvPing(request::Ping),
        GetPingURIs,
        RecvPingList(Vec<request::Ping>),
    }

    #[derive(Debug)]
    pub struct RequestPacket {
        pub response_tx: oneshot::Sender<ResponsePacket>,
        pub message: RequestMessage,
    }
}

pub mod common {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Ping {
        pub sender: String,
        pub peers: Vec<String>,
    }
}

pub mod request {
    use super::*;

    pub use common::Ping;
}

pub mod response {
    use super::*;
    use serde::{Deserialize, Serialize};

    pub use common::Ping;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Info {
        pub hash_seed: Vec<u64>,
        pub replicate: u32,

        pub uri: String,
        pub peers: Vec<String>,
    }

    #[derive(Debug)]
    pub struct GetPingURIs {
        pub peers: Vec<String>,
        pub ping: common::Ping,
    }
}
