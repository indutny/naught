extern crate tokio_sync;
extern crate serde_json;
extern crate futures;

use futures::{IntoFuture};
use futures::prelude::*;
use futures::future::{self, FutureResult};
use hyper::{Body, Method, Request, Response, StatusCode};
use tokio_sync::{mpsc, oneshot};

use crate::error::{Error as RPCError};
use crate::message::{response};
use crate::node::Node;

pub enum ResponseMessage {
    Info(response::Info),
    ListKeys(response::ListKeys),
}

pub enum RequestMessage {
    Info,
    ListKeys,
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
                RequestMessage::Info => node.info().map(|info| {
                    ResponseMessage::Info(info)
                }),
                RequestMessage::ListKeys => node.list_keys().map(|info| {
                    ResponseMessage::ListKeys(info)
                }),
            };

            let res_msg = match maybe_res_msg {
                Err(err) => {
                    return future::err(RPCError::from(err));
                },
                Ok(res_msg) => res_msg,
            };

            if response_tx.send(res_msg).is_err() {
                future::err(RPCError::Every)
            } else {
                future::ok(())
            }
        }).from_err()
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

        let maybe_req_message = match (req.method(), req.uri().path()) {
            (&Method::GET, "/_info") => Some(RequestMessage::Info),
            (&Method::GET, "/_keys") => Some(RequestMessage::ListKeys),
            _ => None,
        };

        let req_message = match maybe_req_message {
            None => {
                res.status(StatusCode::NOT_FOUND);
                let body = Body::from("{\"error\":\"Not found\"}");
                return Box::new(res.body(body).into_future().from_err());
            },
            Some(req_message) => req_message,
        };

        let req_packet = RequestPacket {
            response_tx,
            message: req_message,
        };

        if let Err(fail) = self.request_tx.try_send(req_packet) {
            return Box::new(future::err(RPCError::from(fail)));
        }

        Box::new(
            response_rx
                .from_err::<RPCError>()
                .and_then(|res_msg| {
                    let json = match res_msg {
                        ResponseMessage::Info(info) => {
                            serde_json::to_string(&info)
                        },
                        ResponseMessage::ListKeys(list_keys) => {
                            serde_json::to_string(&list_keys)
                        },
                    };

                    json
                        .map(|json| Body::from(json))
                        .map_err(|err| RPCError::from(err))
                })
                .and_then(move |body| {
                    res.body(body).into_future().from_err()
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
