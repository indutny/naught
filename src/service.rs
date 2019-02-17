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

    fn fetch_json<T: DeserializeOwned>(req: Request<Body>) -> impl Future<Item = T, Error = Error> {
        req.into_body()
            .concat2()
            .from_err::<Error>()
            .and_then(|chunk| serde_json::from_slice::<T>(&chunk).map_err(Error::from))
    }

    fn to_json_body<T: Serialize>(value: &T) -> Result<Body, Error> {
        serde_json::to_string(value)
            .map(Body::from)
            .map_err(|err| Error::from(err))
    }
}

impl hyper::service::Service for RPCService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = Error;
    type Future = Box<Future<Item = Response<Body>, Error = Self::Error> + Send>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut res = Response::builder();
        res.header("content-type", "application/json");

        // TODO(indutny): authorization
        let body: Box<Future<Item = Body, Error = Error> + Send> =
            match (req.method(), req.uri().path()) {
                (&Method::GET, "/_info") => Box::new(future::result(
                    self.node
                        .lock()
                        .expect("lock to acquire")
                        .recv_info()
                        .and_then(|info| RPCService::to_json_body(&info)),
                )),
                (&Method::PUT, "/_ping") => {
                    let node = self.node.clone();
                    Box::new(
                        RPCService::fetch_json(req)
                            .and_then(move |ping| {
                                node.lock().expect("lock to acquire").recv_ping(&ping)
                            })
                            .and_then(|res| RPCService::to_json_body(&res)),
                    )
                }
                _ => {
                    res.status(StatusCode::NOT_FOUND);
                    let result = res.body(Body::from("{\"error\":\"Not found\"}"));
                    return Box::new(result.into_future().from_err());
                }
            };

        Box::new(
            body.or_else(|err| {
                // TODO(indutny): 500 Status Code
                let json = serde_json::to_string(&response::Error { error: err })
                    .unwrap_or("{\"error\":\"unknown error\"}".to_string());
                Ok(Body::from(json))
            })
            .and_then(move |body| res.body(body).into_future().from_err()),
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
