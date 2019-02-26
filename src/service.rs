extern crate futures;
extern crate hmac;
extern crate serde;
extern crate serde_json;
extern crate sha2;

use std::sync::{Arc, Mutex};

use futures::future::{self, FutureResult};
use futures::prelude::*;
use futures::IntoFuture;
use hmac::{Hmac, Mac};
use hyper::{Body, Method, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::error::Error;
use crate::message::response;
use crate::node::Node;

type HmacSha256 = Hmac<Sha256>;

const CONTAINER_ALPHABET: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

pub struct RPCService {
    config: Config,
    node: Arc<Mutex<Node>>,
    auth_hash: Vec<u8>,
}

impl RPCService {
    pub fn new(config: Config, node: Arc<Mutex<Node>>) -> RPCService {
        let mut hasher = Sha256::new();
        hasher.input(config.get_auth().as_bytes());
        let auth_hash = hasher.result().to_vec();

        RPCService {
            config,
            node,
            auth_hash,
        }
    }

    fn fetch_json<T: DeserializeOwned>(body: Body) -> impl Future<Item = T, Error = Error> {
        body.concat2()
            .from_err::<Error>()
            .and_then(|chunk| serde_json::from_slice::<T>(&chunk).map_err(Error::from))
    }

    fn fetch_raw(body: Body) -> impl Future<Item = Vec<u8>, Error = Error> {
        body.concat2()
            .from_err::<Error>()
            .map(|chunk| chunk.to_vec())
    }

    fn stringify_value<T: Serialize>(value: &T) -> Result<Body, Error> {
        serde_json::to_string(value)
            .map(Body::from)
            .map_err(Error::from)
    }

    fn compute_container(secret: &[u8], value: &[u8]) -> Result<String, Error> {
        let mut mac = HmacSha256::new_varkey(secret)?;
        mac.input(&value);

        let mut digest: [u8; 8] = [0; 8];
        digest.copy_from_slice(&mac.result().code()[..8]);
        let mut digest = u64::from_be_bytes(digest);

        let alphabet_len = CONTAINER_ALPHABET.len() as u64;

        let mut result = String::with_capacity(16);
        while digest != 0 {
            let m = digest % alphabet_len;
            digest /= alphabet_len;

            result.push(CONTAINER_ALPHABET[m as usize]);
        }
        Ok(result)
    }

    fn check_auth(&self, parts: &hyper::http::request::Parts) -> bool {
        match parts.method {
            Method::GET | Method::HEAD => {
                return true;
            }
            _ => {}
        };

        let header = parts
            .headers
            .get(hyper::header::AUTHORIZATION)
            .map(|val| val.to_str().unwrap_or(""))
            .unwrap_or("");

        // No auth - no check
        if header.is_empty() {
            return false;
        }

        let mut hasher = Sha256::new();
        hasher.input(header.as_bytes());
        let result = hasher.result();

        for (a, b) in result.into_iter().zip(self.auth_hash.iter()) {
            if a != *b {
                return false;
            }
        }
        true
    }
}

struct Resource {
    status: StatusCode,
    mime: Option<String>,
    sender: Option<String>,
    body: Body,
}

impl hyper::service::Service for RPCService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = Error;
    type Future = Box<Future<Item = Response<Body>, Error = Self::Error> + Send>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut res = Response::builder();

        let (parts, body) = req.into_parts();

        let redirect = parts
            .headers
            .get("x-naught-redirect")
            .map(|val| val.to_str().unwrap_or("true"))
            .unwrap_or("true");
        let redirect: bool = redirect.parse().unwrap_or(true);

        let container = parts
            .headers
            .get(hyper::header::HOST)
            .map(|val| val.to_str().unwrap_or("unknown"))
            .unwrap_or("unknown");
        let container = container.split('.').next().unwrap_or("unknown");

        let is_authorized = self.check_auth(&parts);

        let resource: Box<Future<Item = Resource, Error = Error> + Send> =
            match (parts.method, parts.uri.path()) {
                (Method::GET, "/_info") => Box::new(
                    future::result(self.node.lock().expect("lock to acquire").recv_info())
                        .and_then(|info| RPCService::stringify_value(&info))
                        .map(|body| Resource {
                            status: StatusCode::OK,
                            mime: None,
                            sender: None,
                            body,
                        }),
                ),
                (Method::POST, "/_ping") => {
                    if is_authorized {
                        let node = self.node.clone();
                        Box::new(
                            RPCService::fetch_json(body)
                                .and_then(move |ping| {
                                    node.lock().expect("lock to acquire").recv_ping(&ping)
                                })
                                .and_then(|res| RPCService::stringify_value(&res))
                                .map(|body| Resource {
                                    status: StatusCode::OK,
                                    mime: None,
                                    sender: None,
                                    body,
                                }),
                        )
                    } else {
                        Box::new(future::err(Error::NotAuthorized))
                    }
                }
                (Method::GET, resource) => Box::new(
                    self.node
                        .lock()
                        .expect("lock to acquire")
                        .fetch(&container, &resource[1..], redirect)
                        .map(|response| Resource {
                            status: StatusCode::OK,
                            mime: Some(response.mime),
                            sender: Some(response.peer),
                            body: response.body,
                        }),
                ),
                (Method::HEAD, "/") => Box::new(
                    future::result(self.node.lock().expect("lock to acquire").peek(&container))
                        .and_then(|res| RPCService::stringify_value(&res))
                        .map(|body| Resource {
                            status: StatusCode::OK,
                            mime: None,
                            sender: None,
                            body,
                        }),
                ),
                (Method::PUT, "/_container") => {
                    if is_authorized {
                        let node = self.node.clone();
                        let container_secret = self.config.container_secret.clone();

                        Box::new(
                            RPCService::fetch_raw(body)
                                .and_then(move |value| {
                                    RPCService::compute_container(&container_secret, &value)
                                        .map(|container| (container, value))
                                })
                                .and_then(move |(container, value)| {
                                    node.lock()
                                        .expect("lock to acquire")
                                        .store(&container, value, redirect)
                                })
                                .and_then(|res| RPCService::stringify_value(&res))
                                .map(|body| Resource {
                                    status: StatusCode::CREATED,
                                    mime: None,
                                    sender: None,
                                    body,
                                }),
                        )
                    } else {
                        Box::new(future::err(Error::NotAuthorized))
                    }
                }
                _ => Box::new(future::err(Error::BadRequest)),
            };

        Box::new(
            resource
                .or_else(|err| {
                    let status = match err {
                        Error::NotFound => StatusCode::NOT_FOUND,
                        Error::BadRequest => StatusCode::BAD_REQUEST,
                        Error::NonLocalStore(_) => StatusCode::GONE,
                        Error::NotAuthorized => StatusCode::UNAUTHORIZED,
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    let json = serde_json::to_string(&response::Error { error: err })
                        .unwrap_or_else(|_| "{\"error\":\"unknown error\"}".to_string());
                    Ok(Resource {
                        status,
                        mime: None,
                        sender: None,
                        body: Body::from(json),
                    })
                })
                .and_then(move |resource| {
                    res.status(resource.status);
                    // TODO(indutny): excessive cloning of strings
                    let mime = resource
                        .mime
                        .unwrap_or_else(|| "application/json".to_string());
                    res.header(hyper::header::CONTENT_TYPE, mime);
                    if let Some(sender) = resource.sender {
                        res.header("x-naught-sender", sender);
                    }
                    res.body(resource.body).into_future().from_err()
                }),
        )
    }
}

impl IntoFuture for RPCService {
    type Future = FutureResult<Self::Item, Self::Error>;
    type Item = Self;
    type Error = Error;

    fn into_future(self) -> Self::Future {
        future::ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_compute_container_key() {
        let container =
            RPCService::compute_container(&[0; 8], &[1; 16]).expect("compute to not fail");
        assert_eq!(container, "xi-eugvidbaq1");

        let container =
            RPCService::compute_container(&[1; 8], &[1; 16]).expect("compute to not fail");
        assert_eq!(container, "uanrj2gxuwga2");

        let container =
            RPCService::compute_container(&[1; 8], &[2; 16]).expect("compute to not fail");
        assert_eq!(container, "8bqx1cueyw5h2");
    }
}
