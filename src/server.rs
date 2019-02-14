extern crate hyper;
extern crate futures;
extern crate tokio_sync;

use std::net::{IpAddr, SocketAddr};

use crate::node::Node;

use futures::{Future, Stream, Sink};
use tokio_sync::mpsc;
use hyper::{Body, Method, Request, Response, StatusCode};
use hyper::service::service_fn;

enum RequestMessage {
    Info,
}

enum ResponseMessage {
    Status(StatusCode),
    Body(Body),
}

struct Service {
    tx: mpsc::UnboundedSender<RequestMessage>,
}

impl Service {
    fn call(&self, req: Request<Body>) -> Box<Future<Item=Response<Body>, Error=hyper::http::Error> + Send> {
        let tx = self.tx.clone();
        Box::new(
            tx.send(RequestMessage::Info)
                .then(move |send| {
                    send.expect("Send to succeed");

                    let mut res = Response::builder();

                    res.header("content-type", "application/json");

                    match (req.method(), req.uri().path()) {
                        (&Method::GET, "/_info") => {
                            res.body(Body::from("index"))
                        },
                        _ => {
                            res.status(StatusCode::NOT_FOUND);
                            res.body(Body::empty())
                        },
                    }
                })
        )
    }
}

pub struct Server {
    node: Node,
}

impl Server {
    pub fn new(node: Node) -> Server {
        Server { node }
    }

    pub fn listen(&mut self, port: u16, host: &str) -> Result<(), ()> {
         // TODO(indutny): wrap error
        let ip_addr: IpAddr = host.parse().map_err(|_| ())?;
        let bind_addr: SocketAddr = SocketAddr::new(ip_addr, port);

        let builder = hyper::Server::bind(&bind_addr);

        let (request_tx, request_rx) = mpsc::unbounded_channel::<RequestMessage>();

        let rpc = request_rx.for_each(|msg| {
            Ok(())
        }).map_err(|_| ());

        let server = builder
            .serve(move || {
                let service = Service {
                    tx: request_tx.clone(),
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
