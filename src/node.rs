extern crate jch;
extern crate rand;
extern crate serde;
extern crate twox_hash;

use std::error::Error as StdError;
use std::fmt;

use serde::Serialize;

use crate::message::*;

struct Peer {
    // Random number
    id: u64,

    // Bucket index for consistent hashing
    bucket: u32,

    // http://host:port
    base_uri: Box<str>,
}

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
    peers: Vec<Peer>,
}

impl Node {
    pub fn new() -> Node {
        Node {
            id: rand::random::<u64>(),
            peers: vec![],
        }
    }

    pub fn info(&self) -> Result<response::Info, Error> {
        Ok(response::Info {
            id: self.id.to_be_bytes(),
            peers: vec![],
        })
    }

    pub fn list_keys(&self) -> Result<response::ListKeys, Error> {
        Ok(response::ListKeys { keys: vec![] })
    }

    pub fn add_node(&mut self, _msg: &request::AddNode) -> Result<response::AddNode, Error> {
        Ok(response::AddNode {})
    }

    pub fn remove_node(
        &mut self,
        _msg: &request::RemoveNode,
    ) -> Result<response::RemoveNode, Error> {
        Ok(response::RemoveNode {})
    }
}
