extern crate futures;
extern crate serde_json;
extern crate tokio;

use futures::future::{self, FutureResult};
use futures::prelude::*;
use futures::IntoFuture;
use hyper::{Body, Method, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, oneshot};

use crate::error::Error as RPCError;
use crate::message::rpc::*;
use crate::message::{request, response};
use crate::node::Node;

pub struct RPCService {
    request_tx: mpsc::UnboundedSender<RequestPacket>,
}

impl RPCService {
    pub fn new(request_tx: mpsc::UnboundedSender<RequestPacket>) -> RPCService {
        RPCService { request_tx }
    }

    pub fn run_rpc(
        request_rx: mpsc::UnboundedReceiver<RequestPacket>,
        mut node: Node,
    ) -> impl Future<Item = (), Error = RPCError> {
        request_rx
            .from_err::<RPCError>()
            .for_each(move |packet| {
                let packet = match packet {
                    RequestPacket::HTTP(packet) => packet,
                    RequestPacket::Poll => {
                        // TODO(indutny): poll
                        return future::ok(());
                    }
                };

                let response_tx = packet.response_tx;
                let maybe_res_msg = match packet.message {
                    RequestMessage::Info => node.recv_info().map(ResponseMessage::Info),
                    RequestMessage::Ping(body) => node.recv_ping(body).map(ResponseMessage::Ping),
                };

                let res_msg = match maybe_res_msg {
                    Err(err) => {
                        return future::err(RPCError::from(err));
                    }
                    Ok(res_msg) => res_msg,
                };

                let res_packet = ResponsePacket {
                    poll_at: node.poll_at(),
                    message: res_msg,
                };

                if response_tx.send(res_packet).is_err() {
                    future::err(RPCError::OneShotSend)
                } else {
                    future::ok(())
                }
            })
            .from_err()
    }

    fn fetch_json<T: DeserializeOwned>(
        req: Request<Body>,
    ) -> impl Future<Item = T, Error = RPCError> {
        req.into_body()
            .concat2()
            .from_err::<RPCError>()
            .and_then(|chunk| serde_json::from_slice::<T>(&chunk).map_err(RPCError::from))
    }
}

impl hyper::service::Service for RPCService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = RPCError;
    type Future = Box<Future<Item = Response<Body>, Error = Self::Error> + Send>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut res = Response::builder();
        res.header("content-type", "application/json");

        let (response_tx, response_rx) = oneshot::channel();

        // TODO(indutny): authorization
        let req_message: Box<Future<Item = RequestMessage, Error = RPCError> + Send> =
            match (req.method(), req.uri().path()) {
                (&Method::GET, "/_info") => Box::new(future::ok(RequestMessage::Info)),
                (&Method::PUT, "/_ping") => {
                    Box::new(RPCService::fetch_json(req).map(RequestMessage::Ping))
                }
                _ => {
                    res.status(StatusCode::NOT_FOUND);
                    let body = Body::from("{\"error\":\"Not found\"}");
                    return Box::new(res.body(body).into_future().from_err());
                }
            };

        let request_tx = self.request_tx.clone();

        let handle_request = req_message
            .and_then(move |req_message| {
                let req_packet = RequestPacket::HTTP(RequestHTTPPacket {
                    response_tx,
                    message: req_message,
                });

                request_tx.send(req_packet).from_err()
            })
            .and_then(|_| response_rx.from_err())
            .and_then(|res_packet| {
                let json = match res_packet.message {
                    ResponseMessage::Info(info) => serde_json::to_string(&info),
                    ResponseMessage::Ping(ping) => serde_json::to_string(&ping),
                };

                json.map(Body::from).map_err(RPCError::from)
            })
            .then(move |result| {
                match result {
                    Ok(body) => res.body(body),
                    Err(err) => {
                        let maybe_json = serde_json::to_string(&ResponseError { error: err });
                        match maybe_json {
                            Ok(json) => res.body(Body::from(json)),
                            Err(_) => res.body(Body::from("{\"error\":\"unknown error\"}")),
                        }
                    }
                }
                .into_future()
                .from_err()
            });

        Box::new(handle_request)
    }
}

impl IntoFuture for RPCService {
    type Future = FutureResult<Self::Item, Self::Error>;
    type Item = Self;
    type Error = RPCError;

    fn into_future(self) -> Self::Future {
        future::ok(self)
    }
}
