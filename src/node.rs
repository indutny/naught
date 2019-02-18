extern crate futures;
extern crate hyper;
extern crate serde_json;
extern crate siphasher;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::Hasher;
use std::net::SocketAddr;
use std::time::Instant;

use futures::future;
use futures::prelude::*;
use siphasher::sip::SipHasher;

use crate::config::Config;
use crate::error::Error;
use crate::message::{common, response};
use crate::peer::Peer;

type MaybePing = Option<common::Ping>;
type FuturePingVec = Box<Future<Item = Vec<MaybePing>, Error = Error> + Send>;
type FuturePing = Box<Future<Item = MaybePing, Error = Error> + Send>;

#[derive(Eq, Debug)]
struct Resource {
    uri: String,
    hash: u64,
    local: bool,
}

impl Ord for Resource {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hash.cmp(&other.hash)
    }
}

impl PartialOrd for Resource {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Resource {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Resource {
    fn new(uri: String, local: bool, hash_seed: (u64, u64)) -> Resource {
        let mut hasher = SipHasher::new_with_keys(hash_seed.0, hash_seed.1);
        hasher.write(uri.as_bytes());
        Resource {
            uri,
            local,
            hash: hasher.finish(),
        }
    }
}

pub struct Node {
    config: Config,
    uri: String,
    peers: HashMap<String, Peer>,
    data: HashMap<String, Vec<u8>>,
}

impl Node {
    pub fn new(addr: SocketAddr, config: Config) -> Node {
        let uri = format!("http://{}", addr);

        Node {
            config,
            uri,
            peers: HashMap::new(),
            data: HashMap::new(),
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

    pub fn recv_ping(&mut self, msg: &common::Ping) -> Result<common::Ping, Error> {
        self.on_ping(&msg.sender, &msg.peers);

        Ok(self.construct_ping())
    }

    pub fn peek(&self, resource: &str) -> Result<(), Error> {
        if self.data.contains_key(resource) {
            trace!("peek existing resource: {}", resource);
            Ok(())
        } else {
            trace!("peek missing resource: {}", resource);
            Err(Error::NotFound)
        }
    }

    pub fn fetch(&self, resource: &str, redirect: bool) -> Result<hyper::Body, Error> {
        if let Some(value) = self.data.get(resource) {
            trace!(
                "fetch existing resource: {} redirect: {}",
                resource,
                redirect
            );
            return Ok(hyper::Body::from(value.clone()));
        }

        let resources = if redirect {
            self.find_resource(resource)
        } else {
            vec![]
        };

        // No resources to redirect to
        if resources.is_empty() {
            trace!(
                "fetch missing resource: {} redirect: {}",
                resource,
                redirect
            );
            return Err(Error::NotFound);
        }

        Err(Error::NotFound)
    }

    pub fn store(&mut self, resource: String, value: Vec<u8>) -> Result<response::Empty, Error> {
        if self.data.contains_key(&resource) {
            trace!("duplicate resource: {}", resource);
            return Ok(response::Empty {});
        }

        trace!("new resource: {}", resource);
        self.data.insert(resource, value);
        Ok(response::Empty {})
    }

    pub fn send_pings(&mut self) -> FuturePingVec {
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

    fn on_ping(&mut self, sender: &str, peers: &[String]) {
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

    fn find_resource(&self, resource: &str) -> Vec<Resource> {
        // TODO(indutny): LRU
        let mut resources: Vec<Resource> = self
            .peers
            .values()
            .map(|peer| format!("{}/{}", peer.uri(), resource))
            .map(|uri| Resource::new(uri, false, self.config.hash_seed))
            .collect();
        resources.push(Resource::new(
            format!("{}/{}", self.uri, resource),
            true,
            self.config.hash_seed,
        ));
        resources.sort_unstable();

        resources.truncate(self.config.replicate as usize + 1);
        resources
    }
}
