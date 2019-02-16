extern crate futures;
extern crate hyper;
extern crate serde;
extern crate serde_json;
extern crate siphasher;

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::hash::Hasher;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use futures::future;
use futures::prelude::*;
use serde::Serialize;
use siphasher::sip::SipHasher;

use crate::config::Config;
use crate::error::Error as RPCError;
use crate::message::{common, request, response};
use crate::peer::Peer;

type FutureBox = Box<Future<Item = (), Error = RPCError> + Send>;

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
    pub fn recv_info(&self) -> Result<response::Info, Error> {
        Ok(response::Info {
            hash_seed: vec![self.config.hash_seed.0, self.config.hash_seed.1],
            replicate: self.config.replicate,

            uri: self.uri.clone(),
            peers: self.get_peer_uris(),
        })
    }

    pub fn recv_ping(&mut self, msg: common::Ping) -> Result<common::Ping, Error> {
        self.on_ping(&msg.sender, msg.peers);

        Ok(self.construct_ping())
    }

    pub fn get_ping_uris(&mut self) -> Result<response::GetPingURIs, Error> {
        let now = Instant::now();

        // Remove stale peers
        let to_remove: Vec<String> = self
            .peers
            .values()
            .filter_map(|peer| {
                if peer.remove_at() <= now {
                    Some(peer.uri().to_string())
                } else {
                    None
                }
            })
            .collect();

        for key in to_remove {
            trace!("remove peer: {}", key);
            self.peers.remove(&key);
        }

        // Ping alive peers
        Ok(response::GetPingURIs {
            ping: self.construct_ping(),
            peers: self.get_peer_uris(),
        })
    }

    // Public API

    pub fn send_ping(ping: common::Ping, uris: Vec<String>) -> FutureBox {
        let acc: FutureBox = Box::new(future::ok(()));
        let join = uris
            .into_iter()
            .zip(std::iter::repeat(&ping))
            .map(|(uri, ping)| -> FutureBox { Node::send_single_ping(ping, uri) })
            .fold(acc, |acc: FutureBox, f: FutureBox| {
                Box::new(acc.join(f).map(|_| ()))
            });
        Box::new(join)
    }

    // Internal methods

    fn construct_ping(&self) -> common::Ping {
        common::Ping {
            sender: self.uri.clone(),
            peers: self.get_peer_uris(),
        }
    }

    fn send_single_ping(ping: &common::Ping, uri: String) -> FutureBox {
        let uri = format!("{}/_ping", uri);

        let json = match serde_json::to_string(ping) {
            Ok(json) => json,
            Err(err) => {
                return Box::new(future::err(RPCError::from(err)));
            }
        };

        let request = hyper::Request::builder()
            .method("PUT")
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(hyper::Body::from(json));

        let request = match request {
            Ok(request) => request,
            Err(err) => {
                error!(
                    "ping to {} failed due to error: {:#?}",
                    uri,
                    RPCError::from(err)
                );
                return Box::new(future::ok(()));
            }
        };

        let client = hyper::Client::new();

        let f = client
            .request(request)
            .and_then(|res| res.into_body().concat2())
            .from_err::<RPCError>()
            .and_then(|chunk| {
                serde_json::from_slice::<common::Ping>(&chunk).map_err(RPCError::from)
            })
            .and_then(|ping| {
                // TODO(indutny): return ping
                future::ok(())
            })
            .from_err::<RPCError>()
            .or_else(move |err| {
                // Single failed ping should not prevent other pings
                // from happening
                error!("ping to {} failed due to error: {:#?}", uri, err);
                future::ok(())
            });

        Box::new(f)
    }

    fn on_ping(&mut self, sender: &str, peers: Vec<String>) {
        if sender == self.uri {
            return;
        }

        self.add_peer(&sender);
        for peer_uri in peers {
            self.add_peer(&peer_uri);
        }

        let sender_peer = self.peers.get_mut(sender).expect("Sender to be present");

        sender_peer.mark_alive();
    }

    fn get_peer_uris(&self) -> Vec<String> {
        self.peers.keys().cloned().collect()
    }

    fn add_peer(&mut self, uri: &str) {
        if uri == self.uri {
            return;
        }

        let uri_str = uri.to_string();
        if !self.peers.contains_key(&uri_str) {
            trace!("new peer: {}", uri);
            self.peers
                .insert(uri_str.clone(), Peer::new(uri_str, self.config.clone()));
        }
    }
}
