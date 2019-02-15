extern crate serde;

use serde::{Deserialize, Serialize};

use crate::peer::Peer;

pub type Id = [u8; 8];

#[derive(Serialize, Deserialize, Debug)]
pub struct PeerSummary {
    pub id: Id,
    pub base_uri: String,
}

impl From<&Peer> for PeerSummary {
    fn from(peer: &Peer) -> Self {
        PeerSummary {
            id: peer.id().to_be_bytes(),
            base_uri: peer.base_uri().to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct State {
    pub id: Id,
    pub peers: Vec<PeerSummary>,
}

pub mod request {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct AddNode {
        pub state: State,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct RemoveNode {
        // Id of sender
        pub id: Id,
    }
}

pub mod response {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Info {
        pub state: State,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct AddNode {
        pub state: State,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct RemoveNode {}

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ListKeys {
        pub keys: Vec<String>,
    }
}
