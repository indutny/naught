extern crate serde;

use serde::{Serialize, Deserialize};

pub type Id = [u8; 8];

#[derive(Serialize, Deserialize, Debug)]
pub struct PeerInfo {
    pub host: Box<str>,
    pub port: u16,
}

pub mod request {
    use serde::{Serialize, Deserialize};
    use super::*;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct AddNode {
        // Id of sender
        pub id: Id,

        // Sender's known peers
        pub peers: Vec<PeerInfo>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct RemoveNode {
        // Id of sender
        pub id: Id,
    }
}

pub mod response {
    use serde::{Serialize, Deserialize};
    use super::*;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Info {
        // Id of responder
        pub id: Id,

        // Responder's known peers
        pub peers: Vec<PeerInfo>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct AddNode {}

    #[derive(Serialize, Deserialize, Debug)]
    pub struct RemoveNode {}

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ListKeys {
        pub keys: Vec<Box<str>>,
    }
}
