extern crate tokio_sync;
extern crate serde_json;
extern crate futures;

use futures::{IntoFuture};
use futures::prelude::*;
use futures::future::{self, FutureResult};
use hyper::{Body, Request, Response};
use tokio_sync::{mpsc};

use crate::node::Node;

pub struct RequestPacket {
}

pub struct RPCService {
    request_tx: mpsc::UnboundedSender<RequestPacket>,
}

impl RPCService {
    pub fn new(request_tx: mpsc::UnboundedSender<RequestPacket>) -> RPCService {
        RPCService { request_tx }
    }

    pub fn forward_rpc(mut node: Node) -> FutureResult<(), crate::error::Error> {
        future::ok(())
    }
}

impl hyper::service::Service for RPCService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = crate::error::Error;
    type Future = FutureResult<Response<Body>, Self::Error>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        future::ok(Response::new(Body::empty()))
    }
}

impl IntoFuture for RPCService {
    type Future = future::FutureResult<Self::Item, Self::Error>;
    type Item = Self;
    type Error = crate::error::Error;

    fn into_future(self) -> Self::Future {
        future::ok(self)
    }
}
