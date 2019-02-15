extern crate hyper;
extern crate futures;
extern crate tokio_sync;
extern crate serde_json;

use std::net::{IpAddr, SocketAddr};

use crate::node::Node;
use crate::message::*;

use futures::{future, Future, Stream, Sink, IntoFuture};
use hyper::{Body, Method, Request, Response, StatusCode};
use hyper::service::service_fn;
use serde::Deserialize;
use tokio_sync::{mpsc, oneshot};

#[derive(Debug)]
struct RequestPacket {
    response_tx: oneshot::Sender<ResponseMessage>,
    message: RequestMessage,
}

#[derive(Debug)]
enum RequestMessage {
    Info,
    ListKeys,
    AddNode(request::AddNode),
}

#[derive(Debug)]
enum ResponseMessage {
    Info(response::Info),
    ListKeys(response::ListKeys),
    AddNode(response::AddNode),
}

struct Service {
    request_tx: mpsc::UnboundedSender<RequestPacket>,
}

impl Service {
    fn request_to_msg(req: Request<Body>) -> Option<Box<Future<Item=RequestMessage, Error=()> + Send>> {
        match (req.method(), req.uri().path()) {
            (&Method::GET, "/_info") => Some(Box::new(future::ok(RequestMessage::Info))),
            (&Method::GET, "/_keys") => Some(Box::new(future::ok(RequestMessage::ListKeys))),
            (&Method::PUT, "/_node") => {
                Some(Box::new(future::ok(RequestMessage::AddNode(request::AddNode {
                    id: [0;8],
                    peers: vec![],
                }))))
            },
            _ => None,
        }
    }

    fn call(&self, req: Request<Body>) -> Box<Future<Item=Response<Body>, Error=hyper::http::Error> + Send> {
        let mut res = Response::builder();
        res.header("content-type", "application/json");

        let (response_tx, response_rx) = oneshot::channel();

        let packet = match Service::request_to_msg(req) {
            Some(message) => message.map(|message| {
                RequestPacket { response_tx, message }
            }),
            None => {
                res.status(StatusCode::NOT_FOUND);
                let body = Body::from("{\"error\":\"not found\"}");
                return Box::new(res.body(body).into_future());
            }
        };

        let request_tx = self.request_tx.clone();

        // TODO(indutny): use `try_send` and implement `hyper::Service` trait
        // to remove `Fn` restriction from `.serve()` call below.
        Box::new(packet.map(move |packet| {
            request_tx.send(packet)
        }).then(move |send_res| {
            // TODO(indutny): report error properly
            println!("{:#?}", send_res.expect("Send to succeed"));

            response_rx
        }).then(move |msg| {
            // TODO(indutny): report error properly
            let msg = msg.expect("Response message");

            // TODO(indutny): proper error reporting
            let json = match msg {
                ResponseMessage::Info(info) => serde_json::to_string(&info),
                ResponseMessage::ListKeys(keys) => serde_json::to_string(&keys),
                ResponseMessage::AddNode(result) => serde_json::to_string(&result),
            }.expect("JSON stringify to succeed");
            res.body(Body::from(json))
        }))
    }
}

pub struct Server {
}

impl Server {
    pub fn new() -> Server {
        Server {}
    }

    pub fn listen(&self, mut node: Node, port: u16, host: &str) -> Result<(), ()> {
         // TODO(indutny): wrap error
        let ip_addr: IpAddr = host.parse().map_err(|_| ())?;
        let bind_addr: SocketAddr = SocketAddr::new(ip_addr, port);

        let builder = hyper::Server::bind(&bind_addr);

        let (request_tx, request_rx) = mpsc::unbounded_channel();

        let rpc = request_rx.for_each(move |packet: RequestPacket| {
            // TODO(indutny): proper error reporting
            let res = match packet.message {
                RequestMessage::Info => {
                    ResponseMessage::Info(node.info().expect("To get response"))
                },
                RequestMessage::ListKeys => {
                    ResponseMessage::ListKeys(node.list_keys().expect("To get response"))
                },
                RequestMessage::AddNode(body) => {
                    ResponseMessage::AddNode(node.add_node(&body).expect("To get response"))
                },
            };

            // TODO(indutny): proper error reporting
            packet.response_tx.send(res).expect("Send to succeed");

            Ok(())
        }).map_err(|_| ());

        let server = builder
            .serve(move || {
                let service = Service {
                    request_tx: request_tx.clone(),
                };

                service_fn(move |req| service.call(req))
            })
            .map_err(|e| {
                eprintln!("server error: {:?}", e);
            });

        hyper::rt::run(server.join(rpc).map(|_| ()));

        Ok(())
    }
}
