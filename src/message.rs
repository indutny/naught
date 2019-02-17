extern crate serde;
extern crate tokio;

pub mod common {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Debug)]
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

    #[derive(Serialize, Debug)]
    pub struct Error {
        pub error: crate::error::Error,
    }
}
