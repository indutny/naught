extern crate futures;
extern crate twox_hash;
extern crate jch;
extern crate rand;

use futures::{Future, future};
use crate::message::*;

struct Peer {
    // Random number
    id: u64,

    // Bucket index for consistent hashing
    bucket: u32,

    // http://host:port
    base_uri: Box<str>,
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

    pub fn info(&self) -> Result<response::Info, Box<str>> {
        Ok(response::Info {
            id: self.id.to_be_bytes(),
            peers: vec![],
        })
    }

    pub fn list_keys(&self) -> Result<response::ListKeys, Box<str>> {
        Ok(response::ListKeys {
            keys: vec![],
        })
    }

    pub fn add_node(&mut self, _msg: &request::AddNode) -> Result<response::AddNode, Box<str>> {
        Ok(response::AddNode {})
    }

    pub fn remove_node(&mut self, _msg: &request::RemoveNode) -> Result<response::RemoveNode, Box<str>> {
        Ok(response::RemoveNode {})
    }
}
