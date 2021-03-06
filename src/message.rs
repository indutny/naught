extern crate hyper;
extern crate serde;

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

    #[derive(Serialize, Debug)]
    pub struct Error {
        pub error: crate::error::Error,
    }

    #[derive(Serialize, Debug)]
    pub struct Store {
        pub container: String,
        pub uris: Vec<String>,
    }

    pub struct Fetch {
        pub peer: String,
        pub mime: String,
        pub body: hyper::Body,
    }
}
