extern crate futures;
extern crate hyper;
extern crate siphasher;

use std::cmp::Ordering;
use std::hash::Hasher;

use futures::future;
use futures::prelude::*;
use hyper::{client, Body, Client, Method, Request, Response};
use siphasher::sip::SipHasher;

use crate::data::Data;
use crate::error::Error;

type FutureBody = Box<Future<Item = Body, Error = Error> + Send>;

#[derive(Eq, Debug)]
pub struct Resource {
    peer_uri: String,
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
    pub fn new(peer_uri: &str, uri: &str, local: bool, hash_seed: (u64, u64)) -> Resource {
        let mut hasher = SipHasher::new_with_keys(hash_seed.0, hash_seed.1);

        let peer_uri = peer_uri.to_string();
        let uri = format!("{}/{}", peer_uri, uri);

        hasher.write(uri.as_bytes());
        Resource {
            peer_uri,
            uri,
            local,
            hash: hasher.finish(),
        }
    }

    pub fn peer_uri(&self) -> &str {
        &self.peer_uri
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn is_local(&self) -> bool {
        self.local
    }

    pub fn fetch(&self, client: &Client<client::HttpConnector>, sender: &str) -> FutureBody {
        if self.local {
            return Box::new(future::err(Error::NotFound));
        }

        trace!("fetch remote resource: {}", self.uri);
        let request = Request::builder()
            .method(Method::GET)
            .uri(self.uri.to_string())
            .header("x-naught-sender", sender.to_string())
            .header("x-naught-redirect", "false")
            .body(Body::empty());

        let request = match request {
            Ok(request) => request,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        // TODO(indutny): timeout
        Box::new(
            client
                .request(request)
                .from_err::<Error>()
                .and_then(|response| {
                    if response.status().is_success() {
                        Ok(response)
                    } else {
                        Err(Error::NotFound)
                    }
                })
                .map(|response| response.into_body()),
        )
    }

    pub fn store(
        &self,
        client: &Client<client::HttpConnector>,
        sender: &str,
        data: &Data,
    ) -> Box<Future<Item = (), Error = Error> + Send> {
        if self.local {
            // Should be handled by caller
            return Box::new(future::ok(()));
        }

        let peek = Request::builder()
            .method(Method::HEAD)
            .uri(self.uri.to_string())
            .header("x-naught-sender", sender.to_string())
            .header("x-naught-redirect", "false")
            .body(Body::empty());

        trace!("store remote resource: {}", self.uri);
        let store = Request::builder()
            .method(Method::PUT)
            .uri(self.uri.to_string())
            .header("x-naught-sender", sender.to_string())
            .header("x-naught-redirect", "false")
            .body(Body::from(Vec::from(data)));

        let peek = match peek {
            Ok(peek) => peek,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        let store = match store {
            Ok(store) => store,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        let peek = client
            .request(peek)
            .from_err::<Error>()
            .and_then(|response| {
                if response.status().is_success() {
                    Ok(())
                } else {
                    Err(Error::NotFound)
                }
            });

        // TODO(indutny): excessive cloning?
        let debug_uri = self.uri.to_string();

        let on_store_response = move |response: Response<Body>| {
            if response.status().is_success() {
                Ok(())
            } else {
                Err(Error::StoreFailed(debug_uri))
            }
        };

        let store = client
            .request(store)
            .from_err::<Error>()
            .and_then(on_store_response);

        let peek_or_store = peek.or_else(move |_| store);

        // TODO(indutny): timeout
        // TODO(indutny): retry?
        Box::new(peek_or_store)
    }
}
