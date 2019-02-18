extern crate futures;
extern crate hyper;
extern crate siphasher;

use std::cmp::Ordering;
use std::hash::Hasher;

use futures::future;
use futures::prelude::*;
use siphasher::sip::SipHasher;

use crate::error::Error;

type FutureBody = Box<Future<Item = hyper::Body, Error = Error> + Send>;

#[derive(Eq, Debug)]
pub struct Resource {
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
    pub fn new(uri: String, local: bool, hash_seed: (u64, u64)) -> Resource {
        let mut hasher = SipHasher::new_with_keys(hash_seed.0, hash_seed.1);
        hasher.write(uri.as_bytes());
        Resource {
            uri,
            local,
            hash: hasher.finish(),
        }
    }

    pub fn fetch(&self, sender: &str) -> FutureBody {
        if self.local {
            return Box::new(future::err(Error::NotFound));
        }

        trace!("fetch remote resource: {}", self.uri);
        let request = hyper::Request::builder()
            .method("GET")
            .uri(self.uri.to_string())
            .header("x-naught-sender", sender.to_string())
            .header("x-naught-redirect", "false")
            .body(hyper::Body::empty());

        let request = match request {
            Ok(request) => request,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        Box::new(
            hyper::Client::new()
                .request(request)
                .from_err::<Error>()
                .map(|res| res.into_body()),
        )
    }

    pub fn store(
        &self,
        sender: &str,
        value: &[u8],
    ) -> Box<Future<Item = (), Error = Error> + Send> {
        if self.local {
            // Should be handled by caller
            return Box::new(future::ok(()));
        }

        trace!("store remote resource: {}", self.uri);
        let request = hyper::Request::builder()
            .method("PUT")
            .uri(self.uri.to_string())
            .header("x-naught-sender", sender.to_string())
            .header("x-naught-redirect", "false")
            .body(hyper::Body::from(value.to_vec()));

        let request = match request {
            Ok(request) => request,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        Box::new(
            hyper::Client::new()
                .request(request)
                .from_err::<Error>()
                .map(|_| ()),
        )
    }
}