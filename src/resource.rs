extern crate futures;
extern crate siphasher;

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use futures::future;
use futures::prelude::*;
use siphasher::sip::SipHasher;

use crate::client::Client;
use crate::data::Data;
use crate::error::Error;
use crate::message::response;

type FutureFetch = Box<Future<Item = response::Fetch, Error = Error> + Send>;

#[derive(Eq, Clone, Debug)]
pub struct Resource {
    peer_uri: String,
    store_uri: String,
    container: String,
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
        self.store_uri == other.store_uri
    }
}

impl Hash for Resource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.store_uri.hash(state);
    }
}

impl Resource {
    pub fn new(peer_uri: &str, container: &str, local: bool, hash_seed: (u64, u64)) -> Resource {
        let mut hasher = SipHasher::new_with_keys(hash_seed.0, hash_seed.1);

        let peer_uri = peer_uri.to_string();
        let store_uri = format!("{}/{}", peer_uri, container);

        hasher.write(store_uri.as_bytes());
        Resource {
            peer_uri,
            store_uri,
            container: container.to_string(),
            local,
            hash: hasher.finish(),
        }
    }

    pub fn peer_uri(&self) -> &str {
        &self.peer_uri
    }

    pub fn is_local(&self) -> bool {
        self.local
    }

    pub fn fetch(&self, client: &Client, uri: &str) -> FutureFetch {
        if self.local {
            return Box::new(future::err(Error::NotFound));
        }

        client.fetch(&self.peer_uri, &self.container, uri)
    }

    pub fn store(
        &self,
        client: &Client,
        data: &Data,
    ) -> Box<Future<Item = (), Error = Error> + Send> {
        if self.local {
            // Should be handled by caller
            return Box::new(future::ok(()));
        }

        client.store(&self.peer_uri, &self.container, data)
    }
}
