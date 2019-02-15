extern crate jch;
extern crate rand;
extern crate serde;
extern crate twox_hash;

use std::error::Error as StdError;
use std::fmt;
use std::net::SocketAddr;

use serde::Serialize;

use crate::message::*;
use crate::peer::Peer;

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
    peer_ids: Vec<u64>,
}

impl Node {
    pub fn new(addr: SocketAddr) -> Node {
        let base_uri = format!("http://{}", addr);
        let id = rand::random::<u64>();

        Node {
            id,
            peers: vec![Peer::new(id, base_uri)],
            peer_ids: vec![id],
        }
    }

    // RPC below

    pub fn info(&self) -> Result<response::Info, Error> {
        Ok(response::Info {
            state: self.state(),
        })
    }

    pub fn list_keys(&self) -> Result<response::ListKeys, Error> {
        Ok(response::ListKeys { keys: vec![] })
    }

    pub fn add_node(&mut self, msg: request::AddNode) -> Result<response::AddNode, Error> {
        // Add new peers
        for peer in msg.state.peers.into_iter() {
            self.add_peer(peer);
        }

        Ok(response::AddNode {
            state: self.state(),
        })
    }

    pub fn remove_node(
        &mut self,
        _msg: request::RemoveNode,
    ) -> Result<response::RemoveNode, Error> {
        Ok(response::RemoveNode {})
    }

    // Internal methods

    fn state(&self) -> State {
        State {
            id: self.id.to_be_bytes(),
            peers: self.peers.iter().map(PeerSummary::from).collect(),
        }
    }

    fn add_peer(&mut self, summary: PeerSummary) {
        let id = u64::from_be_bytes(summary.id);

        if let Err(index) = self.peer_ids.binary_search(&id) {
            self.peer_ids.insert(index, id);
            self.peers.insert(index, Peer::new(id, summary.base_uri));
        }
    }
}
