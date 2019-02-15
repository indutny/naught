extern crate tokio_sync;
extern crate serde;
extern crate serde_json;
extern crate futures;

use futures::{IntoFuture};
use futures::prelude::*;
use futures::future::{self, FutureResult};
use hyper::{Body, Method, Request, Response, StatusCode};
use serde::{Serialize};
use serde::de::{DeserializeOwned};
use tokio_sync::{mpsc, oneshot};

use crate::error::{Error as RPCError};
use crate::message::{request, response};
use crate::node::Node;

pub enum ResponseMessage {
    Info(response::Info),
    ListKeys(response::ListKeys),
    AddNode(response::AddNode),
}

#[derive(Serialize, Debug)]
struct ResponseError {
    error: RPCError,
}

pub enum RequestMessage {
    Info,
    ListKeys,
    AddNode(request::AddNode),
}

pub struct RequestPacket {
    response_tx: oneshot::Sender<ResponseMessage>,
    message: RequestMessage,
}

pub struct RPCService {
    request_tx: mpsc::UnboundedSender<RequestPacket>,
}

impl RPCService {
    pub fn new(request_tx: mpsc::UnboundedSender<RequestPacket>) -> RPCService {
        RPCService { request_tx }
    }

    pub fn forward_rpc(request_rx: mpsc::UnboundedReceiver<RequestPacket>,
                       mut node: Node) -> impl Future<Item=(), Error=RPCError> {
        request_rx.from_err::<RPCError>().for_each(move |packet| {
            let response_tx = packet.response_tx;
            let maybe_res_msg = match packet.message {
                RequestMessage::Info => node.info().map(|res| {
                    ResponseMessage::Info(res)
                }),
                RequestMessage::ListKeys => node.list_keys().map(|res| {
                    ResponseMessage::ListKeys(res)
                }),
                RequestMessage::AddNode(body) => node.add_node(&body).map(|res| {
                    ResponseMessage::AddNode(res)
                }),
            };

            let res_msg = match maybe_res_msg {
                Err(err) => {
                    return future::err(RPCError::from(err));
                },
                Ok(res_msg) => res_msg,
            };

            if response_tx.send(res_msg).is_err() {
                future::err(RPCError::OneShotSend)
            } else {
                future::ok(())
            }
        }).from_err()
    }

    fn fetch_json<T: DeserializeOwned>(req: Request<Body>) -> impl Future<Item=T, Error=RPCError> {
        req
            .into_body()
            .concat2()
            .from_err::<RPCError>()
            .and_then(|chunk| {
                serde_json::from_slice::<T>(&chunk).map_err(|err| {
                    RPCError::from(err)
                })
            })
    }
}

impl hyper::service::Service for RPCService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = RPCError;
    type Future = Box<Future<Item=Response<Body>, Error=Self::Error> + Send>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut res = Response::builder();
        res.header("content-type", "application/json");

        let (response_tx, response_rx) = oneshot::channel();

        let req_message: Box<Future<Item=RequestMessage, Error=RPCError> + Send> = match (req.method(), req.uri().path()) {
            (&Method::GET, "/_info") => Box::new(future::ok(RequestMessage::Info)),
            (&Method::GET, "/_keys") => Box::new(future::ok(RequestMessage::ListKeys)),
            (&Method::PUT, "/_nodes") => {
                Box::new(
                    RPCService::fetch_json(req)
                        .map(|body| RequestMessage::AddNode(body))
                )
            },
            _ => {
                res.status(StatusCode::NOT_FOUND);
                let body = Body::from("{\"error\":\"Not found\"}");
                return Box::new(res.body(body).into_future().from_err());
            },
        };

        let request_tx = self.request_tx.clone();

        Box::new(
            req_message
                .and_then(move |req_message| {
                    let req_packet = RequestPacket {
                        response_tx,
                        message: req_message,
                    };

                    request_tx.send(req_packet).from_err()
                })
                .and_then(|_| response_rx.from_err())
                .and_then(|res_msg| {
                    let json = match res_msg {
                        ResponseMessage::Info(info) => {
                            serde_json::to_string(&info)
                        },
                        ResponseMessage::ListKeys(list_keys) => {
                            serde_json::to_string(&list_keys)
                        },
                        ResponseMessage::AddNode(add_node) => {
                            serde_json::to_string(&add_node)
                        },
                    };

                    json
                        .map(|json| Body::from(json))
                        .map_err(|err| RPCError::from(err))
                })
                .then(move |result| {
                    match result {
                        Ok(body) => {
                            res.body(body)
                        },
                        Err(err) => {
                            let maybe_json = serde_json::to_string(
                                &ResponseError { error: err });
                            match maybe_json {
                                Ok(json) => res.body(Body::from(json)),
                                Err(err) => {
                                    res.body(Body::from("{\"error\":\"unknown error\"}"))
                                },
                            }
                        },
                    }.into_future().from_err()
                })
        )
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
