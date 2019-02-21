extern crate futures;
extern crate hyper;
extern crate serde_json;
extern crate tokio;

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::time::Instant;

use futures::future;
use futures::prelude::*;

use crate::config::Config;
use crate::error::Error;
use crate::message::{common, response};
use crate::peer::Peer;
use crate::resource::Resource;

type MaybePing = Option<common::Ping>;
type FuturePingVec = Box<Future<Item = Vec<MaybePing>, Error = Error> + Send>;
type FuturePing = Box<Future<Item = MaybePing, Error = Error> + Send>;
type FutureBody = Box<Future<Item = hyper::Body, Error = Error> + Send>;
type FutureURI = Box<Future<Item = Option<String>, Error = Error> + Send>;
type FutureKeyVec = Box<Future<Item = Vec<String>, Error = Error> + Send>;
type FutureMaybeKey = Box<Future<Item = Option<String>, Error = Error> + Send>;
type FutureBool = Box<Future<Item = bool, Error = Error> + Send>;

struct DataEntry {
    value: Vec<u8>,
}

pub struct Node {
    config: Config,
    uri: String,
    peers: HashMap<String, Peer>,
    data: HashMap<String, DataEntry>,

    // Last peers before rebalance
    last_peers: HashMap<String, Peer>,
    // Their uris
    last_peer_uris: HashSet<String>,

    // Shared client with connection pool
    client: hyper::Client<hyper::client::HttpConnector>,
}

impl Node {
    pub fn new(bind_addr: SocketAddr, config: Config) -> Node {
        let uri = format!("http://{}", bind_addr);

        Node {
            config,
            uri,
            peers: HashMap::new(),
            data: HashMap::new(),

            last_peers: HashMap::new(),
            last_peer_uris: HashSet::new(),

            client: hyper::Client::new(),
        }
    }

