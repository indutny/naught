extern crate serde;

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
