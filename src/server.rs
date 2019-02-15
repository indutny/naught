extern crate futures;
extern crate hyper;
extern crate tokio_sync;

use futures::prelude::*;
use tokio_sync::mpsc;

use std::net::{IpAddr, SocketAddr};

use crate::node::Node;
use crate::service::*;

#[derive(Default)]
pub struct Server {}

impl Server {
    pub fn new() -> Server {
        Server {}
    }

    pub fn listen(&self, port: u16, host: &str) -> Result<(), crate::error::Error> {
        let ip_addr: IpAddr = host.parse().map_err(crate::error::Error::from)?;
        let bind_addr: SocketAddr = SocketAddr::new(ip_addr, port);

        let builder = hyper::Server::bind(&bind_addr);

        let (request_tx, request_rx) = mpsc::unbounded_channel();

        let server = builder.serve(move || RPCService::new(request_tx.clone()));

        let node = Node::new(server.local_addr());

        let rpc = RPCService::forward_rpc(request_rx, node);

        hyper::rt::run(server.from_err().join(rpc).map(|_| ()).map_err(
            move |err: crate::error::Error| {
                eprintln!("Got error: {:#?}", err);
            },
        ));

        Ok(())
    }
}