    pub fn set_local_addr(&mut self, local_addr: SocketAddr) {
        self.uri = format!("http://{}", local_addr);
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

    pub fn peek(&self, uri: &str) -> Result<(), Error> {
        if self.data.contains_key(uri) {
            trace!("peek existing resource: {}", uri);
            Ok(())
        } else {
            trace!("peek missing resource: {}", uri);
            Err(Error::NotFound)
        }
    }

    pub fn fetch(&self, uri: &str, redirect: bool) -> FutureBody {
        if let Some(entry) = self.data.get(uri) {
            trace!("fetch existing resource: {} redirect: {}", uri, redirect);
            return Box::new(future::ok(hyper::Body::from(entry.value.clone())));
        }

        let resources = if redirect {
            self.find_resources(uri)
        } else {
            vec![]
        };

        // No resources to redirect to
        if resources.is_empty() {
            trace!("fetch missing resource: {} redirect: {}", uri, redirect);
            return Box::new(future::err(Error::NotFound));
        }

        // TODO(indutny): store on miss, if has to be present locally
        let reqs: Vec<FutureBody> = resources
            .into_iter()
            .map(|resource| resource.fetch(&self.client, &self.uri))
            .collect();

        Box::new(future::select_ok(reqs).map(|(body, _)| body))
    }

    pub fn store(
        &mut self,
        uri: String,
        value: Vec<u8>,
        redirect: bool,
    ) -> Box<Future<Item = response::Store, Error = Error> + Send> {
        if self.data.contains_key(&uri) {
            trace!("duplicate resource: {}", uri);
            return Box::new(future::ok(response::Store { uris: vec![] }));
        }

        // Store only locally when redirect is `false`
        let resources: Vec<Resource> = self
            .find_resources(&uri)
            .into_iter()
            .filter(|resource| redirect || resource.is_local())
            .collect();

        // Nowhere to store, notify caller
        if resources.is_empty() {
            trace!("no resources for object: {}", uri);
            return Box::new(future::err(Error::NonLocalStore(uri.to_string())));
        }

        trace!("new object: {}", uri);

        let remote: Vec<FutureURI> = resources
            .into_iter()
            .map(|resource| -> FutureURI {
                // TODO(indutny): excessive cloning?
                let target_uri = resource.uri().to_string();

                let store = resource
                    .store(&self.client, &self.uri, &value)
                    .map(move |_| Some(target_uri))
                    .or_else(|err| {
                        // Single failed store should not fail others
                        trace!("remote store failed due to error: {:?}", err);
                        future::ok(None)
                    });
                Box::new(store)
            })
            .collect();

        let uris = future::join_all(remote).and_then(|target_uris| {
            trace!("stored resource remotely at: {:?}", target_uris);
            future::ok(response::Store {
                uris: target_uris.into_iter().filter_map(|uri| uri).collect(),
            })
        });

        self.data.insert(uri, DataEntry { value });

        Box::new(uris)
    }

    pub fn send_pings(&mut self) -> FuturePingVec {
        let now = Instant::now();

        // Remove stale peers
        let to_remove: Vec<String> = self
            .peers
            .values()
            .filter_map(|peer| {
                if peer.should_remove(now) {
                    Some(peer.uri().to_string())
                } else {
                    None
                }
            })
            .collect();

        for key in to_remove.iter() {
            trace!("remove peer: {}", key);
            self.peers.remove(key);
        }

        // Ping alive peers
        let uris: Vec<String> = self
            .peers
            .values_mut()
            .filter(|peer| peer.should_ping(now))
            .map(|peer| peer.uri().to_string())
            .collect();

        let ping = match serde_json::to_string(&self.construct_ping()) {
            Ok(json) => json,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        let pings: Vec<FuturePing> = uris
            .into_iter()
            .map(|uri| -> FuturePing {
                Box::new(self.send_single_ping(&ping, &uri).or_else(move |err| {
                    // Single failed ping should not prevent other pings
                    // from happening
                    trace!("ping to {} failed due to error: {:?}", uri, err);
                    future::ok(None)
                }))
            })
            .collect();
        Box::new(future::join_all(pings))
    }

    pub fn rebalance(&mut self) -> FutureKeyVec {
        let now = Instant::now();

        let new_peers = HashSet::from_iter(self.peers.values().filter_map(|peer| {
            if peer.is_stable(now) && peer.is_active(now) {
                Some(peer.uri().to_string())
            } else {
                None
            }
        }));

        let added_peers = HashSet::from_iter(new_peers.difference(&self.last_peer_uris).cloned());
        let removed_peers = HashSet::from_iter(self.last_peer_uris.difference(&new_peers).cloned());

        if added_peers.is_empty() && removed_peers.is_empty() {
            // No rebalancing needed
            return Box::new(future::ok(vec![]));
        }

        trace!(
            "rebalance: start removed={:?} added={:?}",
            removed_peers,
            added_peers
        );
        let union: Vec<&String> = new_peers.union(&self.last_peer_uris).collect();

        let obsolete_keys: Vec<FutureMaybeKey> = self
            .data
            .iter()
            .map(|(key, entry)| -> FutureMaybeKey {
                let resources =
                    self.find_rebalance_resources(key, &union, &removed_peers, &added_peers);

                let keep_local = resources.iter().any(|resource| resource.is_local());

                let successes: Vec<FutureBool> = resources
                    .into_iter()
                    .map(|resource| -> FutureBool {
                        Box::new(
                            resource
                                .store(&self.client, &self.uri, &entry.value)
                                .map(|_| true)
                                .or_else(|err| {
                                    // Single failed rebalance should not fail others
                                    trace!("remote rebalance failed due to error: {:?}", err);
                                    future::ok(false)
                                }),
                        )
                    })
                    .collect();

                let key = key.clone();

                Box::new(future::join_all(successes).and_then(move |successes| {
                    if !keep_local && successes.into_iter().any(|v| v) {
                        future::ok(Some(key))
                    } else {
                        future::ok(None)
                    }
                }))
            })
            .collect();

        self.last_peer_uris = new_peers;
        self.last_peers = self.peers.clone();

        Box::new(
            future::join_all(obsolete_keys)
                .map(|keys| keys.into_iter().filter_map(|key| key).collect()),
        )
    }

    pub fn remove(&mut self, keys: Vec<String>) {
        for key in keys {
            trace!("remove key: {}", key);
            self.data.remove(&key);
        }
    }

    // Internal methods

    fn construct_ping(&self) -> common::Ping {
        common::Ping {
            sender: self.uri.clone(),
            peers: self.get_peer_uris(),
        }
    }

    fn construct_resource(&self, uri: &str) -> Resource {
        Resource::new(&self.uri, uri, true, self.config.hash_seed)
    }

    fn send_single_ping(&self, ping: &str, uri: &str) -> FuturePing {
        let uri = format!("{}/_ping", uri);

        let request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(uri)
            .header(hyper::header::CONTENT_TYPE, "application/json")
            .body(hyper::Body::from(ping.to_string()));

        let request = match request {
            Ok(request) => request,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        // TODO(indutny): timeout
        let f = self
            .client
            .request(request)
            .from_err::<Error>()
            .and_then(|response| {
                let is_success = if response.status().is_success() {
                    future::ok(())
                } else {
                    future::err(Error::PingFailed)
                };
                is_success.and_then(|_| response.into_body().concat2().from_err())
            })
            .and_then(|chunk| serde_json::from_slice::<common::Ping>(&chunk).map_err(Error::from))
            .and_then(|ping| future::ok(Some(ping)))
            .from_err::<Error>();

        Box::new(f)
    }

    fn on_ping(&mut self, sender: &str, peers: &[String]) {
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
        let now = Instant::now();
        self.peers
            .iter()
            .filter_map(|(key, peer)| {
                if peer.is_active(now) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect()
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

    fn find_resources(&self, uri: &str) -> Vec<Resource> {
        // TODO(indutny): LRU
        let mut resources: Vec<Resource> = self
            .peers
            .values()
            .map(|peer| Resource::new(peer.uri(), uri, false, self.config.hash_seed))
            .collect();
        resources.push(self.construct_resource(uri));
        resources.sort_unstable();
        resources.truncate(self.config.replicate as usize + 1);

        resources
    }

    // TODO(indutny): find new locations for the data
    fn find_rebalance_resources(
        &self,
        uri: &str,
        union: &[&String],
        removed_peers: &HashSet<String>,
        added_peers: &HashSet<String>,
    ) -> Vec<Resource> {
        // TODO(indutny): LRU
        let mut resources: Vec<Resource> = union
            .iter()
            .map(|peer_uri| Resource::new(peer_uri, uri, false, self.config.hash_seed))
            .filter(|resource| !removed_peers.contains(resource.peer_uri()))
            .collect();
        resources.push(self.construct_resource(uri));
        resources.sort_unstable();

        resources.truncate(self.config.replicate as usize + 1);

        resources
            .into_iter()
            .filter(|resource| resource.is_local() || added_peers.contains(resource.peer_uri()))
            .collect()
    }
}
