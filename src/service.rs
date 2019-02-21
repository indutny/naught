extern crate futures;
extern crate serde_json;

use std::sync::{Arc, Mutex};

use futures::future::{self, FutureResult};
use futures::prelude::*;
use futures::IntoFuture;
use hyper::{Body, Method, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::Error;
use crate::message::response;
use crate::node::Node;

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
}

struct Resource {
    status: StatusCode,
    sender: Option<String>,
    body: Body,
    raw: bool,
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
                            sender: None,
                            body,
                            raw: false,
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
                                sender: None,
                                body,
                                raw: false,
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
                        sender: None,
                        body,
                        raw: false,
                    }),
                ),
                (Method::GET, resource) => {
                    let node = self.node.clone();
                    let resource = resource[1..].to_string();

                    Box::new(
                        self.node
                            .lock()
                            .expect("lock to acquire")
                            .fetch(&resource, redirect)
                            .and_then(move |response| {
                                node.lock()
                                    .expect("lock to acquire")
                                    .after_fetch(&resource, response)
                            })
                            .map(|response| Resource {
                                status: StatusCode::OK,
                                sender: Some(response.peer),
                                body: response.body,
                                raw: true,
                            }),
                    )
                }
                (Method::PUT, resource) => {
                    let node = self.node.clone();
                    let resource = resource[1..].to_string();
                    Box::new(
                        RPCService::fetch_raw(body)
                            .and_then(move |value| {
                                node.lock()
                                    .expect("lock to acquire")
                                    .store(&resource, value, redirect)
                            })
                            .and_then(|res| RPCService::stringify_value(&res))
                            .map(|body| Resource {
                                status: StatusCode::CREATED,
                                sender: None,
                                body,
                                raw: false,
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
                        sender: None,
                        body: Body::from(json),
                        raw: false,
                    })
                })
                .and_then(move |resource| {
                    res.status(resource.status);
                    if !resource.raw {
                        res.header(hyper::header::CONTENT_TYPE, "application/json");
                    }
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
