extern crate jch;
extern crate rand;
extern crate serde;
extern crate twox_hash;

use std::error::Error as StdError;
use std::fmt;
use std::net::SocketAddr;

use serde::Serialize;

use crate::message::*;

#[derive(Serialize, Debug)]
pub enum Error {
    Every,
}

impl StdError for Error {
    fn description(&self) -> &str {
        "TODO(indutny): implement me"
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Naught Node Error: {}", self.description())
    }
}

pub struct Node {
    id: u64,
    peers: Vec<PeerInfo>,
}

impl Node {
    pub fn new(addr: SocketAddr) -> Node {
        let base_uri = format!("http://{}", addr);
        let id = rand::random::<u64>();

        Node {
            id,
            peers: vec![PeerInfo {
                id: id.to_be_bytes(),
                base_uri,
            }],
        }
    }

    pub fn info(&self) -> Result<response::Info, Error> {
        Ok(response::Info {
            id: self.id.to_be_bytes(),
            peers: self.peers.clone(),
        })
    }

    pub fn list_keys(&self) -> Result<response::ListKeys, Error> {
        Ok(response::ListKeys { keys: vec![] })
    }

    pub fn add_node(&mut self, _msg: &request::AddNode) -> Result<response::AddNode, Error> {
        Ok(response::AddNode {
            id: self.id.to_be_bytes(),
            peers: self.peers.clone(),
        })
    }

    pub fn remove_node(
        &mut self,
        _msg: &request::RemoveNode,
    ) -> Result<response::RemoveNode, Error> {
        Ok(response::RemoveNode {})
    }
}
