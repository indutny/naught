extern crate futures;
extern crate hyper;
extern crate serde;
extern crate serde_json;
extern crate siphasher;

use std::collections::HashMap;
use std::hash::Hasher;
use std::net::SocketAddr;
use std::time::Instant;

use futures::future;
use futures::prelude::*;
use serde::Serialize;
use siphasher::sip::SipHasher;

use crate::config::Config;
use crate::error::Error;
use crate::message::{common, response};
use crate::peer::Peer;

type MaybePing = Option<common::Ping>;
type FuturePings = Box<Future<Item = Vec<MaybePing>, Error = Error> + Send>;
type FuturePing = Box<Future<Item = MaybePing, Error = Error> + Send>;

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

    pub fn send_pings(&mut self) -> FuturePings {
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
        let uris: Vec<String> = self
            .peers
            .values_mut()
            .filter(|peer| peer.ping_at() <= now)
            .map(|peer| peer.uri().to_string())
            .collect();

        let ping = match serde_json::to_string(&self.construct_ping()) {
            Ok(json) => json,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        let pings = uris.into_iter().map(move |uri| {
            Node::send_single_ping(ping.to_string(), &uri).or_else(move |err| {
                // Single failed ping should not prevent other pings
                // from happening
                error!("ping to {} failed due to error: {:#?}", uri, err);
                future::ok(None)
            })
        });
        Box::new(future::join_all(pings))
    }

    // Internal methods

    fn construct_ping(&self) -> common::Ping {
        common::Ping {
            sender: self.uri.clone(),
            peers: self.get_peer_uris(),
        }
    }

    fn send_single_ping(ping: String, uri: &str) -> FuturePing {
        let uri = format!("{}/_ping", uri);

        let request = hyper::Request::builder()
            .method("PUT")
            .uri(uri)
            .header("content-type", "application/json")
            .body(hyper::Body::from(ping));

        let request = match request {
            Ok(request) => request,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        let client = hyper::Client::new();

        let f = client
            .request(request)
            .and_then(|res| res.into_body().concat2())
            .from_err::<Error>()
            .and_then(|chunk| serde_json::from_slice::<common::Ping>(&chunk).map_err(Error::from))
            .and_then(|ping| future::ok(Some(ping)))
            .from_err::<Error>();

        Box::new(f)
    }

    fn on_ping(&mut self, sender: &str, peers: Vec<String>) {
        if sender == self.uri {
            return;
        }

        trace!("received ping from: {}", sender);

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
