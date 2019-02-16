extern crate serde;
extern crate siphasher;

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::hash::Hasher;
use std::net::SocketAddr;

use serde::Serialize;
use siphasher::sip::SipHasher;

use crate::config::Config;
use crate::message::{request, response};
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
    config: Config,
    uri: String,
    peers: HashMap<String, Peer>,
}

impl Node {
    pub fn new(addr: SocketAddr, config: Config) -> Node {
        let uri = format!("http://{}", addr);

        Node {
            config,
            uri,
            peers: HashMap::new(),
        }
    }

    // RPC below

    // NOTE: This is not a part of p2p protocol, mostly needed for debugging
    pub fn info(&self) -> Result<response::Info, Error> {
        Ok(response::Info {
            hash_seed: vec![self.config.hash_seed.0, self.config.hash_seed.1],
            replicate: self.config.replicate,

            uri: self.uri.clone(),
            peers: self.get_peer_uris(),
        })
    }

    pub fn ping(&mut self, msg: request::Ping) -> Result<response::Ping, Error> {
        self.on_ping(&msg.sender, msg.peers);

        Ok(response::Ping {
            peers: self.get_peer_uris(),
        })
    }

    // Internal methods

    fn get_peer_uris(&self) -> Vec<String> {
        self.peers.iter().map(|(uri, _)| uri.clone()).collect()
    }

    fn on_ping(&mut self, sender: &str, peers: Vec<String>) {
        self.add_peer(&sender);
        for peer_uri in peers {
            self.add_peer(&peer_uri);
        }

        self.peers
            .get_mut(sender)
            .expect("Sender to be present")
            .mark_alive();
    }

    fn add_peer(&mut self, uri: &str) {
        if uri == self.uri {
            return;
        }

        let uri_str = uri.to_string();
        if !self.peers.contains_key(&uri_str) {
            self.peers.insert(uri_str.clone(), Peer::new(uri_str));
        }
    }
}
