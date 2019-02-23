extern crate futures;
extern crate serde_json;
extern crate sha2;

use std::sync::{Arc, Mutex};

use futures::future::{self, FutureResult};
use futures::prelude::*;
use futures::IntoFuture;
use hyper::{Body, Method, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::Error;
use crate::message::response;
use crate::node::Node;

const CONTAINER_KEY_SIZE: usize = 8;

pub struct RPCService {
    node: Arc<Mutex<Node>>,
}

impl RPCService {
    pub fn new(node: Arc<Mutex<Node>>) -> RPCService {
        RPCService { node }
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

    fn compute_container(value: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.input(&value);
        let digest = hasher.result();

        // TODO(indutny): use non-hex?
        let chunks: Vec<String> = digest[..CONTAINER_KEY_SIZE]
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect();

        chunks.concat()
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

        // TODO(indutny): authorization
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
                }
                (Method::HEAD, resource) => Box::new(
                    future::result(
                        self.node
                            .lock()
                            .expect("lock to acquire")
                            .peek(&resource[1..]),
                    )
                    .and_then(|res| RPCService::stringify_value(&res))
                    .map(|body| Resource {
                        status: StatusCode::OK,
                        mime: None,
                        sender: None,
                        body,
                    }),
                ),
                (Method::GET, resource) => {
                    let container = parts
                        .headers
                        .get(hyper::header::HOST)
                        .map(|val| val.to_str().unwrap_or("unknown"))
                        .unwrap_or("unknown");

                    let container = container.split('.').next().unwrap_or("unknown");

                    Box::new(
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
                    )
                }
                (Method::PUT, "/_container") => {
                    let node = self.node.clone();
                    Box::new(
                        RPCService::fetch_raw(body)
                            .and_then(move |value| {
                                let container = RPCService::compute_container(&value);

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
