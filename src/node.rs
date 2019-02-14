extern crate futures;
extern crate hyper;
extern crate twox_hash;
extern crate jch;
extern crate rand;

use std::net::{IpAddr, SocketAddr};

use futures::Future;
use hyper::{Body, Client, Method, Request, Response, Server, StatusCode};
use hyper::service::service_fn;

struct Peer {
    // Random number
    id: u64,

    // Bucket index for consistent hashing
    bucket: u32,

    // http://host:port
    base_uri: Box<str>,
}

pub struct Node {
    id: u64,
    peers: Vec<Peer>,
    client: Client<hyper::client::HttpConnector, hyper::Body>,
}

impl Node {
    pub fn new() -> Node {
        Node {
            id: rand::random::<u64>(),
            peers: vec![],

            // Pre-allocate client
            client: Client::new(),
        }
    }

    fn router(req: Request<Body>) -> Result<Response<Body>, hyper::http::Error> {
        let mut res = Response::builder();

        res.header("content-type", "application/json");

        match (req.method(), req.uri().path()) {
            (&Method::GET, "/") => {
                res.body(Body::from("index"))
            },
            _ => {
                res.status(StatusCode::NOT_FOUND);
                res.body(Body::empty())
            },
        }
    }

    pub fn listen(&mut self, port: u16, host: &str) -> Result<(), ()> {
        // TODO(indutny): wrap error
        let ip_addr: IpAddr = host.parse().map_err(|_| ())?;
        let bind_addr: SocketAddr = SocketAddr::new(ip_addr, port);

        let builder = Server::bind(&bind_addr);

        hyper::rt::run(
            builder.serve(|| service_fn(Node::router))
            .map_err(|e| {
                eprintln!("server error: {}", e);
            })
        );
        Ok(())
    }
}
