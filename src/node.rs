extern crate futures;
extern crate hyper;
extern crate rand;
extern crate serde_json;

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::time::Instant;

use futures::future;
use futures::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::config::Config;
use crate::data::Data;
use crate::error::Error;
use crate::message::{common, response};
use crate::peer::Peer;
use crate::resource::Resource;

type MaybePing = Option<common::Ping>;
type FuturePingVec = Box<Future<Item = Vec<MaybePing>, Error = Error> + Send>;
type FuturePing = Box<Future<Item = MaybePing, Error = Error> + Send>;
type FutureFetch = Box<Future<Item = response::Fetch, Error = Error> + Send>;
type FutureURI = Box<Future<Item = Option<String>, Error = Error> + Send>;
type FutureKeyVec = Box<Future<Item = Vec<String>, Error = Error> + Send>;
type FutureMaybeKey = Box<Future<Item = Option<String>, Error = Error> + Send>;
type FutureBool = Box<Future<Item = bool, Error = Error> + Send>;

pub struct Node {
    config: Config,
    uri: String,
    peers: HashMap<String, Peer>,
    data: HashMap<String, Data>,

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

    pub fn peek(&self, container: &str) -> Result<(), Error> {
        if self.data.contains_key(container) {
            trace!("peek existing container: {}", container);
            Ok(())
        } else {
            trace!("peek missing container: {}", container);
            Err(Error::NotFound)
        }
    }

    pub fn fetch(&self, container: &str, uri: &str, redirect: bool) -> FutureFetch {
        if let Some(entry) = self.data.get(container) {
            trace!(
                "fetch existing container: {} redirect: {}",
                container,
                redirect
            );

            let file = match entry.serve(uri) {
                Some(file) => file,
                None => {
                    trace!("fetch missing uri: {} in container: {}", uri, container,);
                    return Box::new(future::err(Error::NotFound));
                }
            };

            return Box::new(future::ok(response::Fetch {
                peer: self.uri.to_string(),
                mime: file.mime.clone(),
                body: hyper::Body::from(file.content.clone()),
            }));
        }

        let mut resources = if redirect {
            self.find_resources(container)
        } else {
            vec![]
        };

        // No resources to redirect to
        if resources.is_empty() {
            trace!(
                "fetch missing container: {} redirect: {}",
                container,
                redirect
            );
            return Box::new(future::err(Error::NotFound));
        }

        // Shuffle resources to balance requests fairly
        let mut rng = thread_rng();
        resources.shuffle(&mut rng);

        let response: FutureFetch = resources
            .into_iter()
            .map(|resource| resource.fetch(&self.client, &self.uri, uri))
            .fold(Box::new(future::err(Error::Unreachable)), |acc, f| {
                Box::new(acc.or_else(move |_| f))
            });

        Box::new(response)
    }

