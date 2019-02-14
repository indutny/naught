extern crate hyper;
extern crate futures;
extern crate tokio_sync;
extern crate serde_json;

use std::net::{IpAddr, SocketAddr};

use crate::node::Node;
use crate::message::*;

use futures::{Future, Stream, Sink, IntoFuture};
use tokio_sync::{mpsc, oneshot};
use hyper::{Body, Method, Request, Response, StatusCode};
use hyper::service::service_fn;

#[derive(Debug)]
struct RequestPacket {
    response_tx: oneshot::Sender<ResponseMessage>,
    message: RequestMessage,
}

#[derive(Debug)]
enum RequestMessage {
    Info,
}

#[derive(Debug)]
enum ResponseMessage {
    Info(response::Info),
}

struct Service {
    request_tx: mpsc::UnboundedSender<RequestPacket>,
}

impl Service {
    fn call(&self, req: Request<Body>) -> Box<Future<Item=Response<Body>, Error=hyper::http::Error> + Send> {
        let mut res = Response::builder();
        res.header("content-type", "application/json");

        let message = match (req.method(), req.uri().path()) {
            (&Method::GET, "/_info") => RequestMessage::Info,
            _ => {
                res.status(StatusCode::NOT_FOUND);
                let body = Body::from("{\"error\":\"not found\"}");
                return Box::new(res.body(body).into_future());
            },
        };

        let (response_tx, response_rx) = oneshot::channel();

        let packet = RequestPacket { response_tx, message };

        // TODO(indutny): use `try_send` and implement `hyper::Service` trait
        // to remove `Fn` restriction from `.serve()` call below.
        Box::new(self.request_tx.clone().send(packet).then(move |send_res| {
            // TODO(indutny): report error properly
            send_res.expect("Send to succeed");

            response_rx
        }).then(move |msg| {
            // TODO(indutny): report error properly
            let msg = msg.expect("Response message");

            let json = match msg {
                ResponseMessage::Info(info) => {
                    // TODO(indutny): proper error reporting
                    serde_json::to_string(&info)
                        .expect("JSON stringify to succeed")
                },
            };
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

    pub fn listen(&self, node: Node, port: u16, host: &str) -> Result<(), ()> {
         // TODO(indutny): wrap error
        let ip_addr: IpAddr = host.parse().map_err(|_| ())?;
        let bind_addr: SocketAddr = SocketAddr::new(ip_addr, port);

        let builder = hyper::Server::bind(&bind_addr);

        let (request_tx, request_rx) = mpsc::unbounded_channel();

        let rpc = request_rx.for_each(move |packet: RequestPacket| {
            // TODO(indutny): proper error reporting
            let res = node.info().expect("To get response");

            // TODO(indutny): proper error reporting
            packet.response_tx
                .send(ResponseMessage::Info(res))
                .expect("Send to succeed");

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
