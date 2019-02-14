extern crate hyper;
extern crate twox_hash;
extern crate jch;
extern crate rand;

use std::net::{IpAddr, SocketAddr};

use hyper::{Body, Client, Response, Server};
use hyper::rt::{self, Future, Stream};
use hyper::service::service_fn_ok;

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

    pub fn listen(&mut self, port: u16, host: &str) -> Result<(), ()> {
        // TODO(indutny): wrap error
        let ip_addr: IpAddr = host.parse().map_err(|err| ())?;
        let bind_addr: SocketAddr = SocketAddr::new(ip_addr, port);

        let builder = Server::bind(&bind_addr);

        let make_service = || {
            service_fn_ok(|_| {
                Response::new(Body::from("Hello World"))
            })
        };

        rt::run(builder.serve(make_service).map_err(|e| {
            eprintln!("server error: {}", e);
        }));
        Ok(())
    }
}