    pub fn store(
        &mut self,
        container: &str,
        value: Vec<u8>,
        redirect: bool,
    ) -> Box<Future<Item = response::Store, Error = Error> + Send> {
        if self.data.contains_key(container) {
            trace!("duplicate container: {}", container);
            return Box::new(future::ok(response::Store {
                container: container.to_string(),
                uris: vec![],
            }));
        }

        let entry = match Data::from_tar(value) {
            Ok(entry) => entry,
            Err(err) => {
                return Box::new(future::err(err));
            }
        };

        // Store only locally when redirect is `false`
        let resources: Vec<Resource> = self
            .find_resources(container)
            .into_iter()
            .filter(|resource| redirect || resource.is_local())
            .collect();

        // Nowhere to store, notify caller
        if resources.is_empty() {
            trace!("no resources for container: {}", container);
            return Box::new(future::err(Error::NonLocalStore(container.to_string())));
        }

        trace!("new container: {}", container);

        let remote: Vec<FutureURI> = resources
            .into_iter()
            .map(|resource| -> FutureURI {
                // TODO(indutny): excessive cloning?
                let target_uri = resource.peer_uri().to_string();

                let store = resource
                    .store(&self.client, &self.uri, &entry)
                    .map(move |_| Some(target_uri))
                    .or_else(|err| {
                        // Single failed store should not fail others
                        trace!("remote store failed due to error: {:?}", err);
                        future::ok(None)
                    });
                Box::new(store)
            })
            .collect();

        let container_copy = container.to_string();

        let uris = future::join_all(remote).and_then(move |target_uris| {
            trace!("stored container at: {:?}", target_uris);
            future::ok(response::Store {
                container: container_copy,
                uris: target_uris.into_iter().filter_map(|uri| uri).collect(),
            })
        });

        self.data.insert(container.to_string(), entry);

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
            .map(|(container, entry)| -> FutureMaybeKey {
                let resources =
                    self.find_rebalance_resources(container, &union, &added_peers, &removed_peers);

                let keep_local = resources.iter().any(|resource| resource.is_local());

                let successes: Vec<FutureBool> = resources
                    .into_iter()
                    .map(|resource| -> FutureBool {
                        Box::new(
                            resource
                                .store(&self.client, &self.uri, &entry)
                                .map(|_| true)
                                .or_else(|err| {
                                    // Single failed rebalance should not fail others
                                    trace!("remote rebalance failed due to error: {:?}", err);
                                    future::ok(false)
                                }),
                        )
                    })
                    .collect();

                let container = container.clone();

                Box::new(future::join_all(successes).and_then(move |successes| {
                    if !keep_local && successes.into_iter().any(|v| v) {
                        future::ok(Some(container))
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
                .map(|keys| keys.into_iter().filter_map(|container| container).collect()),
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

    fn construct_resource(&self, container: &str) -> Resource {
        Resource::new(&self.uri, container, true, self.config.hash_seed)
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

    fn find_resources(&self, container: &str) -> Vec<Resource> {
        let now = Instant::now();

        // TODO(indutny): LRU
        let mut resources: Vec<Resource> = self
            .peers
            .values()
            .filter(|peer| peer.is_stable(now) && peer.is_active(now))
            .map(|peer| Resource::new(peer.uri(), container, false, self.config.hash_seed))
            .collect();
        resources.push(self.construct_resource(container));
        resources.sort();
        resources.truncate(self.config.replicate as usize + 1);

        resources
    }

    fn find_rebalance_resources(
        &self,
        container: &str,
        union: &[&String],
        added_peers: &HashSet<String>,
        removed_peers: &HashSet<String>,
    ) -> Vec<Resource> {
        let self_resource = self.construct_resource(container);

        // TODO(indutny): LRU
        let mut resources: Vec<Resource> = union
            .iter()
            .map(|peer_uri| Resource::new(peer_uri, container, false, self.config.hash_seed))
            .collect();
        resources.push(self_resource.clone());
        resources.sort();

        // NOTE: Local peer should always appear in new resources, since we
        // are using it for detecting moved keys
        let mut new_resources: Vec<Resource> = resources
            .iter()
            .filter(|resource| !removed_peers.contains(resource.peer_uri()))
            .cloned()
            .collect();

        let mut old_resources: Vec<Resource> = resources
            .into_iter()
            .filter(|resource| !added_peers.contains(resource.peer_uri()))
            .collect();

        old_resources.truncate(self.config.replicate as usize + 1);
        new_resources.truncate(self.config.replicate as usize + 1);

        // TODO(indutny): optimize if ever needed
        let mut old_resources: HashSet<Resource> = HashSet::from_iter(old_resources.into_iter());
        let new_resources = HashSet::from_iter(new_resources.into_iter());

        // Make sure to keep local copy
        if new_resources.contains(&self_resource) {
            old_resources.remove(&self_resource);
        }

        new_resources.difference(&old_resources).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    #[test]
    fn it_should_find_rebalance_resources() {
        let mut config = Config::new(vec![0], (0, 0));
        config.stable_delay = Duration::from_secs(0);
        let node = Node::new(SocketAddr::from(([157, 230, 95, 152], 8007)), config);

        let peers: Vec<String> = [
            "http://157.230.95.152:80",
            "http://157.230.95.152:8001",
            "http://157.230.95.152:8002",
            "http://157.230.95.152:8003",
            "http://157.230.95.152:8004",
            "http://157.230.95.152:8005",
            "http://157.230.95.152:8006",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let union: Vec<&String> = peers.iter().collect();
        let added_peers = HashSet::new();
        let mut removed_peers = HashSet::new();
        removed_peers.insert("http://157.230.95.152:8004".to_string());

        let mut resources: Vec<String> = node
            .find_rebalance_resources("derivepass", &union, &added_peers, &removed_peers)
            .into_iter()
            .map(|resource| resource.peer_uri().to_string())
            .collect();
        resources.sort();

        assert_eq!(
            resources,
            vec!["http://157.230.95.152:8002", "http://157.230.95.152:8007",]
        );
    }
}
